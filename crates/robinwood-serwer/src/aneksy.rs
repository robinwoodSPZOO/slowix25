//! Aneksy do podpisanej umowy: kopia obowiązującej specyfikacji do edycji, zmiana kwoty
//! i/lub terminu montażu, lista różnic, statusy szkic → wysłany → podpisany. Po podpisaniu
//! aneksu obowiązuje jego specyfikacja i warunki (umowa wskazuje specyfikację aneksu, plan
//! płatności, zlecenie i harmonogram są przeliczane).

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{FromRow, SqliteConnection, SqlitePool};

use crate::bledy::{BladApi, Wynik};
use crate::formaty;
use crate::limity::{self, MAKS_DLUGI};
use crate::roznice::Roznica;
use crate::specyfikacje::{self, Specyfikacja};
use crate::umowy::{self, WynikHarmonogramu};
use crate::zmiany::ZmianaCzesciowa;

pub const STATUSY_ANEKSU: &[&str] = &["szkic", "wyslany", "podpisany"];

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct DaneAneksu {
    pub data: Option<String>,
    pub opis_zmian: String,
    /// Zmiana wartości umowy w groszach (ujemna = obniżka).
    pub zmiana_ceny_gr: i64,
    pub nowy_termin_montazu: Option<String>,
}

const POLA_ANEKSU: &[&str] = &[
    "data",
    "opis_zmian",
    "zmiana_ceny_gr",
    "nowy_termin_montazu",
];

fn nazwa_pola(p: &str) -> String {
    match p {
        "data" => "data aneksu",
        "opis_zmian" => "opis zmian",
        "zmiana_ceny_gr" => "zmiana kwoty",
        "nowy_termin_montazu" => "nowy termin montażu",
        inne => inne,
    }
    .into()
}

#[derive(Debug, Clone, Serialize, PartialEq, FromRow)]
pub struct Aneks {
    pub id: i64,
    pub umowa_id: i64,
    pub nr: i64,
    pub status: String,
    pub specyfikacja_id: Option<i64>,
    pub poprzednia_wartosc_gr: Option<i64>,
    pub poprzedni_termin_montazu: Option<String>,
    pub data_podpisania: Option<String>,
    pub wersja: i64,
    pub data: Option<String>,
    pub opis_zmian: String,
    pub zmiana_ceny_gr: i64,
    pub nowy_termin_montazu: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PelnyAneks {
    pub aneks: Aneks,
    pub umowa_numer: String,
    pub umowa_status: String,
    pub nowa_wartosc_gr: Option<i64>,
    pub specyfikacja: Option<Specyfikacja>,
    pub roznice: Vec<Roznica>,
    /// Lista zmian do dokumentu aneksu (opis, kwota, termin, specyfikacja).
    pub lista_zmian: Vec<String>,
    pub tylko_do_odczytu: bool,
}

async fn pobierz_w(db: &mut SqliteConnection, id: i64) -> Wynik<Aneks> {
    sqlx::query_as(
        "SELECT id, umowa_id, nr, status, specyfikacja_id, poprzednia_wartosc_gr, poprzedni_termin_montazu, \
           data_podpisania, wersja, data, opis_zmian, zmiana_ceny_gr, nowy_termin_montazu FROM aneksy WHERE id = ?1",
    )
    .bind(id)
    .fetch_optional(&mut *db)
    .await?
    .ok_or_else(|| BladApi::NieZnaleziono(format!("aneks {id}")))
}

/// Krótkie opisy różnic specyfikacji (do listy zmian w dokumencie aneksu).
pub fn opisy_roznic(r: &[Roznica]) -> Vec<String> {
    r.iter()
        .filter(|x| x.poziom != "zdjecie")
        .map(|x| {
            let co = match x.zmiana.as_str() {
                "dodano" => "dodano",
                "usunieto" => "usunięto",
                "przeniesiono" => "przeniesiono",
                _ => "zmieniono",
            };
            let pola: Vec<String> = x
                .pola
                .iter()
                .filter(|p| p.pole != "kolejność")
                .map(|p| {
                    format!(
                        "{}: {} → {}",
                        p.pole,
                        p.przed.clone().unwrap_or_else(|| "—".into()),
                        p.po.clone().unwrap_or_else(|| "—".into())
                    )
                })
                .collect();
            if pola.is_empty() || x.zmiana != "zmieniono" {
                format!("Specyfikacja – {co}: {}", x.sciezka)
            } else {
                format!("Specyfikacja – {co}: {} ({})", x.sciezka, pola.join("; "))
            }
        })
        .collect()
}

pub async fn pelny_w(db: &mut SqliteConnection, id: i64) -> Wynik<PelnyAneks> {
    let a = pobierz_w(db, id).await?;
    let u = umowy::pobierz_w(db, a.umowa_id).await?;
    let (specyfikacja, roznice) = match a.specyfikacja_id {
        Some(sid) => {
            let s = specyfikacje::pobierz_w(db, sid).await?;
            let r = match s.zrodlo_id {
                Some(z) => {
                    let zr = specyfikacje::pobierz_w(db, z).await?;
                    crate::roznice::porownaj_specyfikacje(&zr, &s)
                }
                None => vec![],
            };
            (Some(s), r)
        }
        None => (None, vec![]),
    };
    let poprzednia = a.poprzednia_wartosc_gr.or(u.dane.wartosc_brutto_gr);
    let nowa = poprzednia.map(|w| w + a.zmiana_ceny_gr);
    let mut lista = Vec::new();
    if !a.opis_zmian.trim().is_empty() {
        lista.extend(
            a.opis_zmian
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .map(String::from),
        );
    }
    if a.zmiana_ceny_gr != 0 {
        lista.push(format!(
            "Wynagrodzenie: {} → {} brutto (zmiana o {})",
            formaty::kwota_opc(poprzednia),
            formaty::kwota_opc(nowa),
            formaty::kwota(a.zmiana_ceny_gr)
        ));
    }
    let poprzedni_termin = a
        .poprzedni_termin_montazu
        .clone()
        .or(u.dane.montaz_do.clone());
    if let Some(t) = &a.nowy_termin_montazu {
        if Some(t) != poprzedni_termin.as_ref() {
            lista.push(format!(
                "Termin rozpoczęcia montażu: {} → {}",
                poprzedni_termin
                    .as_deref()
                    .map(formaty::data)
                    .unwrap_or_else(|| "—".into()),
                formaty::data(t)
            ));
        }
    }
    lista.extend(opisy_roznic(&roznice));
    Ok(PelnyAneks {
        tylko_do_odczytu: a.status != "szkic",
        umowa_numer: u.numer().to_string(),
        umowa_status: u.status.clone(),
        nowa_wartosc_gr: nowa,
        specyfikacja,
        roznice,
        lista_zmian: lista,
        aneks: a,
    })
}

pub async fn pelny(pula: &SqlitePool, id: i64) -> Wynik<PelnyAneks> {
    let mut db = pula.acquire().await?;
    pelny_w(&mut db, id).await
}

pub async fn dodaj(pula: &SqlitePool, umowa_id: i64) -> Wynik<PelnyAneks> {
    let mut tx = crate::baza::transakcja_zapisu(pula).await?;
    let u = umowy::pobierz_w(&mut tx, umowa_id).await?;
    if !["podpisana", "w_realizacji", "odebrana"].contains(&u.status.as_str()) {
        return Err(BladApi::NieprawidloweDane(format!(
            "aneks sporządza się do umowy podpisanej (status: {}){}",
            umowy::nazwa_statusu(&u.status),
            if u.status == "szkic" {
                " – szkic zmieniaj bezpośrednio"
            } else {
                ""
            }
        )));
    }
    let otwarty: Option<i64> =
        sqlx::query_scalar("SELECT nr FROM aneksy WHERE umowa_id = ?1 AND status <> 'podpisany'")
            .bind(umowa_id)
            .fetch_optional(&mut *tx)
            .await?;
    if let Some(nr) = otwarty {
        return Err(BladApi::NieprawidloweDane(format!(
            "aneks nr {nr} nie jest jeszcze podpisany – dokończ go albo usuń przed utworzeniem kolejnego"
        )));
    }
    let nr: i64 =
        sqlx::query_scalar("SELECT COALESCE(MAX(nr), 0) + 1 FROM aneksy WHERE umowa_id = ?1")
            .bind(umowa_id)
            .fetch_one(&mut *tx)
            .await?;
    let spec = specyfikacje::kopiuj_specyfikacje_w(&mut tx, u.specyfikacja_id).await?;
    let id = sqlx::query(
        "INSERT INTO aneksy (umowa_id, nr, data, opis_zmian, zmiana_ceny_gr, specyfikacja_id, status, \
           poprzednia_wartosc_gr, poprzedni_termin_montazu, utworzono, zmieniono) \
         VALUES (?1, ?2, ?3, '', 0, ?4, 'szkic', ?5, ?6, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))",
    )
    .bind(umowa_id)
    .bind(nr)
    .bind(umowy::dzis().format("%Y-%m-%d").to_string())
    .bind(spec)
    .bind(u.dane.wartosc_brutto_gr)
    .bind(&u.dane.montaz_do)
    .execute(&mut *tx)
    .await?
    .last_insert_rowid();
    specyfikacje::dziennik(&mut tx, "aneksy", id, "dodanie").await?;
    let p = pelny_w(&mut tx, id).await?;
    tx.commit().await?;
    Ok(p)
}

pub async fn zmien(pula: &SqlitePool, id: i64, tresc: Value) -> Wynik<PelnyAneks> {
    let z = ZmianaCzesciowa::z_json(tresc, POLA_ANEKSU)?;
    let mut tx = crate::baza::transakcja_zapisu(pula).await?;
    let a = pobierz_w(&mut tx, id).await?;
    if a.status != "szkic" {
        return Err(BladApi::NieprawidloweDane(format!(
            "aneks ma status „{}” – nie można go zmieniać",
            a.status
        )));
    }
    let baza = DaneAneksu {
        data: a.data.clone(),
        opis_zmian: a.opis_zmian.clone(),
        zmiana_ceny_gr: a.zmiana_ceny_gr,
        nowy_termin_montazu: a.nowy_termin_montazu.clone(),
    };
    let json = serde_json::to_value(&baza).unwrap_or(Value::Null);
    z.sprawdz(
        &json,
        serde_json::to_value(&a).unwrap_or(Value::Null),
        &nazwa_pola,
    )?;
    let mut d = z.zastosuj(&baza)?;
    umowy::sprawdz_date("data aneksu", &d.data)?;
    umowy::sprawdz_date("nowy termin montażu", &d.nowy_termin_montazu)?;
    limity::tekst("opis zmian", Some(&d.opis_zmian), MAKS_DLUGI)?;
    limity::kwota("zmiana kwoty", Some(d.zmiana_ceny_gr.abs()))?;
    d.opis_zmian = d.opis_zmian.trim().to_string();
    if let Some(p) = a.poprzednia_wartosc_gr {
        if p + d.zmiana_ceny_gr <= 0 {
            return Err(BladApi::NieprawidloweDane(
                "wartość umowy po aneksie musi być większa od zera".into(),
            ));
        }
    }
    sqlx::query(
        "UPDATE aneksy SET data = ?1, opis_zmian = ?2, zmiana_ceny_gr = ?3, nowy_termin_montazu = ?4, \
           wersja = wersja + 1, zmieniono = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = ?5",
    )
    .bind(&d.data)
    .bind(&d.opis_zmian)
    .bind(d.zmiana_ceny_gr)
    .bind(&d.nowy_termin_montazu)
    .bind(id)
    .execute(&mut *tx)
    .await?;
    specyfikacje::dziennik(&mut tx, "aneksy", id, "zmiana").await?;
    let p = pelny_w(&mut tx, id).await?;
    tx.commit().await?;
    Ok(p)
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct ZmianaStatusuAneksu {
    pub status: String,
    pub data: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WynikStatusuAneksu {
    #[serde(flatten)]
    pub aneks: PelnyAneks,
    pub harmonogram: Option<WynikHarmonogramu>,
}

pub async fn zmien_status(
    pula: &SqlitePool,
    id: i64,
    z: ZmianaStatusuAneksu,
) -> Wynik<WynikStatusuAneksu> {
    if !STATUSY_ANEKSU.contains(&z.status.as_str()) {
        return Err(BladApi::NieprawidloweDane(format!(
            "nieznany status aneksu „{}” (dozwolone: {})",
            z.status,
            STATUSY_ANEKSU.join(", ")
        )));
    }
    let data = umowy::sprawdz_date("data", &z.data)?;
    let mut tx = crate::baza::transakcja_zapisu(pula).await?;
    let a = pobierz_w(&mut tx, id).await?;
    let ok = matches!(
        (a.status.as_str(), z.status.as_str()),
        ("szkic", "wyslany")
            | ("szkic", "podpisany")
            | ("wyslany", "szkic")
            | ("wyslany", "podpisany")
    );
    if !ok {
        return Err(BladApi::NieprawidloweDane(format!(
            "nie można zmienić statusu aneksu z „{}” na „{}”",
            a.status, z.status
        )));
    }
    let p = pelny_w(&mut tx, id).await?;
    if a.status == "szkic" && p.lista_zmian.is_empty() {
        return Err(BladApi::NieprawidloweDane(
            "aneks nie zawiera żadnych zmian – opisz zmiany, zmień kwotę, termin albo specyfikację"
                .into(),
        ));
    }
    let mut harmonogram = None;
    sqlx::query("UPDATE aneksy SET status = ?1, wersja = wersja + 1, zmieniono = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = ?2")
        .bind(&z.status)
        .bind(id)
        .execute(&mut *tx)
        .await?;
    if z.status == "podpisany" {
        let podpis = data
            .or_else(|| {
                a.data
                    .as_deref()
                    .and_then(|t| NaiveDate::parse_from_str(t, "%Y-%m-%d").ok())
            })
            .unwrap_or_else(umowy::dzis);
        harmonogram = Some(zastosuj_w(&mut tx, &a, podpis).await?);
    }
    specyfikacje::dziennik(&mut tx, "aneksy", id, "zmiana").await?;
    let w = WynikStatusuAneksu {
        aneks: pelny_w(&mut tx, id).await?,
        harmonogram,
    };
    tx.commit().await?;
    Ok(w)
}

/// Podpisany aneks: nowa wartość, termin i specyfikacja umowy; plan płatności, zlecenie,
/// materiały i harmonogram – przeliczone.
async fn zastosuj_w(
    db: &mut SqliteConnection,
    a: &Aneks,
    podpis: NaiveDate,
) -> Wynik<WynikHarmonogramu> {
    sqlx::query("UPDATE aneksy SET data_podpisania = ?1 WHERE id = ?2")
        .bind(podpis.format("%Y-%m-%d").to_string())
        .bind(a.id)
        .execute(&mut *db)
        .await?;
    sqlx::query(
        "UPDATE umowy SET wartosc_brutto_gr = COALESCE(wartosc_brutto_gr, 0) + ?1, \
           montaz_do = COALESCE(?2, montaz_do), specyfikacja_id = COALESCE(?3, specyfikacja_id), \
           specyfikacja_pierwotna_id = COALESCE(specyfikacja_pierwotna_id, specyfikacja_id), \
           wersja = wersja + 1, zmieniono = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = ?4",
    )
    .bind(a.zmiana_ceny_gr)
    .bind(&a.nowy_termin_montazu)
    .bind(a.specyfikacja_id)
    .bind(a.umowa_id)
    .execute(&mut *db)
    .await?;
    let u = umowy::pobierz_w(db, a.umowa_id).await?;
    let plan = umowy::plan_w(db, &u).await?;
    if !plan.zgodny {
        return Err(BladApi::NieprawidloweDane(format!(
            "po aneksie plan płatności jest niespójny: {} – popraw kwotę aneksu",
            plan.bledy.join("; ")
        )));
    }
    umowy::zapisz_plan_w(db, a.umowa_id, &plan).await?;
    specyfikacje::dziennik(db, "umowy", a.umowa_id, "zmiana").await?;
    umowy::po_podpisaniu_w(db, a.umowa_id, None).await
}

pub async fn usun(pula: &SqlitePool, id: i64) -> Wynik<i64> {
    let mut tx = crate::baza::transakcja_zapisu(pula).await?;
    let a = pobierz_w(&mut tx, id).await?;
    if a.status == "podpisany" {
        return Err(BladApi::NieprawidloweDane(
            "podpisanego aneksu nie można usunąć".into(),
        ));
    }
    sqlx::query("DELETE FROM aneksy WHERE id = ?1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    if let Some(s) = a.specyfikacja_id {
        specyfikacje::usun_specyfikacje_w(&mut tx, s).await?;
    }
    specyfikacje::dziennik(&mut tx, "aneksy", id, "usuniecie").await?;
    tx.commit().await?;
    Ok(a.umowa_id)
}
