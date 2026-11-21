//! Warstwa bazy silnika terminów: zlecenia, materiały (zamówienia), zadania kalendarza,
//! dostawcy, czasy dostaw, ustawienia odstępów i dni wolne. Logika liczenia jest w
//! [`crate::kalendarz`]; tu tylko wczytanie danych, zapis wyniku i odczyt ze stanami.

use std::collections::{BTreeMap, HashMap};

use chrono::{Datelike, Duration, NaiveDate};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqliteConnection, SqlitePool};

use crate::bledy::{BladApi, Wynik};
use crate::kalendarz::{
    self, scal, stan_terminu, stan_wykonane, stan_zamowienia, DaneZamowienia, Harmonogram,
    KalendarzDni, Kolizja, LiczenieDni, MaterialWejscie, Okres, PlatnoscWejscie, Stan, StanZadania,
    StatystykaDostaw, TypZadania, UstawieniaTerminow, Widelki, ZadanieIstniejace, ZadanieWyliczone,
    ZlecenieWejscie, ZrodloWidelek,
};

pub const ETAPY_ZLECENIA: &[&str] = &[
    "oferta",
    "umowa",
    "pomiar",
    "rysunki",
    "zamowienia",
    "warsztat",
    "montaz",
    "odbior",
    "gwarancja",
    "zakonczone",
];
pub const RODZAJE_DNI_WOLNYCH: &[&str] = &["swieto", "urlop", "przerwa_dostawcow", "inne"];
const MAX_DNI: i64 = 365;

fn data(t: &str) -> Wynik<NaiveDate> {
    NaiveDate::parse_from_str(t, "%Y-%m-%d").map_err(|_| {
        BladApi::NieprawidloweDane(format!("nieprawidłowa data „{t}” (oczekiwano RRRR-MM-DD)"))
    })
}

fn data_opt(t: Option<&str>) -> Wynik<Option<NaiveDate>> {
    t.map(data).transpose()
}

fn tekst(d: NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}

fn dni(nazwa: &str, v: Option<i64>) -> Wynik<Option<i64>> {
    match v {
        Some(n) if !(0..=MAX_DNI).contains(&n) => Err(BladApi::NieprawidloweDane(format!(
            "{nazwa}: liczba dni musi być z zakresu 0–{MAX_DNI}"
        ))),
        _ => Ok(v),
    }
}

fn u32_z(v: i64) -> u32 {
    u32::try_from(v.max(0)).unwrap_or(u32::MAX)
}

async fn istnieje(conn: &mut SqliteConnection, sql: &str, id: i64, co: &str) -> Wynik<()> {
    let jest: Option<i64> = sqlx::query_scalar(sql)
        .bind(id)
        .fetch_optional(&mut *conn)
        .await?;
    jest.map(|_| ())
        .ok_or_else(|| BladApi::NieprawidloweDane(format!("{co} {id} nie istnieje")))
}

async fn dziennik(conn: &mut SqliteConnection, tabela: &str, id: i64, operacja: &str) -> Wynik<()> {
    sqlx::query("INSERT INTO dziennik_zmian (tabela, rekord_id, operacja) VALUES (?1, ?2, ?3)")
        .bind(tabela)
        .bind(id)
        .bind(operacja)
        .execute(&mut *conn)
        .await?;
    Ok(())
}

// ===========================================================================
// Ustawienia
// ===========================================================================

/// Klucze w tabeli `ustawienia` odpowiadające polom [`UstawieniaTerminow`].
const KLUCZE_USTAWIEN: [(&str, &str); 7] = [
    (
        "odstep_materialy_dni",
        "odstep_materialy_przed_warsztatem_dni",
    ),
    ("odstep_rysunki_dni", "odstep_rysunki_przed_zamowieniem_dni"),
    ("odstep_pomiar_dni", "odstep_pomiar_przed_rysunkami_dni"),
    ("prog_pomaranczowy_dni", "prog_pomaranczowy_dni"),
    ("prog_zbliza_sie_dni", "prog_zbliza_sie_dni"),
    (
        "odstep_potwierdzenie_montazu_dni",
        "odstep_potwierdzenie_montazu_dni",
    ),
    (
        "odstep_pomieszczenie_gotowe_dni",
        "odstep_pomieszczenie_gotowe_dni",
    ),
];

pub async fn ustawienia(conn: &mut SqliteConnection) -> Wynik<UstawieniaTerminow> {
    let wiersze: Vec<(String, String)> = sqlx::query_as("SELECT klucz, wartosc FROM ustawienia")
        .fetch_all(&mut *conn)
        .await?;
    let mapa: HashMap<String, String> = wiersze.into_iter().collect();
    let liczba = |k: &str, dom: u32| {
        mapa.get(k)
            .and_then(|v| v.trim().parse::<u32>().ok())
            .unwrap_or(dom)
    };
    let d = UstawieniaTerminow::default();
    Ok(UstawieniaTerminow {
        liczenie_dni: mapa
            .get("liczenie_dni")
            .and_then(|v| LiczenieDni::z_tekstu(v))
            .unwrap_or(d.liczenie_dni),
        odstep_materialy_dni: liczba(KLUCZE_USTAWIEN[0].1, d.odstep_materialy_dni),
        odstep_rysunki_dni: liczba(KLUCZE_USTAWIEN[1].1, d.odstep_rysunki_dni),
        odstep_pomiar_dni: liczba(KLUCZE_USTAWIEN[2].1, d.odstep_pomiar_dni),
        prog_pomaranczowy_dni: liczba(KLUCZE_USTAWIEN[3].1, d.prog_pomaranczowy_dni),
        prog_zbliza_sie_dni: liczba(KLUCZE_USTAWIEN[4].1, d.prog_zbliza_sie_dni),
        odstep_potwierdzenie_montazu_dni: liczba(
            KLUCZE_USTAWIEN[5].1,
            d.odstep_potwierdzenie_montazu_dni,
        ),
        odstep_pomieszczenie_gotowe_dni: liczba(
            KLUCZE_USTAWIEN[6].1,
            d.odstep_pomieszczenie_gotowe_dni,
        ),
    })
}

/// Częściowa zmiana ustawień (pola pominięte zostają bez zmian).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ZmianaUstawien {
    pub liczenie_dni: Option<LiczenieDni>,
    pub odstep_materialy_dni: Option<i64>,
    pub odstep_rysunki_dni: Option<i64>,
    pub odstep_pomiar_dni: Option<i64>,
    pub prog_pomaranczowy_dni: Option<i64>,
    pub prog_zbliza_sie_dni: Option<i64>,
    pub odstep_potwierdzenie_montazu_dni: Option<i64>,
    pub odstep_pomieszczenie_gotowe_dni: Option<i64>,
}

pub async fn zmien_ustawienia(pula: &SqlitePool, z: ZmianaUstawien) -> Wynik<UstawieniaTerminow> {
    let wartosci = [
        z.odstep_materialy_dni,
        z.odstep_rysunki_dni,
        z.odstep_pomiar_dni,
        z.prog_pomaranczowy_dni,
        z.prog_zbliza_sie_dni,
        z.odstep_potwierdzenie_montazu_dni,
        z.odstep_pomieszczenie_gotowe_dni,
    ];
    for ((pole, _), v) in KLUCZE_USTAWIEN.iter().zip(wartosci) {
        dni(pole, v)?;
    }
    let mut tx = pula.begin().await?;
    let mut zapisz = Vec::new();
    if let Some(l) = z.liczenie_dni {
        zapisz.push(("liczenie_dni", l.jako_tekst().to_string()));
    }
    for ((_, klucz), v) in KLUCZE_USTAWIEN.iter().zip(wartosci) {
        if let Some(v) = v {
            zapisz.push((klucz, v.to_string()));
        }
    }
    for (klucz, wartosc) in zapisz {
        sqlx::query(
            "INSERT INTO ustawienia (klucz, wartosc) VALUES (?1, ?2) \
             ON CONFLICT(klucz) DO UPDATE SET wartosc = excluded.wartosc",
        )
        .bind(klucz)
        .bind(wartosc)
        .execute(&mut *tx)
        .await?;
    }
    let u = ustawienia(&mut tx).await?;
    tx.commit().await?;
    Ok(u)
}

// ===========================================================================
// Dni wolne
// ===========================================================================

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq)]
pub struct DzienWolny {
    pub data: String,
    pub rodzaj: String,
    pub opis: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct DaneDniaWolnego {
    pub rodzaj: Option<String>,
    pub opis: Option<String>,
}

pub async fn dni_wolne(
    conn: &mut SqliteConnection,
    od: Option<NaiveDate>,
    do_: Option<NaiveDate>,
) -> Wynik<Vec<DzienWolny>> {
    Ok(sqlx::query_as(
        "SELECT data, rodzaj, opis FROM dni_wolne WHERE (?1 IS NULL OR data >= ?1) AND (?2 IS NULL OR data <= ?2) ORDER BY data",
    )
    .bind(od.map(tekst))
    .bind(do_.map(tekst))
    .fetch_all(&mut *conn)
    .await?)
}

pub async fn ustaw_dzien_wolny(
    pula: &SqlitePool,
    d: &str,
    dane: DaneDniaWolnego,
) -> Wynik<DzienWolny> {
    let d = tekst(data(d)?);
    let rodzaj = dane.rodzaj.unwrap_or_else(|| "inne".into());
    if !RODZAJE_DNI_WOLNYCH.contains(&rodzaj.as_str()) {
        return Err(BladApi::NieprawidloweDane(format!(
            "nieznany rodzaj dnia wolnego „{rodzaj}” (dozwolone: {})",
            RODZAJE_DNI_WOLNYCH.join(", ")
        )));
    }
    let opis = dane
        .opis
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    sqlx::query(
        "INSERT INTO dni_wolne (data, rodzaj, opis) VALUES (?1, ?2, ?3) \
         ON CONFLICT(data) DO UPDATE SET rodzaj = excluded.rodzaj, opis = excluded.opis",
    )
    .bind(&d)
    .bind(&rodzaj)
    .bind(&opis)
    .execute(pula)
    .await?;
    Ok(DzienWolny {
        data: d,
        rodzaj,
        opis,
    })
}

pub async fn usun_dzien_wolny(pula: &SqlitePool, d: &str) -> Wynik<()> {
    let d = tekst(data(d)?);
    let n = sqlx::query("DELETE FROM dni_wolne WHERE data = ?1")
        .bind(&d)
        .execute(pula)
        .await?
        .rows_affected();
    if n == 0 {
        return Err(BladApi::NieZnaleziono(format!("dzień wolny {d}")));
    }
    Ok(())
}

/// Kalendarz dni roboczych z ustawień i dni wolnych użytkownika.
pub async fn kalendarz_dni(
    conn: &mut SqliteConnection,
) -> Wynik<(UstawieniaTerminow, KalendarzDni)> {
    let u = ustawienia(conn).await?;
    let wolne = dni_wolne(conn, None, None).await?;
    let daty = wolne
        .iter()
        .filter_map(|w| data(&w.data).ok())
        .collect::<Vec<_>>();
    Ok((u, KalendarzDni::nowy(u.liczenie_dni, daty)))
}

// ===========================================================================
// Dostawcy, widełki kategorii, czasy dostaw
// ===========================================================================

#[derive(Debug, Clone, Serialize, FromRow, PartialEq)]
pub struct Dostawca {
    pub id: i64,
    pub nazwa: String,
    pub email: Option<String>,
    pub telefon: Option<String>,
    pub uwagi: Option<String>,
    pub aktywny: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DaneDostawcy {
    pub nazwa: String,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub telefon: Option<String>,
    #[serde(default)]
    pub uwagi: Option<String>,
    #[serde(default = "prawda")]
    pub aktywny: bool,
}

fn prawda() -> bool {
    true
}

pub async fn dostawcy(pula: &SqlitePool) -> Wynik<Vec<Dostawca>> {
    Ok(sqlx::query_as("SELECT id, nazwa, email, telefon, uwagi, aktywny FROM dostawcy ORDER BY nazwa COLLATE NOCASE")
        .fetch_all(pula)
        .await?)
}

async fn dostawca(pula: &SqlitePool, id: i64) -> Wynik<Dostawca> {
    sqlx::query_as("SELECT id, nazwa, email, telefon, uwagi, aktywny FROM dostawcy WHERE id = ?1")
        .bind(id)
        .fetch_optional(pula)
        .await?
        .ok_or_else(|| BladApi::NieZnaleziono(format!("dostawca {id}")))
}

pub async fn zapisz_dostawce(
    pula: &SqlitePool,
    id: Option<i64>,
    d: DaneDostawcy,
) -> Wynik<Dostawca> {
    let nazwa = d.nazwa.trim().to_string();
    if nazwa.is_empty() {
        return Err(BladApi::NieprawidloweDane(
            "nazwa dostawcy jest wymagana".into(),
        ));
    }
    let powtorzona: Option<i64> =
        sqlx::query_scalar("SELECT id FROM dostawcy WHERE nazwa = ?1 AND id IS NOT ?2")
            .bind(&nazwa)
            .bind(id)
            .fetch_optional(pula)
            .await?;
    if powtorzona.is_some() {
        return Err(BladApi::NieprawidloweDane(format!(
            "dostawca „{nazwa}” już istnieje"
        )));
    }
    let id = match id {
        None => sqlx::query("INSERT INTO dostawcy (nazwa, email, telefon, uwagi, aktywny) VALUES (?1, ?2, ?3, ?4, ?5)")
            .bind(&nazwa)
            .bind(&d.email)
            .bind(&d.telefon)
            .bind(&d.uwagi)
            .bind(d.aktywny)
            .execute(pula)
            .await?
            .last_insert_rowid(),
        Some(id) => {
            let n = sqlx::query("UPDATE dostawcy SET nazwa = ?1, email = ?2, telefon = ?3, uwagi = ?4, aktywny = ?5 WHERE id = ?6")
                .bind(&nazwa)
                .bind(&d.email)
                .bind(&d.telefon)
                .bind(&d.uwagi)
                .bind(d.aktywny)
                .bind(id)
                .execute(pula)
                .await?
                .rows_affected();
            if n == 0 {
                return Err(BladApi::NieZnaleziono(format!("dostawca {id}")));
            }
            id
        }
    };
    dostawca(pula, id).await
}

/// Zmiana domyślnych widełek kategorii (0–0 = „brak widełek”) i domyślnego dostawcy.
#[derive(Debug, Clone, Deserialize)]
pub struct WidelkiKategorii {
    pub dni_dostawy_min: i64,
    pub dni_dostawy_max: i64,
    #[serde(default)]
    pub domyslny_dostawca_id: Option<i64>,
}

pub async fn zmien_widelki_kategorii(pula: &SqlitePool, id: i64, w: WidelkiKategorii) -> Wynik<()> {
    sprawdz_widelki(w.dni_dostawy_min, w.dni_dostawy_max)?;
    let mut conn = pula.acquire().await?;
    if let Some(d) = w.domyslny_dostawca_id {
        istnieje(
            &mut conn,
            "SELECT id FROM dostawcy WHERE id = ?1",
            d,
            "dostawca",
        )
        .await?;
    }
    let n = sqlx::query(
        "UPDATE kategorie_materialow SET dni_dostawy_min = ?1, dni_dostawy_max = ?2, domyslny_dostawca_id = ?3 WHERE id = ?4",
    )
    .bind(w.dni_dostawy_min)
    .bind(w.dni_dostawy_max)
    .bind(w.domyslny_dostawca_id)
    .bind(id)
    .execute(&mut *conn)
    .await?
    .rows_affected();
    if n == 0 {
        return Err(BladApi::NieZnaleziono(format!("kategoria materiału {id}")));
    }
    dziennik(&mut conn, "kategorie_materialow", id, "zmiana").await
}

fn sprawdz_widelki(min: i64, max: i64) -> Wynik<()> {
    dni("dni_min", Some(min))?;
    dni("dni_max", Some(max))?;
    if min > max {
        return Err(BladApi::NieprawidloweDane(
            "minimalny czas dostawy nie może być większy niż maksymalny".into(),
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, FromRow, PartialEq)]
pub struct CzasDostawy {
    pub id: i64,
    pub kategoria_id: i64,
    pub kategoria_nazwa: String,
    pub dostawca_id: i64,
    pub dostawca_nazwa: String,
    pub dni_min: i64,
    pub dni_max: i64,
    pub dni_srednia_rzeczywista: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DaneCzasuDostawy {
    pub kategoria_id: i64,
    pub dostawca_id: i64,
    pub dni_min: i64,
    pub dni_max: i64,
}

const SQL_CZASY: &str = "SELECT c.id, c.kategoria_id, k.nazwa AS kategoria_nazwa, c.dostawca_id, d.nazwa AS dostawca_nazwa, \
     c.dni_min, c.dni_max, c.dni_srednia_rzeczywista FROM czasy_dostaw c \
     JOIN kategorie_materialow k ON k.id = c.kategoria_id JOIN dostawcy d ON d.id = c.dostawca_id";

pub async fn czasy_dostaw(pula: &SqlitePool) -> Wynik<Vec<CzasDostawy>> {
    Ok(
        sqlx::query_as(&format!("{SQL_CZASY} ORDER BY k.nazwa, d.nazwa"))
            .fetch_all(pula)
            .await?,
    )
}

async fn czas_dostawy(pula: &SqlitePool, id: i64) -> Wynik<CzasDostawy> {
    sqlx::query_as(&format!("{SQL_CZASY} WHERE c.id = ?1"))
        .bind(id)
        .fetch_optional(pula)
        .await?
        .ok_or_else(|| BladApi::NieZnaleziono(format!("czas dostawy {id}")))
}

pub async fn zapisz_czas_dostawy(
    pula: &SqlitePool,
    id: Option<i64>,
    d: DaneCzasuDostawy,
) -> Wynik<CzasDostawy> {
    sprawdz_widelki(d.dni_min, d.dni_max)?;
    let mut conn = pula.acquire().await?;
    istnieje(
        &mut conn,
        "SELECT id FROM kategorie_materialow WHERE id = ?1",
        d.kategoria_id,
        "kategoria materiału",
    )
    .await?;
    istnieje(
        &mut conn,
        "SELECT id FROM dostawcy WHERE id = ?1",
        d.dostawca_id,
        "dostawca",
    )
    .await?;
    let para: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM czasy_dostaw WHERE kategoria_id = ?1 AND dostawca_id = ?2 AND id IS NOT ?3",
    )
    .bind(d.kategoria_id)
    .bind(d.dostawca_id)
    .bind(id)
    .fetch_optional(&mut *conn)
    .await?;
    if let Some(p) = para {
        return Err(BladApi::NieprawidloweDane(format!(
            "czas dostawy dla tej pary kategoria + dostawca już istnieje (id {p})"
        )));
    }
    let id = match id {
        None => sqlx::query("INSERT INTO czasy_dostaw (kategoria_id, dostawca_id, dni_min, dni_max) VALUES (?1, ?2, ?3, ?4)")
            .bind(d.kategoria_id)
            .bind(d.dostawca_id)
            .bind(d.dni_min)
            .bind(d.dni_max)
            .execute(&mut *conn)
            .await?
            .last_insert_rowid(),
        Some(id) => {
            let n = sqlx::query("UPDATE czasy_dostaw SET kategoria_id = ?1, dostawca_id = ?2, dni_min = ?3, dni_max = ?4 WHERE id = ?5")
                .bind(d.kategoria_id)
                .bind(d.dostawca_id)
                .bind(d.dni_min)
                .bind(d.dni_max)
                .bind(id)
                .execute(&mut *conn)
                .await?
                .rows_affected();
            if n == 0 {
                return Err(BladApi::NieZnaleziono(format!("czas dostawy {id}")));
            }
            id
        }
    };
    dziennik(&mut conn, "czasy_dostaw", id, "zmiana").await?;
    drop(conn);
    czas_dostawy(pula, id).await
}

pub async fn usun_czas_dostawy(pula: &SqlitePool, id: i64) -> Wynik<()> {
    let n = sqlx::query("DELETE FROM czasy_dostaw WHERE id = ?1")
        .bind(id)
        .execute(pula)
        .await?
        .rows_affected();
    if n == 0 {
        return Err(BladApi::NieZnaleziono(format!("czas dostawy {id}")));
    }
    Ok(())
}

// ===========================================================================
// Zlecenia
// ===========================================================================

#[derive(Debug, Clone, Serialize, FromRow, PartialEq)]
pub struct Zlecenie {
    pub id: i64,
    pub klient_id: i64,
    pub umowa_id: Option<i64>,
    pub nazwa: String,
    pub etap: String,
    pub montaz_od: Option<String>,
    pub czas_warsztat_dni: Option<i64>,
    pub czas_montazu_dni: Option<i64>,
    pub odstep_materialy_dni: Option<i64>,
    pub odstep_rysunki_dni: Option<i64>,
    pub odstep_pomiar_dni: Option<i64>,
    pub utworzono: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DaneZlecenia {
    pub klient_id: i64,
    #[serde(default)]
    pub umowa_id: Option<i64>,
    pub nazwa: String,
    #[serde(default)]
    pub etap: Option<String>,
    #[serde(default)]
    pub montaz_od: Option<NaiveDate>,
    #[serde(default)]
    pub czas_warsztat_dni: Option<i64>,
    #[serde(default)]
    pub czas_montazu_dni: Option<i64>,
    #[serde(default)]
    pub odstep_materialy_dni: Option<i64>,
    #[serde(default)]
    pub odstep_rysunki_dni: Option<i64>,
    #[serde(default)]
    pub odstep_pomiar_dni: Option<i64>,
}

const KOLUMNY_ZLECENIA: &str =
    "id, klient_id, umowa_id, nazwa, etap, montaz_od, czas_warsztat_dni, czas_montazu_dni, \
     odstep_materialy_dni, odstep_rysunki_dni, odstep_pomiar_dni, utworzono";

pub async fn zlecenia(pula: &SqlitePool) -> Wynik<Vec<Zlecenie>> {
    Ok(sqlx::query_as(&format!(
        "SELECT {KOLUMNY_ZLECENIA} FROM zlecenia ORDER BY montaz_od IS NULL, montaz_od, id"
    ))
    .fetch_all(pula)
    .await?)
}

pub async fn zlecenie(conn: &mut SqliteConnection, id: i64) -> Wynik<Zlecenie> {
    sqlx::query_as(&format!(
        "SELECT {KOLUMNY_ZLECENIA} FROM zlecenia WHERE id = ?1"
    ))
    .bind(id)
    .fetch_optional(&mut *conn)
    .await?
    .ok_or_else(|| BladApi::NieZnaleziono(format!("zlecenie {id}")))
}

pub async fn zapisz_zlecenie(
    pula: &SqlitePool,
    id: Option<i64>,
    d: DaneZlecenia,
) -> Wynik<Zlecenie> {
    let nazwa = d.nazwa.trim().to_string();
    if nazwa.is_empty() {
        return Err(BladApi::NieprawidloweDane(
            "nazwa zlecenia jest wymagana".into(),
        ));
    }
    let etap = d.etap.unwrap_or_else(|| "umowa".into());
    if !ETAPY_ZLECENIA.contains(&etap.as_str()) {
        return Err(BladApi::NieprawidloweDane(format!(
            "nieznany etap „{etap}” (dozwolone: {})",
            ETAPY_ZLECENIA.join(", ")
        )));
    }
    dni("czas_warsztat_dni", d.czas_warsztat_dni)?;
    dni("czas_montazu_dni", d.czas_montazu_dni)?;
    if d.czas_montazu_dni == Some(0) {
        return Err(BladApi::NieprawidloweDane(
            "czas montażu musi wynosić co najmniej 1 dzień".into(),
        ));
    }
    dni("odstep_materialy_dni", d.odstep_materialy_dni)?;
    dni("odstep_rysunki_dni", d.odstep_rysunki_dni)?;
    dni("odstep_pomiar_dni", d.odstep_pomiar_dni)?;
    let mut tx = pula.begin().await?;
    istnieje(
        &mut tx,
        "SELECT id FROM klienci WHERE id = ?1 AND usunieto IS NULL",
        d.klient_id,
        "klient",
    )
    .await?;
    if let Some(u) = d.umowa_id {
        istnieje(&mut tx, "SELECT id FROM umowy WHERE id = ?1", u, "umowa").await?;
        let inne: Option<i64> =
            sqlx::query_scalar("SELECT id FROM zlecenia WHERE umowa_id = ?1 AND id IS NOT ?2")
                .bind(u)
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?;
        if let Some(inne) = inne {
            return Err(BladApi::NieprawidloweDane(format!(
                "umowa {u} ma już zlecenie {inne}"
            )));
        }
    }
    let montaz = d.montaz_od.map(tekst);
    let id = match id {
        None => sqlx::query(
            "INSERT INTO zlecenia (klient_id, umowa_id, nazwa, etap, montaz_od, czas_warsztat_dni, czas_montazu_dni, \
             odstep_materialy_dni, odstep_rysunki_dni, odstep_pomiar_dni) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        )
        .bind(d.klient_id)
        .bind(d.umowa_id)
        .bind(&nazwa)
        .bind(&etap)
        .bind(&montaz)
        .bind(d.czas_warsztat_dni)
        .bind(d.czas_montazu_dni)
        .bind(d.odstep_materialy_dni)
        .bind(d.odstep_rysunki_dni)
        .bind(d.odstep_pomiar_dni)
        .execute(&mut *tx)
        .await?
        .last_insert_rowid(),
        Some(id) => {
            let n = sqlx::query(
                "UPDATE zlecenia SET klient_id = ?1, umowa_id = ?2, nazwa = ?3, etap = ?4, montaz_od = ?5, \
                 czas_warsztat_dni = ?6, czas_montazu_dni = ?7, odstep_materialy_dni = ?8, odstep_rysunki_dni = ?9, \
                 odstep_pomiar_dni = ?10 WHERE id = ?11",
            )
            .bind(d.klient_id)
            .bind(d.umowa_id)
            .bind(&nazwa)
            .bind(&etap)
            .bind(&montaz)
            .bind(d.czas_warsztat_dni)
            .bind(d.czas_montazu_dni)
            .bind(d.odstep_materialy_dni)
            .bind(d.odstep_rysunki_dni)
            .bind(d.odstep_pomiar_dni)
            .bind(id)
            .execute(&mut *tx)
            .await?
            .rows_affected();
            if n == 0 {
                return Err(BladApi::NieZnaleziono(format!("zlecenie {id}")));
            }
            id
        }
    };
    dziennik(&mut tx, "zlecenia", id, "zmiana").await?;
    let z = zlecenie(&mut tx, id).await?;
    tx.commit().await?;
    Ok(z)
}

// ===========================================================================
// Materiały zlecenia (zamowienia_materialow)
// ===========================================================================

#[derive(Debug, Clone, Serialize, FromRow, PartialEq)]
pub struct Material {
    pub id: i64,
    pub zlecenie_id: i64,
    pub kategoria_id: i64,
    pub kategoria_nazwa: String,
    /// Dostawca wpisany przy materiale (NULL = domyślny dostawca kategorii).
    pub dostawca_id: Option<i64>,
    /// Dostawca faktycznie użyty (wpisany albo domyślny kategorii).
    pub dostawca_efektywny_id: Option<i64>,
    pub dostawca_nazwa: Option<String>,
    pub opis: Option<String>,
    pub status: String,
    pub data_wymaganej_dostawy: Option<String>,
    pub data_sweet_spot: Option<String>,
    pub data_planowa: Option<String>,
    pub data_krytyczna: Option<String>,
    pub dni_min: Option<i64>,
    pub dni_max: Option<i64>,
    pub dni_srednia: Option<i64>,
    pub data_zamowienia: Option<String>,
    pub data_dostawy: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DaneMaterialu {
    pub kategoria_id: i64,
    #[serde(default)]
    pub dostawca_id: Option<i64>,
    #[serde(default)]
    pub opis: Option<String>,
    #[serde(default)]
    pub data_zamowienia: Option<NaiveDate>,
    #[serde(default)]
    pub data_dostawy: Option<NaiveDate>,
}

const SQL_MATERIALY: &str = "SELECT m.id, m.zlecenie_id, m.kategoria_id, k.nazwa AS kategoria_nazwa, m.dostawca_id, \
     COALESCE(m.dostawca_id, k.domyslny_dostawca_id) AS dostawca_efektywny_id, d.nazwa AS dostawca_nazwa, m.opis, m.status, \
     m.data_wymaganej_dostawy, m.data_sweet_spot, m.data_planowa, m.data_krytyczna, m.dni_min, m.dni_max, m.dni_srednia, \
     m.data_zamowienia, m.data_dostawy \
     FROM zamowienia_materialow m JOIN kategorie_materialow k ON k.id = m.kategoria_id \
     LEFT JOIN dostawcy d ON d.id = COALESCE(m.dostawca_id, k.domyslny_dostawca_id)";

pub async fn materialy(conn: &mut SqliteConnection, zlecenie_id: i64) -> Wynik<Vec<Material>> {
    Ok(sqlx::query_as(&format!(
        "{SQL_MATERIALY} WHERE m.zlecenie_id = ?1 ORDER BY m.id"
    ))
    .bind(zlecenie_id)
    .fetch_all(&mut *conn)
    .await?)
}

pub async fn material(conn: &mut SqliteConnection, id: i64) -> Wynik<Material> {
    sqlx::query_as(&format!("{SQL_MATERIALY} WHERE m.id = ?1"))
        .bind(id)
        .fetch_optional(&mut *conn)
        .await?
        .ok_or_else(|| BladApi::NieZnaleziono(format!("materiał (zamówienie) {id}")))
}

fn status_materialu(zam: Option<NaiveDate>, dost: Option<NaiveDate>) -> Wynik<&'static str> {
    match (zam, dost) {
        (None, Some(_)) => Err(BladApi::NieprawidloweDane(
            "data dostawy wymaga daty zamówienia".into(),
        )),
        (Some(z), Some(d)) if d < z => Err(BladApi::NieprawidloweDane(
            "data dostawy nie może być wcześniejsza niż data zamówienia".into(),
        )),
        (Some(_), Some(_)) => Ok("dostarczone"),
        (Some(_), None) => Ok("zamowione"),
        (None, None) => Ok("do_zamowienia"),
    }
}

/// Dodaje (`id = None`) lub zmienia materiał zlecenia. Zwraca materiał i id zlecenia.
pub async fn zapisz_material(
    pula: &SqlitePool,
    zlecenie_id: Option<i64>,
    id: Option<i64>,
    d: DaneMaterialu,
) -> Wynik<Material> {
    let status = status_materialu(d.data_zamowienia, d.data_dostawy)?;
    let mut tx = pula.begin().await?;
    istnieje(
        &mut tx,
        "SELECT id FROM kategorie_materialow WHERE id = ?1",
        d.kategoria_id,
        "kategoria materiału",
    )
    .await?;
    if let Some(dost) = d.dostawca_id {
        istnieje(
            &mut tx,
            "SELECT id FROM dostawcy WHERE id = ?1",
            dost,
            "dostawca",
        )
        .await?;
    }
    let opis = d
        .opis
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let (zam, dost) = (d.data_zamowienia.map(tekst), d.data_dostawy.map(tekst));
    let id = match (id, zlecenie_id) {
        (None, Some(zl)) => {
            zlecenie(&mut tx, zl).await?;
            sqlx::query(
                "INSERT INTO zamowienia_materialow (zlecenie_id, kategoria_id, dostawca_id, opis, status, data_zamowienia, data_dostawy) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )
            .bind(zl)
            .bind(d.kategoria_id)
            .bind(d.dostawca_id)
            .bind(&opis)
            .bind(status)
            .bind(&zam)
            .bind(&dost)
            .execute(&mut *tx)
            .await?
            .last_insert_rowid()
        }
        (Some(id), _) => {
            let n = sqlx::query(
                "UPDATE zamowienia_materialow SET kategoria_id = ?1, dostawca_id = ?2, opis = ?3, status = ?4, \
                 data_zamowienia = ?5, data_dostawy = ?6 WHERE id = ?7",
            )
            .bind(d.kategoria_id)
            .bind(d.dostawca_id)
            .bind(&opis)
            .bind(status)
            .bind(&zam)
            .bind(&dost)
            .bind(id)
            .execute(&mut *tx)
            .await?
            .rows_affected();
            if n == 0 {
                return Err(BladApi::NieZnaleziono(format!(
                    "materiał (zamówienie) {id}"
                )));
            }
            // zadanie „zamówić” ma ten sam stan wykonania co materiał
            sqlx::query("UPDATE zadania_kalendarza SET wykonano = ?1 WHERE zamowienie_id = ?2 AND klucz LIKE 'zamowienie:%'")
                .bind(&zam)
                .bind(id)
                .execute(&mut *tx)
                .await?;
            id
        }
        (None, None) => return Err(BladApi::NieprawidloweDane("brak zlecenia".into())),
    };
    dziennik(&mut tx, "zamowienia_materialow", id, "zmiana").await?;
    if d.data_dostawy.is_some() {
        aktualizuj_srednia_rzeczywista(&mut tx, id).await?;
    }
    let m = material(&mut tx, id).await?;
    tx.commit().await?;
    Ok(m)
}

pub async fn usun_material(pula: &SqlitePool, id: i64) -> Wynik<i64> {
    let mut tx = pula.begin().await?;
    let m = material(&mut tx, id).await?;
    sqlx::query("DELETE FROM zamowienia_materialow WHERE id = ?1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    dziennik(&mut tx, "zamowienia_materialow", id, "usuniecie").await?;
    tx.commit().await?;
    Ok(m.zlecenie_id)
}

// ===========================================================================
// Harmonogram: wczytanie wejścia, liczenie, zapis
// ===========================================================================

#[derive(FromRow)]
struct WierszMaterialuWejscia {
    id: i64,
    kategoria_id: i64,
    kategoria_nazwa: String,
    dostawca_id: Option<i64>,
    opis: Option<String>,
    para_min: Option<i64>,
    para_max: Option<i64>,
    kat_min: i64,
    kat_max: i64,
    data_zamowienia: Option<String>,
    data_dostawy: Option<String>,
}

/// Wiersze specyfikacji „do potwierdzenia”: (zlecenie, kategoria) → id wierszy.
async fn blokady(
    conn: &mut SqliteConnection,
    zlecenie_id: Option<i64>,
) -> Wynik<HashMap<(i64, i64), Vec<i64>>> {
    let wiersze: Vec<(i64, i64, i64)> = sqlx::query_as(
        "SELECT z.id, w.kategoria_materialu_id, w.id FROM wiersze_specyfikacji w \
         JOIN czesci_specyfikacji c ON c.id = w.czesc_id \
         JOIN sekcje_specyfikacji s ON s.id = c.sekcja_id \
         JOIN umowy u ON u.specyfikacja_id = s.specyfikacja_id \
         JOIN zlecenia z ON z.umowa_id = u.id \
         WHERE w.status = 'do_potwierdzenia' AND w.kategoria_materialu_id IS NOT NULL AND (?1 IS NULL OR z.id = ?1) \
         ORDER BY w.id",
    )
    .bind(zlecenie_id)
    .fetch_all(&mut *conn)
    .await?;
    let mut mapa: HashMap<(i64, i64), Vec<i64>> = HashMap::new();
    for (z, k, w) in wiersze {
        mapa.entry((z, k)).or_default().push(w);
    }
    Ok(mapa)
}

/// Dane wejściowe silnika dla zlecenia. Błąd 422, gdy brak terminu lub czasów.
pub async fn wejscie(
    conn: &mut SqliteConnection,
    id: i64,
) -> Wynik<(ZlecenieWejscie, Vec<MaterialWejscie>)> {
    let z = zlecenie(conn, id).await?;
    let brak = |co: &str| {
        BladApi::NieprawidloweDane(format!(
            "zlecenie {id}: uzupełnij {co}, aby policzyć harmonogram"
        ))
    };
    let montaz_od = data(
        z.montaz_od
            .as_deref()
            .ok_or_else(|| brak("termin rozpoczęcia montażu (montaz_od)"))?,
    )?;
    let czas_montazu = z
        .czas_montazu_dni
        .ok_or_else(|| brak("czas montażu (czas_montazu_dni)"))?;
    let czas_warsztatu = z
        .czas_warsztat_dni
        .ok_or_else(|| brak("czas warsztatu (czas_warsztat_dni)"))?;

    let (data_umowy, dni_info) = match z.umowa_id {
        Some(u) => {
            let r: Option<(Option<String>, Option<i64>)> = sqlx::query_as(
                "SELECT data_zawarcia, dni_na_brakujace_informacje FROM umowy WHERE id = ?1",
            )
            .bind(u)
            .fetch_optional(&mut *conn)
            .await?;
            let (dz, n) = r.unwrap_or((None, None));
            (data_opt(dz.as_deref())?, n.map(u32_z))
        }
        None => (None, None),
    };
    let platnosci: Vec<(i64, String, Option<String>, String)> = sqlx::query_as(
        "SELECT p.id, p.rodzaj, p.opis, p.termin FROM platnosci_planowane p JOIN zlecenia z ON z.umowa_id = p.umowa_id \
         WHERE z.id = ?1 AND p.termin IS NOT NULL ORDER BY p.kolejnosc, p.id",
    )
    .bind(id)
    .fetch_all(&mut *conn)
    .await?;
    let platnosci = platnosci
        .into_iter()
        .filter_map(|(pid, rodzaj, opis, termin)| {
            data(&termin).ok().map(|t| PlatnoscWejscie {
                id: pid,
                opis: opis.unwrap_or(rodzaj.replace('_', " ")),
                termin: t,
            })
        })
        .collect();

    let wejscie_zlecenia = ZlecenieWejscie {
        id,
        nazwa: z.nazwa.clone(),
        montaz_od,
        czas_montazu_dni: u32_z(czas_montazu),
        czas_warsztatu_dni: u32_z(czas_warsztatu),
        odstep_materialy_dni: z.odstep_materialy_dni.map(u32_z),
        odstep_rysunki_dni: z.odstep_rysunki_dni.map(u32_z),
        odstep_pomiar_dni: z.odstep_pomiar_dni.map(u32_z),
        data_umowy,
        dni_na_brakujace_informacje: dni_info,
        platnosci,
    };

    let wiersze: Vec<WierszMaterialuWejscia> = sqlx::query_as(
        "SELECT m.id, m.kategoria_id, k.nazwa AS kategoria_nazwa, COALESCE(m.dostawca_id, k.domyslny_dostawca_id) AS dostawca_id, \
         m.opis, c.dni_min AS para_min, c.dni_max AS para_max, k.dni_dostawy_min AS kat_min, k.dni_dostawy_max AS kat_max, \
         m.data_zamowienia, m.data_dostawy \
         FROM zamowienia_materialow m JOIN kategorie_materialow k ON k.id = m.kategoria_id \
         LEFT JOIN czasy_dostaw c ON c.kategoria_id = m.kategoria_id AND c.dostawca_id = COALESCE(m.dostawca_id, k.domyslny_dostawca_id) \
         WHERE m.zlecenie_id = ?1 ORDER BY m.id",
    )
    .bind(id)
    .fetch_all(&mut *conn)
    .await?;
    let blok = blokady(conn, Some(id)).await?;
    let mut mats = Vec::with_capacity(wiersze.len());
    for w in wiersze {
        // Para kategoria + dostawca ma pierwszeństwo; domyślne widełki kategorii 0–0 oznaczają „nie ustawiono”.
        let (widelki, zrodlo) = match (w.para_min, w.para_max) {
            (Some(min), Some(max)) => (
                Some(Widelki {
                    min: u32_z(min),
                    max: u32_z(max),
                }),
                ZrodloWidelek::KategoriaDostawca,
            ),
            _ if w.kat_max > 0 => (
                Some(Widelki {
                    min: u32_z(w.kat_min),
                    max: u32_z(w.kat_max),
                }),
                ZrodloWidelek::Kategoria,
            ),
            _ => (None, ZrodloWidelek::Brak),
        };
        mats.push(MaterialWejscie {
            id: w.id,
            kategoria_id: w.kategoria_id,
            kategoria_nazwa: w.kategoria_nazwa,
            dostawca_id: w.dostawca_id,
            opis: w.opis,
            widelki,
            zrodlo_widelek: zrodlo,
            data_zamowienia: data_opt(w.data_zamowienia.as_deref())?,
            data_dostawy: data_opt(w.data_dostawy.as_deref())?,
            wiersze_do_potwierdzenia: blok
                .get(&(id, w.kategoria_id))
                .map(|v| v.len() as u32)
                .unwrap_or(0),
        });
    }
    Ok((wejscie_zlecenia, mats))
}

/// Harmonogram bez zapisu (podgląd).
pub async fn podglad(pula: &SqlitePool, id: i64) -> Wynik<Harmonogram> {
    let mut conn = pula.acquire().await?;
    let (u, kal) = kalendarz_dni(&mut conn).await?;
    let (z, m) = wejscie(&mut conn, id).await?;
    kalendarz::policz_z_kalendarzem(&z, &m, &u, &kal)
        .map_err(|e| BladApi::NieprawidloweDane(e.to_string()))
}

#[derive(Debug, Clone, Serialize)]
pub struct WynikGenerowania {
    pub harmonogram: Harmonogram,
    pub dodane: usize,
    pub zmienione: usize,
    pub usuniete: usize,
    /// Zadania pozostawione bez zmian (wykonane lub wpisane ręcznie).
    pub zamrozone: usize,
}

/// Liczy harmonogram i zapisuje zadania zlecenia (wykonane i ręczne zostają bez zmian).
pub async fn generuj(pula: &SqlitePool, id: i64) -> Wynik<WynikGenerowania> {
    let mut tx = pula.begin().await?;
    let (u, kal) = kalendarz_dni(&mut tx).await?;
    let wynik = generuj_w(&mut tx, id, &u, &kal).await?;
    tx.commit().await?;
    Ok(wynik)
}

async fn generuj_w(
    conn: &mut SqliteConnection,
    id: i64,
    u: &UstawieniaTerminow,
    kal: &KalendarzDni,
) -> Wynik<WynikGenerowania> {
    let (z, m) = wejscie(conn, id).await?;
    let h = kalendarz::policz_z_kalendarzem(&z, &m, u, kal)
        .map_err(|e| BladApi::NieprawidloweDane(e.to_string()))?;

    let istniejace: Vec<(i64, Option<String>, bool, Option<String>)> = sqlx::query_as(
        "SELECT id, klucz, wyliczone, wykonano FROM zadania_kalendarza WHERE zlecenie_id = ?1",
    )
    .bind(id)
    .fetch_all(&mut *conn)
    .await?;
    let istniejace: Vec<ZadanieIstniejace> = istniejace
        .into_iter()
        .map(|(id, klucz, wyliczone, wykonano)| ZadanieIstniejace {
            id,
            klucz,
            wyliczone,
            wykonane: wykonano.is_some(),
        })
        .collect();
    let plan = scal(h.zadania.clone(), &istniejace);

    for zid in &plan.do_usuniecia {
        sqlx::query("DELETE FROM zadania_kalendarza WHERE id = ?1")
            .bind(zid)
            .execute(&mut *conn)
            .await?;
        dziennik(conn, "zadania_kalendarza", *zid, "usuniecie").await?;
    }
    for (zid, zad) in &plan.do_zmiany {
        sqlx::query(
            "UPDATE zadania_kalendarza SET zamowienie_id = ?1, typ = ?2, tytul = ?3, data_od = ?4, data_do = ?5, \
             data_sweet_spot = ?6, data_planowa = ?7, data_krytyczna = ?8, wykonano = COALESCE(?9, wykonano) WHERE id = ?10",
        )
        .bind(zad.zamowienie_id)
        .bind(zad.typ.jako_tekst())
        .bind(&zad.tytul)
        .bind(tekst(zad.data_od))
        .bind(zad.data_do.map(tekst))
        .bind(zad.data_sweet_spot.map(tekst))
        .bind(zad.data_planowa.map(tekst))
        .bind(zad.data_krytyczna.map(tekst))
        .bind(zad.wykonano.map(tekst))
        .bind(zid)
        .execute(&mut *conn)
        .await?;
        dziennik(conn, "zadania_kalendarza", *zid, "zmiana").await?;
    }
    for zad in &plan.do_dodania {
        let nowe = wstaw_zadanie(conn, Some(id), zad, true).await?;
        dziennik(conn, "zadania_kalendarza", nowe, "dodanie").await?;
    }
    // migawka terminów w materiałach niezamówionych (zamówione są zamrożone)
    for t in &h.materialy {
        sqlx::query(
            "UPDATE zamowienia_materialow SET data_wymaganej_dostawy = ?1, data_sweet_spot = ?2, data_planowa = ?3, \
             data_krytyczna = ?4, dni_min = ?5, dni_max = ?6, dni_srednia = ?7 WHERE id = ?8 AND data_zamowienia IS NULL",
        )
        .bind(tekst(t.data_wymaganej_dostawy))
        .bind(t.data_sweet_spot.map(tekst))
        .bind(t.data_planowa.map(tekst))
        .bind(t.data_krytyczna.map(tekst))
        .bind(t.widelki.map(|w| i64::from(w.min)))
        .bind(t.widelki.map(|w| i64::from(w.max)))
        .bind(t.dni_srednia.map(i64::from))
        .bind(t.material_id)
        .execute(&mut *conn)
        .await?;
    }
    Ok(WynikGenerowania {
        dodane: plan.do_dodania.len(),
        zmienione: plan.do_zmiany.len(),
        usuniete: plan.do_usuniecia.len(),
        zamrozone: plan.zamrozone.len(),
        harmonogram: h,
    })
}

async fn wstaw_zadanie(
    conn: &mut SqliteConnection,
    zlecenie_id: Option<i64>,
    zad: &ZadanieWyliczone,
    wyliczone: bool,
) -> Wynik<i64> {
    Ok(sqlx::query(
        "INSERT INTO zadania_kalendarza (zlecenie_id, zamowienie_id, typ, klucz, tytul, data_od, data_do, data_sweet_spot, \
         data_planowa, data_krytyczna, wyliczone, wykonano) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
    )
    .bind(zlecenie_id)
    .bind(zad.zamowienie_id)
    .bind(zad.typ.jako_tekst())
    .bind(if wyliczone { Some(&zad.klucz) } else { None })
    .bind(&zad.tytul)
    .bind(tekst(zad.data_od))
    .bind(zad.data_do.map(tekst))
    .bind(zad.data_sweet_spot.map(tekst))
    .bind(zad.data_planowa.map(tekst))
    .bind(zad.data_krytyczna.map(tekst))
    .bind(wyliczone)
    .bind(zad.wykonano.map(tekst))
    .execute(&mut *conn)
    .await?
    .last_insert_rowid())
}

async fn ma_harmonogram(conn: &mut SqliteConnection, id: i64) -> Wynik<bool> {
    let n: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM zadania_kalendarza WHERE zlecenie_id = ?1 AND klucz IS NOT NULL",
    )
    .bind(id)
    .fetch_one(&mut *conn)
    .await?;
    Ok(n > 0)
}

/// Przelicza harmonogram, jeśli zlecenie już go ma (po zmianie zlecenia lub materiałów).
/// Zwraca `true`, gdy przeliczono. Błąd danych (np. usunięty termin) zostawia stare zadania.
pub async fn przelicz_jesli_jest(pula: &SqlitePool, id: i64) -> Wynik<bool> {
    let mut tx = pula.begin().await?;
    if !ma_harmonogram(&mut tx, id).await? {
        return Ok(false);
    }
    let (u, kal) = kalendarz_dni(&mut tx).await?;
    match generuj_w(&mut tx, id, &u, &kal).await {
        Ok(_) => {
            tx.commit().await?;
            Ok(true)
        }
        Err(BladApi::NieprawidloweDane(m)) => {
            tracing::warn!("Nie przeliczono harmonogramu zlecenia {id}: {m}");
            Ok(false)
        }
        Err(e) => Err(e),
    }
}

/// Przelicza wszystkie zlecenia z harmonogramem (po zmianie ustawień, dni wolnych, widełek).
/// Zwraca id przeliczonych zleceń.
pub async fn przelicz_wszystkie(pula: &SqlitePool) -> Wynik<Vec<i64>> {
    let mut tx = pula.begin().await?;
    let ids: Vec<i64> = sqlx::query_scalar(
        "SELECT DISTINCT t.zlecenie_id FROM zadania_kalendarza t JOIN zlecenia z ON z.id = t.zlecenie_id \
         WHERE t.klucz IS NOT NULL AND z.etap <> 'zakonczone' ORDER BY 1",
    )
    .fetch_all(&mut *tx)
    .await?;
    let (u, kal) = kalendarz_dni(&mut tx).await?;
    let mut przeliczone = Vec::new();
    for id in ids {
        match generuj_w(&mut tx, id, &u, &kal).await {
            Ok(_) => przeliczone.push(id),
            Err(BladApi::NieprawidloweDane(m)) => {
                tracing::warn!("Nie przeliczono zlecenia {id}: {m}")
            }
            Err(e) => return Err(e),
        }
    }
    tx.commit().await?;
    Ok(przeliczone)
}

// ===========================================================================
// Zadania kalendarza: odczyt ze stanami, wykonanie, zadania ręczne
// ===========================================================================

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct InfoZamowienia {
    pub kategoria_id: i64,
    pub kategoria_nazwa: String,
    pub dostawca_id: Option<i64>,
    pub dostawca_nazwa: Option<String>,
    pub opis: Option<String>,
    pub status: String,
    pub data_wymaganej_dostawy: Option<String>,
    pub dni_min: Option<i64>,
    pub dni_max: Option<i64>,
    pub dni_srednia: Option<i64>,
    pub data_zamowienia: Option<String>,
    pub data_dostawy: Option<String>,
    /// Zamówione, niedostarczone: data zamówienia + średni czas dostawy.
    pub oczekiwana_dostawa: Option<String>,
    /// Oczekiwana dostawa później niż data wymaganej dostawy.
    pub dostawa_zagrozona: bool,
    /// Wiersze specyfikacji „do potwierdzenia” w tej kategorii blokują zamówienie.
    pub zablokowane: bool,
    pub potwierdzic_do: Option<String>,
    pub wiersze_do_potwierdzenia: Vec<i64>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ZadanieKalendarza {
    pub id: i64,
    pub zlecenie_id: Option<i64>,
    pub zlecenie_nazwa: Option<String>,
    pub zamowienie_id: Option<i64>,
    pub typ: String,
    pub klucz: Option<String>,
    pub tytul: String,
    pub data_od: String,
    pub data_do: Option<String>,
    pub data_sweet_spot: Option<String>,
    pub data_planowa: Option<String>,
    pub data_krytyczna: Option<String>,
    /// Termin, od którego liczy się stan: planowa (zamówienie) albo data_do / data_od.
    pub termin: String,
    pub wyliczone: bool,
    pub wykonano: Option<String>,
    pub uwagi: Option<String>,
    pub stan: StanZadania,
    pub zamowienie: Option<InfoZamowienia>,
}

#[derive(FromRow)]
struct WierszZadania {
    id: i64,
    zlecenie_id: Option<i64>,
    zlecenie_nazwa: Option<String>,
    montaz_od: Option<String>,
    zamowienie_id: Option<i64>,
    typ: String,
    klucz: Option<String>,
    tytul: String,
    data_od: String,
    data_do: Option<String>,
    data_sweet_spot: Option<String>,
    data_planowa: Option<String>,
    data_krytyczna: Option<String>,
    wyliczone: bool,
    wykonano: Option<String>,
    uwagi: Option<String>,
    kategoria_id: Option<i64>,
    kategoria_nazwa: Option<String>,
    dostawca_id: Option<i64>,
    dostawca_nazwa: Option<String>,
    zam_opis: Option<String>,
    zam_status: Option<String>,
    data_wymaganej_dostawy: Option<String>,
    dni_min: Option<i64>,
    dni_max: Option<i64>,
    dni_srednia: Option<i64>,
    data_zamowienia: Option<String>,
    data_dostawy: Option<String>,
}

/// Filtr listy zadań. Zakres dat: zadania, których okres `[data_od, data_do]` nachodzi na `[od, do]`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct FiltrZadan {
    pub od: Option<NaiveDate>,
    #[serde(rename = "do")]
    pub do_: Option<NaiveDate>,
    /// Typ albo kilka typów po przecinku, np. `zamowienie,pomiar`.
    pub typ: Option<String>,
    pub zlecenie_id: Option<i64>,
    /// Stan albo kilka stanów po przecinku, np. `zolty,pomaranczowy,czerwony`.
    pub stan: Option<String>,
    pub tylko_niewykonane: bool,
    /// Data „dziś” do wyliczenia stanów (domyślnie dzisiejsza data serwera).
    pub dzis: Option<NaiveDate>,
}

fn lista_wartosci<T>(
    t: Option<&str>,
    parsuj: impl Fn(&str) -> Option<T>,
    co: &str,
) -> Wynik<Option<Vec<T>>> {
    let Some(t) = t.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    t.split(',')
        .map(|s| {
            let s = s.trim();
            parsuj(s).ok_or_else(|| BladApi::NieprawidloweDane(format!("nieznany {co} „{s}”")))
        })
        .collect::<Wynik<Vec<T>>>()
        .map(Some)
}

pub async fn zadania(
    pula: &SqlitePool,
    f: &FiltrZadan,
    dzis: NaiveDate,
) -> Wynik<Vec<ZadanieKalendarza>> {
    let typy = lista_wartosci(f.typ.as_deref(), TypZadania::z_tekstu, "typ zadania")?;
    let stany = lista_wartosci(f.stan.as_deref(), Stan::z_tekstu, "stan")?;
    let mut conn = pula.acquire().await?;
    let wiersze: Vec<WierszZadania> = sqlx::query_as(
        "SELECT t.id, t.zlecenie_id, z.nazwa AS zlecenie_nazwa, z.montaz_od, t.zamowienie_id, t.typ, t.klucz, t.tytul, \
         t.data_od, t.data_do, t.data_sweet_spot, t.data_planowa, t.data_krytyczna, t.wyliczone, t.wykonano, t.uwagi, \
         m.kategoria_id, k.nazwa AS kategoria_nazwa, COALESCE(m.dostawca_id, k.domyslny_dostawca_id) AS dostawca_id, \
         d.nazwa AS dostawca_nazwa, m.opis AS zam_opis, m.status AS zam_status, m.data_wymaganej_dostawy, m.dni_min, \
         m.dni_max, m.dni_srednia, m.data_zamowienia, m.data_dostawy \
         FROM zadania_kalendarza t \
         LEFT JOIN zlecenia z ON z.id = t.zlecenie_id \
         LEFT JOIN zamowienia_materialow m ON m.id = t.zamowienie_id AND t.typ = 'zamowienie' \
         LEFT JOIN kategorie_materialow k ON k.id = m.kategoria_id \
         LEFT JOIN dostawcy d ON d.id = COALESCE(m.dostawca_id, k.domyslny_dostawca_id) \
         WHERE (?1 IS NULL OR COALESCE(t.data_do, t.data_od) >= ?1) AND (?2 IS NULL OR t.data_od <= ?2) \
           AND (?3 IS NULL OR t.zlecenie_id = ?3) AND (?4 = 0 OR t.wykonano IS NULL) \
         ORDER BY t.data_od, t.id",
    )
    .bind(f.od.map(tekst))
    .bind(f.do_.map(tekst))
    .bind(f.zlecenie_id)
    .bind(f.tylko_niewykonane)
    .fetch_all(&mut *conn)
    .await?;
    let (u, kal) = kalendarz_dni(&mut conn).await?;
    let blok = blokady(&mut conn, f.zlecenie_id).await?;
    let mut wynik = Vec::with_capacity(wiersze.len());
    for w in wiersze {
        if let Some(t) = &typy {
            if !t.iter().any(|t| t.jako_tekst() == w.typ) {
                continue;
            }
        }
        let z = zbuduj_zadanie(w, dzis, &u, &kal, &blok)?;
        if let Some(s) = &stany {
            if !s.contains(&z.stan.stan) {
                continue;
            }
        }
        wynik.push(z);
    }
    Ok(wynik)
}

fn zbuduj_zadanie(
    w: WierszZadania,
    dzis: NaiveDate,
    u: &UstawieniaTerminow,
    kal: &KalendarzDni,
    blok: &HashMap<(i64, i64), Vec<i64>>,
) -> Wynik<ZadanieKalendarza> {
    let data_od = data(&w.data_od)?;
    let data_do = data_opt(w.data_do.as_deref())?;
    let planowa = data_opt(w.data_planowa.as_deref())?;
    let termin = if w.typ == "zamowienie" {
        planowa.unwrap_or(data_od)
    } else {
        data_do.unwrap_or(data_od)
    };
    let wykonano = data_opt(w.wykonano.as_deref())?;

    let zamowienie = match (w.kategoria_id, w.zlecenie_id) {
        (Some(kat), zl) => {
            let wiersze = zl
                .and_then(|zl| blok.get(&(zl, kat)))
                .cloned()
                .unwrap_or_default();
            let zamowione = w.data_zamowienia.is_some();
            let zablokowane = !wiersze.is_empty() && !zamowione;
            let wymagana = data_opt(w.data_wymaganej_dostawy.as_deref())?;
            let oczekiwana = match (
                data_opt(w.data_zamowienia.as_deref())?,
                &w.data_dostawy,
                w.dni_srednia,
            ) {
                (Some(zam), None, Some(sr)) => Some(kal.naprzod(zam, u32_z(sr))),
                _ => None,
            };
            Some(InfoZamowienia {
                kategoria_id: kat,
                kategoria_nazwa: w.kategoria_nazwa.clone().unwrap_or_default(),
                dostawca_id: w.dostawca_id,
                dostawca_nazwa: w.dostawca_nazwa.clone(),
                opis: w.zam_opis.clone(),
                status: w.zam_status.clone().unwrap_or_default(),
                data_wymaganej_dostawy: w.data_wymaganej_dostawy.clone(),
                dni_min: w.dni_min,
                dni_max: w.dni_max,
                dni_srednia: w.dni_srednia,
                data_zamowienia: w.data_zamowienia.clone(),
                data_dostawy: w.data_dostawy.clone(),
                oczekiwana_dostawa: oczekiwana.map(tekst),
                dostawa_zagrozona: matches!((oczekiwana, wymagana), (Some(o), Some(wy)) if o > wy),
                zablokowane,
                potwierdzic_do: if zablokowane {
                    w.data_planowa.clone()
                } else {
                    None
                },
                wiersze_do_potwierdzenia: wiersze,
            })
        }
        _ => None,
    };

    let stan = if let Some(k) = wykonano {
        stan_wykonane(k)
    } else if let (true, Some(p), Some(s), Some(kr), Some(zam)) = (
        w.typ == "zamowienie",
        planowa,
        data_opt(w.data_sweet_spot.as_deref())?,
        data_opt(w.data_krytyczna.as_deref())?,
        zamowienie.as_ref(),
    ) {
        match (
            data_opt(zam.data_wymaganej_dostawy.as_deref())?,
            zam.dni_srednia,
            data_opt(w.montaz_od.as_deref())?,
        ) {
            (Some(wymagana), Some(sr), Some(montaz)) => {
                let mut st = stan_zamowienia(
                    &DaneZamowienia {
                        sweet_spot: s,
                        planowa: p,
                        krytyczna: kr,
                        wymagana_dostawa: wymagana,
                        dni_srednia: u32_z(sr),
                        montaz_od: montaz,
                    },
                    dzis,
                    u,
                    kal,
                );
                if zam.zablokowane {
                    st.opis = format!(
                        "{} · wstrzymane: {} poz. do potwierdzenia przez klienta (do {p})",
                        st.opis,
                        zam.wiersze_do_potwierdzenia.len()
                    );
                }
                st
            }
            _ => stan_terminu(termin, dzis, u, kal),
        }
    } else {
        stan_terminu(termin, dzis, u, kal)
    };

    Ok(ZadanieKalendarza {
        id: w.id,
        zlecenie_id: w.zlecenie_id,
        zlecenie_nazwa: w.zlecenie_nazwa,
        zamowienie_id: w.zamowienie_id,
        typ: w.typ,
        klucz: w.klucz,
        tytul: w.tytul,
        data_od: w.data_od,
        data_do: w.data_do,
        data_sweet_spot: w.data_sweet_spot,
        data_planowa: w.data_planowa,
        data_krytyczna: w.data_krytyczna,
        termin: tekst(termin),
        wyliczone: w.wyliczone,
        wykonano: w.wykonano,
        uwagi: w.uwagi,
        stan,
        zamowienie,
    })
}

pub async fn zadanie(pula: &SqlitePool, id: i64, dzis: NaiveDate) -> Wynik<ZadanieKalendarza> {
    let zl: Option<Option<i64>> =
        sqlx::query_scalar("SELECT zlecenie_id FROM zadania_kalendarza WHERE id = ?1")
            .bind(id)
            .fetch_optional(pula)
            .await?;
    let zl = zl.ok_or_else(|| BladApi::NieZnaleziono(format!("zadanie {id}")))?;
    let f = FiltrZadan {
        zlecenie_id: zl,
        ..Default::default()
    };
    zadania(pula, &f, dzis)
        .await?
        .into_iter()
        .find(|z| z.id == id)
        .ok_or_else(|| BladApi::NieZnaleziono(format!("zadanie {id}")))
}

/// Oznaczenie wykonania. `wykonano: null` cofa wykonanie (zadanie znów będzie przeliczane).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Wykonanie {
    pub wykonano: Option<NaiveDate>,
    /// Tylko zamówienia: faktyczna data zamówienia (domyślnie = `wykonano`).
    pub data_zamowienia: Option<NaiveDate>,
    /// Tylko zamówienia: faktyczna data dostawy (można dopisać później kolejnym wywołaniem).
    pub data_dostawy: Option<NaiveDate>,
    pub uwagi: Option<String>,
}

/// (zlecenie_id, zamowienie_id, typ, klucz)
type DaneZadaniaDoWykonania = (Option<i64>, Option<i64>, String, Option<String>);

/// Zwraca id zlecenia zadania.
pub async fn wykonaj(pula: &SqlitePool, id: i64, w: Wykonanie) -> Wynik<Option<i64>> {
    let mut tx = pula.begin().await?;
    let r: Option<DaneZadaniaDoWykonania> = sqlx::query_as(
        "SELECT zlecenie_id, zamowienie_id, typ, klucz FROM zadania_kalendarza WHERE id = ?1",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    let (zlecenie_id, zamowienie_id, typ, klucz) =
        r.ok_or_else(|| BladApi::NieZnaleziono(format!("zadanie {id}")))?;
    let jest_zamowieniem = typ == "zamowienie"
        && klucz
            .as_deref()
            .is_some_and(|k| k.starts_with("zamowienie:"));
    if !jest_zamowieniem && (w.data_zamowienia.is_some() || w.data_dostawy.is_some()) {
        return Err(BladApi::NieprawidloweDane(
            "daty zamówienia i dostawy można podać tylko dla zadania zamówienia materiału".into(),
        ));
    }
    let uwagi = w.uwagi.map(|s| s.trim().to_string());
    let (wykonano, zam, dost) = match w.wykonano {
        None => (None, None, None),
        Some(d) => (
            Some(d),
            Some(w.data_zamowienia.unwrap_or(d)),
            w.data_dostawy,
        ),
    };
    sqlx::query(
        "UPDATE zadania_kalendarza SET wykonano = ?1, uwagi = COALESCE(?2, uwagi) WHERE id = ?3",
    )
    .bind(wykonano.map(tekst))
    .bind(uwagi.filter(|s| !s.is_empty()))
    .bind(id)
    .execute(&mut *tx)
    .await?;
    if let (true, Some(mid)) = (jest_zamowieniem, zamowienie_id) {
        let status = status_materialu(zam, dost)?;
        sqlx::query("UPDATE zamowienia_materialow SET data_zamowienia = ?1, data_dostawy = ?2, status = ?3 WHERE id = ?4")
            .bind(zam.map(tekst))
            .bind(dost.map(tekst))
            .bind(status)
            .bind(mid)
            .execute(&mut *tx)
            .await?;
        dziennik(&mut tx, "zamowienia_materialow", mid, "zmiana").await?;
        if dost.is_some() {
            aktualizuj_srednia_rzeczywista(&mut tx, mid).await?;
        }
    }
    dziennik(&mut tx, "zadania_kalendarza", id, "zmiana").await?;
    // cofnięcie wykonania: zadanie wraca pod przeliczanie
    if wykonano.is_none() {
        if let Some(zl) = zlecenie_id {
            if ma_harmonogram(&mut tx, zl).await? {
                let (u, kal) = kalendarz_dni(&mut tx).await?;
                if let Err(e) = generuj_w(&mut tx, zl, &u, &kal).await {
                    tracing::warn!("Nie przeliczono zlecenia {zl} po cofnięciu wykonania: {e}");
                }
            }
        }
    }
    tx.commit().await?;
    Ok(zlecenie_id)
}

/// Uaktualnia `czasy_dostaw.dni_srednia_rzeczywista` dla pary materiału (jeśli para istnieje).
/// Widełek nie zmienia – to tylko podpowiedź dla użytkownika.
async fn aktualizuj_srednia_rzeczywista(
    conn: &mut SqliteConnection,
    material_id: i64,
) -> Wynik<()> {
    let para: Option<(i64, Option<i64>)> = sqlx::query_as(
        "SELECT m.kategoria_id, COALESCE(m.dostawca_id, k.domyslny_dostawca_id) FROM zamowienia_materialow m \
         JOIN kategorie_materialow k ON k.id = m.kategoria_id WHERE m.id = ?1",
    )
    .bind(material_id)
    .fetch_optional(&mut *conn)
    .await?;
    let Some((kat, Some(dost))) = para else {
        return Ok(());
    };
    let (_, kal) = kalendarz_dni(conn).await?;
    let historia = historia_dostaw(conn, Some((kat, dost))).await?;
    if let Some(s) = historia
        .get(&(kat, Some(dost)))
        .and_then(|p| kalendarz::statystyka_dostaw(p, &kal))
    {
        sqlx::query("UPDATE czasy_dostaw SET dni_srednia_rzeczywista = ?1 WHERE kategoria_id = ?2 AND dostawca_id = ?3")
            .bind(s.srednia)
            .bind(kat)
            .bind(dost)
            .execute(&mut *conn)
            .await?;
    }
    Ok(())
}

type HistoriaDostaw = BTreeMap<(i64, Option<i64>), Vec<(NaiveDate, NaiveDate)>>;

async fn historia_dostaw(
    conn: &mut SqliteConnection,
    para: Option<(i64, i64)>,
) -> Wynik<HistoriaDostaw> {
    let wiersze: Vec<(i64, Option<i64>, String, String)> = sqlx::query_as(
        "SELECT m.kategoria_id, COALESCE(m.dostawca_id, k.domyslny_dostawca_id) AS dost, m.data_zamowienia, m.data_dostawy \
         FROM zamowienia_materialow m JOIN kategorie_materialow k ON k.id = m.kategoria_id \
         WHERE m.data_zamowienia IS NOT NULL AND m.data_dostawy IS NOT NULL \
           AND (?1 IS NULL OR (m.kategoria_id = ?1 AND COALESCE(m.dostawca_id, k.domyslny_dostawca_id) = ?2))",
    )
    .bind(para.map(|p| p.0))
    .bind(para.map(|p| p.1))
    .fetch_all(&mut *conn)
    .await?;
    let mut mapa: HistoriaDostaw = BTreeMap::new();
    for (k, d, z, dost) in wiersze {
        if let (Ok(z), Ok(dost)) = (data(&z), data(&dost)) {
            mapa.entry((k, d)).or_default().push((z, dost));
        }
    }
    Ok(mapa)
}

#[derive(Debug, Clone, Serialize)]
pub struct StatystykaPary {
    pub kategoria_id: i64,
    pub kategoria_nazwa: String,
    pub dostawca_id: Option<i64>,
    pub dostawca_nazwa: Option<String>,
    pub liczenie_dni: LiczenieDni,
    /// Widełki obecnie używane do liczenia (para albo domyślne kategorii).
    pub aktualne_widelki: Option<Widelki>,
    pub zrodlo_widelek: ZrodloWidelek,
    pub statystyka: StatystykaDostaw,
    /// Czy rzeczywiste min–max wykracza poza aktualne widełki (warto rozważyć korektę).
    pub sugerowana_korekta: bool,
}

pub async fn statystyka_dostaw(pula: &SqlitePool) -> Wynik<Vec<StatystykaPary>> {
    let mut conn = pula.acquire().await?;
    let (u, kal) = kalendarz_dni(&mut conn).await?;
    let historia = historia_dostaw(&mut conn, None).await?;
    let mut wynik = Vec::new();
    for ((kat, dost), pary) in historia {
        let Some(stat) = kalendarz::statystyka_dostaw(&pary, &kal) else {
            continue;
        };
        let r: (String, i64, i64, Option<String>, Option<i64>, Option<i64>) = sqlx::query_as(
            "SELECT k.nazwa, k.dni_dostawy_min, k.dni_dostawy_max, d.nazwa, c.dni_min, c.dni_max \
             FROM kategorie_materialow k LEFT JOIN dostawcy d ON d.id = ?2 \
             LEFT JOIN czasy_dostaw c ON c.kategoria_id = k.id AND c.dostawca_id = ?2 WHERE k.id = ?1",
        )
        .bind(kat)
        .bind(dost)
        .fetch_one(&mut *conn)
        .await?;
        let (aktualne, zrodlo) = match (r.4, r.5) {
            (Some(a), Some(b)) => (
                Some(Widelki {
                    min: u32_z(a),
                    max: u32_z(b),
                }),
                ZrodloWidelek::KategoriaDostawca,
            ),
            _ if r.2 > 0 => (
                Some(Widelki {
                    min: u32_z(r.1),
                    max: u32_z(r.2),
                }),
                ZrodloWidelek::Kategoria,
            ),
            _ => (None, ZrodloWidelek::Brak),
        };
        let korekta = aktualne.is_none_or(|w| w != stat.sugerowane_widelki);
        wynik.push(StatystykaPary {
            kategoria_id: kat,
            kategoria_nazwa: r.0,
            dostawca_id: dost,
            dostawca_nazwa: r.3,
            liczenie_dni: u.liczenie_dni,
            aktualne_widelki: aktualne,
            zrodlo_widelek: zrodlo,
            statystyka: stat,
            sugerowana_korekta: korekta,
        });
    }
    Ok(wynik)
}

/// Zadanie wpisane ręcznie (`wyliczone = 0`) – nigdy nie jest przeliczane.
#[derive(Debug, Clone, Deserialize)]
pub struct DaneZadaniaRecznego {
    #[serde(default)]
    pub zlecenie_id: Option<i64>,
    pub typ: String,
    pub tytul: String,
    pub data_od: NaiveDate,
    #[serde(rename = "data_do", default)]
    pub data_do: Option<NaiveDate>,
    #[serde(default)]
    pub uwagi: Option<String>,
}

pub async fn zapisz_zadanie_reczne(
    pula: &SqlitePool,
    id: Option<i64>,
    d: DaneZadaniaRecznego,
) -> Wynik<i64> {
    let typ = TypZadania::z_tekstu(&d.typ)
        .ok_or_else(|| BladApi::NieprawidloweDane(format!("nieznany typ zadania „{}”", d.typ)))?;
    let tytul = d.tytul.trim().to_string();
    if tytul.is_empty() {
        return Err(BladApi::NieprawidloweDane(
            "tytuł zadania jest wymagany".into(),
        ));
    }
    if d.data_do.is_some_and(|k| k < d.data_od) {
        return Err(BladApi::NieprawidloweDane(
            "data końca przed datą początku".into(),
        ));
    }
    let mut tx = pula.begin().await?;
    if let Some(z) = d.zlecenie_id {
        zlecenie(&mut tx, z).await?;
    }
    let uwagi = d
        .uwagi
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let id = match id {
        None => {
            let zad = ZadanieWyliczone {
                klucz: String::new(),
                typ,
                tytul,
                data_od: d.data_od,
                data_do: d.data_do,
                data_sweet_spot: None,
                data_planowa: None,
                data_krytyczna: None,
                zamowienie_id: None,
                wykonano: None,
            };
            let id = wstaw_zadanie(&mut tx, d.zlecenie_id, &zad, false).await?;
            sqlx::query("UPDATE zadania_kalendarza SET uwagi = ?1 WHERE id = ?2")
                .bind(&uwagi)
                .bind(id)
                .execute(&mut *tx)
                .await?;
            id
        }
        Some(id) => {
            let n = sqlx::query(
                "UPDATE zadania_kalendarza SET zlecenie_id = ?1, typ = ?2, tytul = ?3, data_od = ?4, data_do = ?5, uwagi = ?6 \
                 WHERE id = ?7 AND wyliczone = 0",
            )
            .bind(d.zlecenie_id)
            .bind(typ.jako_tekst())
            .bind(&tytul)
            .bind(tekst(d.data_od))
            .bind(d.data_do.map(tekst))
            .bind(&uwagi)
            .bind(id)
            .execute(&mut *tx)
            .await?
            .rows_affected();
            if n == 0 {
                return Err(BladApi::NieprawidloweDane(format!(
                    "zadanie {id} nie istnieje albo jest wyliczane – zmień dane zlecenia i przelicz harmonogram"
                )));
            }
            id
        }
    };
    dziennik(&mut tx, "zadania_kalendarza", id, "zmiana").await?;
    tx.commit().await?;
    Ok(id)
}

/// Usuwa zadanie ręczne. Zadań wyliczonych nie usuwa się (znikają przy przeliczeniu).
pub async fn usun_zadanie_reczne(pula: &SqlitePool, id: i64) -> Wynik<Option<i64>> {
    let zl: Option<(Option<i64>, bool)> =
        sqlx::query_as("SELECT zlecenie_id, wyliczone FROM zadania_kalendarza WHERE id = ?1")
            .bind(id)
            .fetch_optional(pula)
            .await?;
    match zl {
        None => Err(BladApi::NieZnaleziono(format!("zadanie {id}"))),
        Some((_, true)) => Err(BladApi::NieprawidloweDane(
            "zadania wyliczonego nie można usunąć – zniknie po zmianie danych zlecenia i przeliczeniu".into(),
        )),
        Some((zl, false)) => {
            sqlx::query("DELETE FROM zadania_kalendarza WHERE id = ?1")
                .bind(id)
                .execute(pula)
                .await?;
            Ok(zl)
        }
    }
}

// ===========================================================================
// Widoki: tydzień, do zamówienia, kolizje
// ===========================================================================

#[derive(Debug, Clone, Serialize)]
pub struct Tydzien {
    pub od: NaiveDate,
    #[serde(rename = "do")]
    pub do_: NaiveDate,
    pub dzis: NaiveDate,
    /// Niewykonane zadania z terminem/oknem w tym tygodniu oraz zaległe (po terminie).
    pub zadania: Vec<ZadanieKalendarza>,
}

pub async fn tydzien(pula: &SqlitePool, dzis: NaiveDate) -> Wynik<Tydzien> {
    let od = dzis - Duration::days(i64::from(dzis.weekday().num_days_from_monday()));
    let do_ = od + Duration::days(6);
    let f = FiltrZadan {
        do_: Some(do_),
        tylko_niewykonane: true,
        ..Default::default()
    };
    let wszystkie = zadania(pula, &f, dzis).await?;
    let zadania = wszystkie
        .into_iter()
        .filter(|z| {
            let koniec = z.data_do.as_deref().unwrap_or(&z.data_od);
            koniec >= tekst(od).as_str() || z.stan.stan == Stan::Czerwony
        })
        .collect();
    Ok(Tydzien {
        od,
        do_,
        dzis,
        zadania,
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct GrupaDostawcy {
    pub dostawca_id: Option<i64>,
    pub dostawca_nazwa: Option<String>,
    /// Najpilniejszy stan w grupie.
    pub stan: Stan,
    pub pozycje: Vec<ZadanieKalendarza>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DoZamowienia {
    pub dzis: NaiveDate,
    pub grupy: Vec<GrupaDostawcy>,
    /// Materiały niezamówione bez terminów (brak widełek albo brak harmonogramu).
    pub bez_terminow: Vec<Material>,
}

/// Niezamówione materiały pogrupowane wg dostawcy (bez dostawcy = grupa `dostawca_id: null` na końcu),
/// w grupie po dacie planowej. `horyzont` – tylko pozycje z sweet spotem nie później niż ta data.
pub async fn do_zamowienia(
    pula: &SqlitePool,
    dzis: NaiveDate,
    horyzont: Option<NaiveDate>,
) -> Wynik<DoZamowienia> {
    let f = FiltrZadan {
        typ: Some("zamowienie".into()),
        do_: horyzont,
        tylko_niewykonane: true,
        ..Default::default()
    };
    let mut pozycje: Vec<ZadanieKalendarza> = zadania(pula, &f, dzis)
        .await?
        .into_iter()
        .filter(|z| {
            z.zamowienie
                .as_ref()
                .is_some_and(|m| m.data_zamowienia.is_none())
        })
        .collect();
    pozycje.sort_by(|a, b| a.termin.cmp(&b.termin).then(a.id.cmp(&b.id)));
    let mut grupy: Vec<GrupaDostawcy> = Vec::new();
    for p in pozycje {
        let (id, nazwa) = p
            .zamowienie
            .as_ref()
            .map(|m| (m.dostawca_id, m.dostawca_nazwa.clone()))
            .unwrap_or((None, None));
        match grupy.iter_mut().find(|g| g.dostawca_id == id) {
            Some(g) => {
                g.stan = g.stan.max(p.stan.stan);
                g.pozycje.push(p);
            }
            None => grupy.push(GrupaDostawcy {
                dostawca_id: id,
                dostawca_nazwa: nazwa,
                stan: p.stan.stan,
                pozycje: vec![p],
            }),
        }
    }
    grupy.sort_by(|a, b| {
        (
            a.dostawca_id.is_none(),
            a.dostawca_nazwa.as_deref().unwrap_or(""),
        )
            .cmp(&(
                b.dostawca_id.is_none(),
                b.dostawca_nazwa.as_deref().unwrap_or(""),
            ))
    });
    let bez_terminow = sqlx::query_as(&format!(
        "{SQL_MATERIALY} WHERE m.data_zamowienia IS NULL AND m.data_planowa IS NULL ORDER BY m.zlecenie_id, m.id"
    ))
    .fetch_all(pula)
    .await?;
    Ok(DoZamowienia {
        dzis,
        grupy,
        bez_terminow,
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct KolizjaZlecen {
    #[serde(flatten)]
    pub kolizja: Kolizja,
    pub zlecenie_a_nazwa: String,
    pub zlecenie_b_nazwa: String,
}

pub async fn kolizje(
    pula: &SqlitePool,
    od: Option<NaiveDate>,
    do_: Option<NaiveDate>,
) -> Wynik<Vec<KolizjaZlecen>> {
    let wiersze: Vec<(i64, String, String, String, Option<String>)> = sqlx::query_as(
        "SELECT t.zlecenie_id, z.nazwa, t.typ, t.data_od, t.data_do FROM zadania_kalendarza t \
         JOIN zlecenia z ON z.id = t.zlecenie_id \
         WHERE t.typ IN ('warsztat', 'montaz') AND z.etap <> 'zakonczone' \
           AND (?1 IS NULL OR COALESCE(t.data_do, t.data_od) >= ?1) AND (?2 IS NULL OR t.data_od <= ?2)",
    )
    .bind(od.map(tekst))
    .bind(do_.map(tekst))
    .fetch_all(pula)
    .await?;
    let mut nazwy = HashMap::new();
    let mut okresy = Vec::new();
    for (zl, nazwa, typ, od, do_) in wiersze {
        let od = data(&od)?;
        okresy.push(Okres {
            zlecenie_id: zl,
            typ: TypZadania::z_tekstu(&typ).unwrap_or(TypZadania::Inne),
            od,
            do_: data_opt(do_.as_deref())?.unwrap_or(od),
        });
        nazwy.insert(zl, nazwa);
    }
    Ok(kalendarz::wykryj_kolizje(&okresy)
        .into_iter()
        .map(|k| KolizjaZlecen {
            zlecenie_a_nazwa: nazwy.get(&k.zlecenie_a).cloned().unwrap_or_default(),
            zlecenie_b_nazwa: nazwy.get(&k.zlecenie_b).cloned().unwrap_or_default(),
            kolizja: k,
        })
        .collect())
}

#[cfg(test)]
mod testy {
    use super::*;
    use crate::baza::otworz_w_pamieci;

    fn d(r: i32, m: u32, dz: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(r, m, dz).unwrap()
    }

    async fn klient(pula: &SqlitePool) -> i64 {
        sqlx::query("INSERT INTO klienci (nazwa) VALUES ('Klient Testowy')")
            .execute(pula)
            .await
            .unwrap()
            .last_insert_rowid()
    }

    async fn kategoria(pula: &SqlitePool, nazwa: &str) -> i64 {
        sqlx::query_scalar("SELECT id FROM kategorie_materialow WHERE nazwa = ?1")
            .bind(nazwa)
            .fetch_one(pula)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn ustawienia_domyslne_i_zmiana() {
        let pula = otworz_w_pamieci().await.unwrap();
        let u = ustawienia(&mut pula.acquire().await.unwrap())
            .await
            .unwrap();
        assert_eq!(u, UstawieniaTerminow::default());
        let u = zmien_ustawienia(
            &pula,
            ZmianaUstawien {
                liczenie_dni: Some(LiczenieDni::Kalendarzowe),
                prog_pomaranczowy_dni: Some(3),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(u.liczenie_dni, LiczenieDni::Kalendarzowe);
        assert_eq!(u.prog_pomaranczowy_dni, 3);
        assert_eq!(u.odstep_materialy_dni, 7);
        assert!(zmien_ustawienia(
            &pula,
            ZmianaUstawien {
                odstep_pomiar_dni: Some(-1),
                ..Default::default()
            }
        )
        .await
        .is_err());
    }

    /// Wiersz specyfikacji „do potwierdzenia” w umowie zlecenia blokuje zamówienie kategorii.
    #[tokio::test]
    async fn blokada_do_potwierdzenia_z_specyfikacji_umowy() {
        let pula = otworz_w_pamieci().await.unwrap();
        zmien_ustawienia(
            &pula,
            ZmianaUstawien {
                liczenie_dni: Some(LiczenieDni::Kalendarzowe),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let k = klient(&pula).await;
        let fronty = kategoria(&pula, "Fronty MDF frezowane").await;
        let plyta = kategoria(&pula, "Płyta meblowa").await;
        zmien_widelki_kategorii(
            &pula,
            fronty,
            WidelkiKategorii {
                dni_dostawy_min: 18,
                dni_dostawy_max: 24,
                domyslny_dostawca_id: None,
            },
        )
        .await
        .unwrap();
        zmien_widelki_kategorii(
            &pula,
            plyta,
            WidelkiKategorii {
                dni_dostawy_min: 5,
                dni_dostawy_max: 10,
                domyslny_dostawca_id: None,
            },
        )
        .await
        .unwrap();
        // fikcyjna umowa ze specyfikacją: jeden wiersz frontów do potwierdzenia
        let spec = sqlx::query("INSERT INTO specyfikacje DEFAULT VALUES")
            .execute(&pula)
            .await
            .unwrap()
            .last_insert_rowid();
        let sekcja = sqlx::query(
            "INSERT INTO sekcje_specyfikacji (specyfikacja_id, nazwa) VALUES (?1, 'Kuchnia')",
        )
        .bind(spec)
        .execute(&pula)
        .await
        .unwrap()
        .last_insert_rowid();
        let czesc =
            sqlx::query("INSERT INTO czesci_specyfikacji (sekcja_id, nazwa) VALUES (?1, 'Słupki')")
                .bind(sekcja)
                .execute(&pula)
                .await
                .unwrap()
                .last_insert_rowid();
        let wiersz = sqlx::query(
            "INSERT INTO wiersze_specyfikacji (czesc_id, rodzaj, status, kategoria_materialu_id) VALUES (?1, 'Fronty', 'do_potwierdzenia', ?2)",
        )
        .bind(czesc)
        .bind(fronty)
        .execute(&pula)
        .await
        .unwrap()
        .last_insert_rowid();
        let umowa = sqlx::query(
            "INSERT INTO umowy (rok, nr_kolejny, numer, klient_id, specyfikacja_id, data_zawarcia, dni_na_brakujace_informacje) \
             VALUES (2026, 1, '01/2026', ?1, ?2, '2026-11-30', 14)",
        )
        .bind(k)
        .bind(spec)
        .execute(&pula)
        .await
        .unwrap()
        .last_insert_rowid();
        let z = zapisz_zlecenie(
            &pula,
            None,
            DaneZlecenia {
                klient_id: k,
                umowa_id: Some(umowa),
                nazwa: "Kuchnia testowa".into(),
                etap: None,
                montaz_od: Some(d(2027, 2, 8)),
                czas_warsztat_dni: Some(14),
                czas_montazu_dni: Some(3),
                odstep_materialy_dni: None,
                odstep_rysunki_dni: None,
                odstep_pomiar_dni: None,
            },
        )
        .await
        .unwrap();
        for kat in [fronty, plyta] {
            zapisz_material(
                &pula,
                Some(z.id),
                None,
                DaneMaterialu {
                    kategoria_id: kat,
                    dostawca_id: None,
                    opis: None,
                    data_zamowienia: None,
                    data_dostawy: None,
                },
            )
            .await
            .unwrap();
        }
        let w = generuj(&pula, z.id).await.unwrap();
        assert!(w.harmonogram.materialy[0].zablokowane);
        assert!(!w.harmonogram.materialy[1].zablokowane);
        assert!(w
            .harmonogram
            .zadania
            .iter()
            .any(|z| z.klucz == "brakujace_informacje" && z.data_od == d(2026, 12, 14)));

        let lista = zadania(
            &pula,
            &FiltrZadan {
                typ: Some("zamowienie".into()),
                ..Default::default()
            },
            d(2026, 12, 1),
        )
        .await
        .unwrap();
        let info = lista[0].zamowienie.as_ref().unwrap();
        assert!(info.zablokowane);
        assert_eq!(info.wiersze_do_potwierdzenia, vec![wiersz]);
        assert_eq!(info.potwierdzic_do.as_deref(), Some("2026-12-28"));
        assert!(!lista[1].zamowienie.as_ref().unwrap().zablokowane);

        // po potwierdzeniu wiersza blokada znika
        sqlx::query("UPDATE wiersze_specyfikacji SET status = 'ustalone' WHERE id = ?1")
            .bind(wiersz)
            .execute(&pula)
            .await
            .unwrap();
        let lista = zadania(
            &pula,
            &FiltrZadan {
                typ: Some("zamowienie".into()),
                ..Default::default()
            },
            d(2026, 12, 1),
        )
        .await
        .unwrap();
        assert!(!lista[0].zamowienie.as_ref().unwrap().zablokowane);
    }
}
