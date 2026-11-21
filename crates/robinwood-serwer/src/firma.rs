//! Dane wykonawcy (nagłówek oferty i umowy) oraz domyślne wartości nowej oferty – zapisane
//! w tabeli `ustawienia`, edytowane w module Ustawienia. Żadnych danych firmy w kodzie.

use serde::{Deserialize, Serialize};
use sqlx::{SqliteConnection, SqlitePool};

use crate::bledy::{BladApi, Wynik};

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default)]
pub struct DaneFirmy {
    pub firma_nazwa: String,
    pub firma_adres: String,
    pub firma_nip: String,
    pub firma_telefon: String,
    pub firma_email: String,
    pub oferta_waznosc_dni: i64,
    pub oferta_termin_realizacji: String,
}

const KLUCZE_TEKSTOWE: &[&str] = &[
    "firma_nazwa",
    "firma_adres",
    "firma_nip",
    "firma_telefon",
    "firma_email",
    "oferta_termin_realizacji",
];

pub async fn pobierz_w(db: &mut SqliteConnection) -> Wynik<DaneFirmy> {
    let wiersze: Vec<(String, String)> = sqlx::query_as(
        "SELECT klucz, wartosc FROM ustawienia WHERE klucz LIKE 'firma_%' OR klucz LIKE 'oferta_%'",
    )
    .fetch_all(&mut *db)
    .await?;
    let mut d = DaneFirmy {
        oferta_waznosc_dni: 30,
        ..Default::default()
    };
    for (k, v) in wiersze {
        match k.as_str() {
            "firma_nazwa" => d.firma_nazwa = v,
            "firma_adres" => d.firma_adres = v,
            "firma_nip" => d.firma_nip = v,
            "firma_telefon" => d.firma_telefon = v,
            "firma_email" => d.firma_email = v,
            "oferta_waznosc_dni" => d.oferta_waznosc_dni = v.parse().unwrap_or(30),
            "oferta_termin_realizacji" => d.oferta_termin_realizacji = v,
            _ => {}
        }
    }
    Ok(d)
}

pub async fn pobierz(pula: &SqlitePool) -> Wynik<DaneFirmy> {
    let mut db = pula.acquire().await?;
    pobierz_w(&mut db).await
}

pub async fn zapisz(pula: &SqlitePool, d: DaneFirmy) -> Wynik<DaneFirmy> {
    if !(0..=365).contains(&d.oferta_waznosc_dni) {
        return Err(BladApi::NieprawidloweDane(
            "ważność oferty: od 0 do 365 dni".into(),
        ));
    }
    let mut tx = pula.begin().await?;
    let wartosci = [
        d.firma_nazwa.trim().to_string(),
        d.firma_adres.trim().to_string(),
        d.firma_nip.trim().to_string(),
        d.firma_telefon.trim().to_string(),
        d.firma_email.trim().to_string(),
        d.oferta_termin_realizacji.trim().to_string(),
    ];
    for (k, v) in KLUCZE_TEKSTOWE.iter().zip(wartosci) {
        sqlx::query(
            "INSERT INTO ustawienia (klucz, wartosc) VALUES (?1, ?2) \
             ON CONFLICT(klucz) DO UPDATE SET wartosc = excluded.wartosc",
        )
        .bind(k)
        .bind(v)
        .execute(&mut *tx)
        .await?;
    }
    sqlx::query(
        "INSERT INTO ustawienia (klucz, wartosc) VALUES ('oferta_waznosc_dni', ?1) \
         ON CONFLICT(klucz) DO UPDATE SET wartosc = excluded.wartosc",
    )
    .bind(d.oferta_waznosc_dni.to_string())
    .execute(&mut *tx)
    .await?;
    let wynik = pobierz_w(&mut tx).await?;
    tx.commit().await?;
    Ok(wynik)
}
