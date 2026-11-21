//! Rejestr faktycznych wpłat umowy i protokół odbioru (załącznik nr 5).

use serde::Deserialize;
use sqlx::SqlitePool;

use crate::bledy::{BladApi, Wynik};
use crate::limity::{self, MAKS_DLUGI};
use crate::specyfikacje;
use crate::umowy::{self, PelnaUmowa};

pub const SPOSOBY: &[&str] = &["przelew", "gotowka", "karta", "inny"];

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DaneWplaty {
    pub data: String,
    pub kwota_gr: i64,
    #[serde(default)]
    pub sposob: Option<String>,
    #[serde(default)]
    pub uwagi: Option<String>,
    #[serde(default)]
    pub platnosc_planowana_id: Option<i64>,
}

pub async fn dodaj(pula: &SqlitePool, umowa_id: i64, d: DaneWplaty) -> Wynik<PelnaUmowa> {
    umowy::sprawdz_date("data wpłaty", &Some(d.data.clone()))?;
    if d.kwota_gr == 0 {
        return Err(BladApi::NieprawidloweDane("kwota wpłaty nie może być zerowa".into()));
    }
    // kwota ujemna = zwrot (np. po anulowaniu)
    limity::kwota("kwota wpłaty", Some(d.kwota_gr.abs()))?;
    if let Some(s) = &d.sposob {
        if !SPOSOBY.contains(&s.as_str()) {
            return Err(BladApi::NieprawidloweDane(format!(
                "forma wpłaty: {}",
                SPOSOBY.join(", ")
            )));
        }
    }
    limity::tekst("uwagi", d.uwagi.as_deref(), MAKS_DLUGI)?;
    let mut tx = crate::baza::transakcja_zapisu(pula).await?;
    let u = umowy::pobierz_w(&mut tx, umowa_id).await?;
    if u.status == "szkic" {
        return Err(BladApi::NieprawidloweDane(
            "wpłaty rejestruje się po wysłaniu albo podpisaniu umowy".into(),
        ));
    }
    if let Some(p) = d.platnosc_planowana_id {
        let ok: Option<i64> = sqlx::query_scalar(
            "SELECT id FROM platnosci_planowane WHERE id = ?1 AND umowa_id = ?2",
        )
        .bind(p)
        .bind(umowa_id)
        .fetch_optional(&mut *tx)
        .await?;
        if ok.is_none() {
            return Err(BladApi::NieprawidloweDane(
                "płatność planowana należy do innej umowy".into(),
            ));
        }
    }
    let id = sqlx::query(
        "INSERT INTO wplaty (umowa_id, platnosc_planowana_id, data, kwota_gr, sposob, uwagi) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )
    .bind(umowa_id)
    .bind(d.platnosc_planowana_id)
    .bind(&d.data)
    .bind(d.kwota_gr)
    .bind(&d.sposob)
    .bind(d.uwagi.map(|t| t.trim().to_string()).filter(|t| !t.is_empty()))
    .execute(&mut *tx)
    .await?
    .last_insert_rowid();
    specyfikacje::dziennik(&mut tx, "wplaty", id, "dodanie").await?;
    oznacz_zaplacone_w(&mut tx, umowa_id).await?;
    let p = umowy::pelna_w(&mut tx, umowa_id).await?;
    tx.commit().await?;
    Ok(p)
}

pub async fn usun(pula: &SqlitePool, id: i64) -> Wynik<(i64, PelnaUmowa)> {
    let mut tx = crate::baza::transakcja_zapisu(pula).await?;
    let umowa_id: i64 = sqlx::query_scalar("SELECT umowa_id FROM wplaty WHERE id = ?1")
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| BladApi::NieZnaleziono(format!("wpłata {id}")))?;
    sqlx::query("DELETE FROM wplaty WHERE id = ?1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    specyfikacje::dziennik(&mut tx, "wplaty", id, "usuniecie").await?;
    oznacz_zaplacone_w(&mut tx, umowa_id).await?;
    let p = umowy::pelna_w(&mut tx, umowa_id).await?;
    tx.commit().await?;
    Ok((umowa_id, p))
}

/// Zadania kalendarza „płatność” rat w pełni pokrytych wpłatami są oznaczane jako wykonane
/// (data ostatniej wpłaty), a po usunięciu wpłaty – znów otwierane.
async fn oznacz_zaplacone_w(db: &mut sqlx::SqliteConnection, umowa_id: i64) -> Wynik<()> {
    let platnosci = umowy::platnosci_w(db, umowa_id).await?;
    let wplaty = umowy::wplaty_w(db, umowa_id).await?;
    let suma: i64 = wplaty.iter().map(|w| w.kwota_gr).sum();
    let ostatnia = wplaty.last().map(|w| w.data.clone());
    let zid: Option<i64> = sqlx::query_scalar("SELECT id FROM zlecenia WHERE umowa_id = ?1")
        .bind(umowa_id)
        .fetch_optional(&mut *db)
        .await?;
    let Some(zid) = zid else { return Ok(()) };
    let mut narastajaco = 0;
    for p in platnosci {
        narastajaco += p.kwota_gr;
        let zaplacona = suma >= narastajaco;
        let klucz = format!("platnosc:{}", p.id);
        if zaplacona {
            sqlx::query("UPDATE zadania_kalendarza SET wykonano = ?1 WHERE zlecenie_id = ?2 AND klucz = ?3 AND wykonano IS NULL")
                .bind(&ostatnia)
                .bind(zid)
                .bind(&klucz)
                .execute(&mut *db)
                .await?;
        } else {
            sqlx::query("UPDATE zadania_kalendarza SET wykonano = NULL WHERE zlecenie_id = ?1 AND klucz = ?2 AND wykonano IS NOT NULL")
                .bind(zid)
                .bind(&klucz)
                .execute(&mut *db)
                .await?;
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DaneProtokolu {
    pub data: String,
    #[serde(default)]
    pub uwagi: Option<String>,
    #[serde(default = "prawda")]
    pub bez_zastrzezen: bool,
}

fn prawda() -> bool {
    true
}

/// Protokół odbioru: zapis (jeden na umowę – kolejny zastępuje datę i uwagi), status
/// „odebrana”, termin pozostałości (wariant B) od daty odbioru, etap zlecenia „odbiór”.
pub async fn zapisz_protokol(pula: &SqlitePool, umowa_id: i64, d: DaneProtokolu) -> Wynik<PelnaUmowa> {
    umowy::sprawdz_date("data odbioru", &Some(d.data.clone()))?;
    limity::tekst("uwagi", d.uwagi.as_deref(), MAKS_DLUGI)?;
    let mut tx = crate::baza::transakcja_zapisu(pula).await?;
    let u = umowy::pobierz_w(&mut tx, umowa_id).await?;
    if !["podpisana", "w_realizacji", "odebrana", "zamknieta"].contains(&u.status.as_str()) {
        return Err(BladApi::NieprawidloweDane(format!(
            "protokół odbioru sporządza się dla umowy podpisanej (status: {})",
            umowy::nazwa_statusu(&u.status)
        )));
    }
    let uwagi = d.uwagi.map(|t| t.trim().to_string()).filter(|t| !t.is_empty());
    match umowy::protokol_w(&mut tx, umowa_id).await? {
        Some(p) => {
            sqlx::query("UPDATE protokoly_odbioru SET data = ?1, uwagi = ?2, bez_zastrzezen = ?3 WHERE id = ?4")
                .bind(&d.data)
                .bind(&uwagi)
                .bind(d.bez_zastrzezen)
                .bind(p.id)
                .execute(&mut *tx)
                .await?;
            specyfikacje::dziennik(&mut tx, "protokoly_odbioru", p.id, "zmiana").await?;
        }
        None => {
            let id = sqlx::query(
                "INSERT INTO protokoly_odbioru (umowa_id, data, uwagi, bez_zastrzezen, utworzono) \
                 VALUES (?1, ?2, ?3, ?4, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))",
            )
            .bind(umowa_id)
            .bind(&d.data)
            .bind(&uwagi)
            .bind(d.bez_zastrzezen)
            .execute(&mut *tx)
            .await?
            .last_insert_rowid();
            specyfikacje::dziennik(&mut tx, "protokoly_odbioru", id, "dodanie").await?;
        }
    }
    if u.status == "podpisana" || u.status == "w_realizacji" {
        sqlx::query("UPDATE umowy SET status = 'odebrana', wersja = wersja + 1, zmieniono = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = ?1")
            .bind(umowa_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE zlecenia SET etap = 'odbior' WHERE umowa_id = ?1")
            .bind(umowa_id)
            .execute(&mut *tx)
            .await?;
    }
    let u2 = umowy::pobierz_w(&mut tx, umowa_id).await?;
    let plan = umowy::plan_w(&mut tx, &u2).await?;
    if plan.zgodny {
        umowy::zapisz_plan_w(&mut tx, umowa_id, &plan).await?;
    }
    umowy::przelicz_zlecenie_umowy(&mut tx, umowa_id).await?;
    specyfikacje::dziennik(&mut tx, "umowy", umowa_id, "zmiana").await?;
    let p = umowy::pelna_w(&mut tx, umowa_id).await?;
    tx.commit().await?;
    Ok(p)
}
