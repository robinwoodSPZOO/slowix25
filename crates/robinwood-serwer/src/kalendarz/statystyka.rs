//! Statystyka rzeczywistych czasów dostaw (z historii zamówień) – sugestia korekty widełek.

use chrono::NaiveDate;
use serde::Serialize;

use super::dni::KalendarzDni;
use super::harmonogram::Widelki;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StatystykaDostaw {
    /// Liczba zamówień z datą zamówienia i dostawy.
    pub liczba: usize,
    pub min: u32,
    pub max: u32,
    /// Średnia arytmetyczna (do pokazania), zaokrąglona do 0,1 dnia.
    pub srednia: f64,
    /// Proponowane widełki = rzeczywiste min–max. Tylko sugestia – nie nadpisujemy automatycznie.
    pub sugerowane_widelki: Widelki,
}

/// Czas dostawy jednego zamówienia: liczba dni w `[data_zamowienia, data_dostawy)` w trybie
/// kalendarza (ta sama konwencja co przy liczeniu terminów). Pary z dostawą przed
/// zamówieniem są pomijane (błąd danych).
pub fn statystyka_dostaw(
    pary: &[(NaiveDate, NaiveDate)],
    kal: &KalendarzDni,
) -> Option<StatystykaDostaw> {
    let czasy: Vec<u32> = pary
        .iter()
        .filter(|(z, d)| d >= z)
        .map(|(z, d)| u32::try_from(kal.roznica(*z, *d)).unwrap_or(0))
        .collect();
    let min = *czasy.iter().min()?;
    let max = *czasy.iter().max()?;
    let suma: u64 = czasy.iter().map(|&c| u64::from(c)).sum();
    let srednia = (suma as f64 / czasy.len() as f64 * 10.0).round() / 10.0;
    Some(StatystykaDostaw {
        liczba: czasy.len(),
        min,
        max,
        srednia,
        sugerowane_widelki: Widelki { min, max },
    })
}

#[cfg(test)]
mod testy {
    use super::*;
    use crate::kalendarz::dni::LiczenieDni;

    fn d(m: u32, dz: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2027, m, dz).unwrap()
    }

    #[test]
    fn statystyka_w_obu_trybach() {
        // pon. 8.03 → pon. 15.03 (5 roboczych / 7 kalendarzowych),
        // pon. 22.03 → pt. 2.04 (Wielkanoc po drodze nie przeszkadza: 22–26, 30–1 = 8 roboczych / 11 kal.)
        // dostawa przed zamówieniem – pomijana
        let pary = [
            (d(3, 8), d(3, 15)),
            (d(3, 22), d(4, 2)),
            (d(3, 10), d(3, 1)),
        ];
        let rob = statystyka_dostaw(&pary, &KalendarzDni::nowy(LiczenieDni::Robocze, [])).unwrap();
        assert_eq!((rob.liczba, rob.min, rob.max), (2, 5, 8));
        assert_eq!(rob.srednia, 6.5);
        assert_eq!(rob.sugerowane_widelki, Widelki { min: 5, max: 8 });
        let kal =
            statystyka_dostaw(&pary, &KalendarzDni::nowy(LiczenieDni::Kalendarzowe, [])).unwrap();
        assert_eq!((kal.min, kal.max), (7, 11));
        assert_eq!(kal.srednia, 9.0);
    }

    #[test]
    fn brak_historii() {
        assert!(statystyka_dostaw(&[], &KalendarzDni::nowy(LiczenieDni::Robocze, [])).is_none());
    }
}
