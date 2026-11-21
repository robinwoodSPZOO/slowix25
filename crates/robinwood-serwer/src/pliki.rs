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

/// Największy bok przyjmowanego obrazu (piksele).
pub const MAKS_BOK: u32 = 12_000;
/// Największa liczba pikseli obrazu (50 Mpx – zdjęcia z telefonów 48 Mpx przechodzą).
pub const MAKS_PIKSELI: u64 = 50_000_000;
/// Największy bok podglądu zapisywanego obok oryginału (dokumenty i miniatury).
pub const BOK_PODGLADU: u32 = 1600;

/// Obraz sprawdzony i pomniejszony (wynik pracy w `spawn_blocking`).
pub struct PrzygotowanyObraz {
    pub mime: &'static str,
    pub rozszerzenie: &'static str,
    pub szerokosc: u32,
    pub wysokosc: u32,
    /// JPEG z najdłuższym bokiem ≤ [`BOK_PODGLADU`].
    pub podglad_jpg: Vec<u8>,
}

/// Wymiary z nagłówka (bez dekodowania pikseli) i sprawdzenie limitów.
pub fn sprawdz_wymiary(bajty: &[u8]) -> Wynik<(u32, u32)> {
    let czytnik = image023::io::Reader::new(std::io::Cursor::new(bajty))
        .with_guessed_format()
        .map_err(|_| BladApi::NieprawidloweDane("nie udało się odczytać obrazu".into()))?;
    let (w, h) = czytnik.into_dimensions().map_err(|_| {
        BladApi::NieprawidloweDane("uszkodzony plik obrazu – nie da się odczytać wymiarów".into())
    })?;
    if w == 0 || h == 0 {
        return Err(BladApi::NieprawidloweDane("obraz ma zerowy wymiar".into()));
    }
    if w > MAKS_BOK || h > MAKS_BOK || u64::from(w) * u64::from(h) > MAKS_PIKSELI {
        return Err(BladApi::NieprawidloweDane(format!(
            "obraz {w}×{h} px jest za duży – najwyżej {MAKS_BOK} px na bok i 50 Mpx; zmniejsz go przed dodaniem"
        )));
    }
    Ok((w, h))
}

/// Pełne sprawdzenie obrazu: wymiary, faktyczne zdekodowanie (uszkodzony plik z poprawnym
/// nagłówkiem → 422) i podgląd JPEG. Ciężka praca – wywoływać w `spawn_blocking`.
pub fn przygotuj_obraz(bajty: &[u8]) -> Wynik<PrzygotowanyObraz> {
    let (mime, rozszerzenie) = typ_obrazu(bajty)
        .ok_or_else(|| BladApi::NieprawidloweDane("obsługiwane są zdjęcia JPG i PNG".into()))?;
    let (szerokosc, wysokosc) = sprawdz_wymiary(bajty)?;
    let img = image023::load_from_memory(bajty).map_err(|_| {
        BladApi::NieprawidloweDane(
            "uszkodzony plik obrazu – nie da się go odczytać (zapisz go ponownie w programie graficznym)"
                .into(),
        )
    })?;
    let img = if szerokosc > BOK_PODGLADU || wysokosc > BOK_PODGLADU {
        img.thumbnail(BOK_PODGLADU, BOK_PODGLADU)
    } else {
        img
    };
    let rgb = img.to_rgb8();
    let mut podglad_jpg = Vec::new();
    image023::jpeg::JpegEncoder::new_with_quality(&mut podglad_jpg, 85)
        .encode(
            rgb.as_raw(),
            rgb.width(),
            rgb.height(),
            image023::ColorType::Rgb8,
        )
        .map_err(|e| BladApi::Wewnetrzny(format!("podgląd obrazu: {e}")))?;
    Ok(PrzygotowanyObraz {
        mime,
        rozszerzenie,
        szerokosc,
        wysokosc,
        podglad_jpg,
    })
}

/// Zapis pliku przez plik tymczasowy w tym samym katalogu i zmianę nazwy (bez połówek pliku).
fn zapisz_atomowo(pelna: &Path, bajty: &[u8]) -> Wynik<()> {
    if let Some(k) = pelna.parent() {
        std::fs::create_dir_all(k).map_err(|e| blad_pliku(pelna, e))?;
    }
    let tymczasowa = pelna.with_extension(format!(
        "tmp-{}-{}",
        std::process::id(),
        crate::specyfikacje::nowy_ident()
    ));
    std::fs::write(&tymczasowa, bajty).map_err(|e| blad_pliku(&tymczasowa, e))?;
    std::fs::rename(&tymczasowa, pelna).map_err(|e| {
        let _ = std::fs::remove_file(&tymczasowa);
        blad_pliku(pelna, e)
    })
}

/// Pliki zapisane w tej operacji; usuwane, jeśli transakcja nie zostanie zatwierdzona.
struct NoweZapisane(Vec<std::path::PathBuf>);

impl Drop for NoweZapisane {
    fn drop(&mut self) {
        for p in &self.0 {
            let _ = std::fs::remove_file(p);
        }
    }
}

/// Zapisuje zdjęcie i przypina je do specyfikacji (całej oferty), sekcji albo części.
///
/// Obraz jest sprawdzany (wymiary z nagłówka, pełne dekodowanie) i pomniejszany w
/// `spawn_blocking`. Ten sam plik (sha256) jest zapisywany raz – dwa równoczesne wysłania
/// tego samego zdjęcia czekają na siebie (transakcja zapisu) i drugie używa pliku pierwszego.
/// Błąd przed zatwierdzeniem transakcji usuwa świeżo zapisane pliki (bez sierot).
pub async fn dodaj_zdjecie(
    pula: &SqlitePool,
    folder: &Path,
    spec_id: i64,
    p: ParametryZdjecia,
    bajty: axum::body::Bytes,
) -> Wynik<WynikZmiany> {
    if bajty.is_empty() || bajty.len() > MAKS_ROZMIAR {
        return Err(BladApi::NieprawidloweDane(format!(
            "plik musi mieć od 1 B do {} MB",
            MAKS_ROZMIAR / 1024 / 1024
        )));
    }
    typ_obrazu(&bajty)
        .ok_or_else(|| BladApi::NieprawidloweDane("obsługiwane są zdjęcia JPG i PNG".into()))?;
    // Szybkie odrzucenie za dużych obrazów jeszcze przed dekodowaniem.
    sprawdz_wymiary(&bajty)?;
    let b2 = bajty.clone();
    let (obraz, sha) = tokio::task::spawn_blocking(move || {
        let sha = format!("{:x}", Sha256::digest(&b2));
        przygotuj_obraz(&b2).map(|o| (o, sha))
    })
    .await
    .map_err(|e| BladApi::Wewnetrzny(format!("przetwarzanie obrazu: {e}")))??;

    let mut tx = crate::baza::transakcja_zapisu(pula).await?;
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

    let mut zapisane = NoweZapisane(Vec::new());
    let istniejacy: Option<(i64, Option<String>)> = sqlx::query_as(
        "SELECT id, podglad_sciezka FROM pliki WHERE sha256 = ?1 ORDER BY id LIMIT 1",
    )
    .bind(&sha)
    .fetch_optional(&mut *tx)
    .await?;
    let plik_id = match istniejacy {
        Some((id, podglad)) => {
            if podglad.is_none() {
                let wzgledna = format!("pliki/podglad/{}.jpg", &sha[..32]);
                let pelna = bezpieczna_sciezka(folder, &wzgledna)?;
                zapisz_atomowo(&pelna, &obraz.podglad_jpg)?;
                zapisane.0.push(pelna);
                sqlx::query("UPDATE pliki SET podglad_sciezka = ?1, szerokosc = ?2, wysokosc = ?3 WHERE id = ?4")
                    .bind(&wzgledna)
                    .bind(obraz.szerokosc)
                    .bind(obraz.wysokosc)
                    .bind(id)
                    .execute(&mut *tx)
                    .await?;
            }
            id
        }
        None => {
            let teraz = chrono::Utc::now();
            let wzgledna = format!(
                "pliki/{}/{}.{}",
                teraz.format("%Y/%m"),
                &sha[..32],
                obraz.rozszerzenie
            );
            let podglad = format!("pliki/podglad/{}.jpg", &sha[..32]);
            let pelna = bezpieczna_sciezka(folder, &wzgledna)?;
            let pelna_podglad = bezpieczna_sciezka(folder, &podglad)?;
            let istnial = pelna.exists();
            zapisz_atomowo(&pelna, &bajty)?;
            if !istnial {
                zapisane.0.push(pelna);
            }
            zapisz_atomowo(&pelna_podglad, &obraz.podglad_jpg)?;
            zapisane.0.push(pelna_podglad);
            let nazwa = p
                .nazwa
                .as_deref()
                .map(str::trim)
                .filter(|n| !n.is_empty())
                .map(|n| n.chars().take(200).collect::<String>())
                .unwrap_or_else(|| format!("zdjecie.{}", obraz.rozszerzenie));
            sqlx::query(
                "INSERT INTO pliki (sciezka_wzgledna, nazwa, typ_mime, rozmiar_bajty, sha256, podglad_sciezka, szerokosc, wysokosc) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            )
            .bind(&wzgledna)
            .bind(nazwa)
            .bind(obraz.mime)
            .bind(bajty.len() as i64)
            .bind(&sha)
            .bind(&podglad)
            .bind(obraz.szerokosc)
            .bind(obraz.wysokosc)
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
    let podpis = p
        .podpis
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    crate::limity::tekst("podpis", podpis.as_deref(), crate::limity::MAKS_NAZWA)?;
    let id = sqlx::query(
        "INSERT INTO zdjecia_specyfikacji (specyfikacja_id, sekcja_id, czesc_id, plik_id, podpis, kolejnosc) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )
    .bind(spec_id)
    .bind(sekcja_id)
    .bind(p.czesc_id)
    .bind(plik_id)
    .bind(podpis)
    .bind(kolejnosc)
    .execute(&mut *tx)
    .await?
    .last_insert_rowid();
    specyfikacje::dziennik(&mut tx, "zdjecia_specyfikacji", id, "dodanie").await?;
    let w = specyfikacje::pobierz_w_wynik(&mut tx, id, spec_id).await?;
    tx.commit().await?;
    // Zatwierdzone – pliki zostają.
    zapisane.0.clear();
    Ok(w)
}

/// Usuwa pliki, do których nic się już nie odwołuje (zdjęcia specyfikacji, załączniki
/// korespondencji). Wywoływane po usunięciu oferty i odpięciu zdjęcia; zwraca liczbę usuniętych.
pub async fn sprzataj_nieuzywane(pula: &SqlitePool, folder: &Path) -> Wynik<usize> {
    let mut tx = crate::baza::transakcja_zapisu(pula).await?;
    let sieroty: Vec<(i64, String, Option<String>)> = sqlx::query_as(
        "SELECT p.id, p.sciezka_wzgledna, p.podglad_sciezka FROM pliki p \
         WHERE NOT EXISTS (SELECT 1 FROM zdjecia_specyfikacji z WHERE z.plik_id = p.id) \
           AND NOT EXISTS (SELECT 1 FROM zalaczniki_korespondencji z WHERE z.plik_id = p.id)",
    )
    .fetch_all(&mut *tx)
    .await?;
    for (id, _, _) in &sieroty {
        sqlx::query("DELETE FROM pliki WHERE id = ?1")
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    // Pliki na dysku dopiero po zatwierdzeniu (wycofanie nie zostawi rekordu bez pliku).
    for (_, sciezka, podglad) in &sieroty {
        for s in std::iter::once(sciezka).chain(podglad.iter()) {
            if let Ok(p) = bezpieczna_sciezka(folder, s) {
                if let Err(e) = std::fs::remove_file(&p) {
                    if e.kind() != std::io::ErrorKind::NotFound {
                        tracing::warn!("Nie udało się usunąć pliku {}: {e}", p.display());
                    }
                }
            }
        }
    }
    Ok(sieroty.len())
}

fn blad_pliku(sciezka: &Path, e: std::io::Error) -> BladApi {
    tracing::error!("Błąd zapisu pliku {}: {e}", sciezka.display());
    BladApi::Wewnetrzny("nie udało się zapisać pliku na dysku serwera".into())
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
    let podpis = d
        .podpis
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    crate::limity::tekst("podpis", podpis.as_deref(), crate::limity::MAKS_NAZWA)?;
    let mut tx = crate::baza::transakcja_zapisu(pula).await?;
    specyfikacje::sprawdz_zapis(&mut tx, spec).await?;
    sqlx::query("UPDATE zdjecia_specyfikacji SET podpis = ?1 WHERE id = ?2")
        .bind(podpis)
        .bind(id)
        .execute(&mut *tx)
        .await?;
    let w = specyfikacje::pobierz_w_wynik(&mut tx, id, spec).await?;
    tx.commit().await?;
    Ok(w)
}

/// Odpina zdjęcie. Plik zostaje, jeśli używa go inna wersja oferty – nieużywane pliki
/// usuwa potem [`sprzataj_nieuzywane`].
pub async fn usun_zdjecie(pula: &SqlitePool, id: i64) -> Wynik<WynikZmiany> {
    let spec = spec_zdjecia(pula, id).await?;
    let mut tx = crate::baza::transakcja_zapisu(pula).await?;
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

/// Odczyt pliku: (typ MIME, nazwa, bajty). `podglad` = pomniejszona kopia JPEG (jeśli jest).
pub async fn odczytaj(
    pula: &SqlitePool,
    folder: &Path,
    id: i64,
    podglad: bool,
) -> Wynik<(String, String, Vec<u8>)> {
    let r: Option<(String, String, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT sciezka_wzgledna, nazwa, typ_mime, podglad_sciezka FROM pliki WHERE id = ?1",
    )
    .bind(id)
    .fetch_optional(pula)
    .await?;
    let (wzgledna, nazwa, mime, sciezka_podgladu) =
        r.ok_or_else(|| BladApi::NieZnaleziono(format!("plik {id}")))?;
    let (wzgledna, mime) = match (podglad, sciezka_podgladu) {
        (true, Some(p)) => (p, Some("image/jpeg".to_string())),
        _ => (wzgledna, mime),
    };
    let pelna = bezpieczna_sciezka(folder, &wzgledna)?;
    let bajty = tokio::fs::read(&pelna).await.map_err(|e| {
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

#[cfg(test)]
mod testy_obrazow {
    use super::*;

    fn png(w: u32, h: u32) -> Vec<u8> {
        let img = image023::RgbImage::from_pixel(w, h, image023::Rgb([200, 100, 50]));
        let mut b = Vec::new();
        image023::png::PngEncoder::new(&mut b)
            .encode(img.as_raw(), w, h, image023::ColorType::Rgb8)
            .unwrap();
        b
    }

    fn crc32(dane: &[u8]) -> u32 {
        let mut c = 0xFFFF_FFFFu32;
        for b in dane {
            c ^= u32::from(*b);
            for _ in 0..8 {
                c = if c & 1 != 0 {
                    0xEDB8_8320 ^ (c >> 1)
                } else {
                    c >> 1
                };
            }
        }
        !c
    }

    /// PNG z nagłówkiem deklarującym wymiary w×h (dane obrazu 1×1) – jak „bomba” 15000×15000.
    pub(crate) fn png_z_wymiarami(w: u32, h: u32) -> Vec<u8> {
        let mut b = png(1, 1);
        // IHDR: długość(4) „IHDR”(4) od bajtu 8; szerokość i wysokość od bajtu 16
        b[16..20].copy_from_slice(&w.to_be_bytes());
        b[20..24].copy_from_slice(&h.to_be_bytes());
        let crc = crc32(&b[12..29]);
        b[29..33].copy_from_slice(&crc.to_be_bytes());
        b
    }

    #[test]
    fn limity_wymiarow_bez_dekodowania() {
        // nagłówek PNG 15000×15000 – odrzucony po nagłówku, bez dekodowania
        let duzy = png_z_wymiarami(15_000, 15_000);
        assert_eq!(sprawdz_wymiary(&png_z_wymiarami(20, 30)).unwrap(), (20, 30));
        let start = std::time::Instant::now();
        let e = przygotuj_obraz(&duzy).err().unwrap();
        assert!(start.elapsed().as_secs_f64() < 1.0);
        assert!(e.to_string().contains("za duży"));
        // poprawny obraz → podgląd JPEG ≤ 1600 px
        let o = przygotuj_obraz(&png(3200, 1000)).unwrap();
        let pod = image023::load_from_memory(&o.podglad_jpg).unwrap();
        use image023::GenericImageView;
        assert_eq!(pod.width(), 1600);
        assert_eq!((o.szerokosc, o.wysokosc), (3200, 1000));
    }

    #[test]
    fn uszkodzony_obraz_z_poprawnym_naglowkiem() {
        let mut b = png(400, 300);
        let n = b.len();
        // zostaw nagłówek IHDR, zniszcz dane obrazu
        for x in b.iter_mut().take(n - 12).skip(60) {
            *x = 0x55;
        }
        let e = przygotuj_obraz(&b)
            .err()
            .expect("uszkodzony obraz ma być odrzucony");
        assert!(e.to_string().contains("uszkodzony"), "{e}");
    }
}
