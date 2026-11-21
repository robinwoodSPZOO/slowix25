//! Wykrywanie nakładania się okresów warsztatu lub montażu różnych zleceń.

use chrono::NaiveDate;
use serde::Serialize;

use super::harmonogram::TypZadania;

/// Okres zajętości (daty włącznie).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Okres {
    pub zlecenie_id: i64,
    pub typ: TypZadania,
    pub od: NaiveDate,
    pub do_: NaiveDate,
}

/// Dwa zlecenia zajmują warsztat (albo ekipę montażową) w tych samych dniach.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Kolizja {
    pub typ: TypZadania,
    pub zlecenie_a: i64,
    pub zlecenie_b: i64,
    /// Część wspólna okresów (włącznie).
    pub od: NaiveDate,
    #[serde(rename = "do")]
    pub do_: NaiveDate,
}

/// Zwraca pary zleceń (a < b), których okresy tego samego typu (warsztat/montaż) się nakładają.
/// Inne typy są pomijane. Wynik posortowany po dacie początku kolizji.
pub fn wykryj_kolizje(okresy: &[Okres]) -> Vec<Kolizja> {
    let mut wynik = Vec::new();
    for (i, a) in okresy.iter().enumerate() {
        if !matches!(a.typ, TypZadania::Warsztat | TypZadania::Montaz) {
            continue;
        }
        for b in &okresy[i + 1..] {
            if b.typ != a.typ || b.zlecenie_id == a.zlecenie_id {
                continue;
            }
            let od = a.od.max(b.od);
            let do_ = a.do_.min(b.do_);
            if od <= do_ {
                let (x, y) = if a.zlecenie_id < b.zlecenie_id {
                    (a.zlecenie_id, b.zlecenie_id)
                } else {
                    (b.zlecenie_id, a.zlecenie_id)
                };
                wynik.push(Kolizja {
                    typ: a.typ,
                    zlecenie_a: x,
                    zlecenie_b: y,
                    od,
                    do_,
                });
            }
        }
    }
    wynik.sort_by(|p, q| {
        (p.od, p.zlecenie_a, p.zlecenie_b, p.typ.jako_tekst()).cmp(&(
            q.od,
            q.zlecenie_a,
            q.zlecenie_b,
            q.typ.jako_tekst(),
        ))
    });
    wynik
}

#[cfg(test)]
mod testy {
    use super::*;

    fn d(m: u32, dz: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2027, m, dz).unwrap()
    }

    fn o(z: i64, typ: TypZadania, od: NaiveDate, do_: NaiveDate) -> Okres {
        Okres {
            zlecenie_id: z,
            typ,
            od,
            do_,
        }
    }

    #[test]
    fn wykrywa_nakladanie_warsztatu_i_montazu() {
        let okresy = vec![
            o(1, TypZadania::Warsztat, d(1, 25), d(2, 5)),
            o(2, TypZadania::Warsztat, d(2, 1), d(2, 12)),
            o(3, TypZadania::Warsztat, d(2, 6), d(2, 6)), // styka się tylko z 2
            o(1, TypZadania::Montaz, d(2, 8), d(2, 10)),
            o(2, TypZadania::Montaz, d(2, 10), d(2, 11)), // wspólny 1 dzień
            o(3, TypZadania::Montaz, d(2, 11), d(2, 12)),
            o(4, TypZadania::Rysunki, d(2, 1), d(2, 28)), // ignorowane
            o(1, TypZadania::Zamowienie, d(2, 1), d(2, 28)),
        ];
        let k = wykryj_kolizje(&okresy);
        assert_eq!(
            k,
            vec![
                Kolizja {
                    typ: TypZadania::Warsztat,
                    zlecenie_a: 1,
                    zlecenie_b: 2,
                    od: d(2, 1),
                    do_: d(2, 5)
                },
                Kolizja {
                    typ: TypZadania::Warsztat,
                    zlecenie_a: 2,
                    zlecenie_b: 3,
                    od: d(2, 6),
                    do_: d(2, 6)
                },
                Kolizja {
                    typ: TypZadania::Montaz,
                    zlecenie_a: 1,
                    zlecenie_b: 2,
                    od: d(2, 10),
                    do_: d(2, 10)
                },
                Kolizja {
                    typ: TypZadania::Montaz,
                    zlecenie_a: 2,
                    zlecenie_b: 3,
                    od: d(2, 11),
                    do_: d(2, 11)
                },
            ]
        );
    }

    #[test]
    fn brak_kolizji_gdy_okresy_rozlaczne_lub_to_samo_zlecenie() {
        let okresy = vec![
            o(1, TypZadania::Montaz, d(2, 8), d(2, 10)),
            o(2, TypZadania::Montaz, d(2, 11), d(2, 12)),
            o(2, TypZadania::Montaz, d(2, 11), d(2, 12)),
            o(1, TypZadania::Warsztat, d(2, 8), d(2, 10)),
        ];
        assert!(wykryj_kolizje(&okresy).is_empty());
    }
}
