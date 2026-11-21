//! Pliki (zdjęcia) przechowywane w folderze danych serwera: `<folder>/pliki/RRRR/MM/<sha256>.<rozszerzenie>`.
//! W bazie: tabela `pliki` (ścieżka względna) i `zdjecia_specyfikacji` (przypięcie do oferty,
//! sekcji albo części).

use std::path::{Path, PathBuf};

use serde::Deserialize;
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;

use crate::bledy::{BladApi, Wynik};
use crate::specyfikacje::{self, WynikZmiany};

/// Największy przyjmowany plik.
pub const MAKS_ROZMIAR: usize = 20 * 1024 * 1024;

/// Rozpoznaje obraz po nagłówku (nie ufamy nazwie ani nagłówkowi Content-Type).
pub fn typ_obrazu(bajty: &[u8]) -> Option<(&'static str, &'static str)> {
    if bajty.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some(("image/jpeg", "jpg"))
    } else if bajty.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some(("image/png", "png"))
    } else {
        None
    }
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct ParametryZdjecia {
    pub sekcja_id: Option<i64>,
    pub czesc_id: Option<i64>,
    pub nazwa: Option<String>,
    pub podpis: Option<String>,
}

/// Zapisuje zdjęcie i przypina je do specyfikacji (całej oferty), sekcji albo części.
pub async fn dodaj_zdjecie(
    pula: &SqlitePool,
    folder: &Path,
    spec_id: i64,
    p: ParametryZdjecia,
    bajty: &[u8],
) -> Wynik<WynikZmiany> {
    if bajty.is_empty() || bajty.len() > MAKS_ROZMIAR {
        return Err(BladApi::NieprawidloweDane(format!(
            "plik musi mieć od 1 B do {} MB",
            MAKS_ROZMIAR / 1024 / 1024
        )));
    }
    let (mime, rozszerzenie) = typ_obrazu(bajty)
        .ok_or_else(|| BladApi::NieprawidloweDane("obsługiwane są zdjęcia JPG i PNG".into()))?;
    let sha = format!("{:x}", Sha256::digest(bajty));
    let mut tx = pula.begin().await?;
    specyfikacje::sprawdz_zapis(&mut tx, spec_id).await?;

    // Sekcja/część musi należeć do tej specyfikacji; część ustala też sekcję.
    let mut sekcja_id = p.sekcja_id;
    if let Some(c) = p.czesc_id {
        let r: Option<(i64, i64)> = sqlx::query_as(
            "SELECT c.sekcja_id, s.specyfikacja_id FROM czesci_specyfikacji c \
             JOIN sekcje_specyfikacji s ON s.id = c.sekcja_id WHERE c.id = ?1",
        )
        .bind(c)
        .fetch_optional(&mut *tx)
        .await?;
        match r {
            Some((s, sp)) if sp == spec_id => sekcja_id = Some(s),
            _ => {
                return Err(BladApi::NieprawidloweDane(format!(
                    "część {c} nie należy do specyfikacji"
                )))
            }
        }
    } else if let Some(s) = sekcja_id {
        if specyfikacje::spec_sekcji_pub(&mut tx, s).await? != spec_id {
            return Err(BladApi::NieprawidloweDane(format!(
                "sekcja {s} nie należy do specyfikacji"
            )));
        }
    }

    let istniejacy: Option<i64> = sqlx::query_scalar("SELECT id FROM pliki WHERE sha256 = ?1")
        .bind(&sha)
        .fetch_optional(&mut *tx)
        .await?;
    let plik_id = match istniejacy {
        Some(id) => id,
        None => {
            let teraz = chrono::Utc::now();
            let wzgledna = format!(
                "pliki/{}/{}.{rozszerzenie}",
                teraz.format("%Y/%m"),
                &sha[..32]
            );
            let pelna = folder.join(&wzgledna);
            if let Some(k) = pelna.parent() {
                std::fs::create_dir_all(k).map_err(|e| blad_pliku(&pelna, e))?;
            }
            std::fs::write(&pelna, bajty).map_err(|e| blad_pliku(&pelna, e))?;
            let nazwa = p
                .nazwa
                .as_deref()
                .map(str::trim)
                .filter(|n| !n.is_empty())
                .map(|n| n.chars().take(200).collect::<String>())
                .unwrap_or_else(|| format!("zdjecie.{rozszerzenie}"));
            sqlx::query(
                "INSERT INTO pliki (sciezka_wzgledna, nazwa, typ_mime, rozmiar_bajty, sha256) VALUES (?1, ?2, ?3, ?4, ?5)",
            )
            .bind(&wzgledna)
            .bind(nazwa)
            .bind(mime)
            .bind(bajty.len() as i64)
            .bind(&sha)
            .execute(&mut *tx)
            .await?
            .last_insert_rowid()
        }
    };
    let kolejnosc: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(kolejnosc), -1) + 1 FROM zdjecia_specyfikacji WHERE specyfikacja_id = ?1",
    )
    .bind(spec_id)
    .fetch_one(&mut *tx)
    .await?;
    let id = sqlx::query(
        "INSERT INTO zdjecia_specyfikacji (specyfikacja_id, sekcja_id, czesc_id, plik_id, podpis, kolejnosc) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )
    .bind(spec_id)
    .bind(sekcja_id)
    .bind(p.czesc_id)
    .bind(plik_id)
    .bind(p.podpis.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()))
    .bind(kolejnosc)
    .execute(&mut *tx)
    .await?
    .last_insert_rowid();
    specyfikacje::dziennik(&mut tx, "zdjecia_specyfikacji", id, "dodanie").await?;
    let w = specyfikacje::pobierz_w_wynik(&mut tx, id, spec_id).await?;
    tx.commit().await?;
    Ok(w)
}

fn blad_pliku(sciezka: &Path, e: std::io::Error) -> BladApi {
    tracing::error!("Błąd zapisu pliku {}: {e}", sciezka.display());
    BladApi::NieprawidloweDane("nie udało się zapisać pliku na serwerze".into())
}

async fn spec_zdjecia(pula: &SqlitePool, id: i64) -> Wynik<i64> {
    sqlx::query_scalar("SELECT specyfikacja_id FROM zdjecia_specyfikacji WHERE id = ?1")
        .bind(id)
        .fetch_optional(pula)
        .await?
        .ok_or_else(|| BladApi::NieZnaleziono(format!("zdjęcie {id}")))
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct DaneZdjecia {
    pub podpis: Option<String>,
}

pub async fn zmien_zdjecie(pula: &SqlitePool, id: i64, d: DaneZdjecia) -> Wynik<WynikZmiany> {
    let spec = spec_zdjecia(pula, id).await?;
    let mut tx = pula.begin().await?;
    specyfikacje::sprawdz_zapis(&mut tx, spec).await?;
    sqlx::query("UPDATE zdjecia_specyfikacji SET podpis = ?1 WHERE id = ?2")
        .bind(
            d.podpis
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
        )
        .bind(id)
        .execute(&mut *tx)
        .await?;
    let w = specyfikacje::pobierz_w_wynik(&mut tx, id, spec).await?;
    tx.commit().await?;
    Ok(w)
}

/// Odpina zdjęcie. Plik zostaje (może być użyty w innej wersji oferty).
pub async fn usun_zdjecie(pula: &SqlitePool, id: i64) -> Wynik<WynikZmiany> {
    let spec = spec_zdjecia(pula, id).await?;
    let mut tx = pula.begin().await?;
    specyfikacje::sprawdz_zapis(&mut tx, spec).await?;
    sqlx::query("DELETE FROM zdjecia_specyfikacji WHERE id = ?1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    specyfikacje::dziennik(&mut tx, "zdjecia_specyfikacji", id, "usuniecie").await?;
    let w = specyfikacje::pobierz_w_wynik(&mut tx, id, spec).await?;
    tx.commit().await?;
    Ok(w)
}

/// Odczyt pliku: (typ MIME, nazwa, bajty).
pub async fn odczytaj(
    pula: &SqlitePool,
    folder: &Path,
    id: i64,
) -> Wynik<(String, String, Vec<u8>)> {
    let r: Option<(String, String, Option<String>)> =
        sqlx::query_as("SELECT sciezka_wzgledna, nazwa, typ_mime FROM pliki WHERE id = ?1")
            .bind(id)
            .fetch_optional(pula)
            .await?;
    let (wzgledna, nazwa, mime) = r.ok_or_else(|| BladApi::NieZnaleziono(format!("plik {id}")))?;
    let pelna = bezpieczna_sciezka(folder, &wzgledna)?;
    let bajty = std::fs::read(&pelna).map_err(|e| {
        tracing::error!("Brak pliku {}: {e}", pelna.display());
        BladApi::NieZnaleziono(format!("plik {id} (brak na dysku serwera)"))
    })?;
    Ok((
        mime.unwrap_or_else(|| "application/octet-stream".into()),
        nazwa,
        bajty,
    ))
}

/// Ścieżka względna nie może wyjść poza folder danych.
pub fn bezpieczna_sciezka(folder: &Path, wzgledna: &str) -> Wynik<PathBuf> {
    let p = Path::new(wzgledna);
    if p.is_absolute()
        || p.components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
    {
        return Err(BladApi::NieprawidloweDane(
            "nieprawidłowa ścieżka pliku".into(),
        ));
    }
    Ok(folder.join(p))
}

#[cfg(test)]
mod testy {
    use super::*;

    #[test]
    fn rozpoznawanie_i_sciezki() {
        assert_eq!(typ_obrazu(&[0xFF, 0xD8, 0xFF, 0xE0]).unwrap().1, "jpg");
        assert_eq!(typ_obrazu(b"\x89PNG\r\n\x1a\n....").unwrap().0, "image/png");
        assert!(typ_obrazu(b"GIF89a").is_none());
        let f = Path::new("/dane");
        assert!(bezpieczna_sciezka(f, "pliki/2026/11/a.png").is_ok());
        assert!(bezpieczna_sciezka(f, "../x").is_err());
        assert!(bezpieczna_sciezka(f, "/etc/passwd").is_err());
    }
}
