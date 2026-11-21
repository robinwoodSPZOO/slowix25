//! Trasy pomocnicze dla interfejsu kalendarza (Faza 4b), których brakowało w API silnika terminów:
//!
//! * `POST /import/kategorie-materialow` – import wierszy pliku `kategorie-materialow.csv`
//!   (kategorie, dostawcy, czasy dostaw) w **jednej transakcji** z jednym przeliczeniem wszystkich
//!   zleceń; wiersze błędne są pomijane z powodem (raport), poprawne – zapisywane,
//! * `POST /kalendarz/dni-wolne/zakres` i `POST /kalendarz/dni-wolne/usun-zakres` – zakres dni wolnych
//!   (np. urlop 2 tygodnie) jednym zapisem i jednym przeliczeniem zamiast N osobnych,
//! * `GET /kalendarz/wiersze-specyfikacji?ids=1,2` – opis wierszy „do potwierdzenia” (sekcja, część,
//!   rodzaj, materiał, opis) do okna „zamówić mimo to?”.
//!
//! Plik CSV parsuje interfejs (`src/moduly/kalendarz/importCsv.ts`); serwer i tak sprawdza każdy wiersz.

use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use sqlx::SqliteConnection;

use crate::api::StanApi;
use crate::bledy::{BladApi, Wynik};
use crate::kalendarz;
use crate::terminarz::{self as t, WynikPrzeliczeniaWszystkich};
use crate::wejscie::{Cialo, Parametry};
use crate::zdarzenia::{Operacja, Zdarzenie};

/// Największa liczba wierszy jednego importu i dni jednego zakresu.
pub const MAKS_WIERSZY_IMPORTU: usize = 5000;
pub const MAKS_DNI_ZAKRESU: i64 = 366;
/// Największy czas dostawy przyjmowany z pliku (dni).
pub const MAKS_DNI_DOSTAWY: i64 = 365;

pub fn trasy() -> Router<StanApi> {
    Router::new()
        .route("/import/kategorie-materialow", post(importuj))
        .route("/kalendarz/dni-wolne/zakres", post(dodaj_zakres))
        .route("/kalendarz/dni-wolne/usun-zakres", post(usun_zakres))
        .route("/kalendarz/wiersze-specyfikacji", get(wiersze_specyfikacji))
}

fn wyslij(s: &StanApi, zasob: &'static str, id: i64) {
    s.zdarzenia.wyslij(Zdarzenie {
        zasob,
        id,
        operacja: Operacja::Zmiana,
    });
}

fn powiadom(s: &StanApi, p: &WynikPrzeliczeniaWszystkich) {
    for id in &p.zmienione_zlecenia {
        wyslij(s, "zadania_kalendarza", *id);
    }
    for n in &p.nieprzeliczone_zlecenia {
        wyslij(s, "zlecenia", n.zlecenie_id);
    }
}

async fn do_dziennika(
    c: &mut SqliteConnection,
    tabela: &str,
    id: i64,
    operacja: &str,
    opis: &str,
) -> Wynik<()> {
    sqlx::query(
        "INSERT INTO dziennik_zmian (tabela, rekord_id, operacja, opis) VALUES (?1, ?2, ?3, ?4)",
    )
    .bind(tabela)
    .bind(id)
    .bind(operacja)
    .bind(opis)
    .execute(&mut *c)
    .await?;
    Ok(())
}

// --- Import kategorii, dostawców i czasów dostaw ------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WierszImportu {
    /// Numer wiersza w pliku (do raportu); brak = kolejny numer w liście.
    #[serde(default)]
    pub wiersz: Option<usize>,
    pub kategoria: String,
    #[serde(default)]
    pub przykladowe_materialy: Option<String>,
    #[serde(default)]
    pub dostawca: Option<String>,
    #[serde(default)]
    pub czas_dostawy_min_dni: Option<i64>,
    #[serde(default)]
    pub czas_dostawy_max_dni: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DaneImportu {
    pub wiersze: Vec<WierszImportu>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PominietyWiersz {
    pub wiersz: usize,
    pub powod: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct RaportImportuKategorii {
    pub kategorie_dodane: usize,
    pub kategorie_zaktualizowane: usize,
    pub dostawcy_dodani: usize,
    pub czasy_dostaw_ustawione: usize,
    /// Wiersze bez czasu dostawy (kategoria/dostawca zapisane, widełki „nie ustawiono”).
    pub bez_czasu_dostawy: Vec<usize>,
    pub pominiete: Vec<PominietyWiersz>,
    #[serde(flatten)]
    pub przeliczenie: WynikPrzeliczeniaWszystkich,
}

fn bez_pustych(s: Option<String>) -> Option<String> {
    s.map(|x| x.trim().to_string()).filter(|x| !x.is_empty())
}

/// Sprawdza widełki wiersza: obie wartości albo żadna, 0 ≤ min ≤ max ≤ 365.
pub fn sprawdz_wiersz(w: &WierszImportu) -> Result<Option<(i64, i64)>, String> {
    if w.kategoria.trim().is_empty() {
        return Err("brak nazwy kategorii".into());
    }
    if w.kategoria.trim().chars().count() > 200 {
        return Err("nazwa kategorii dłuższa niż 200 znaków".into());
    }
    match (w.czas_dostawy_min_dni, w.czas_dostawy_max_dni) {
        (None, None) => Ok(None),
        (Some(_), None) | (None, Some(_)) => {
            Err("podano tylko jeden z czasów dostawy (min albo max) – uzupełnij oba".into())
        }
        (Some(a), Some(b)) => {
            if a < 0 || b < 0 {
                Err("czas dostawy nie może być ujemny".into())
            } else if a > MAKS_DNI_DOSTAWY || b > MAKS_DNI_DOSTAWY {
                Err(format!("czas dostawy ponad {MAKS_DNI_DOSTAWY} dni"))
            } else if a > b {
                Err(format!(
                    "minimalny czas dostawy ({a}) większy niż maksymalny ({b})"
                ))
            } else {
                Ok(Some((a, b)))
            }
        }
    }
}

async fn znajdz_lub_dodaj_dostawce(
    c: &mut SqliteConnection,
    nazwa: &str,
    raport: &mut RaportImportuKategorii,
) -> Wynik<i64> {
    let id: Option<i64> =
        sqlx::query_scalar("SELECT id FROM dostawcy WHERE nazwa = ?1 COLLATE NOCASE")
            .bind(nazwa)
            .fetch_optional(&mut *c)
            .await?;
    if let Some(id) = id {
        return Ok(id);
    }
    let id = sqlx::query("INSERT INTO dostawcy (nazwa) VALUES (?1)")
        .bind(nazwa)
        .execute(&mut *c)
        .await?
        .last_insert_rowid();
    do_dziennika(
        c,
        "dostawcy",
        id,
        "dodanie",
        "import kategorie-materialow.csv",
    )
    .await?;
    raport.dostawcy_dodani += 1;
    Ok(id)
}

pub async fn importuj_kategorie(
    pula: &sqlx::SqlitePool,
    d: DaneImportu,
) -> Wynik<RaportImportuKategorii> {
    if d.wiersze.len() > MAKS_WIERSZY_IMPORTU {
        return Err(BladApi::NieprawidloweDane(format!(
            "plik ma {} wierszy – najwyżej {MAKS_WIERSZY_IMPORTU}",
            d.wiersze.len()
        )));
    }
    let mut raport = RaportImportuKategorii::default();
    let mut tx = crate::baza::transakcja_zapisu(pula).await?;
    for (i, w) in d.wiersze.into_iter().enumerate() {
        let nr = w.wiersz.unwrap_or(i + 2);
        let widelki = match sprawdz_wiersz(&w) {
            Ok(x) => x,
            Err(powod) => {
                raport.pominiete.push(PominietyWiersz { wiersz: nr, powod });
                continue;
            }
        };
        let nazwa = w.kategoria.trim().to_string();
        let przyklady = bez_pustych(w.przykladowe_materialy);
        let dostawca = bez_pustych(w.dostawca);

        let istniejaca: Option<(i64, Option<String>, Option<i64>)> = sqlx::query_as(
            "SELECT id, opis, domyslny_dostawca_id FROM kategorie_materialow WHERE nazwa = ?1 COLLATE NOCASE",
        )
        .bind(&nazwa)
        .fetch_optional(&mut *tx)
        .await?;
        let (kat_id, domyslny) = match istniejaca {
            Some((id, opis, dom)) => {
                if opis.is_none() && przyklady.is_some() {
                    sqlx::query("UPDATE kategorie_materialow SET opis = ?1 WHERE id = ?2")
                        .bind(&przyklady)
                        .bind(id)
                        .execute(&mut *tx)
                        .await?;
                }
                raport.kategorie_zaktualizowane += 1;
                (id, dom)
            }
            None => {
                let id =
                    sqlx::query("INSERT INTO kategorie_materialow (nazwa, opis) VALUES (?1, ?2)")
                        .bind(&nazwa)
                        .bind(&przyklady)
                        .execute(&mut *tx)
                        .await?
                        .last_insert_rowid();
                raport.kategorie_dodane += 1;
                (id, None)
            }
        };
        do_dziennika(
            &mut tx,
            "kategorie_materialow",
            kat_id,
            "zmiana",
            &format!("import kategorie-materialow.csv, wiersz {nr}"),
        )
        .await?;

        let dostawca_id = match &dostawca {
            Some(n) => Some(znajdz_lub_dodaj_dostawce(&mut tx, n, &mut raport).await?),
            None => None,
        };
        if let (Some(dost), None) = (dostawca_id, domyslny) {
            sqlx::query("UPDATE kategorie_materialow SET domyslny_dostawca_id = ?1 WHERE id = ?2")
                .bind(dost)
                .bind(kat_id)
                .execute(&mut *tx)
                .await?;
        }
        match (widelki, dostawca_id) {
            (None, _) => raport.bez_czasu_dostawy.push(nr),
            (Some((min, max)), Some(dost)) => {
                sqlx::query(
                    "INSERT INTO czasy_dostaw (kategoria_id, dostawca_id, dni_min, dni_max) VALUES (?1, ?2, ?3, ?4) \
                     ON CONFLICT(kategoria_id, dostawca_id) DO UPDATE SET dni_min = excluded.dni_min, dni_max = excluded.dni_max",
                )
                .bind(kat_id)
                .bind(dost)
                .bind(min)
                .bind(max)
                .execute(&mut *tx)
                .await?;
                let id: i64 = sqlx::query_scalar(
                    "SELECT id FROM czasy_dostaw WHERE kategoria_id = ?1 AND dostawca_id = ?2",
                )
                .bind(kat_id)
                .bind(dost)
                .fetch_one(&mut *tx)
                .await?;
                do_dziennika(
                    &mut tx,
                    "czasy_dostaw",
                    id,
                    "zmiana",
                    "import kategorie-materialow.csv",
                )
                .await?;
                raport.czasy_dostaw_ustawione += 1;
            }
            (Some((min, max)), None) => {
                sqlx::query(
                    "UPDATE kategorie_materialow SET dni_dostawy_min = ?1, dni_dostawy_max = ?2 WHERE id = ?3",
                )
                .bind(min)
                .bind(max)
                .bind(kat_id)
                .execute(&mut *tx)
                .await?;
                raport.czasy_dostaw_ustawione += 1;
            }
        }
    }
    raport.przeliczenie = t::przelicz_wszystkie_w(&mut tx).await?;
    tx.commit().await?;
    Ok(raport)
}

async fn importuj(
    State(s): State<StanApi>,
    Cialo(d): Cialo<DaneImportu>,
) -> Wynik<Json<RaportImportuKategorii>> {
    let r = importuj_kategorie(&s.pula, d).await?;
    wyslij(&s, "kategorie_materialow", 0);
    wyslij(&s, "dostawcy", 0);
    wyslij(&s, "czasy_dostaw", 0);
    powiadom(&s, &r.przeliczenie);
    Ok(Json(r))
}

// --- Zakres dni wolnych -----------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ZakresDniWolnych {
    #[serde(deserialize_with = "t::wej::data")]
    pub od: NaiveDate,
    #[serde(rename = "do", deserialize_with = "t::wej::data")]
    pub do_: NaiveDate,
    #[serde(default)]
    pub rodzaj: Option<String>,
    #[serde(default)]
    pub opis: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WynikZakresu {
    pub dni: usize,
    #[serde(flatten)]
    pub przeliczenie: WynikPrzeliczeniaWszystkich,
}

fn sprawdz_zakres(od: NaiveDate, do_: NaiveDate) -> Wynik<Vec<String>> {
    for d in [od, do_] {
        if !kalendarz::obslugiwany_rok(chrono::Datelike::year(&d)) {
            return Err(BladApi::NieprawidloweDane(format!(
                "data {d} spoza obsługiwanego zakresu lat {}–{}",
                kalendarz::ROK_MIN,
                kalendarz::ROK_MAX
            )));
        }
    }
    if do_ < od {
        return Err(BladApi::NieprawidloweDane(
            "koniec zakresu jest wcześniej niż początek".into(),
        ));
    }
    let n = (do_ - od).num_days() + 1;
    if n > MAKS_DNI_ZAKRESU {
        return Err(BladApi::NieprawidloweDane(format!(
            "zakres ma {n} dni – najwyżej {MAKS_DNI_ZAKRESU}"
        )));
    }
    Ok(od
        .iter_days()
        .take(n as usize)
        .map(|d| d.format("%Y-%m-%d").to_string())
        .collect())
}

const RODZAJE: [&str; 4] = ["swieto", "urlop", "przerwa_dostawcow", "inne"];

async fn dodaj_zakres(
    State(s): State<StanApi>,
    Cialo(z): Cialo<ZakresDniWolnych>,
) -> Wynik<Json<WynikZakresu>> {
    let dni = sprawdz_zakres(z.od, z.do_)?;
    let rodzaj = z.rodzaj.unwrap_or_else(|| "urlop".into());
    if !RODZAJE.contains(&rodzaj.as_str()) {
        return Err(BladApi::NieprawidloweDane(format!(
            "nieznany rodzaj dnia wolnego „{rodzaj}” (dozwolone: {})",
            RODZAJE.join(", ")
        )));
    }
    let opis = bez_pustych(z.opis);
    let mut tx = crate::baza::transakcja_zapisu(&s.pula).await?;
    for d in &dni {
        sqlx::query(
            "INSERT INTO dni_wolne (data, rodzaj, opis) VALUES (?1, ?2, ?3) \
             ON CONFLICT(data) DO UPDATE SET rodzaj = excluded.rodzaj, opis = excluded.opis",
        )
        .bind(d)
        .bind(&rodzaj)
        .bind(&opis)
        .execute(&mut *tx)
        .await?;
    }
    do_dziennika(
        &mut tx,
        "dni_wolne",
        0,
        "zmiana",
        &format!("zakres {} – {}", z.od, z.do_),
    )
    .await?;
    let p = t::przelicz_wszystkie_w(&mut tx).await?;
    tx.commit().await?;
    wyslij(&s, "dni_wolne", 0);
    powiadom(&s, &p);
    Ok(Json(WynikZakresu {
        dni: dni.len(),
        przeliczenie: p,
    }))
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ZakresDoUsuniecia {
    #[serde(deserialize_with = "t::wej::data")]
    pub od: NaiveDate,
    #[serde(rename = "do", deserialize_with = "t::wej::data")]
    pub do_: NaiveDate,
}

async fn usun_zakres(
    State(s): State<StanApi>,
    Cialo(z): Cialo<ZakresDoUsuniecia>,
) -> Wynik<Json<WynikZakresu>> {
    sprawdz_zakres(z.od, z.do_)?;
    let mut tx = crate::baza::transakcja_zapisu(&s.pula).await?;
    let n = sqlx::query("DELETE FROM dni_wolne WHERE data >= ?1 AND data <= ?2")
        .bind(z.od.format("%Y-%m-%d").to_string())
        .bind(z.do_.format("%Y-%m-%d").to_string())
        .execute(&mut *tx)
        .await?
        .rows_affected();
    do_dziennika(
        &mut tx,
        "dni_wolne",
        0,
        "usuniecie",
        &format!("zakres {} – {}", z.od, z.do_),
    )
    .await?;
    let p = t::przelicz_wszystkie_w(&mut tx).await?;
    tx.commit().await?;
    wyslij(&s, "dni_wolne", 0);
    powiadom(&s, &p);
    Ok(Json(WynikZakresu {
        dni: n as usize,
        przeliczenie: p,
    }))
}

// --- Wiersze specyfikacji do okna „zamówić mimo to?” --------------------------------

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
struct ParametryWierszy {
    ids: String,
    #[serde(rename = "klucz")]
    _klucz: Option<String>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct OpisWiersza {
    pub id: i64,
    pub sekcja: String,
    pub czesc: String,
    pub rodzaj: Option<String>,
    pub material: Option<String>,
    pub opis: Option<String>,
    pub status: String,
}

async fn wiersze_specyfikacji(
    State(s): State<StanApi>,
    Parametry(p): Parametry<ParametryWierszy>,
) -> Wynik<Json<Vec<OpisWiersza>>> {
    let mut ids = Vec::new();
    for x in p.ids.split(',').map(str::trim).filter(|x| !x.is_empty()) {
        ids.push(
            x.parse::<i64>().map_err(|_| {
                BladApi::NieprawidloweDane(format!("„{x}” nie jest numerem wiersza"))
            })?,
        );
    }
    if ids.len() > 200 {
        return Err(BladApi::NieprawidloweDane(
            "najwyżej 200 wierszy naraz".into(),
        ));
    }
    let mut wynik = Vec::with_capacity(ids.len());
    let mut c = s.pula.acquire().await?;
    for id in ids {
        let w: Option<OpisWiersza> = sqlx::query_as(
            "SELECT w.id, TRIM(COALESCE(s.numer || ' ', '') || s.nazwa) AS sekcja, c.nazwa AS czesc, \
                    w.rodzaj, w.material, w.opis, w.status \
               FROM wiersze_specyfikacji w \
               JOIN czesci_specyfikacji c ON c.id = w.czesc_id \
               JOIN sekcje_specyfikacji s ON s.id = c.sekcja_id \
              WHERE w.id = ?1",
        )
        .bind(id)
        .fetch_optional(&mut *c)
        .await?;
        wynik.extend(w);
    }
    Ok(Json(wynik))
}

#[cfg(test)]
mod testy {
    use super::*;

    fn w(min: Option<i64>, max: Option<i64>) -> WierszImportu {
        WierszImportu {
            wiersz: None,
            kategoria: "Płyta".into(),
            przykladowe_materialy: None,
            dostawca: None,
            czas_dostawy_min_dni: min,
            czas_dostawy_max_dni: max,
        }
    }

    #[test]
    fn walidacja_widelek_wiersza() {
        assert_eq!(sprawdz_wiersz(&w(None, None)), Ok(None));
        assert_eq!(sprawdz_wiersz(&w(Some(5), Some(10))), Ok(Some((5, 10))));
        assert!(sprawdz_wiersz(&w(Some(5), None)).is_err());
        assert!(sprawdz_wiersz(&w(Some(10), Some(5))).is_err());
        assert!(sprawdz_wiersz(&w(Some(-1), Some(5))).is_err());
        assert!(sprawdz_wiersz(&w(Some(1), Some(400))).is_err());
        let mut pusty = w(None, None);
        pusty.kategoria = "  ".into();
        assert!(sprawdz_wiersz(&pusty).is_err());
    }
}
