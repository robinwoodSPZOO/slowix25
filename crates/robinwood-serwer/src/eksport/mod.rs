//! Eksport wersji oferty do DOCX i PDF.
//!
//! Najpierw budujemy niezależny od formatu model [`DokumentOferty`] (teksty już sformatowane),
//! potem renderujemy go w `docx.rs` i `pdf.rs`. Dane inwestora i wykonawcy: z migawki wersji
//! (zapisanej przy wysłaniu oferty), a dla szkicu – bieżące z bazy klientów i ustawień.
//! Koszt własny i marża NIGDY nie trafiają do dokumentu. Wycena z błędami (`Wycena::bledy`,
//! np. rabaty większe niż suma) blokuje eksport (422).

mod docx;
mod pdf;

use std::path::Path;

use sqlx::SqlitePool;

use crate::bledy::{BladApi, Wynik};
use crate::formaty;
use crate::oferty::{self, DaneInwestora, DaneWykonawcy, PelnaWersja};
use crate::pliki;
use crate::specyfikacje::{Wiersz, Zdjecie};
use crate::wycena::TrybCen;

pub use docx::renderuj as renderuj_docx;
pub use pdf::renderuj as renderuj_pdf;

#[derive(Debug, Clone, Default)]
pub struct WierszDok {
    pub rodzaj: String,
    pub material: String,
    pub opis: String,
    pub ilosc: String,
    /// Tylko AGD: wartość pozycji.
    pub cena: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct CzescDok {
    pub nazwa: String,
    pub cena: Option<String>,
    pub wiersze: Vec<WierszDok>,
    pub wyposazenie: Vec<WierszDok>,
    pub frezowanie: Vec<WierszDok>,
    pub zdjecia: Vec<ObrazDok>,
}

#[derive(Debug, Clone, Default)]
pub struct SekcjaDok {
    pub tytul: String,
    pub agd: bool,
    pub czesci: Vec<CzescDok>,
    /// Np. „Suma za meble: 12 000,00 zł”.
    pub suma: Option<String>,
    pub zdjecia: Vec<ObrazDok>,
}

/// Obraz przygotowany do obu formatów (zmniejszony, RGB).
#[derive(Clone)]
pub struct ObrazDok {
    pub obraz: image023::RgbImage,
    pub png: Vec<u8>,
    pub podpis: Option<String>,
}

impl std::fmt::Debug for ObrazDok {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ObrazDok({}x{})",
            self.obraz.width(),
            self.obraz.height()
        )
    }
}

#[derive(Debug, Clone, Default)]
pub struct DokumentOferty {
    pub numer: String,
    pub inwestor: Vec<String>,
    pub wykonawca: Vec<String>,
    pub data: String,
    pub termin_realizacji: Option<String>,
    pub inwestycja: Option<String>,
    pub zdanie_waznosci: Option<String>,
    pub sekcje: Vec<SekcjaDok>,
    /// (etykieta, kwota, wyróżnienie)
    pub podsumowanie: Vec<(String, String, bool)>,
    pub stawka_vat: String,
    pub klauzule: Vec<String>,
    pub zdjecia: Vec<ObrazDok>,
}

/// Największy bok obrazu w dokumencie (piksele).
const MAKS_BOK: u32 = 1200;

fn przygotuj_obraz(bajty: &[u8], podpis: Option<String>) -> Option<ObrazDok> {
    use image023::GenericImageView;
    // Pliki sprzed podglądów: sprawdzamy wymiary przed dekodowaniem.
    pliki::sprawdz_wymiary(bajty).ok()?;
    let img = image023::load_from_memory(bajty).ok()?;
    let img = if img.width() > MAKS_BOK || img.height() > MAKS_BOK {
        img.thumbnail(MAKS_BOK, MAKS_BOK)
    } else {
        img
    };
    let rgb = img.to_rgb8();
    let mut png = Vec::new();
    image023::png::PngEncoder::new(&mut png)
        .encode(
            rgb.as_raw(),
            rgb.width(),
            rgb.height(),
            image023::ColorType::Rgb8,
        )
        .ok()?;
    Some(ObrazDok {
        obraz: rgb,
        png,
        podpis,
    })
}

fn wiersz_dok(w: &Wiersz, agd: bool) -> WierszDok {
    let status = match w.status.as_str() {
        "do_potwierdzenia" => Some("do potwierdzenia"),
        "po_stronie_klienta" => Some("po stronie klienta"),
        "poza_wycena" => Some("poza wyceną"),
        _ => None,
    };
    let mut opis = w.opis.clone().unwrap_or_default();
    if let Some(s) = status {
        if !opis.to_lowercase().contains(s) {
            opis = if opis.is_empty() {
                format!("({s})")
            } else {
                format!("{opis} ({s})")
            };
        }
    }
    let ilosc = w
        .ilosc
        .map(|i| {
            format!(
                "{} {}",
                formaty::ilosc(i),
                w.jednostka.clone().unwrap_or_default()
            )
            .trim()
            .to_string()
        })
        .unwrap_or_default();
    let cena = if agd {
        w.cena_gr.map(|c| {
            if w.status == "po_stronie_klienta" || w.status == "poza_wycena" {
                "—".to_string()
            } else {
                formaty::kwota(crate::wycena::razy_ilosc(c, w.ilosc.unwrap_or(1.0)))
            }
        })
    } else {
        None
    };
    WierszDok {
        rodzaj: w.rodzaj.clone().unwrap_or_default(),
        material: w.material.clone().unwrap_or_default(),
        opis,
        ilosc,
        cena,
    }
}

/// Buduje model dokumentu z pełnej wersji oferty.
pub async fn zbuduj(
    pula: &SqlitePool,
    folder: &Path,
    wersja_id: i64,
    ze_zdjeciami: bool,
) -> Wynik<DokumentOferty> {
    let p: PelnaWersja = oferty::pelna_wersja(pula, wersja_id).await?;
    if !p.wycena.bledy.is_empty() {
        return Err(BladApi::NieprawidloweDane(format!(
            "nie można utworzyć dokumentu dla klienta: {}",
            p.wycena.bledy.join("; ")
        )));
    }
    let strony = match &p.wersja.migawka {
        Some(m) => m.clone(),
        None => {
            let mut db = pula.acquire().await?;
            oferty::migawka_w(&mut db, p.oferta.id).await?
        }
    };

    let mut zdjecia_bajty: Vec<(Zdjecie, Vec<u8>)> = Vec::new();
    if ze_zdjeciami {
        for z in &p.specyfikacja.zdjecia {
            match pliki::odczytaj(pula, folder, z.plik_id, true).await {
                Ok((_, _, b)) => zdjecia_bajty.push((z.clone(), b)),
                Err(e) => tracing::warn!("Pomijam zdjęcie {} w eksporcie: {e}", z.id),
            }
        }
    }
    // Dekodowanie i pomniejszanie obrazów poza wątkami async.
    let przygotowane: Vec<(Zdjecie, ObrazDok)> = tokio::task::spawn_blocking(move || {
        zdjecia_bajty
            .into_iter()
            .filter_map(|(z, b)| {
                let o = przygotuj_obraz(&b, z.podpis.clone());
                if o.is_none() {
                    tracing::warn!("Pomijam zdjęcie {} w eksporcie: nie da się odczytać", z.id);
                }
                o.map(|o| (z, o))
            })
            .collect()
    })
    .await
    .map_err(blad_renderowania)?;
    let obrazy = |f: &dyn Fn(&Zdjecie) -> bool| -> Vec<ObrazDok> {
        przygotowane
            .iter()
            .filter(|(z, _)| f(z))
            .map(|(_, o)| o.clone())
            .collect()
    };

    let w = &p.wersja;
    let inwestor = linie_inwestora(&strony.inwestor);
    let wykonawca = linie_wykonawcy(&strony.wykonawca);

    let mut sekcje = Vec::new();
    for s in &p.specyfikacja.sekcje {
        let agd = s.typ == "agd";
        let wyc = p.wycena.sekcje.iter().find(|x| x.sekcja_id == s.id);
        let czesci = s
            .czesci
            .iter()
            .map(|c| {
                let cena = wyc
                    .and_then(|x| x.czesci.iter().find(|y| y.czesc_id == c.id))
                    .and_then(|y| y.cena_gr)
                    .map(formaty::kwota);
                let wg = |g: &str| {
                    c.wiersze
                        .iter()
                        .filter(|r| r.podgrupa == g)
                        .map(|r| wiersz_dok(r, agd))
                        .collect::<Vec<_>>()
                };
                let cid = c.id;
                CzescDok {
                    nazwa: c.nazwa.clone(),
                    cena,
                    wiersze: wg("glowna"),
                    wyposazenie: wg("wyposazenie"),
                    frezowanie: wg("frezowanie"),
                    zdjecia: obrazy(&|z: &Zdjecie| z.czesc_id == Some(cid)),
                }
            })
            .collect();
        let sid = s.id;
        let suma = wyc.and_then(|x| x.suma_gr).map(|k| {
            if agd {
                format!("Suma za sprzęt AGD: {}", formaty::kwota(k))
            } else {
                format!("Suma za meble: {}", formaty::kwota(k))
            }
        });
        sekcje.push(SekcjaDok {
            tytul: match &s.numer {
                Some(n) => format!("{n} {}", s.nazwa),
                None => s.nazwa.clone(),
            },
            agd,
            czesci,
            suma,
            zdjecia: obrazy(&|z: &Zdjecie| z.sekcja_id == Some(sid) && z.czesc_id.is_none()),
        });
    }

    let wy = &p.wycena;
    let mut pod = Vec::new();
    let tryb_netto = wy.tryb_cen == TrybCen::Netto;
    let dopisek = if tryb_netto { " (netto)" } else { "" };
    pod.push((
        format!("Suma za meble{dopisek}"),
        formaty::kwota(wy.suma_meble_gr),
        false,
    ));
    if wy.rabat_termin_gr > 0 {
        pod.push((
            "Rabat za termin".into(),
            formaty::kwota(-wy.rabat_termin_gr),
            false,
        ));
    }
    if wy.rabat_calosc_gr > 0 {
        pod.push((
            format!(
                "Rabat za całość ({})",
                formaty::procent_bp(wy.rabat_calosc_bp)
            ),
            formaty::kwota(-wy.rabat_calosc_gr),
            false,
        ));
    }
    if wy.rabat_termin_gr > 0 || wy.rabat_calosc_gr > 0 {
        pod.push((
            format!("Meble po rabatach{dopisek}"),
            formaty::kwota(wy.meble_po_rabatach_gr),
            false,
        ));
    }
    if wy.suma_agd_gr > 0 || p.specyfikacja.sekcje.iter().any(|s| s.typ == "agd") {
        pod.push((
            format!("Sprzęt AGD{dopisek}"),
            formaty::kwota(wy.suma_agd_gr),
            false,
        ));
    }
    if tryb_netto {
        pod.push(("Suma netto".into(), formaty::kwota(wy.suma_netto_gr), false));
        pod.push(("VAT".into(), formaty::kwota(wy.suma_vat_gr), false));
    }
    pod.push((
        "SUMA brutto".into(),
        formaty::kwota(wy.suma_brutto_gr),
        true,
    ));

    let mut stawki: Vec<i64> = vec![wy.meble.stawka];
    for a in &wy.agd {
        if !stawki.contains(&a.stawka) {
            stawki.push(a.stawka);
        }
    }
    let stawka_vat = if stawki.len() == 1 {
        format!("Stawka VAT: {}%", stawki[0])
    } else {
        format!(
            "Stawka VAT: meble {}%, sprzęt AGD {}",
            wy.meble.stawka,
            wy.agd
                .iter()
                .map(|a| format!("{}%", a.stawka))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };

    Ok(DokumentOferty {
        numer: format!("{} (wersja {})", p.oferta.numer, w.nr_wersji),
        inwestor,
        wykonawca,
        data: w
            .data_oferty
            .as_deref()
            .map(formaty::data)
            .unwrap_or_default(),
        termin_realizacji: w.termin_realizacji.clone(),
        inwestycja: w.adres_inwestycji.clone(),
        zdanie_waznosci: w
            .data_waznosci_cen
            .as_deref()
            .map(|d| format!("Ceny zawarte w ofercie są ważne do {}.", formaty::data(d))),
        sekcje,
        podsumowanie: pod,
        stawka_vat,
        klauzule: w.klauzule.clone(),
        zdjecia: obrazy(&|z: &Zdjecie| z.sekcja_id.is_none() && z.czesc_id.is_none()),
    })
}

/// Linie bloku „Inwestor” w dokumencie.
pub fn linie_inwestora(k: &DaneInwestora) -> Vec<String> {
    let mut inwestor = vec![k.nazwa.clone()];
    if let Some(u) = k.adres_ulica.as_ref().filter(|u| !u.is_empty()) {
        inwestor.push(u.clone());
    }
    let miasto = [k.adres_kod.clone(), k.adres_miasto.clone()]
        .into_iter()
        .flatten()
        .filter(|x| !x.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    if !miasto.is_empty() {
        inwestor.push(miasto);
    }
    inwestor
}

/// Linie bloku „Wykonawca” w dokumencie.
pub fn linie_wykonawcy(f: &DaneWykonawcy) -> Vec<String> {
    let mut wykonawca: Vec<String> = Vec::new();
    if f.nazwa.is_empty() {
        wykonawca.push("(uzupełnij dane firmy w Ustawieniach)".into());
    } else {
        wykonawca.push(f.nazwa.clone());
    }
    wykonawca.extend(
        f.adres
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(String::from),
    );
    if !f.nip.is_empty() {
        wykonawca.push(format!("NIP: {}", f.nip));
    }
    let kontakt = [f.telefon.as_str(), f.email.as_str()]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(", ");
    if !kontakt.is_empty() {
        wykonawca.push(kontakt);
    }
    wykonawca
}

/// Nazwa pliku eksportu, np. `Oferta_OF-2026-001_v2.pdf` (bez polskich znaków).
pub fn nazwa_pliku(numer_oferty: &str, nr_wersji: i64, rozszerzenie: &str) -> String {
    let n: String = numer_oferty
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    format!("Oferta_{n}_v{nr_wersji}.{rozszerzenie}")
}

pub fn blad_renderowania(e: impl std::fmt::Display) -> BladApi {
    BladApi::Wewnetrzny(format!("nie udało się utworzyć dokumentu: {e}"))
}
