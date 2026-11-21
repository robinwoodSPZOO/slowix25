//! Stany ostrzeżeń zadań względem podanej daty „dziś” (logika nie sięga do zegara).

use chrono::NaiveDate;
use serde::Serialize;

use super::dni::KalendarzDni;
use super::harmonogram::UstawieniaTerminow;

/// Kolor ostrzeżenia. Kolejność = waga (większy = pilniejszy).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Stan {
    Wykonane,
    Spokojnie,
    Zolty,
    Pomaranczowy,
    Czerwony,
}

impl Stan {
    pub fn z_tekstu(t: &str) -> Option<Self> {
        match t {
            "wykonane" => Some(Self::Wykonane),
            "spokojnie" => Some(Self::Spokojnie),
            "zolty" => Some(Self::Zolty),
            "pomaranczowy" => Some(Self::Pomaranczowy),
            "czerwony" => Some(Self::Czerwony),
            _ => None,
        }
    }
}

/// Stan zadania z opisem dla użytkownika.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StanZadania {
    pub stan: Stan,
    pub opis: String,
    /// Dni kalendarzowe od „dziś” do terminu (ujemne = po terminie).
    pub dni_do_terminu: Option<i64>,
    /// Tylko dla czerwonego zamówienia: o ile dni (w trybie liczenia) przesunie się montaż,
    /// jeśli zamówić dziś, przy średnim czasie dostawy.
    pub przesuniecie_montazu_dni: Option<i64>,
    pub nowy_termin_montazu: Option<NaiveDate>,
}

impl StanZadania {
    fn nowy(stan: Stan, opis: impl Into<String>, dni_do_terminu: Option<i64>) -> Self {
        Self {
            stan,
            opis: opis.into(),
            dni_do_terminu,
            przesuniecie_montazu_dni: None,
            nowy_termin_montazu: None,
        }
    }
}

/// Dane niewykonanego zamówienia potrzebne do wyznaczenia stanu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DaneZamowienia {
    pub sweet_spot: NaiveDate,
    pub planowa: NaiveDate,
    pub krytyczna: NaiveDate,
    pub wymagana_dostawa: NaiveDate,
    pub dni_srednia: u32,
    pub montaz_od: NaiveDate,
}

/// Stan niewykonanego zamówienia materiału:
///
/// * `dziś < sweet spot` → spokojnie („przed oknem zamówienia”),
/// * `sweet spot ≤ dziś ≤ planowa` → spokojnie („w oknie zamówienia”),
/// * `dziś > planowa` → żółty,
/// * `dziś ≥ krytyczna − prog_pomaranczowy` (w trybie liczenia dni) → pomarańczowy,
/// * `dziś > krytyczna` → czerwony + przesunięcie montażu przy zamówieniu dziś
///   (`nowy_termin_montazu` zawsze w dniu roboczym).
///
/// Gdy kilka warunków jest spełnionych, wygrywa najpilniejszy.
pub fn stan_zamowienia(
    z: &DaneZamowienia,
    dzis: NaiveDate,
    ustawienia: &UstawieniaTerminow,
    kal: &KalendarzDni,
) -> StanZadania {
    let dni_do_planowej = (z.planowa - dzis).num_days();
    if dzis > z.krytyczna {
        let dostawa = kal.naprzod(dzis, z.dni_srednia);
        let przesuniecie = kal.roznica(z.wymagana_dostawa, dostawa).max(0);
        let nowy_montaz = przesun(kal, z.montaz_od, przesuniecie);
        return StanZadania {
            stan: Stan::Czerwony,
            opis: format!(
                "Po terminie krytycznym ({}). Zamówione dziś dotrze ok. {dostawa} – montaż przesunie się o {przesuniecie} dni (na {nowy_montaz})",
                z.krytyczna
            ),
            dni_do_terminu: Some((z.krytyczna - dzis).num_days()),
            przesuniecie_montazu_dni: Some(przesuniecie),
            nowy_termin_montazu: Some(nowy_montaz),
        };
    }
    let prog = kal.wstecz(z.krytyczna, ustawienia.prog_pomaranczowy_dni);
    if dzis >= prog {
        return StanZadania::nowy(
            Stan::Pomaranczowy,
            format!("Zamów natychmiast – termin krytyczny {}", z.krytyczna),
            Some((z.krytyczna - dzis).num_days()),
        );
    }
    if dzis > z.planowa {
        return StanZadania::nowy(
            Stan::Zolty,
            format!(
                "Minęła data planowa ({}), termin krytyczny {}",
                z.planowa, z.krytyczna
            ),
            Some((z.krytyczna - dzis).num_days()),
        );
    }
    if dzis >= z.sweet_spot {
        StanZadania::nowy(
            Stan::Spokojnie,
            format!("W oknie zamówienia – zamów do {}", z.planowa),
            Some(dni_do_planowej),
        )
    } else {
        StanZadania::nowy(
            Stan::Spokojnie,
            format!(
                "Okno zamówienia od {} (planowo {})",
                z.sweet_spot, z.planowa
            ),
            Some(dni_do_planowej),
        )
    }
}

/// Stan prostego zadania z terminem (pomiar, rysunki, terminy klienta, płatności):
/// po terminie → czerwony; w ciągu `prog_zbliza_sie_dni` przed terminem (włącznie z dniem
/// terminu) → żółty; wcześniej → spokojnie.
pub fn stan_terminu(
    termin: NaiveDate,
    dzis: NaiveDate,
    ustawienia: &UstawieniaTerminow,
    kal: &KalendarzDni,
) -> StanZadania {
    let dni = (termin - dzis).num_days();
    if dzis > termin {
        StanZadania::nowy(
            Stan::Czerwony,
            format!("Termin minął ({termin})"),
            Some(dni),
        )
    } else if dzis >= kal.wstecz(termin, ustawienia.prog_zbliza_sie_dni) {
        StanZadania::nowy(
            Stan::Zolty,
            format!("Zbliża się termin ({termin})"),
            Some(dni),
        )
    } else {
        StanZadania::nowy(Stan::Spokojnie, format!("Termin {termin}"), Some(dni))
    }
}

/// Stan zadania wykonanego.
pub fn stan_wykonane(kiedy: NaiveDate) -> StanZadania {
    StanZadania::nowy(Stan::Wykonane, format!("Wykonano {kiedy}"), None)
}

/// Nowy termin rozpoczęcia montażu po przesunięciu o `dni` (w trybie liczenia). Wynik zawsze
/// wypada w dzień roboczy: w trybie kalendarzowym `d + dni` przypadające w dzień wolny
/// przechodzi na najbliższy **następny** dzień roboczy (w roboczym `naprzod` już to zapewnia).
fn przesun(kal: &KalendarzDni, d: NaiveDate, dni: i64) -> NaiveDate {
    if dni <= 0 {
        return d;
    }
    kal.nastepny_roboczy(kal.naprzod(d, u32::try_from(dni).unwrap_or(u32::MAX)))
}

#[cfg(test)]
mod testy {
    use super::*;
    use crate::kalendarz::dni::LiczenieDni;

    fn d(r: i32, m: u32, dz: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(r, m, dz).unwrap()
    }

    fn ust(tryb: LiczenieDni) -> UstawieniaTerminow {
        UstawieniaTerminow {
            liczenie_dni: tryb,
            ..Default::default()
        }
    }

    /// Fronty MDF z przykładu kontrolnego (kalendarzowe): 23.12 / 28.12 / 31.12, dostawa 18.01.
    fn mdf() -> DaneZamowienia {
        DaneZamowienia {
            sweet_spot: d(2026, 12, 23),
            planowa: d(2026, 12, 28),
            krytyczna: d(2026, 12, 31),
            wymagana_dostawa: d(2027, 1, 18),
            dni_srednia: 21,
            montaz_od: d(2027, 2, 8),
        }
    }

    #[test]
    fn progi_zamowienia_kalendarzowe() {
        let u = ust(LiczenieDni::Kalendarzowe);
        let k = KalendarzDni::nowy(LiczenieDni::Kalendarzowe, []);
        let s = |dzis| stan_zamowienia(&mdf(), dzis, &u, &k).stan;
        // przed oknem i w oknie
        assert_eq!(s(d(2026, 12, 1)), Stan::Spokojnie);
        assert_eq!(s(d(2026, 12, 22)), Stan::Spokojnie);
        assert_eq!(s(d(2026, 12, 23)), Stan::Spokojnie);
        // planowa: dzień przed, w dniu, dzień po
        assert_eq!(s(d(2026, 12, 27)), Stan::Spokojnie);
        assert_eq!(s(d(2026, 12, 28)), Stan::Spokojnie);
        // 29.12 jest jednocześnie po planowej (żółty) i na progu pomarańczowym
        // (krytyczna − 2 = 29.12) – wygrywa pilniejszy pomarańczowy;
        // dzień przed progiem (28.12 = planowa) jest spokojnie (sprawdzone wyżej)
        assert_eq!(s(d(2026, 12, 29)), Stan::Pomaranczowy);
        assert_eq!(s(d(2026, 12, 30)), Stan::Pomaranczowy);
        // krytyczna: w dniu pomarańczowy, dzień po czerwony
        assert_eq!(s(d(2026, 12, 31)), Stan::Pomaranczowy);
        let cz = stan_zamowienia(&mdf(), d(2027, 1, 1), &u, &k);
        assert_eq!(cz.stan, Stan::Czerwony);
        // zamówione 1.01 + 21 dni = 22.01; wymagana 18.01 → przesunięcie 4 dni → montaż 12.02
        assert_eq!(cz.przesuniecie_montazu_dni, Some(4));
        assert_eq!(cz.nowy_termin_montazu, Some(d(2027, 2, 12)));
    }

    #[test]
    fn progi_zamowienia_szersze_okno() {
        // planowa daleko od krytycznej – widać osobno żółty i pomarańczowy
        let z = DaneZamowienia {
            sweet_spot: d(2027, 3, 1),
            planowa: d(2027, 3, 10),
            krytyczna: d(2027, 3, 19),
            wymagana_dostawa: d(2027, 4, 5),
            dni_srednia: 10,
            montaz_od: d(2027, 4, 26),
        };
        let u = ust(LiczenieDni::Robocze);
        let k = KalendarzDni::nowy(LiczenieDni::Robocze, []);
        let s = |dzis| stan_zamowienia(&z, dzis, &u, &k).stan;
        assert_eq!(s(d(2027, 2, 28)), Stan::Spokojnie);
        assert_eq!(s(d(2027, 3, 10)), Stan::Spokojnie);
        assert_eq!(s(d(2027, 3, 11)), Stan::Zolty);
        // próg = 2 dni robocze przed pt. 19.03 = śr. 17.03
        assert_eq!(s(d(2027, 3, 16)), Stan::Zolty);
        assert_eq!(s(d(2027, 3, 17)), Stan::Pomaranczowy);
        assert_eq!(s(d(2027, 3, 19)), Stan::Pomaranczowy);
        let cz = stan_zamowienia(&z, d(2027, 3, 22), &u, &k);
        assert_eq!(cz.stan, Stan::Czerwony);
        // pon. 22.03 + 10 dni roboczych (Wielkanoc 28–29.03) → dostępne 6.04 (wt.)
        // przesunięcie: dni robocze w [5.04, 6.04) = 1 → montaż 27.04
        assert_eq!(cz.przesuniecie_montazu_dni, Some(1));
        assert_eq!(cz.nowy_termin_montazu, Some(d(2027, 4, 27)));
    }

    #[test]
    fn progi_terminu_prostego() {
        let u = ust(LiczenieDni::Kalendarzowe);
        let k = KalendarzDni::nowy(LiczenieDni::Kalendarzowe, []);
        let termin = d(2026, 12, 21); // rysunki z przykładu
        let s = |dzis| stan_terminu(termin, dzis, &u, &k).stan;
        // próg żółty = 21.12 − 3 = 18.12
        assert_eq!(s(d(2026, 12, 17)), Stan::Spokojnie);
        assert_eq!(s(d(2026, 12, 18)), Stan::Zolty);
        assert_eq!(s(d(2026, 12, 20)), Stan::Zolty);
        assert_eq!(s(d(2026, 12, 21)), Stan::Zolty);
        assert_eq!(s(d(2026, 12, 22)), Stan::Czerwony);
        assert_eq!(
            stan_terminu(termin, d(2026, 12, 22), &u, &k).dni_do_terminu,
            Some(-1)
        );
        assert_eq!(stan_wykonane(d(2026, 12, 19)).stan, Stan::Wykonane);
    }
    /// Wartości oczekiwane policzone ręcznie: progi w trybie roboczym, próg pomarańczowy
    /// wypadający w weekend (oba tryby), nowy termin montażu na dzień roboczy.
    #[test]
    fn wartosci_oczekiwane_progow() {
        let rob = (
            ust(LiczenieDni::Robocze),
            KalendarzDni::nowy(LiczenieDni::Robocze, []),
        );
        let kal = (
            ust(LiczenieDni::Kalendarzowe),
            KalendarzDni::nowy(LiczenieDni::Kalendarzowe, []),
        );
        // kalendarzowo: krytyczna pon. 8.03.2027 − 2 = sob. 6.03 → próg pt. 5.03
        let z = DaneZamowienia {
            sweet_spot: d(2027, 2, 24),
            planowa: d(2027, 3, 1),
            krytyczna: d(2027, 3, 8),
            wymagana_dostawa: d(2027, 3, 15),
            dni_srednia: 5,
            montaz_od: d(2027, 4, 12),
        };
        let s = |dzis, (u, k): &(UstawieniaTerminow, KalendarzDni), z: &DaneZamowienia| {
            stan_zamowienia(z, dzis, u, k).stan
        };
        assert_eq!(s(d(2027, 3, 4), &kal, &z), Stan::Zolty);
        assert_eq!(s(d(2027, 3, 5), &kal, &z), Stan::Pomaranczowy);
        assert_eq!(s(d(2027, 3, 8), &kal, &z), Stan::Pomaranczowy);
        assert_eq!(s(d(2027, 3, 9), &kal, &z), Stan::Czerwony);
        // robocze: krytyczna wt. 9.03 − 2 dni rob. = pt. 5.03; sobota 6.03 już pomarańczowa
        let zr = DaneZamowienia {
            krytyczna: d(2027, 3, 9),
            ..z
        };
        assert_eq!(s(d(2027, 3, 1), &rob, &zr), Stan::Spokojnie);
        assert_eq!(s(d(2027, 3, 2), &rob, &zr), Stan::Zolty);
        assert_eq!(s(d(2027, 3, 4), &rob, &zr), Stan::Zolty);
        assert_eq!(s(d(2027, 3, 5), &rob, &zr), Stan::Pomaranczowy);
        assert_eq!(s(d(2027, 3, 6), &rob, &zr), Stan::Pomaranczowy);
        // czerwony robocze: śr. 10.03 + 5 dni rob. = 17.03; [15.03, 17.03) = 2 → montaż 12.04 → 14.04
        let cz = stan_zamowienia(&zr, d(2027, 3, 10), &rob.0, &rob.1);
        assert_eq!(cz.stan, Stan::Czerwony);
        assert_eq!(cz.przesuniecie_montazu_dni, Some(2));
        assert_eq!(cz.nowy_termin_montazu, Some(d(2027, 4, 14)));
        // czerwony kalendarzowo: 20.03 + 10 = 30.03, wymagana 25.03 → 5 dni; 26.04 + 5 = sob. 1.05
        // (święto), 2.05 niedz., 3.05 święto → wt. 4.05
        let zk = DaneZamowienia {
            sweet_spot: d(2027, 3, 1),
            planowa: d(2027, 3, 10),
            krytyczna: d(2027, 3, 19),
            wymagana_dostawa: d(2027, 3, 25),
            dni_srednia: 10,
            montaz_od: d(2027, 4, 26),
        };
        let cz = stan_zamowienia(&zk, d(2027, 3, 20), &kal.0, &kal.1);
        assert_eq!(cz.przesuniecie_montazu_dni, Some(5));
        assert_eq!(cz.nowy_termin_montazu, Some(d(2027, 5, 4)));
        // przykład kontrolny MDF, dziś 2.01.2027: 8.02 + 5 = sob. 13.02 → pon. 15.02
        let cz = stan_zamowienia(&mdf(), d(2027, 1, 2), &kal.0, &kal.1);
        assert_eq!(cz.przesuniecie_montazu_dni, Some(5));
        assert_eq!(cz.nowy_termin_montazu, Some(d(2027, 2, 15)));
        // termin prosty robocze: pon. 15.03 − 3 dni rob. = śr. 10.03
        let t = |dzis| stan_terminu(d(2027, 3, 15), dzis, &rob.0, &rob.1).stan;
        assert_eq!(t(d(2027, 3, 9)), Stan::Spokojnie);
        assert_eq!(t(d(2027, 3, 10)), Stan::Zolty);
        assert_eq!(t(d(2027, 3, 15)), Stan::Zolty);
        assert_eq!(t(d(2027, 3, 16)), Stan::Czerwony);
    }
}
