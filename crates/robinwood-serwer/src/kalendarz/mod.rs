//! Silnik terminów kalendarza – czysta, deterministyczna logika bez bazy i HTTP.
//!
//! * [`dni`] – kalendarz dni roboczych (polskie święta, weekendy, dni wolne użytkownika)
//!   i konwencja liczenia „N dni przed/po”,
//! * [`harmonogram`] – [`policz_harmonogram`]: montaż → warsztat → dostawa materiałów →
//!   terminy zamówień (sweet spot / planowa / krytyczna) → rysunki → pomiar (+ zadania pomocnicze),
//! * [`stany`] – kolory ostrzeżeń względem podanej daty „dziś”,
//! * [`kolizje`] – nakładanie się warsztatu/montażu różnych zleceń,
//! * [`statystyka`] – rzeczywiste czasy dostaw z historii,
//! * [`scalanie`] – zamrażanie wykonanych zadań przy przeliczeniu.
//!
//! Warstwa bazy i API: `crate::terminarz`, `crate::api_kalendarz`.

pub mod dni;
pub mod harmonogram;
pub mod kolizje;
pub mod scalanie;
pub mod stany;
pub mod statystyka;

pub use dni::{obslugiwany_rok, swieta, KalendarzDni, LiczenieDni, Swieto, ROK_MAX, ROK_MIN};
pub use harmonogram::{
    policz_harmonogram, policz_z_kalendarzem, BladHarmonogramu, Harmonogram, MaterialWejscie,
    Ostrzezenie, PlatnoscWejscie, TerminyMaterialu, TypZadania, UstawieniaTerminow, Widelki,
    ZadanieWyliczone, ZlecenieWejscie, ZrodloWidelek,
};
pub use kolizje::{wykryj_kolizje, Kolizja, Okres};
pub use scalanie::{scal, PlanZapisu, ZadanieIstniejace};
pub use stany::{stan_terminu, stan_wykonane, stan_zamowienia, DaneZamowienia, Stan, StanZadania};
pub use statystyka::{statystyka_dostaw, StatystykaDostaw};

/// Data do tekstów dla użytkownika: „23.11.2026” (API nadal zwraca daty w polach jako ISO).
pub fn data_pl(d: chrono::NaiveDate) -> String {
    d.format("%d.%m.%Y").to_string()
}

/// „1 dzień”, „2 dni”, „5 dni” (w języku polskim tylko 1 ma inną formę).
pub fn dni_pl(n: i64) -> String {
    if n.abs() == 1 {
        format!("{n} dzień")
    } else {
        format!("{n} dni")
    }
}

#[cfg(test)]
mod testy_tekstow {
    use super::*;

    #[test]
    fn daty_i_dni_po_polsku() {
        let d = chrono::NaiveDate::from_ymd_opt(2026, 9, 2).unwrap();
        assert_eq!(data_pl(d), "02.09.2026");
        assert_eq!(dni_pl(1), "1 dzień");
        assert_eq!(dni_pl(2), "2 dni");
        assert_eq!(dni_pl(0), "0 dni");
    }
}
