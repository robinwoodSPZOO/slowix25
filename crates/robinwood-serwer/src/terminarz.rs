//! Warstwa bazy silnika terminów: zlecenia, materiały (zamówienia), zadania kalendarza,
//! dostawcy, czasy dostaw, ustawienia odstępów i dni wolne. Logika liczenia jest w
//! [`crate::kalendarz`]; tu tylko wczytanie danych, zapis wyniku i odczyt ze stanami.

use std::collections::{BTreeMap, HashMap, HashSet};

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
/// Etapy po montażu: zlecenia w tych etapach nie są przeliczane automatycznie (zmiana
/// ustawień, dni wolnych, widełek ani zmiana samego zlecenia nie przepisuje ich dat).
pub const ETAPY_PO_MONTAZU: &[&str] = &["odbior", "gwarancja", "zakonczone"];
const SQL_ETAP_PRZELICZANY: &str = "z.etap NOT IN ('odbior', 'gwarancja', 'zakonczone')";
pub const RODZAJE_DNI_WOLNYCH: &[&str] = &["swieto", "urlop", "przerwa_dostawcow", "inne"];
const MAX_DNI: i64 = 365;

/// Data `RRRR-MM-DD` z obsługiwanego zakresu lat (patrz [`kalendarz::ROK_MIN`]).
fn parsuj_date(t: &str) -> Result<NaiveDate, String> {
    let d = NaiveDate::parse_from_str(t.trim(), "%Y-%m-%d")
        .map_err(|_| format!("nieprawidłowa data „{t}” (oczekiwano RRRR-MM-DD)"))?;
    if !kalendarz::obslugiwany_rok(d.year()) {
        return Err(format!(
            "data {t} spoza obsługiwanego zakresu lat {}–{}",
            kalendarz::ROK_MIN,
            kalendarz::ROK_MAX
        ));
    }
    Ok(d)
}

fn data(t: &str) -> Wynik<NaiveDate> {
    parsuj_date(t).map_err(BladApi::NieprawidloweDane)
}

/// Deserializacja pól wejściowych: daty z kontrolą zakresu lat i pola trójstanowe
/// (`Option<Option<T>>`: pole pominięte = `None` → bez zmian, `null` = `Some(None)` → wyczyść).
pub mod wej {
    use chrono::NaiveDate;
    use serde::{Deserialize, Deserializer};

    pub fn data<'de, D: Deserializer<'de>>(d: D) -> Result<NaiveDate, D::Error> {
        let t = String::deserialize(d)?;
        super::parsuj_date(&t).map_err(serde::de::Error::custom)
    }

    pub fn data_opt<'de, D: Deserializer<'de>>(d: D) -> Result<Option<NaiveDate>, D::Error> {
        Option::<String>::deserialize(d)?
            .map(|t| super::parsuj_date(&t).map_err(serde::de::Error::custom))
            .transpose()
    }

    pub fn data_tri<'de, D: Deserializer<'de>>(
        d: D,
    ) -> Result<Option<Option<NaiveDate>>, D::Error> {
        data_opt(d).map(Some)
    }

    pub fn tri<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
        d: D,
    ) -> Result<Option<Option<T>>, D::Error> {
        Option::<T>::deserialize(d).map(Some)
    }
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
    dziennik_opis(conn, tabela, id, operacja, None).await
}

async fn dziennik_opis(
    conn: &mut SqliteConnection,
    tabela: &str,
    id: i64,
    operacja: &str,
    opis: Option<&str>,
) -> Wynik<()> {
    sqlx::query(
        "INSERT INTO dziennik_zmian (tabela, rekord_id, operacja, opis) VALUES (?1, ?2, ?3, ?4)",
    )
    .bind(tabela)
    .bind(id)
    .bind(operacja)
    .bind(opis)
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

/// Zmienia ustawienia i w tej samej transakcji przelicza wszystkie zlecenia z harmonogramem.
pub async fn zmien_ustawienia(
    pula: &SqlitePool,
    z: ZmianaUstawien,
) -> Wynik<(UstawieniaTerminow, WynikPrzeliczeniaWszystkich)> {
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
    let mut tx = crate::baza::transakcja_zapisu(pula).await?;
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
    dziennik(&mut tx, "ustawienia", 0, "zmiana").await?;
    let u = ustawienia(&mut tx).await?;
    let p = przelicz_wszystkie_w(&mut tx).await?;
    tx.commit().await?;
    Ok((u, p))
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
#[serde(default, deny_unknown_fields)]
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
) -> Wynik<(DzienWolny, WynikPrzeliczeniaWszystkich)> {
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
    let mut tx = crate::baza::transakcja_zapisu(pula).await?;
    sqlx::query(
        "INSERT INTO dni_wolne (data, rodzaj, opis) VALUES (?1, ?2, ?3) \
         ON CONFLICT(data) DO UPDATE SET rodzaj = excluded.rodzaj, opis = excluded.opis",
    )
    .bind(&d)
    .bind(&rodzaj)
    .bind(&opis)
    .execute(&mut *tx)
    .await?;
    dziennik_opis(&mut tx, "dni_wolne", 0, "zmiana", Some(&d)).await?;
    let p = przelicz_wszystkie_w(&mut tx).await?;
    tx.commit().await?;
    Ok((
        DzienWolny {
            data: d,
            rodzaj,
            opis,
        },
        p,
    ))
}

pub async fn usun_dzien_wolny(pula: &SqlitePool, d: &str) -> Wynik<WynikPrzeliczeniaWszystkich> {
    let d = tekst(data(d)?);
    let mut tx = crate::baza::transakcja_zapisu(pula).await?;
    let n = sqlx::query("DELETE FROM dni_wolne WHERE data = ?1")
        .bind(&d)
        .execute(&mut *tx)
        .await?
        .rows_affected();
    if n == 0 {
        return Err(BladApi::NieZnaleziono(format!("dzień wolny {d}")));
    }
    dziennik_opis(&mut tx, "dni_wolne", 0, "usuniecie", Some(&d)).await?;
    let p = przelicz_wszystkie_w(&mut tx).await?;
    tx.commit().await?;
    Ok(p)
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
#[serde(deny_unknown_fields)]
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
    let nazwa = crate::nazwy::oczysc(&d.nazwa);
    if nazwa.is_empty() {
        return Err(BladApi::NieprawidloweDane(
            "nazwa dostawcy jest wymagana".into(),
        ));
    }
    if nazwa.chars().count() > crate::nazwy::MAKS_NAZWY {
        return Err(BladApi::NieprawidloweDane(format!(
            "nazwa dostawcy: najwyżej {} znaków",
            crate::nazwy::MAKS_NAZWY
        )));
    }
    for (pole, t) in [("e-mail", &d.email), ("telefon", &d.telefon)] {
        crate::limity::tekst(pole, t.as_deref(), crate::nazwy::MAKS_NAZWY)?;
    }
    crate::limity::tekst("uwagi", d.uwagi.as_deref(), 2000)?;
    let mut c = pula.acquire().await?;
    let powtorzona =
        crate::nazwy::znajdz(&mut c, crate::nazwy::Tabela::Dostawcy, &nazwa, id).await?;
    drop(c);
    if powtorzona.is_some() {
        return Err(BladApi::NieprawidloweDane(format!(
            "dostawca „{nazwa}” już istnieje (wielkość liter i odstępy nie mają znaczenia)"
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
#[serde(deny_unknown_fields)]
pub struct WidelkiKategorii {
    pub dni_dostawy_min: i64,
    pub dni_dostawy_max: i64,
    #[serde(default)]
    pub domyslny_dostawca_id: Option<i64>,
}

pub async fn zmien_widelki_kategorii(
    pula: &SqlitePool,
    id: i64,
    w: WidelkiKategorii,
) -> Wynik<WynikPrzeliczeniaWszystkich> {
    sprawdz_widelki(w.dni_dostawy_min, w.dni_dostawy_max)?;
    let mut conn = crate::baza::transakcja_zapisu(pula).await?;
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
    dziennik(&mut conn, "kategorie_materialow", id, "zmiana").await?;
    let p = przelicz_wszystkie_w(&mut conn).await?;
    conn.commit().await?;
    Ok(p)
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
#[serde(deny_unknown_fields)]
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

async fn czas_dostawy(conn: &mut SqliteConnection, id: i64) -> Wynik<CzasDostawy> {
    sqlx::query_as(&format!("{SQL_CZASY} WHERE c.id = ?1"))
        .bind(id)
        .fetch_optional(&mut *conn)
        .await?
        .ok_or_else(|| BladApi::NieZnaleziono(format!("czas dostawy {id}")))
}

pub async fn zapisz_czas_dostawy(
    pula: &SqlitePool,
    id: Option<i64>,
    d: DaneCzasuDostawy,
) -> Wynik<(CzasDostawy, WynikPrzeliczeniaWszystkich)> {
    sprawdz_widelki(d.dni_min, d.dni_max)?;
    let mut conn = crate::baza::transakcja_zapisu(pula).await?;
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
    let p = przelicz_wszystkie_w(&mut conn).await?;
    let c = czas_dostawy(&mut conn, id).await?;
    conn.commit().await?;
    Ok((c, p))
}

pub async fn usun_czas_dostawy(pula: &SqlitePool, id: i64) -> Wynik<WynikPrzeliczeniaWszystkich> {
    let mut tx = crate::baza::transakcja_zapisu(pula).await?;
    let n = sqlx::query("DELETE FROM czasy_dostaw WHERE id = ?1")
        .bind(id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
    if n == 0 {
        return Err(BladApi::NieZnaleziono(format!("czas dostawy {id}")));
    }
    dziennik(&mut tx, "czasy_dostaw", id, "usuniecie").await?;
    let p = przelicz_wszystkie_w(&mut tx).await?;
    tx.commit().await?;
    Ok(p)
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
    /// Ostatnie automatyczne przeliczenie się nie udało – zapisane zadania są nieaktualne.
    pub harmonogram_nieaktualny: bool,
    /// Powód (komunikat błędu przeliczenia), `null` gdy harmonogram aktualny.
    pub harmonogram_nieaktualny_powod: Option<String>,
}

/// Dane zlecenia (POST i PUT – pełna zmiana; pola pominięte = brak wartości).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DaneZlecenia {
    pub klient_id: i64,
    #[serde(default)]
    pub umowa_id: Option<i64>,
    pub nazwa: String,
    #[serde(default)]
    pub etap: Option<String>,
    #[serde(default, deserialize_with = "wej::data_opt")]
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
     odstep_materialy_dni, odstep_rysunki_dni, odstep_pomiar_dni, utworzono, \
     (harmonogram_nieaktualny_powod IS NOT NULL) AS harmonogram_nieaktualny, harmonogram_nieaktualny_powod";

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

/// Dodaje (`id = None`) albo zmienia zlecenie. Przy zmianie – w tej samej transakcji –
/// przelicza istniejący harmonogram; zwraca też, czy go przeliczono.
pub async fn zapisz_zlecenie(
    pula: &SqlitePool,
    id: Option<i64>,
    d: DaneZlecenia,
) -> Wynik<(Zlecenie, bool)> {
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
    let mut tx = crate::baza::transakcja_zapisu(pula).await?;
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
    let (id, operacja) = match id {
        None => (
            sqlx::query(
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
            "dodanie",
        ),
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
            (id, "zmiana")
        }
    };
    dziennik(&mut tx, "zlecenia", id, operacja).await?;
    let przeliczono = matches!(
        przelicz_jesli_jest_w(&mut tx, id).await?,
        Przeliczenie::Przeliczone(_)
    );
    let z = zlecenie(&mut tx, id).await?;
    tx.commit().await?;
    Ok((z, przeliczono))
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

/// Nowy materiał zlecenia (`POST /zlecenia/{id}/materialy`).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DaneMaterialu {
    pub kategoria_id: i64,
    #[serde(default)]
    pub dostawca_id: Option<i64>,
    #[serde(default)]
    pub opis: Option<String>,
    #[serde(default, deserialize_with = "wej::data_opt")]
    pub data_zamowienia: Option<NaiveDate>,
    #[serde(default, deserialize_with = "wej::data_opt")]
    pub data_dostawy: Option<NaiveDate>,
    /// Potwierdzenie zamówienia mimo wierszy specyfikacji „do potwierdzenia” (patrz 409).
    #[serde(default)]
    pub mimo_blokady: bool,
}

/// Częściowa zmiana materiału (`PUT /zamowienia-materialow/{id}`): pole pominięte – bez zmian,
/// `null` – wyczyszczenie (dla `dostawca_id`, `opis`, `data_dostawy`). Daty zamówienia nie
/// czyści się tutaj – służy do tego `POST /zamowienia-materialow/{id}/cofnij-zamowienie`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ZmianaMaterialu {
    pub kategoria_id: Option<i64>,
    #[serde(deserialize_with = "wej::tri")]
    pub dostawca_id: Option<Option<i64>>,
    #[serde(deserialize_with = "wej::tri")]
    pub opis: Option<Option<String>>,
    #[serde(deserialize_with = "wej::data_tri")]
    pub data_zamowienia: Option<Option<NaiveDate>>,
    #[serde(deserialize_with = "wej::data_tri")]
    pub data_dostawy: Option<Option<NaiveDate>>,
    pub mimo_blokady: bool,
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

fn opis_tekst(t: Option<String>) -> Option<String> {
    t.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

/// Miękka blokada „do potwierdzenia”: zamówienie materiału, na którego kategorię czekają
/// wiersze specyfikacji umowy zlecenia, wymaga `mimo_blokady: true` (inaczej 409 z listą wierszy).
/// Zwraca opis do dziennika, gdy zamówiono mimo blokady.
async fn sprawdz_blokade(
    conn: &mut SqliteConnection,
    zlecenie_id: i64,
    kategoria_id: i64,
    mimo_blokady: bool,
) -> Wynik<Option<String>> {
    let wiersze = blokady(conn, Some(zlecenie_id))
        .await?
        .remove(&(zlecenie_id, kategoria_id))
        .unwrap_or_default();
    if wiersze.is_empty() {
        return Ok(None);
    }
    let lista = wiersze
        .iter()
        .map(i64::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    if !mimo_blokady {
        return Err(BladApi::Zablokowane {
            komunikat: format!(
                "Zamówienie wstrzymane: {} poz. specyfikacji czeka na potwierdzenie klienta (wiersze: {lista}). \
                 Potwierdź wiersze albo wyślij ponownie z \"mimo_blokady\": true",
                wiersze.len()
            ),
            wiersze_do_potwierdzenia: wiersze,
        });
    }
    Ok(Some(format!(
        "zamówiono mimo blokady – wiersze specyfikacji do potwierdzenia: {lista}"
    )))
}

/// Dodaje materiał do zlecenia (w tej samej transakcji przelicza istniejący harmonogram).
pub async fn dodaj_material(
    pula: &SqlitePool,
    zlecenie_id: i64,
    d: DaneMaterialu,
) -> Wynik<Material> {
    let status = status_materialu(d.data_zamowienia, d.data_dostawy)?;
    let mut tx = crate::baza::transakcja_zapisu(pula).await?;
    zlecenie(&mut tx, zlecenie_id).await?;
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
    let opis_blokady = match d.data_zamowienia {
        Some(_) => sprawdz_blokade(&mut tx, zlecenie_id, d.kategoria_id, d.mimo_blokady).await?,
        None => None,
    };
    let id = sqlx::query(
        "INSERT INTO zamowienia_materialow (zlecenie_id, kategoria_id, dostawca_id, opis, status, data_zamowienia, data_dostawy) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )
    .bind(zlecenie_id)
    .bind(d.kategoria_id)
    .bind(d.dostawca_id)
    .bind(opis_tekst(d.opis))
    .bind(status)
    .bind(d.data_zamowienia.map(tekst))
    .bind(d.data_dostawy.map(tekst))
    .execute(&mut *tx)
    .await?
    .last_insert_rowid();
    dziennik_opis(
        &mut tx,
        "zamowienia_materialow",
        id,
        "dodanie",
        opis_blokady.as_deref(),
    )
    .await?;
    if d.data_dostawy.is_some() {
        aktualizuj_srednia_rzeczywista(&mut tx, id).await?;
    }
    przelicz_jesli_jest_w(&mut tx, zlecenie_id).await?;
    let m = material(&mut tx, id).await?;
    tx.commit().await?;
    Ok(m)
}

/// Częściowa zmiana materiału. Pola pominięte zostają bez zmian – w szczególności daty
/// zamówienia i dostawy oraz stan wykonania zadania „zamówić”.
pub async fn zmien_material(pula: &SqlitePool, id: i64, z: ZmianaMaterialu) -> Wynik<Material> {
    let mut tx = crate::baza::transakcja_zapisu(pula).await?;
    let m = material(&mut tx, id).await?;
    let kategoria = z.kategoria_id.unwrap_or(m.kategoria_id);
    istnieje(
        &mut tx,
        "SELECT id FROM kategorie_materialow WHERE id = ?1",
        kategoria,
        "kategoria materiału",
    )
    .await?;
    let dostawca = z.dostawca_id.unwrap_or(m.dostawca_id);
    if let Some(dost) = dostawca {
        istnieje(
            &mut tx,
            "SELECT id FROM dostawcy WHERE id = ?1",
            dost,
            "dostawca",
        )
        .await?;
    }
    let opis = match z.opis {
        None => m.opis.clone(),
        Some(o) => opis_tekst(o),
    };
    let zam_bylo = data_opt(m.data_zamowienia.as_deref())?;
    let dost_bylo = data_opt(m.data_dostawy.as_deref())?;
    let zam = match z.data_zamowienia {
        None => zam_bylo,
        Some(None) if zam_bylo.is_some() => {
            return Err(BladApi::NieprawidloweDane(format!(
                "materiał {id} jest zamówiony – aby cofnąć zamówienie użyj POST /zamowienia-materialow/{id}/cofnij-zamowienie"
            )))
        }
        Some(nowa) => nowa,
    };
    let dost = z.data_dostawy.unwrap_or(dost_bylo);
    let status = status_materialu(zam, dost)?;
    let opis_blokady = if zam_bylo.is_none() && zam.is_some() {
        sprawdz_blokade(&mut tx, m.zlecenie_id, kategoria, z.mimo_blokady).await?
    } else {
        None
    };
    sqlx::query(
        "UPDATE zamowienia_materialow SET kategoria_id = ?1, dostawca_id = ?2, opis = ?3, status = ?4, \
         data_zamowienia = ?5, data_dostawy = ?6 WHERE id = ?7",
    )
    .bind(kategoria)
    .bind(dostawca)
    .bind(&opis)
    .bind(status)
    .bind(zam.map(tekst))
    .bind(dost.map(tekst))
    .bind(id)
    .execute(&mut *tx)
    .await?;
    if zam != zam_bylo {
        // zadanie „zamówić” ma ten sam stan wykonania co materiał (tylko przy jawnej zmianie daty)
        sqlx::query("UPDATE zadania_kalendarza SET wykonano = ?1 WHERE zamowienie_id = ?2 AND klucz LIKE 'zamowienie:%'")
            .bind(zam.map(tekst))
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    dziennik_opis(
        &mut tx,
        "zamowienia_materialow",
        id,
        "zmiana",
        opis_blokady.as_deref(),
    )
    .await?;
    if dost != dost_bylo {
        aktualizuj_srednia_rzeczywista(&mut tx, id).await?;
    }
    przelicz_jesli_jest_w(&mut tx, m.zlecenie_id).await?;
    let m = material(&mut tx, id).await?;
    tx.commit().await?;
    Ok(m)
}

/// Jawne cofnięcie zamówienia materiału: czyści daty zamówienia i dostawy, przywraca status
/// `do_zamowienia` i zadanie „zamówić” jako niewykonane, przelicza zlecenie.
pub async fn cofnij_zamowienie(pula: &SqlitePool, id: i64) -> Wynik<Material> {
    let mut tx = crate::baza::transakcja_zapisu(pula).await?;
    let m = material(&mut tx, id).await?;
    if m.data_zamowienia.is_none() {
        return Err(BladApi::NieprawidloweDane(format!(
            "materiał {id} nie jest zamówiony – nie ma czego cofać"
        )));
    }
    cofnij_zamowienie_w(&mut tx, &m).await?;
    if m.data_dostawy.is_some() {
        aktualizuj_srednia_rzeczywista(&mut tx, id).await?;
    }
    przelicz_jesli_jest_w(&mut tx, m.zlecenie_id).await?;
    let m = material(&mut tx, id).await?;
    tx.commit().await?;
    Ok(m)
}

async fn cofnij_zamowienie_w(conn: &mut SqliteConnection, m: &Material) -> Wynik<()> {
    sqlx::query(
        "UPDATE zamowienia_materialow SET data_zamowienia = NULL, data_dostawy = NULL, status = 'do_zamowienia' WHERE id = ?1",
    )
    .bind(m.id)
    .execute(&mut *conn)
    .await?;
    let zadania: Vec<i64> = sqlx::query_scalar(
        "SELECT id FROM zadania_kalendarza WHERE zamowienie_id = ?1 AND klucz LIKE 'zamowienie:%' AND wykonano IS NOT NULL",
    )
    .bind(m.id)
    .fetch_all(&mut *conn)
    .await?;
    for zid in zadania {
        sqlx::query("UPDATE zadania_kalendarza SET wykonano = NULL WHERE id = ?1")
            .bind(zid)
            .execute(&mut *conn)
            .await?;
        dziennik(conn, "zadania_kalendarza", zid, "zmiana").await?;
    }
    let opis = format!(
        "cofnięto zamówienie (było: zamówione {}, dostawa {})",
        m.data_zamowienia.as_deref().unwrap_or("–"),
        m.data_dostawy.as_deref().unwrap_or("–")
    );
    dziennik_opis(conn, "zamowienia_materialow", m.id, "zmiana", Some(&opis)).await
}

/// Usuwa materiał niezamówiony. Materiał zamówiony lub dostarczony – 422 (najpierw cofnij
/// zamówienie), bo usunięcie skasowałoby (ON DELETE CASCADE) jego wykonane zadanie.
/// Niewykonane zadania materiału są usuwane z wpisem w dzienniku; wykonane (np. potwierdzenie
/// klienta) zostają jako historia, odłączone od materiału.
pub async fn usun_material(pula: &SqlitePool, id: i64) -> Wynik<i64> {
    let mut tx = crate::baza::transakcja_zapisu(pula).await?;
    let m = material(&mut tx, id).await?;
    if m.data_zamowienia.is_some() || m.status != "do_zamowienia" {
        return Err(BladApi::NieprawidloweDane(format!(
            "materiał {id} ({}) ma status „{}” – usunięcie skasowałoby historię zamówienia. \
             Najpierw cofnij zamówienie (POST /zamowienia-materialow/{id}/cofnij-zamowienie), potem usuń materiał",
            m.kategoria_nazwa, m.status
        )));
    }
    sqlx::query(
        "UPDATE zadania_kalendarza SET zamowienie_id = NULL WHERE zamowienie_id = ?1 AND wykonano IS NOT NULL",
    )
    .bind(id)
    .execute(&mut *tx)
    .await?;
    let zadania: Vec<i64> =
        sqlx::query_scalar("SELECT id FROM zadania_kalendarza WHERE zamowienie_id = ?1")
            .bind(id)
            .fetch_all(&mut *tx)
            .await?;
    for zid in zadania {
        sqlx::query("DELETE FROM zadania_kalendarza WHERE id = ?1")
            .bind(zid)
            .execute(&mut *tx)
            .await?;
        dziennik_opis(
            &mut tx,
            "zadania_kalendarza",
            zid,
            "usuniecie",
            Some(&format!("usunięto materiał {id}")),
        )
        .await?;
    }
    sqlx::query("DELETE FROM zamowienia_materialow WHERE id = ?1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    let opis = format!(
        "zlecenie {}, {}{}",
        m.zlecenie_id,
        m.kategoria_nazwa,
        m.opis
            .as_deref()
            .map(|o| format!(" – {o}"))
            .unwrap_or_default()
    );
    dziennik_opis(
        &mut tx,
        "zamowienia_materialow",
        id,
        "usuniecie",
        Some(&opis),
    )
    .await?;
    przelicz_jesli_jest_w(&mut tx, m.zlecenie_id).await?;
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
    let nazwa = z.nazwa.clone();
    let brak = |co: &str| {
        BladApi::NieprawidloweDane(format!(
            "zlecenie „{nazwa}”: uzupełnij {co}, aby policzyć harmonogram"
        ))
    };
    let montaz_od = data(
        z.montaz_od
            .as_deref()
            .ok_or_else(|| brak("termin rozpoczęcia montażu"))?,
    )?;
    let czas_montazu = z
        .czas_montazu_dni
        .ok_or_else(|| brak("czas montażu (w dniach)"))?;
    let czas_warsztatu = z
        .czas_warsztat_dni
        .ok_or_else(|| brak("czas składania na warsztacie (w dniach)"))?;

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
    /// Liczby rzeczywistych zmian w zadaniach (przeliczenie bez różnic daje same zera).
    pub dodane: usize,
    pub zmienione: usize,
    pub usuniete: usize,
    /// Zadania pozostawione bez zmian (wykonane lub wpisane ręcznie).
    pub zamrozone: usize,
}

impl WynikGenerowania {
    fn cokolwiek_zmieniono(&self) -> bool {
        self.dodane + self.zmienione + self.usuniete > 0
    }
}

/// Zlecenie, którego harmonogramu nie udało się przeliczyć (zadania w bazie są nieaktualne).
#[derive(Debug, Clone, Serialize, PartialEq, FromRow)]
pub struct NieaktualnyHarmonogram {
    pub zlecenie_id: i64,
    pub zlecenie_nazwa: String,
    pub powod: String,
}

/// Wynik przeliczenia wszystkich zleceń (po zmianie ustawień, dni wolnych, widełek).
#[derive(Debug, Clone, Default, Serialize)]
pub struct WynikPrzeliczeniaWszystkich {
    pub przeliczone_zlecenia: Vec<i64>,
    /// Zlecenia, których nie udało się przeliczyć – z powodem; są oznaczone
    /// `harmonogram_nieaktualny`.
    pub nieprzeliczone_zlecenia: Vec<NieaktualnyHarmonogram>,
    /// Zlecenia, w których zadania faktycznie się zmieniły (do zdarzeń WebSocket).
    #[serde(skip)]
    pub zmienione_zlecenia: Vec<i64>,
}

/// Wynik automatycznego przeliczenia jednego zlecenia.
#[derive(Debug, Clone)]
pub enum Przeliczenie {
    /// Brak harmonogramu albo etap po montażu – nic nie liczono.
    Pominiete,
    Przeliczone(WynikGenerowania),
    /// Nie udało się (powód); zlecenie oznaczone jako `harmonogram_nieaktualny`.
    Nieaktualne(String),
}

/// Liczy harmonogram i zapisuje zadania zlecenia (wykonane i ręczne zostają bez zmian).
/// Błąd danych → 422; jeśli zlecenie miało już harmonogram, zostaje on oznaczony jako nieaktualny.
pub async fn generuj(pula: &SqlitePool, id: i64) -> Wynik<WynikGenerowania> {
    let mut tx = crate::baza::transakcja_zapisu(pula).await?;
    let (u, kal) = kalendarz_dni(&mut tx).await?;
    match przelicz_w(&mut tx, id, &u, &kal).await? {
        Ok(w) => {
            tx.commit().await?;
            Ok(w)
        }
        Err(powod) => {
            if ma_harmonogram(&mut tx, id).await? {
                tx.commit().await?;
            }
            Err(BladApi::NieprawidloweDane(powod))
        }
    }
}

/// Przeliczenie w punkcie zapisu (SAVEPOINT): sukces zdejmuje znacznik nieaktualności,
/// błąd danych wycofuje częściowe zmiany i ustawia znacznik z powodem (zwracany jako `Err`
/// w wyniku). Inne błędy (baza) przechodzą dalej – wywołujący wycofuje całą transakcję.
async fn przelicz_w(
    conn: &mut SqliteConnection,
    id: i64,
    u: &UstawieniaTerminow,
    kal: &KalendarzDni,
) -> Wynik<Result<WynikGenerowania, String>> {
    // savepoint: punkt zapisu wewnątrz transakcji zapisu wywołującego (BEGIN IMMEDIATE)
    let mut sp = sqlx::Connection::begin(&mut *conn).await?; // savepoint
    match generuj_w(&mut sp, id, u, kal).await {
        Ok(w) => {
            let n = sqlx::query(
                "UPDATE zlecenia SET harmonogram_nieaktualny_powod = NULL WHERE id = ?1 AND harmonogram_nieaktualny_powod IS NOT NULL",
            )
            .bind(id)
            .execute(&mut *sp)
            .await?
            .rows_affected();
            if n > 0 {
                dziennik_opis(
                    &mut sp,
                    "zlecenia",
                    id,
                    "zmiana",
                    Some("harmonogram przeliczony – znów aktualny"),
                )
                .await?;
            }
            sp.commit().await?;
            Ok(Ok(w))
        }
        Err(BladApi::NieprawidloweDane(powod)) => {
            sp.rollback().await?;
            let n = sqlx::query(
                "UPDATE zlecenia SET harmonogram_nieaktualny_powod = ?1 WHERE id = ?2 AND harmonogram_nieaktualny_powod IS NOT ?1",
            )
            .bind(&powod)
            .bind(id)
            .execute(&mut *conn)
            .await?
            .rows_affected();
            if n > 0 {
                dziennik_opis(
                    conn,
                    "zlecenia",
                    id,
                    "zmiana",
                    Some(&format!("harmonogram nieaktualny: {powod}")),
                )
                .await?;
            }
            Ok(Err(powod))
        }
        Err(e) => Err(e),
    }
}

/// Zadanie zapisane w bazie (do scalenia i wykrywania rzeczywistych zmian).
#[derive(FromRow)]
struct ZapisaneZadanie {
    id: i64,
    klucz: Option<String>,
    wyliczone: bool,
    wykonano: Option<String>,
    zamowienie_id: Option<i64>,
    typ: String,
    tytul: String,
    data_od: String,
    data_do: Option<String>,
    data_sweet_spot: Option<String>,
    data_planowa: Option<String>,
    data_krytyczna: Option<String>,
}

fn bez_zmian(s: &ZapisaneZadanie, z: &ZadanieWyliczone) -> bool {
    s.zamowienie_id == z.zamowienie_id
        && s.typ == z.typ.jako_tekst()
        && s.tytul == z.tytul
        && s.data_od == tekst(z.data_od)
        && s.data_do == z.data_do.map(tekst)
        && s.data_sweet_spot == z.data_sweet_spot.map(tekst)
        && s.data_planowa == z.data_planowa.map(tekst)
        && s.data_krytyczna == z.data_krytyczna.map(tekst)
        && (z.wykonano.is_none() || s.wykonano == z.wykonano.map(tekst))
}

/// Zadania `wyliczone = 1` bez klucza (sprzed migracji 0003 albo wstawione przez inne moduły).
/// Przy przeliczeniu dopasowujemy je do wyliczonych zadań – po typie (montaż, warsztat, rysunki,
/// pomiar), materiale (zamówienie, potwierdzenie) albo typie i tytule (pozostałe) – i nadajemy
/// klucz: zachowują id i stan wykonania, a przeliczenie nie tworzy duplikatu. Wiersze, których
/// nie da się jednoznacznie dopasować, stają się zadaniami ręcznymi (`wyliczone = 0`), więc
/// użytkownik może je zmienić albo usunąć. Każda zmiana trafia do dziennika.
async fn dopasuj_zadania_bez_klucza(
    conn: &mut SqliteConnection,
    zapisane: &mut [ZapisaneZadanie],
    wyliczone: &[ZadanieWyliczone],
) -> Wynik<()> {
    let mut zajete: HashSet<String> = zapisane.iter().filter_map(|s| s.klucz.clone()).collect();
    for s in zapisane
        .iter_mut()
        .filter(|s| s.wyliczone && s.klucz.is_none())
    {
        let kandydat = match (s.typ.as_str(), s.zamowienie_id) {
            ("montaz" | "warsztat" | "rysunki" | "pomiar", _) => Some(s.typ.clone()),
            ("zamowienie", Some(m)) => Some(format!("zamowienie:{m}")),
            ("klient", Some(m)) => Some(format!("potwierdzenie:{m}")),
            _ => wyliczone
                .iter()
                .find(|w| {
                    w.typ.jako_tekst() == s.typ && w.tytul == s.tytul && !zajete.contains(&w.klucz)
                })
                .map(|w| w.klucz.clone()),
        }
        .filter(|k| !zajete.contains(k) && wyliczone.iter().any(|w| &w.klucz == k));
        match kandydat {
            Some(k) => {
                sqlx::query("UPDATE zadania_kalendarza SET klucz = ?1 WHERE id = ?2")
                    .bind(&k)
                    .bind(s.id)
                    .execute(&mut *conn)
                    .await?;
                dziennik_opis(
                    conn,
                    "zadania_kalendarza",
                    s.id,
                    "zmiana",
                    Some(&format!("zadanie wyliczone bez klucza dopasowane do „{k}”")),
                )
                .await?;
                zajete.insert(k.clone());
                s.klucz = Some(k);
            }
            None => {
                sqlx::query("UPDATE zadania_kalendarza SET wyliczone = 0 WHERE id = ?1")
                    .bind(s.id)
                    .execute(&mut *conn)
                    .await?;
                dziennik_opis(
                    conn,
                    "zadania_kalendarza",
                    s.id,
                    "zmiana",
                    Some("zadanie wyliczone bez klucza, bez odpowiednika – zamienione na ręczne"),
                )
                .await?;
                s.wyliczone = false;
            }
        }
    }
    Ok(())
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

    let mut zapisane: Vec<ZapisaneZadanie> = sqlx::query_as(
        "SELECT id, klucz, wyliczone, wykonano, zamowienie_id, typ, tytul, data_od, data_do, data_sweet_spot, \
         data_planowa, data_krytyczna FROM zadania_kalendarza WHERE zlecenie_id = ?1 ORDER BY id",
    )
    .bind(id)
    .fetch_all(&mut *conn)
    .await?;
    dopasuj_zadania_bez_klucza(conn, &mut zapisane, &h.zadania).await?;
    let istniejace: Vec<ZadanieIstniejace> = zapisane
        .iter()
        .map(|s| ZadanieIstniejace {
            id: s.id,
            klucz: s.klucz.clone(),
            wyliczone: s.wyliczone,
            wykonane: s.wykonano.is_some(),
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
    let mut zmienione = 0;
    for (zid, zad) in &plan.do_zmiany {
        if zapisane
            .iter()
            .find(|s| s.id == *zid)
            .is_some_and(|s| bez_zmian(s, zad))
        {
            continue;
        }
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
        zmienione += 1;
    }
    for zad in &plan.do_dodania {
        let nowe = wstaw_zadanie(conn, Some(id), zad, true).await?;
        dziennik(conn, "zadania_kalendarza", nowe, "dodanie").await?;
    }
    // Migawka terminów w materiałach. Zamówione: daty zamówienia zamrożone, ale data wymaganej
    // dostawy zawsze z aktualnego harmonogramu (od niej liczy się `dostawa_zagrozona`).
    for t in &h.materialy {
        if t.zamowione {
            sqlx::query(
                "UPDATE zamowienia_materialow SET data_wymaganej_dostawy = ?1 WHERE id = ?2 AND data_wymaganej_dostawy IS NOT ?1",
            )
            .bind(tekst(t.data_wymaganej_dostawy))
            .bind(t.material_id)
            .execute(&mut *conn)
            .await?;
            continue;
        }
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
        zmienione,
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

/// Przelicza harmonogram, jeśli zlecenie już go ma i nie jest po montażu – w transakcji
/// wywołującego (zapis danych i przeliczenie razem). Nieudane przeliczenie oznacza zlecenie
/// jako `harmonogram_nieaktualny` (z powodem) i nie przerywa zapisu.
pub async fn przelicz_jesli_jest_w(conn: &mut SqliteConnection, id: i64) -> Wynik<Przeliczenie> {
    let etap: Option<String> = sqlx::query_scalar("SELECT etap FROM zlecenia WHERE id = ?1")
        .bind(id)
        .fetch_optional(&mut *conn)
        .await?;
    let etap = etap.ok_or_else(|| BladApi::NieZnaleziono(format!("zlecenie {id}")))?;
    if ETAPY_PO_MONTAZU.contains(&etap.as_str()) || !ma_harmonogram(conn, id).await? {
        return Ok(Przeliczenie::Pominiete);
    }
    let (u, kal) = kalendarz_dni(conn).await?;
    Ok(match przelicz_w(conn, id, &u, &kal).await? {
        Ok(w) => Przeliczenie::Przeliczone(w),
        Err(powod) => Przeliczenie::Nieaktualne(powod),
    })
}

/// `POST /zlecenia/{id}/przelicz`: przeliczenie istniejącego harmonogramu na żądanie (np. po
/// zmianie statusu wiersza specyfikacji). Nie tworzy harmonogramu, gdy go nie ma.
pub async fn przelicz_zlecenie(pula: &SqlitePool, id: i64) -> Wynik<(Zlecenie, Przeliczenie)> {
    let mut tx = crate::baza::transakcja_zapisu(pula).await?;
    let p = przelicz_jesli_jest_w(&mut tx, id).await?;
    let z = zlecenie(&mut tx, id).await?;
    tx.commit().await?;
    Ok((z, p))
}

/// Dla modułu specyfikacji: po zmianie statusu wiersza (np. `do_potwierdzenia` → `ustalone`)
/// przelicza – w transakcji wywołującego – zlecenia, których umowa wskazuje tę specyfikację.
/// Zwraca id zleceń i wynik przeliczenia każdego z nich.
pub async fn przelicz_po_zmianie_specyfikacji(
    conn: &mut SqliteConnection,
    specyfikacja_id: i64,
) -> Wynik<Vec<(i64, Przeliczenie)>> {
    let ids: Vec<i64> = sqlx::query_scalar(
        "SELECT z.id FROM zlecenia z JOIN umowy u ON u.id = z.umowa_id WHERE u.specyfikacja_id = ?1 ORDER BY z.id",
    )
    .bind(specyfikacja_id)
    .fetch_all(&mut *conn)
    .await?;
    let mut wynik = Vec::with_capacity(ids.len());
    for id in ids {
        wynik.push((id, przelicz_jesli_jest_w(conn, id).await?));
    }
    Ok(wynik)
}

/// Przelicza (w transakcji wywołującego) wszystkie zlecenia z harmonogramem poza etapami po
/// montażu. Nieudane są oznaczane i zwracane z powodem.
pub async fn przelicz_wszystkie_w(
    conn: &mut SqliteConnection,
) -> Wynik<WynikPrzeliczeniaWszystkich> {
    let zlecenia: Vec<(i64, String)> = sqlx::query_as(&format!(
        "SELECT DISTINCT z.id, z.nazwa FROM zadania_kalendarza t JOIN zlecenia z ON z.id = t.zlecenie_id \
         WHERE t.klucz IS NOT NULL AND {SQL_ETAP_PRZELICZANY} ORDER BY 1"
    ))
    .fetch_all(&mut *conn)
    .await?;
    let (u, kal) = kalendarz_dni(conn).await?;
    let mut wynik = WynikPrzeliczeniaWszystkich::default();
    for (id, nazwa) in zlecenia {
        match przelicz_w(conn, id, &u, &kal).await? {
            Ok(w) => {
                wynik.przeliczone_zlecenia.push(id);
                if w.cokolwiek_zmieniono() {
                    wynik.zmienione_zlecenia.push(id);
                }
            }
            Err(powod) => wynik.nieprzeliczone_zlecenia.push(NieaktualnyHarmonogram {
                zlecenie_id: id,
                zlecenie_nazwa: nazwa,
                powod,
            }),
        }
    }
    Ok(wynik)
}

/// Przelicza wszystkie zlecenia z harmonogramem (`POST /kalendarz/przelicz`).
pub async fn przelicz_wszystkie(pula: &SqlitePool) -> Wynik<WynikPrzeliczeniaWszystkich> {
    let mut tx = crate::baza::transakcja_zapisu(pula).await?;
    let w = przelicz_wszystkie_w(&mut tx).await?;
    tx.commit().await?;
    Ok(w)
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
    /// Harmonogram zlecenia jest nieaktualny (ostatnie przeliczenie się nie udało) – UI
    /// powinno pokazać ostrzeżenie z `harmonogram_nieaktualny_powod`.
    pub harmonogram_nieaktualny: bool,
    pub harmonogram_nieaktualny_powod: Option<String>,
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
    harmonogram_nieaktualny_powod: Option<String>,
}

/// Filtr listy zadań. Zakres dat: zadania, których okres `[data_od, data_do]` nachodzi na `[od, do]`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FiltrZadan {
    #[serde(deserialize_with = "wej::data_opt")]
    pub od: Option<NaiveDate>,
    #[serde(rename = "do", deserialize_with = "wej::data_opt")]
    pub do_: Option<NaiveDate>,
    /// Typ albo kilka typów po przecinku, np. `zamowienie,pomiar`.
    pub typ: Option<String>,
    pub zlecenie_id: Option<i64>,
    /// Stan albo kilka stanów po przecinku, np. `zolty,pomaranczowy,czerwony`.
    pub stan: Option<String>,
    pub tylko_niewykonane: bool,
    /// Data „dziś” do wyliczenia stanów (domyślnie dzisiejsza data serwera).
    #[serde(deserialize_with = "wej::data_opt")]
    pub dzis: Option<NaiveDate>,
    /// Klucz API w parametrze (dozwolony na każdej trasie, sprawdza go warstwa dostępu).
    #[serde(rename = "klucz")]
    pub _klucz: Option<String>,
    /// Tylko wewnętrznie: jedno zadanie po id.
    #[serde(skip)]
    pub id: Option<i64>,
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
    // Warunki dokładane tylko gdy podane – pozwala to SQLite użyć indeksów
    // (ix_zadania_koniec dla `od`, ix_zadania_data dla `do`, częściowy ix_zadania_niewykonane).
    let mut q = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
        "SELECT t.id, t.zlecenie_id, z.nazwa AS zlecenie_nazwa, z.montaz_od, t.zamowienie_id, t.typ, t.klucz, t.tytul, \
         t.data_od, t.data_do, t.data_sweet_spot, t.data_planowa, t.data_krytyczna, t.wyliczone, t.wykonano, t.uwagi, \
         m.kategoria_id, k.nazwa AS kategoria_nazwa, COALESCE(m.dostawca_id, k.domyslny_dostawca_id) AS dostawca_id, \
         d.nazwa AS dostawca_nazwa, m.opis AS zam_opis, m.status AS zam_status, m.data_wymaganej_dostawy, m.dni_min, \
         m.dni_max, m.dni_srednia, m.data_zamowienia, m.data_dostawy, z.harmonogram_nieaktualny_powod \
         FROM zadania_kalendarza t \
         LEFT JOIN zlecenia z ON z.id = t.zlecenie_id \
         LEFT JOIN zamowienia_materialow m ON m.id = t.zamowienie_id AND t.typ = 'zamowienie' \
         LEFT JOIN kategorie_materialow k ON k.id = m.kategoria_id \
         LEFT JOIN dostawcy d ON d.id = COALESCE(m.dostawca_id, k.domyslny_dostawca_id) \
         WHERE 1 = 1",
    );
    if let Some(id) = f.id {
        q.push(" AND t.id = ").push_bind(id);
    }
    if let Some(od) = f.od {
        q.push(" AND COALESCE(t.data_do, t.data_od) >= ")
            .push_bind(tekst(od));
    }
    if let Some(do_) = f.do_ {
        q.push(" AND t.data_od <= ").push_bind(tekst(do_));
    }
    if let Some(zl) = f.zlecenie_id {
        q.push(" AND t.zlecenie_id = ").push_bind(zl);
    }
    if f.tylko_niewykonane {
        q.push(" AND t.wykonano IS NULL");
    }
    if let Some(t) = &typy {
        q.push(" AND t.typ IN (");
        let mut lista = q.separated(", ");
        for typ in t {
            lista.push_bind(typ.jako_tekst());
        }
        q.push(")");
    }
    q.push(" ORDER BY t.data_od, t.id");
    let wiersze: Vec<WierszZadania> = q.build_query_as().fetch_all(&mut *conn).await?;
    let (u, kal) = kalendarz_dni(&mut conn).await?;
    let blok = blokady(&mut conn, f.zlecenie_id).await?;
    let mut wynik = Vec::with_capacity(wiersze.len());
    for w in wiersze {
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
        harmonogram_nieaktualny: w.harmonogram_nieaktualny_powod.is_some(),
        harmonogram_nieaktualny_powod: w.harmonogram_nieaktualny_powod,
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
        id: Some(id),
        zlecenie_id: zl,
        ..Default::default()
    };
    zadania(pula, &f, dzis)
        .await?
        .into_iter()
        .find(|z| z.id == id)
        .ok_or_else(|| BladApi::NieZnaleziono(format!("zadanie {id}")))
}

/// Oznaczenie wykonania (`PUT /kalendarz/zadania/{id}/wykonanie`). Pola trójstanowe: pominięte –
/// bez zmian, `null` – wyczyść.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Wykonanie {
    /// Wymagane: data wykonania (nie z przyszłości) albo `null` = cofnięcie wykonania.
    #[serde(deserialize_with = "wej::data_tri")]
    pub wykonano: Option<Option<NaiveDate>>,
    /// Tylko zamówienia: faktyczna data zamówienia. Pominięta – bez zmian (przy pierwszym
    /// zamówieniu = `wykonano`); `null` – ustaw na `wykonano`.
    #[serde(deserialize_with = "wej::data_tri")]
    pub data_zamowienia: Option<Option<NaiveDate>>,
    /// Tylko zamówienia: faktyczna data dostawy. Pominięta – bez zmian; `null` – wyczyść.
    #[serde(deserialize_with = "wej::data_tri")]
    pub data_dostawy: Option<Option<NaiveDate>>,
    /// Pominięte – bez zmian; `null` albo pusty tekst – wyczyść.
    #[serde(deserialize_with = "wej::tri")]
    pub uwagi: Option<Option<String>>,
    /// Zamówienie mimo wierszy specyfikacji „do potwierdzenia” (bez tego – 409).
    pub mimo_blokady: bool,
}

/// (zlecenie_id, zamowienie_id, typ, klucz)
type DaneZadaniaDoWykonania = (Option<i64>, Option<i64>, String, Option<String>);

fn nie_z_przyszlosci(co: &str, d: Option<NaiveDate>, dzis: NaiveDate) -> Wynik<()> {
    match d {
        Some(d) if d > dzis => Err(BladApi::NieprawidloweDane(format!(
            "{co} {} jest z przyszłości (dziś {})",
            crate::kalendarz::data_pl(d),
            crate::kalendarz::data_pl(dzis)
        ))),
        _ => Ok(()),
    }
}

/// Zapisuje wykonanie (albo jego cofnięcie) i – przy cofnięciu – w tej samej transakcji
/// przelicza zlecenie. Zwraca id zlecenia zadania.
pub async fn wykonaj(
    pula: &SqlitePool,
    id: i64,
    w: Wykonanie,
    dzis: NaiveDate,
) -> Wynik<Option<i64>> {
    let wykonano = w.wykonano.ok_or_else(|| {
        BladApi::NieprawidloweDane(
            "pole wykonano jest wymagane (data RRRR-MM-DD albo null, aby cofnąć wykonanie)".into(),
        )
    })?;
    nie_z_przyszlosci("data wykonania", wykonano, dzis)?;
    let mut tx = crate::baza::transakcja_zapisu(pula).await?;
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
    let uwagi_sql = match w.uwagi {
        None => "uwagi",
        Some(_) => "?2",
    };
    sqlx::query(&format!(
        "UPDATE zadania_kalendarza SET wykonano = ?1, uwagi = {uwagi_sql} WHERE id = ?3"
    ))
    .bind(wykonano.map(tekst))
    .bind(w.uwagi.flatten().and_then(|s| opis_tekst(Some(s))))
    .bind(id)
    .execute(&mut *tx)
    .await?;
    if let (true, Some(mid)) = (jest_zamowieniem, zamowienie_id) {
        let m = material(&mut tx, mid).await?;
        match wykonano {
            None => {
                if matches!(w.data_zamowienia, Some(Some(_)))
                    || matches!(w.data_dostawy, Some(Some(_)))
                {
                    return Err(BladApi::NieprawidloweDane(
                        "przy cofnięciu wykonania (wykonano: null) nie podaje się dat zamówienia ani dostawy".into(),
                    ));
                }
                if m.data_zamowienia.is_some() {
                    cofnij_zamowienie_w(&mut tx, &m).await?;
                    if m.data_dostawy.is_some() {
                        aktualizuj_srednia_rzeczywista(&mut tx, mid).await?;
                    }
                }
            }
            Some(d) => {
                let zam_bylo = data_opt(m.data_zamowienia.as_deref())?;
                let dost_bylo = data_opt(m.data_dostawy.as_deref())?;
                let zam = match w.data_zamowienia {
                    None => zam_bylo.unwrap_or(d),
                    Some(z) => z.unwrap_or(d),
                };
                let dost = w.data_dostawy.unwrap_or(dost_bylo);
                nie_z_przyszlosci("data zamówienia", Some(zam), dzis)?;
                nie_z_przyszlosci("data dostawy", dost, dzis)?;
                let status = status_materialu(Some(zam), dost)?;
                let opis_blokady = if zam_bylo.is_none() {
                    sprawdz_blokade(&mut tx, m.zlecenie_id, m.kategoria_id, w.mimo_blokady).await?
                } else {
                    None
                };
                sqlx::query("UPDATE zamowienia_materialow SET data_zamowienia = ?1, data_dostawy = ?2, status = ?3 WHERE id = ?4")
                    .bind(tekst(zam))
                    .bind(dost.map(tekst))
                    .bind(status)
                    .bind(mid)
                    .execute(&mut *tx)
                    .await?;
                dziennik_opis(
                    &mut tx,
                    "zamowienia_materialow",
                    mid,
                    "zmiana",
                    opis_blokady.as_deref(),
                )
                .await?;
                if dost != dost_bylo {
                    aktualizuj_srednia_rzeczywista(&mut tx, mid).await?;
                }
            }
        }
    }
    dziennik(&mut tx, "zadania_kalendarza", id, "zmiana").await?;
    // cofnięcie wykonania: zadanie wraca pod przeliczanie (nieudane → znacznik na zleceniu)
    if wykonano.is_none() {
        if let Some(zl) = zlecenie_id {
            przelicz_jesli_jest_w(&mut tx, zl).await?;
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
#[serde(deny_unknown_fields)]
pub struct DaneZadaniaRecznego {
    #[serde(default)]
    pub zlecenie_id: Option<i64>,
    pub typ: String,
    pub tytul: String,
    #[serde(deserialize_with = "wej::data")]
    pub data_od: NaiveDate,
    #[serde(default, deserialize_with = "wej::data_opt")]
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
    let mut tx = crate::baza::transakcja_zapisu(pula).await?;
    if let Some(z) = d.zlecenie_id {
        zlecenie(&mut tx, z).await?;
    }
    let uwagi = opis_tekst(d.uwagi);
    let nowe = id.is_none();
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
            // ręczne oraz stare „wyliczone” bez klucza (nikt ich nie przelicza) – po zmianie ręczne
            let n = sqlx::query(
                "UPDATE zadania_kalendarza SET zlecenie_id = ?1, typ = ?2, tytul = ?3, data_od = ?4, data_do = ?5, uwagi = ?6, \
                 wyliczone = 0 WHERE id = ?7 AND (wyliczone = 0 OR klucz IS NULL)",
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
    dziennik(
        &mut tx,
        "zadania_kalendarza",
        id,
        if nowe { "dodanie" } else { "zmiana" },
    )
    .await?;
    tx.commit().await?;
    Ok(id)
}

/// Usuwa zadanie ręczne (także stare „wyliczone” bez klucza – nikt ich nie przelicza).
/// Zadań wyliczonych z kluczem nie usuwa się (znikają przy przeliczeniu).
pub async fn usun_zadanie_reczne(pula: &SqlitePool, id: i64) -> Wynik<Option<i64>> {
    let mut tx = crate::baza::transakcja_zapisu(pula).await?;
    let zl: Option<(Option<i64>, bool, Option<String>)> = sqlx::query_as(
        "SELECT zlecenie_id, wyliczone, klucz FROM zadania_kalendarza WHERE id = ?1",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    match zl {
        None => Err(BladApi::NieZnaleziono(format!("zadanie {id}"))),
        Some((_, true, Some(_))) => Err(BladApi::NieprawidloweDane(
            "zadania wyliczonego nie można usunąć – zniknie po zmianie danych zlecenia i przeliczeniu".into(),
        )),
        Some((zl, _, _)) => {
            sqlx::query("DELETE FROM zadania_kalendarza WHERE id = ?1")
                .bind(id)
                .execute(&mut *tx)
                .await?;
            dziennik(&mut tx, "zadania_kalendarza", id, "usuniecie").await?;
            tx.commit().await?;
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
    /// Zlecenia z nieaktualnym harmonogramem (do ostrzeżenia w UI/widżecie).
    pub nieaktualne_harmonogramy: Vec<NieaktualnyHarmonogram>,
}

/// Zlecenia oznaczone jako `harmonogram_nieaktualny` (poza etapami po montażu).
pub async fn nieaktualne_harmonogramy(pula: &SqlitePool) -> Wynik<Vec<NieaktualnyHarmonogram>> {
    Ok(sqlx::query_as(&format!(
        "SELECT z.id AS zlecenie_id, z.nazwa AS zlecenie_nazwa, z.harmonogram_nieaktualny_powod AS powod \
         FROM zlecenia z WHERE z.harmonogram_nieaktualny_powod IS NOT NULL AND {SQL_ETAP_PRZELICZANY} ORDER BY z.id"
    ))
    .fetch_all(pula)
    .await?)
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
        nieaktualne_harmonogramy: nieaktualne_harmonogramy(pula).await?,
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
    /// Zlecenia z nieaktualnym harmonogramem – ich terminy zamówień mogą być błędne.
    pub nieaktualne_harmonogramy: Vec<NieaktualnyHarmonogram>,
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
        nieaktualne_harmonogramy: nieaktualne_harmonogramy(pula).await?,
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct KolizjaZlecen {
    #[serde(flatten)]
    pub kolizja: Kolizja,
    pub zlecenie_a_nazwa: String,
    pub zlecenie_b_nazwa: String,
    /// Co najmniej jedno ze zleceń ma nieaktualny harmonogram – kolizja może być nieaktualna.
    pub harmonogram_nieaktualny: bool,
}

pub async fn kolizje(
    pula: &SqlitePool,
    od: Option<NaiveDate>,
    do_: Option<NaiveDate>,
) -> Wynik<Vec<KolizjaZlecen>> {
    let wiersze: Vec<(i64, String, bool, String, String, Option<String>)> = sqlx::query_as(
        "SELECT t.zlecenie_id, z.nazwa, z.harmonogram_nieaktualny_powod IS NOT NULL, t.typ, t.data_od, t.data_do FROM zadania_kalendarza t \
         JOIN zlecenia z ON z.id = t.zlecenie_id \
         WHERE t.typ IN ('warsztat', 'montaz') AND z.etap <> 'zakonczone' \
           AND (?1 IS NULL OR COALESCE(t.data_do, t.data_od) >= ?1) AND (?2 IS NULL OR t.data_od <= ?2)",
    )
    .bind(od.map(tekst))
    .bind(do_.map(tekst))
    .fetch_all(pula)
    .await?;
    let mut nazwy = HashMap::new();
    let mut nieaktualne = HashSet::new();
    let mut okresy = Vec::new();
    for (zl, nazwa, nieaktualny, typ, od, do_) in wiersze {
        if nieaktualny {
            nieaktualne.insert(zl);
        }
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
            harmonogram_nieaktualny: nieaktualne.contains(&k.zlecenie_a)
                || nieaktualne.contains(&k.zlecenie_b),
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
        .unwrap()
        .0;
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
        .unwrap()
        .0;
        for kat in [fronty, plyta] {
            dodaj_material(
                &pula,
                z.id,
                DaneMaterialu {
                    kategoria_id: kat,
                    ..Default::default()
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
