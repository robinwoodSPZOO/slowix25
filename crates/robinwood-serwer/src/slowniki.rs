//! Słowniki podpowiedzi i kategorie materiałów (odczyt; edycja w kolejnych fazach).

use serde::Serialize;
use sqlx::{FromRow, SqlitePool};

use crate::bledy::Wynik;

#[derive(Debug, Serialize, FromRow)]
pub struct Slownik {
    pub kod: String,
    pub nazwa: String,
}

#[derive(Debug, Serialize, FromRow)]
pub struct PozycjaSlownika {
    pub id: i64,
    pub slownik_kod: String,
    pub wartosc: String,
    pub producent: Option<String>,
    pub dekor: Option<String>,
    pub kategoria_materialu_id: Option<i64>,
    pub kolejnosc: i64,
}

#[derive(Debug, Serialize, FromRow)]
pub struct KategoriaMaterialu {
    pub id: i64,
    pub nazwa: String,
    pub opis: Option<String>,
    pub domyslny_dostawca_id: Option<i64>,
    pub dni_dostawy_min: i64,
    pub dni_dostawy_max: i64,
}

pub async fn lista_slownikow(pula: &SqlitePool) -> Wynik<Vec<Slownik>> {
    Ok(
        sqlx::query_as("SELECT kod, nazwa FROM slowniki ORDER BY nazwa")
            .fetch_all(pula)
            .await?,
    )
}

pub async fn pozycje(pula: &SqlitePool, kod: &str) -> Wynik<Vec<PozycjaSlownika>> {
    Ok(sqlx::query_as(
        "SELECT id, slownik_kod, wartosc, producent, dekor, kategoria_materialu_id, kolejnosc \
         FROM pozycje_slownika WHERE slownik_kod = ?1 AND aktywna = 1 ORDER BY kolejnosc, wartosc",
    )
    .bind(kod)
    .fetch_all(pula)
    .await?)
}

pub async fn kategorie_materialow(pula: &SqlitePool) -> Wynik<Vec<KategoriaMaterialu>> {
    Ok(sqlx::query_as(
        "SELECT id, nazwa, opis, domyslny_dostawca_id, dni_dostawy_min, dni_dostawy_max \
         FROM kategorie_materialow WHERE aktywna = 1 ORDER BY nazwa",
    )
    .fetch_all(pula)
    .await?)
}
