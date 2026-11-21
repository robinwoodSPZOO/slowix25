//! Kalendarz dni roboczych: polskie święta ustawowo wolne od pracy, weekendy i dni wolne
//! zdefiniowane przez użytkownika, oraz arytmetyka „N dni wstecz / naprzód”.
//!
//! # Konwencja liczenia (obowiązuje w całym module `kalendarz`)
//!
//! Okres o długości N dni zaczynający się w dniu X **obejmuje dzień X i nie obejmuje dnia
//! końcowego D** – jest to przedział półotwarty `[X, D)`:
//!
//! * zamówienie złożone w dniu X przy czasie dostawy N dni dociera w dniu D, gdzie w `[X, D)`
//!   jest dokładnie N dni (roboczych albo kalendarzowych – zależnie od trybu);
//!   dzień złożenia zamówienia jest pierwszym dniem czasu dostawy, dzień dostawy się nie liczy,
//! * odstęp „materiały N dni przed składaniem” = materiały w dniu X, składanie od dnia D,
//!   `[X, D)` ma N dni,
//! * składanie na warsztacie trwające N dni zaczyna się w dniu X i kończy dzień przed montażem D,
//! * montaż trwający N dni zaczyna się w dniu S (z umowy); jego ostatni dzień to N-ty dzień
//!   liczony od S włącznie.
//!
//! Tryb **robocze**: liczą się tylko dni robocze (pon.–pt. bez świąt i dni wolnych użytkownika);
//! wynik liczenia wstecz jest zawsze dniem roboczym.
//!
//! Tryb **kalendarzowe**: liczą się wszystkie dni; wynik „najpóźniej do” wypadający w dzień
//! wolny (weekend, święto, dzień wolny użytkownika) cofa się na najbliższy wcześniejszy
//! dzień roboczy.

use std::collections::BTreeSet;

use chrono::{Datelike, Duration, NaiveDate, Weekday};
use serde::{Deserialize, Serialize};

/// Sposób liczenia czasów dostaw, czasu warsztatu, montażu i odstępów.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum LiczenieDni {
    #[default]
    Robocze,
    Kalendarzowe,
}

impl LiczenieDni {
    pub fn z_tekstu(t: &str) -> Option<Self> {
        match t.trim() {
            "robocze" => Some(Self::Robocze),
            "kalendarzowe" => Some(Self::Kalendarzowe),
            _ => None,
        }
    }

    pub fn jako_tekst(self) -> &'static str {
        match self {
            Self::Robocze => "robocze",
            Self::Kalendarzowe => "kalendarzowe",
        }
    }
}

/// Święto ustawowo wolne od pracy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Swieto {
    pub data: NaiveDate,
    pub nazwa: &'static str,
}

/// Niedziela Wielkanocna (algorytm Meeusa/Jonesa/Butchera, kalendarz gregoriański).
pub fn wielkanoc(rok: i32) -> NaiveDate {
    let a = rok % 19;
    let b = rok / 100;
    let c = rok % 100;
    let d = b / 4;
    let e = b % 4;
    let f = (b + 8) / 25;
    let g = (b - f + 1) / 3;
    let h = (19 * a + b - d - g + 15) % 30;
    let i = c / 4;
    let k = c % 4;
    let l = (32 + 2 * e + 2 * i - h - k) % 7;
    let m = (a + 11 * h + 22 * l) / 451;
    let miesiac = (h + l - 7 * m + 114) / 31;
    let dzien = (h + l - 7 * m + 114) % 31 + 1;
    NaiveDate::from_ymd_opt(rok, miesiac as u32, dzien as u32).expect("poprawna data Wielkanocy")
}

/// Polskie święta ustawowo wolne od pracy w danym roku (ustawa z 18.01.1951 r. z późn. zm.),
/// posortowane po dacie. Trzech Króli od 2011 r., Wigilia od 2025 r.
pub fn swieta(rok: i32) -> Vec<Swieto> {
    let d = |m: u32, dz: u32| NaiveDate::from_ymd_opt(rok, m, dz).expect("poprawna data");
    let wn = wielkanoc(rok);
    let mut v = vec![
        Swieto {
            data: d(1, 1),
            nazwa: "Nowy Rok",
        },
        Swieto {
            data: wn,
            nazwa: "Wielkanoc",
        },
        Swieto {
            data: wn + Duration::days(1),
            nazwa: "Poniedziałek Wielkanocny",
        },
        Swieto {
            data: d(5, 1),
            nazwa: "Święto Pracy",
        },
        Swieto {
            data: d(5, 3),
            nazwa: "Święto Konstytucji 3 Maja",
        },
        Swieto {
            data: wn + Duration::days(49),
            nazwa: "Zielone Świątki",
        },
        Swieto {
            data: wn + Duration::days(60),
            nazwa: "Boże Ciało",
        },
        Swieto {
            data: d(8, 15),
            nazwa: "Wniebowzięcie NMP",
        },
        Swieto {
            data: d(11, 1),
            nazwa: "Wszystkich Świętych",
        },
        Swieto {
            data: d(11, 11),
            nazwa: "Narodowe Święto Niepodległości",
        },
        Swieto {
            data: d(12, 25),
            nazwa: "Boże Narodzenie (pierwszy dzień)",
        },
        Swieto {
            data: d(12, 26),
            nazwa: "Boże Narodzenie (drugi dzień)",
        },
    ];
    if rok >= 2011 {
        v.push(Swieto {
            data: d(1, 6),
            nazwa: "Trzech Króli",
        });
    }
    if rok >= 2025 {
        v.push(Swieto {
            data: d(12, 24),
            nazwa: "Wigilia Bożego Narodzenia",
        });
    }
    v.sort_by_key(|s| s.data);
    v
}

/// Nazwa święta, jeśli `data` jest świętem ustawowo wolnym.
pub fn nazwa_swieta(data: NaiveDate) -> Option<&'static str> {
    // Szybka ścieżka bez alokacji (wywoływana dla każdego dnia przy liczeniu wstecz);
    // zgodność z [`swieta`] sprawdza test `nazwa_swieta_zgodna_z_lista`.
    let rok = data.year();
    let stale = match (data.month(), data.day()) {
        (1, 1) => Some("Nowy Rok"),
        (1, 6) if rok >= 2011 => Some("Trzech Króli"),
        (5, 1) => Some("Święto Pracy"),
        (5, 3) => Some("Święto Konstytucji 3 Maja"),
        (8, 15) => Some("Wniebowzięcie NMP"),
        (11, 1) => Some("Wszystkich Świętych"),
        (11, 11) => Some("Narodowe Święto Niepodległości"),
        (12, 24) if rok >= 2025 => Some("Wigilia Bożego Narodzenia"),
        (12, 25) => Some("Boże Narodzenie (pierwszy dzień)"),
        (12, 26) => Some("Boże Narodzenie (drugi dzień)"),
        _ => None,
    };
    if stale.is_some() || !(3..=6).contains(&data.month()) {
        return stale;
    }
    match (data - wielkanoc(rok)).num_days() {
        0 => Some("Wielkanoc"),
        1 => Some("Poniedziałek Wielkanocny"),
        49 => Some("Zielone Świątki"),
        60 => Some("Boże Ciało"),
        _ => None,
    }
}

/// Kalendarz dni roboczych z trybem liczenia.
#[derive(Debug, Clone)]
pub struct KalendarzDni {
    tryb: LiczenieDni,
    wolne_uzytkownika: BTreeSet<NaiveDate>,
}

impl KalendarzDni {
    pub fn nowy(tryb: LiczenieDni, wolne_uzytkownika: impl IntoIterator<Item = NaiveDate>) -> Self {
        Self {
            tryb,
            wolne_uzytkownika: wolne_uzytkownika.into_iter().collect(),
        }
    }

    pub fn tryb(&self) -> LiczenieDni {
        self.tryb
    }

    /// Dzień roboczy = pon.–pt., nie święto, nie dzień wolny użytkownika.
    pub fn czy_roboczy(&self, d: NaiveDate) -> bool {
        !matches!(d.weekday(), Weekday::Sat | Weekday::Sun)
            && nazwa_swieta(d).is_none()
            && !self.wolne_uzytkownika.contains(&d)
    }

    /// `d`, jeśli jest roboczy; w przeciwnym razie najbliższy wcześniejszy dzień roboczy.
    pub fn cofnij_do_roboczego(&self, mut d: NaiveDate) -> NaiveDate {
        while !self.czy_roboczy(d) {
            d -= Duration::days(1);
        }
        d
    }

    /// `d`, jeśli jest roboczy; w przeciwnym razie najbliższy późniejszy dzień roboczy.
    pub fn nastepny_roboczy(&self, mut d: NaiveDate) -> NaiveDate {
        while !self.czy_roboczy(d) {
            d += Duration::days(1);
        }
        d
    }

    /// Termin „najpóźniej do” leżący `n` dni przed `d`: taki dzień X, że okres `[X, d)` ma `n` dni.
    /// Wynik jest zawsze dniem roboczym (patrz konwencja w dokumentacji modułu).
    pub fn wstecz(&self, d: NaiveDate, n: u32) -> NaiveDate {
        match self.tryb {
            LiczenieDni::Kalendarzowe => self.cofnij_do_roboczego(d - Duration::days(n.into())),
            LiczenieDni::Robocze => {
                if n == 0 {
                    return self.cofnij_do_roboczego(d);
                }
                let mut x = d;
                for _ in 0..n {
                    x -= Duration::days(1);
                    x = self.cofnij_do_roboczego(x);
                }
                x
            }
        }
    }

    /// Dzień, w którym kończy się okres `n` dni rozpoczęty w dniu `d` (np. dostawa
    /// zamówienia złożonego w `d`): pierwszy dzień D, dla którego `[d, D)` ma `n` dni.
    /// W trybie roboczym wynik jest przesuwany na najbliższy dzień roboczy (dostawa w sobotę
    /// po pięciu dniach roboczych liczy się jako dostępna w poniedziałek).
    pub fn naprzod(&self, d: NaiveDate, n: u32) -> NaiveDate {
        match self.tryb {
            LiczenieDni::Kalendarzowe => d + Duration::days(n.into()),
            LiczenieDni::Robocze => {
                let mut x = d;
                let mut policzone = 0;
                while policzone < n {
                    if self.czy_roboczy(x) {
                        policzone += 1;
                    }
                    x += Duration::days(1);
                }
                self.nastepny_roboczy(x)
            }
        }
    }

    /// Ostatni dzień okresu `n` dni (n ≥ 1) zaczynającego się w `start` (np. ostatni dzień
    /// montażu). W trybie roboczym okres zaczyna się w najbliższym dniu roboczym ≥ `start`.
    pub fn ostatni_dzien(&self, start: NaiveDate, n: u32) -> NaiveDate {
        let n = n.max(1);
        match self.tryb {
            LiczenieDni::Kalendarzowe => start + Duration::days(i64::from(n) - 1),
            LiczenieDni::Robocze => self.wstecz(self.naprzod(start, n), 1),
        }
    }

    /// Liczba dni w okresie `[od, do_)` w bieżącym trybie (ujemna, gdy `do_ < od`).
    pub fn roznica(&self, od: NaiveDate, do_: NaiveDate) -> i64 {
        match self.tryb {
            LiczenieDni::Kalendarzowe => (do_ - od).num_days(),
            LiczenieDni::Robocze => {
                let (a, b, znak) = if od <= do_ {
                    (od, do_, 1)
                } else {
                    (do_, od, -1)
                };
                let mut x = a;
                let mut n = 0;
                while x < b {
                    if self.czy_roboczy(x) {
                        n += 1;
                    }
                    x += Duration::days(1);
                }
                znak * n
            }
        }
    }

    /// Termin liczony naprzód od zdarzenia (np. „N dni od podpisania umowy”) – termin dla
    /// klienta. Tryb roboczy: N-ty dzień roboczy po `d`. Tryb kalendarzowy: `d + N`, a gdy
    /// wypada w dzień wolny – przesunięcie na następny dzień roboczy (jak w art. 115 KC).
    pub fn termin_od(&self, d: NaiveDate, n: u32) -> NaiveDate {
        match self.tryb {
            LiczenieDni::Kalendarzowe => self.nastepny_roboczy(d + Duration::days(n.into())),
            LiczenieDni::Robocze => {
                let mut x = d;
                for _ in 0..n {
                    x = self.nastepny_roboczy(x + Duration::days(1));
                }
                self.nastepny_roboczy(x)
            }
        }
    }
}

#[cfg(test)]
mod testy {
    use super::*;

    fn d(r: i32, m: u32, dz: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(r, m, dz).unwrap()
    }

    fn rob() -> KalendarzDni {
        KalendarzDni::nowy(LiczenieDni::Robocze, [])
    }

    fn kal() -> KalendarzDni {
        KalendarzDni::nowy(LiczenieDni::Kalendarzowe, [])
    }

    #[test]
    fn swieta_ruchome_2026_2028() {
        // Wielkanoc, Poniedziałek Wielkanocny, Zielone Świątki, Boże Ciało
        let oczekiwane = [
            (
                2026,
                d(2026, 4, 5),
                d(2026, 4, 6),
                d(2026, 5, 24),
                d(2026, 6, 4),
            ),
            (
                2027,
                d(2027, 3, 28),
                d(2027, 3, 29),
                d(2027, 5, 16),
                d(2027, 5, 27),
            ),
            (
                2028,
                d(2028, 4, 16),
                d(2028, 4, 17),
                d(2028, 6, 4),
                d(2028, 6, 15),
            ),
        ];
        for (rok, wn, pon, ziel, boze) in oczekiwane {
            assert_eq!(wielkanoc(rok), wn, "Wielkanoc {rok}");
            assert_eq!(nazwa_swieta(pon), Some("Poniedziałek Wielkanocny"));
            assert_eq!(nazwa_swieta(ziel), Some("Zielone Świątki"));
            assert_eq!(nazwa_swieta(boze), Some("Boże Ciało"));
            assert!(!rob().czy_roboczy(pon));
            assert!(!rob().czy_roboczy(boze));
            assert_eq!(swieta(rok).len(), 14);
        }
        // znane daty Wielkanocy dla kontroli algorytmu
        assert_eq!(wielkanoc(2024), d(2024, 3, 31));
        assert_eq!(wielkanoc(2025), d(2025, 4, 20));
        assert_eq!(wielkanoc(2038), d(2038, 4, 25));
    }

    #[test]
    fn swieta_stale_i_wigilia() {
        for (data, nazwa) in [
            (d(2027, 1, 1), "Nowy Rok"),
            (d(2027, 1, 6), "Trzech Króli"),
            (d(2027, 5, 3), "Święto Konstytucji 3 Maja"),
            (d(2027, 8, 15), "Wniebowzięcie NMP"),
            (d(2027, 11, 11), "Narodowe Święto Niepodległości"),
            (d(2026, 12, 24), "Wigilia Bożego Narodzenia"),
        ] {
            assert_eq!(nazwa_swieta(data), Some(nazwa));
        }
        assert_eq!(
            nazwa_swieta(d(2024, 12, 24)),
            None,
            "Wigilia wolna dopiero od 2025"
        );
        assert_eq!(
            nazwa_swieta(d(2010, 1, 6)),
            None,
            "Trzech Króli wolne od 2011"
        );
    }

    #[test]
    fn nazwa_swieta_zgodna_z_lista() {
        for rok in [2010, 2024, 2025, 2026, 2027, 2028, 2035] {
            let lista = swieta(rok);
            let mut dzien = d(rok, 1, 1);
            while dzien.year() == rok {
                let z_listy = lista.iter().find(|s| s.data == dzien).map(|s| s.nazwa);
                assert_eq!(nazwa_swieta(dzien), z_listy, "{dzien}");
                dzien += Duration::days(1);
            }
        }
    }

    #[test]
    fn dni_wolne_uzytkownika() {
        let k = KalendarzDni::nowy(LiczenieDni::Robocze, [d(2027, 3, 3)]);
        assert!(!k.czy_roboczy(d(2027, 3, 3)));
        assert!(k.czy_roboczy(d(2027, 3, 4)));
        assert_eq!(k.wstecz(d(2027, 3, 4), 1), d(2027, 3, 2));
    }

    #[test]
    fn wstecz_robocze_przez_przelom_roku() {
        // 4.01.2027 (pon.) − 1 dzień roboczy: 1.01 święto, 2–3.01 weekend, 31.12 czw. roboczy
        assert_eq!(rob().wstecz(d(2027, 1, 4), 1), d(2026, 12, 31));
        // −5 dni roboczych: 31, 30, 29, 28.12, (24–27 wolne), 23.12
        assert_eq!(rob().wstecz(d(2027, 1, 4), 5), d(2026, 12, 23));
        // 7.01.2027 (czw.) − 1: 6.01 święto → 5.01
        assert_eq!(rob().wstecz(d(2027, 1, 7), 1), d(2027, 1, 5));
        assert_eq!(rob().roznica(d(2026, 12, 23), d(2027, 1, 4)), 5);
    }

    #[test]
    fn dlugi_weekend_majowy_i_wielkanoc() {
        // 2027: 1.05 sob., 3.05 pon. święto → 4.05 (wt.) − 1 dzień roboczy = 30.04 (pt.)
        assert_eq!(rob().wstecz(d(2027, 5, 4), 1), d(2027, 4, 30));
        // Wielkanoc 2027: 28–29.03; 30.03 (wt.) − 1 = 26.03 (pt.)
        assert_eq!(rob().wstecz(d(2027, 3, 30), 1), d(2027, 3, 26));
        // kalendarzowo: 30.03 − 1 = 29.03 (Poniedziałek Wielkanocny) → cofnięte na 26.03
        assert_eq!(kal().wstecz(d(2027, 3, 30), 1), d(2027, 3, 26));
        // Boże Ciało 2027 (czw. 27.05) – przedłużony weekend
        assert_eq!(rob().wstecz(d(2027, 5, 31), 1), d(2027, 5, 28));
        assert_eq!(rob().wstecz(d(2027, 5, 31), 2), d(2027, 5, 26));
    }

    #[test]
    fn naprzod_i_ostatni_dzien() {
        // zamówienie w pon. 4.01.2027, 2 dni robocze: liczą się 4 i 5.01 → dostawa 7.01 (6.01 święto)
        assert_eq!(rob().naprzod(d(2027, 1, 4), 2), d(2027, 1, 7));
        // zamówienie w sobotę, 1 dzień roboczy: liczy się pon. → dostawa wt.
        assert_eq!(rob().naprzod(d(2027, 1, 9), 1), d(2027, 1, 12));
        // 5 dni roboczych od pon. 11.01 kończy się w pt. → dostępne w pon. 18.01
        assert_eq!(rob().naprzod(d(2027, 1, 11), 5), d(2027, 1, 18));
        assert_eq!(kal().naprzod(d(2027, 1, 11), 5), d(2027, 1, 16));
        // montaż 3 dni od czw. 7.01.2027: 7, 8, 11.01
        assert_eq!(rob().ostatni_dzien(d(2027, 1, 7), 3), d(2027, 1, 11));
        assert_eq!(kal().ostatni_dzien(d(2027, 1, 7), 3), d(2027, 1, 9));
        assert_eq!(rob().ostatni_dzien(d(2027, 1, 7), 1), d(2027, 1, 7));
    }

    #[test]
    fn termin_od_daty() {
        // 14 dni kalendarzowych od pt. 4.12.2026 = 18.12 (pt.)
        assert_eq!(kal().termin_od(d(2026, 12, 4), 14), d(2026, 12, 18));
        // 10 dni kalendarzowych od 15.12.2026 = 25.12 (święto) → 28.12 (pon.)
        assert_eq!(kal().termin_od(d(2026, 12, 15), 10), d(2026, 12, 28));
        // 3 dni robocze od pt. 18.12.2026: 21, 22, 23.12
        assert_eq!(rob().termin_od(d(2026, 12, 18), 3), d(2026, 12, 23));
        assert_eq!(rob().termin_od(d(2026, 12, 18), 4), d(2026, 12, 28));
    }

    #[test]
    fn wlasnosci_wstecz_i_naprzod() {
        // Własności sprawdzane na każdym dniu lat 2026–2028 i odstępach 0–30.
        for k in [rob(), kal()] {
            let mut dzien = d(2026, 1, 1);
            while dzien <= d(2028, 12, 31) {
                for n in 0..=30u32 {
                    let w = k.wstecz(dzien, n);
                    assert!(k.czy_roboczy(w));
                    assert!(w <= dzien);
                    if n > 0 {
                        assert!(w < dzien);
                    }
                    // monotoniczność względem n
                    assert!(k.wstecz(dzien, n + 1) <= w);
                    if k.tryb() == LiczenieDni::Robocze {
                        // [w, dzien) ma dokładnie n dni roboczych (dla n > 0)
                        if n > 0 {
                            assert_eq!(k.roznica(w, dzien), i64::from(n));
                        }
                        // zamówienie złożone w w przy czasie n dociera nie później niż dzien
                        assert!(k.naprzod(w, n) <= k.nastepny_roboczy(dzien));
                    } else {
                        assert!(k.roznica(w, dzien) >= i64::from(n));
                    }
                }
                dzien += Duration::days(1);
            }
        }
    }
}
