//! Baza klientów: model i operacje na tabeli `klienci`.

use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};

use crate::bledy::{BladApi, Wynik};

pub const TYPY_KLIENTA: &[&str] = &["indywidualny", "firma", "architekt", "instytucja"];

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq)]
pub struct Klient {
    pub id: i64,
    pub typ: String,
    pub nazwa: String,
    pub nip: Option<String>,
    pub email: Option<String>,
    pub telefon: Option<String>,
    pub adres_ulica: Option<String>,
    pub adres_kod: Option<String>,
    pub adres_miasto: Option<String>,
    pub polecajacy_id: Option<i64>,
    pub zgoda_marketing: bool,
    pub zgoda_marketing_data: Option<String>,
    pub uwagi: Option<String>,
    pub utworzono: String,
    pub zmieniono: String,
}

/// Dane do utworzenia lub pełnej aktualizacji klienta.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct DaneKlienta {
    pub typ: Option<String>,
    pub nazwa: String,
    pub nip: Option<String>,
    pub email: Option<String>,
    pub telefon: Option<String>,
    pub adres_ulica: Option<String>,
    pub adres_kod: Option<String>,
    pub adres_miasto: Option<String>,
    pub polecajacy_id: Option<i64>,
    pub zgoda_marketing: bool,
    pub uwagi: Option<String>,
}

impl DaneKlienta {
    /// Walidacja i normalizacja (przycięcie spacji, puste napisy → NULL).
    pub fn oczysc(mut self) -> Wynik<Self> {
        self.nazwa = self.nazwa.trim().to_string();
        if self.nazwa.is_empty() {
            return Err(BladApi::NieprawidloweDane(
                "nazwa klienta jest wymagana".into(),
            ));
        }
        let typ = self.typ.take().unwrap_or_else(|| "indywidualny".into());
        if !TYPY_KLIENTA.contains(&typ.as_str()) {
            return Err(BladApi::NieprawidloweDane(format!(
                "nieznany typ klienta „{typ}” (dozwolone: {})",
                TYPY_KLIENTA.join(", ")
            )));
        }
        self.typ = Some(typ);
        for pole in [
            &mut self.nip,
            &mut self.email,
            &mut self.telefon,
            &mut self.adres_ulica,
            &mut self.adres_kod,
            &mut self.adres_miasto,
            &mut self.uwagi,
        ] {
            *pole = pole
                .take()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
        }
        if let Some(email) = &self.email {
            if !email.contains('@') {
                return Err(BladApi::NieprawidloweDane(
                    "nieprawidłowy adres e-mail".into(),
                ));
            }
        }
        Ok(self)
    }
}

const KOLUMNY: &str = "id, typ, nazwa, nip, email, telefon, adres_ulica, adres_kod, adres_miasto, \
    polecajacy_id, zgoda_marketing, zgoda_marketing_data, uwagi, utworzono, zmieniono";

pub async fn lista(pula: &SqlitePool, szukaj: Option<&str>) -> Wynik<Vec<Klient>> {
    let sql = format!(
        "SELECT {KOLUMNY} FROM klienci WHERE usunieto IS NULL \
         AND (?1 IS NULL OR nazwa LIKE '%' || ?1 || '%' OR email LIKE '%' || ?1 || '%' OR telefon LIKE '%' || ?1 || '%') \
         ORDER BY nazwa COLLATE NOCASE"
    );
    let szukaj = szukaj.map(str::trim).filter(|s| !s.is_empty());
    Ok(sqlx::query_as::<_, Klient>(&sql)
        .bind(szukaj)
        .fetch_all(pula)
        .await?)
}

pub async fn pobierz(pula: &SqlitePool, id: i64) -> Wynik<Klient> {
    let sql = format!("SELECT {KOLUMNY} FROM klienci WHERE id = ?1 AND usunieto IS NULL");
    sqlx::query_as::<_, Klient>(&sql)
        .bind(id)
        .fetch_optional(pula)
        .await?
        .ok_or_else(|| BladApi::NieZnaleziono(format!("klient {id}")))
}

pub async fn dodaj(pula: &SqlitePool, dane: DaneKlienta) -> Wynik<Klient> {
    let d = dane.oczysc()?;
    let id = sqlx::query(
        "INSERT INTO klienci (typ, nazwa, nip, email, telefon, adres_ulica, adres_kod, adres_miasto, \
         polecajacy_id, zgoda_marketing, zgoda_marketing_data, uwagi) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, \
         CASE WHEN ?10 THEN strftime('%Y-%m-%dT%H:%M:%SZ', 'now') END, ?11)",
    )
    .bind(&d.typ)
    .bind(&d.nazwa)
    .bind(&d.nip)
    .bind(&d.email)
    .bind(&d.telefon)
    .bind(&d.adres_ulica)
    .bind(&d.adres_kod)
    .bind(&d.adres_miasto)
    .bind(d.polecajacy_id)
    .bind(d.zgoda_marketing)
    .bind(&d.uwagi)
    .execute(pula)
    .await?
    .last_insert_rowid();
    zapisz_w_dzienniku(pula, id, "dodanie").await?;
    pobierz(pula, id).await
}

pub async fn zmien(pula: &SqlitePool, id: i64, dane: DaneKlienta) -> Wynik<Klient> {
    let d = dane.oczysc()?;
    if d.polecajacy_id == Some(id) {
        return Err(BladApi::NieprawidloweDane(
            "klient nie może polecać sam siebie".into(),
        ));
    }
    let zmienione = sqlx::query(
        "UPDATE klienci SET typ = ?1, nazwa = ?2, nip = ?3, email = ?4, telefon = ?5, adres_ulica = ?6, \
         adres_kod = ?7, adres_miasto = ?8, polecajacy_id = ?9, \
         zgoda_marketing_data = CASE WHEN ?10 AND zgoda_marketing = 0 THEN strftime('%Y-%m-%dT%H:%M:%SZ', 'now') \
                                     WHEN ?10 THEN zgoda_marketing_data END, \
         zgoda_marketing = ?10, uwagi = ?11, \
         zmieniono = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') \
         WHERE id = ?12 AND usunieto IS NULL",
    )
    .bind(&d.typ)
    .bind(&d.nazwa)
    .bind(&d.nip)
    .bind(&d.email)
    .bind(&d.telefon)
    .bind(&d.adres_ulica)
    .bind(&d.adres_kod)
    .bind(&d.adres_miasto)
    .bind(d.polecajacy_id)
    .bind(d.zgoda_marketing)
    .bind(&d.uwagi)
    .bind(id)
    .execute(pula)
    .await?
    .rows_affected();
    if zmienione == 0 {
        return Err(BladApi::NieZnaleziono(format!("klient {id}")));
    }
    zapisz_w_dzienniku(pula, id, "zmiana").await?;
    pobierz(pula, id).await
}

/// Usunięcie miękkie: klient znika z list, ale oferty i umowy zachowują powiązanie.
pub async fn usun(pula: &SqlitePool, id: i64) -> Wynik<()> {
    let zmienione = sqlx::query(
        "UPDATE klienci SET usunieto = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = ?1 AND usunieto IS NULL",
    )
    .bind(id)
    .execute(pula)
    .await?
    .rows_affected();
    if zmienione == 0 {
        return Err(BladApi::NieZnaleziono(format!("klient {id}")));
    }
    zapisz_w_dzienniku(pula, id, "usuniecie").await?;
    Ok(())
}

async fn zapisz_w_dzienniku(pula: &SqlitePool, id: i64, operacja: &str) -> Wynik<()> {
    sqlx::query(
        "INSERT INTO dziennik_zmian (tabela, rekord_id, operacja) VALUES ('klienci', ?1, ?2)",
    )
    .bind(id)
    .bind(operacja)
    .execute(pula)
    .await?;
    Ok(())
}

#[cfg(test)]
mod testy {
    use super::*;
    use crate::baza::otworz_w_pamieci;

    fn przyklad(nazwa: &str) -> DaneKlienta {
        DaneKlienta {
            nazwa: nazwa.into(),
            email: Some("  test@example.com ".into()),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn pelny_cykl_crud() {
        let pula = otworz_w_pamieci().await.unwrap();
        let k = dodaj(&pula, przyklad("Klient Testowy")).await.unwrap();
        assert_eq!(k.typ, "indywidualny");
        assert_eq!(k.email.as_deref(), Some("test@example.com"));
        assert!(!k.zgoda_marketing);

        let mut dane = przyklad("Klient Testowy 2");
        dane.typ = Some("firma".into());
        dane.zgoda_marketing = true;
        let k2 = zmien(&pula, k.id, dane).await.unwrap();
        assert_eq!(k2.nazwa, "Klient Testowy 2");
        assert!(k2.zgoda_marketing);
        assert!(k2.zgoda_marketing_data.is_some());

        assert_eq!(lista(&pula, Some("testowy 2")).await.unwrap().len(), 1);
        usun(&pula, k.id).await.unwrap();
        assert!(lista(&pula, None).await.unwrap().is_empty());
        assert!(matches!(
            pobierz(&pula, k.id).await,
            Err(BladApi::NieZnaleziono(_))
        ));
    }

    #[tokio::test]
    async fn walidacja() {
        let pula = otworz_w_pamieci().await.unwrap();
        assert!(dodaj(&pula, przyklad("   ")).await.is_err());
        let mut zly_typ = przyklad("X");
        zly_typ.typ = Some("kosmita".into());
        assert!(dodaj(&pula, zly_typ).await.is_err());
        let mut zly_email = przyklad("X");
        zly_email.email = Some("bez-malpy".into());
        assert!(dodaj(&pula, zly_email).await.is_err());
    }
}
