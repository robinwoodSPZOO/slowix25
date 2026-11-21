//! Limity danych wejściowych (kwoty, ilości, długości tekstów) – wspólne dla ofert, umów
//! i słowników. Te same limity kwot stosuje interfejs (`src/lib/formaty.ts`, `MAKS_KWOTA_GR`).
//!
//! Przy tych limitach wycena mieści się w `i64` z ogromnym zapasem (np. 10 000 pozycji AGD po
//! 1 mld zł × 1 mln szt. to ~10^21 gr – dlatego wycena i tak liczy z kontrolą przepełnienia).

use crate::bledy::{BladApi, Wynik};

/// Największa kwota (cena, rabat kwotowy, koszt własny): 1 000 000 000,00 zł.
pub const MAKS_KWOTA_GR: i64 = 100_000_000_000;
/// Największa ilość w wierszu specyfikacji.
pub const MAKS_ILOSC: f64 = 1_000_000.0;

/// Krótkie teksty (nazwy, rodzaj, jednostka, numer).
pub const MAKS_NAZWA: usize = 200;
/// Teksty w komórkach (materiał, opis).
pub const MAKS_TEKST: usize = 1_000;
/// Długie teksty (uwagi, adres, klauzula).
pub const MAKS_DLUGI: usize = 5_000;
/// Liczba klauzul w wersji oferty.
pub const MAKS_KLAUZUL: usize = 50;

/// Kwota w groszach: nieujemna i nie większa niż [`MAKS_KWOTA_GR`].
pub fn kwota(nazwa: &str, k: Option<i64>) -> Wynik<()> {
    match k {
        Some(x) if x < 0 => Err(BladApi::NieprawidloweDane(format!(
            "{nazwa}: kwota nie może być ujemna"
        ))),
        Some(x) if x > MAKS_KWOTA_GR => Err(BladApi::NieprawidloweDane(format!(
            "{nazwa}: najwyżej 1 000 000 000,00 zł"
        ))),
        _ => Ok(()),
    }
}

/// Ilość: skończona, nieujemna, najwyżej [`MAKS_ILOSC`].
pub fn ilosc(i: Option<f64>) -> Wynik<()> {
    match i {
        Some(x) if !x.is_finite() || x < 0.0 => Err(BladApi::NieprawidloweDane(
            "ilość musi być liczbą nieujemną".into(),
        )),
        Some(x) if x > MAKS_ILOSC => Err(BladApi::NieprawidloweDane(
            "ilość: najwyżej 1 000 000".into(),
        )),
        _ => Ok(()),
    }
}

/// Długość tekstu w znakach.
pub fn tekst(nazwa: &str, t: Option<&str>, maks: usize) -> Wynik<()> {
    match t {
        Some(s) if s.chars().count() > maks => Err(BladApi::NieprawidloweDane(format!(
            "{nazwa}: najwyżej {maks} znaków"
        ))),
        _ => Ok(()),
    }
}
