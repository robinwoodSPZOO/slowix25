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

pub use dni::{swieta, KalendarzDni, LiczenieDni, Swieto};
pub use harmonogram::{
    policz_harmonogram, policz_z_kalendarzem, BladHarmonogramu, Harmonogram, MaterialWejscie,
    Ostrzezenie, PlatnoscWejscie, TerminyMaterialu, TypZadania, UstawieniaTerminow, Widelki,
    ZadanieWyliczone, ZlecenieWejscie, ZrodloWidelek,
};
pub use kolizje::{wykryj_kolizje, Kolizja, Okres};
pub use scalanie::{scal, PlanZapisu, ZadanieIstniejace};
pub use stany::{stan_terminu, stan_wykonane, stan_zamowienia, DaneZamowienia, Stan, StanZadania};
pub use statystyka::{statystyka_dostaw, StatystykaDostaw};
