//! Plan płatności umowy (wariant A / B) – czysta logika bez bazy, testowana jednostkowo.
//!
//! Warianty (ZAŁOŻENIE do potwierdzenia przez użytkownika; liczby dni w ustawieniach umów):
//! * **A** – zadatek + pozostała część najpóźniej `a_dni_przed_montazem` dni roboczych przed
//!   uzgodnionym rozpoczęciem montażu;
//! * **B** – zadatek + kwota przed montażem (najpóźniej `b_dni_przed_montazem` dni roboczych
//!   przed rozpoczęciem montażu) + pozostałość w terminie `b_dni_po_odbiorze` dni roboczych od
//!   prawidłowego oddania przedmiotu umowy. Po zadatku i kwocie przed montażem musi zostać kwota
//!   dodatnia.
//!
//! Zadatek: w terminie `termin_zadatku_dni` dni kalendarzowych od zawarcia umowy (dzień wolny →
//! następny dzień roboczy), chyba że umowa ma wpisaną konkretną datę.
//! Suma pozycji planu zawsze równa się wartości umowy (pozycja końcowa „domyka” sumę).

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::kalendarz::dni::{KalendarzDni, LiczenieDni};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct UstawieniaPlatnosci {
    pub termin_zadatku_dni: u32,
    pub a_dni_przed_montazem: u32,
    pub b_dni_przed_montazem: u32,
    pub b_dni_po_odbiorze: u32,
}

impl Default for UstawieniaPlatnosci {
    fn default() -> Self {
        Self {
            termin_zadatku_dni: 3,
            a_dni_przed_montazem: 3,
            b_dni_przed_montazem: 3,
            b_dni_po_odbiorze: 3,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct WejsciePlanu {
    pub wartosc_gr: Option<i64>,
    pub zadatek_gr: Option<i64>,
    /// Data wpisana w umowie (pierwszeństwo przed liczeniem od daty zawarcia).
    pub termin_zadatku: Option<NaiveDate>,
    pub data_zawarcia: Option<NaiveDate>,
    pub wariant: Option<String>,
    pub kwota_przed_montazem_gr: Option<i64>,
    pub montaz_do: Option<NaiveDate>,
    pub czas_montazu_dni: Option<u32>,
    /// Data protokołu odbioru (gdy jest – termin pozostałości w wariancie B liczony od niej).
    pub data_odbioru: Option<NaiveDate>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PozycjaPlanu {
    /// `zadatek`, `przed_montazem`, `koncowa` (jak `platnosci_planowane.rodzaj`).
    pub rodzaj: String,
    pub opis: String,
    pub kwota_gr: i64,
    pub termin: Option<String>,
    /// Opis terminu do dokumentu („3 dni robocze od prawidłowego oddania…”).
    pub termin_opis: String,
    /// Termin szacunkowy (np. pozostałość B przed odbiorem: liczona od planowanego końca montażu).
    pub szacunkowy: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Default)]
pub struct PlanPlatnosci {
    pub wariant: Option<String>,
    pub pozycje: Vec<PozycjaPlanu>,
    pub suma_gr: i64,
    /// Suma pozycji = wartość umowy i brak błędów.
    pub zgodny: bool,
    /// Błędy blokujące wyjście umowy ze szkicu.
    pub bledy: Vec<String>,
    pub ostrzezenia: Vec<String>,
}

fn data(d: NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}

/// „N dni roboczych/robocze” z poprawną odmianą.
pub fn opis_dni_roboczych(n: u32) -> String {
    let forma = crate::slownie::forma(
        u64::from(n),
        ["dzień roboczy", "dni robocze", "dni roboczych"],
    );
    format!("{n} {forma}")
}

/// Liczy plan płatności. `kal_wolne` – dni wolne użytkownika (dni robocze liczone zawsze
/// w trybie roboczym, niezależnie od ustawienia kalendarza produkcji).
pub fn wylicz_plan(
    w: &WejsciePlanu,
    u: &UstawieniaPlatnosci,
    kal_wolne: &[NaiveDate],
) -> PlanPlatnosci {
    let robocze = KalendarzDni::nowy(LiczenieDni::Robocze, kal_wolne.iter().copied());
    let kalendarzowe = KalendarzDni::nowy(LiczenieDni::Kalendarzowe, kal_wolne.iter().copied());
    let mut plan = PlanPlatnosci {
        wariant: w.wariant.clone(),
        ..Default::default()
    };
    let Some(wartosc) = w.wartosc_gr.filter(|v| *v > 0) else {
        plan.bledy.push("uzupełnij wartość umowy brutto".into());
        return plan;
    };
    let zadatek = w.zadatek_gr.unwrap_or(0);
    if zadatek < 0 {
        plan.bledy.push("zadatek nie może być ujemny".into());
        return plan;
    }
    if zadatek > wartosc {
        plan.bledy
            .push("zadatek nie może być większy niż wartość umowy".into());
        return plan;
    }
    let wariant = match w.wariant.as_deref() {
        Some("A") | Some("B") => w.wariant.clone().unwrap_or_default(),
        Some(inny) => {
            plan.bledy
                .push(format!("nieznany wariant płatności „{inny}” (A albo B)"));
            return plan;
        }
        None => {
            plan.bledy
                .push("wybierz wariant płatności (A albo B)".into());
            return plan;
        }
    };

    if zadatek > 0 {
        let termin = w.termin_zadatku.or_else(|| {
            w.data_zawarcia
                .map(|d| kalendarzowe.termin_od(d, u.termin_zadatku_dni))
        });
        if termin.is_none() {
            plan.ostrzezenia
                .push("zadatek: brak daty zawarcia umowy – termin nieznany".into());
        }
        plan.pozycje.push(PozycjaPlanu {
            rodzaj: "zadatek".into(),
            opis: "Zadatek".into(),
            kwota_gr: zadatek,
            termin: termin.map(data),
            termin_opis: format!(
                "w terminie {} dni kalendarzowych od zawarcia umowy",
                u.termin_zadatku_dni
            ),
            szacunkowy: false,
        });
    }

    let przed_montazem = |n: u32| w.montaz_do.map(|m| robocze.wstecz(m, n));
    if w.montaz_do.is_none() {
        plan.ostrzezenia.push(
            "brak terminu rozpoczęcia montażu – terminy płatności przed montażem nieznane".into(),
        );
    }

    match wariant.as_str() {
        "A" => {
            let reszta = wartosc - zadatek;
            if reszta > 0 {
                plan.pozycje.push(PozycjaPlanu {
                    rodzaj: "koncowa".into(),
                    opis: "Pozostała część wynagrodzenia".into(),
                    kwota_gr: reszta,
                    termin: przed_montazem(u.a_dni_przed_montazem).map(data),
                    termin_opis: format!(
                        "najpóźniej {} przed uzgodnionym rozpoczęciem montażu",
                        opis_dni_roboczych(u.a_dni_przed_montazem)
                    ),
                    szacunkowy: false,
                });
            }
        }
        _ => {
            let przed = w.kwota_przed_montazem_gr.unwrap_or(0);
            if przed <= 0 {
                plan.bledy
                    .push("wariant B: wpisz kwotę płatną przed montażem".into());
                return plan;
            }
            let reszta = wartosc - zadatek - przed;
            if reszta <= 0 {
                plan.bledy.push(
                    "wariant B: po zadatku i kwocie przed montażem musi zostać kwota do zapłaty po odbiorze (większa od zera)"
                        .into(),
                );
                return plan;
            }
            plan.pozycje.push(PozycjaPlanu {
                rodzaj: "przed_montazem".into(),
                opis: "Kwota przed montażem".into(),
                kwota_gr: przed,
                termin: przed_montazem(u.b_dni_przed_montazem).map(data),
                termin_opis: format!(
                    "najpóźniej {} przed uzgodnionym rozpoczęciem montażu",
                    opis_dni_roboczych(u.b_dni_przed_montazem)
                ),
                szacunkowy: false,
            });
            // Odbiór: data protokołu, a przed odbiorem – planowany koniec montażu (szacunek).
            let (od, szac) = match w.data_odbioru {
                Some(d) => (Some(d), false),
                None => (
                    w.montaz_do.map(|m| {
                        let kal =
                            KalendarzDni::nowy(LiczenieDni::Robocze, kal_wolne.iter().copied());
                        kal.ostatni_dzien(m, w.czas_montazu_dni.unwrap_or(1).max(1))
                    }),
                    true,
                ),
            };
            plan.pozycje.push(PozycjaPlanu {
                rodzaj: "koncowa".into(),
                opis: "Pozostała część wynagrodzenia (po odbiorze)".into(),
                kwota_gr: reszta,
                termin: od.map(|d| data(robocze.termin_od(d, u.b_dni_po_odbiorze))),
                termin_opis: format!(
                    "w terminie {} od prawidłowego oddania przedmiotu umowy",
                    if u.b_dni_po_odbiorze == 1 {
                        "1 dnia roboczego".to_string()
                    } else {
                        format!("{} dni roboczych", u.b_dni_po_odbiorze)
                    }
                ),
                szacunkowy: szac,
            });
        }
    }
    plan.suma_gr = plan.pozycje.iter().map(|p| p.kwota_gr).sum();
    plan.zgodny = plan.bledy.is_empty() && plan.suma_gr == wartosc;
    if plan.suma_gr != wartosc {
        plan.bledy
            .push("suma płatności nie równa się wartości umowy".into());
    }
    plan
}

/// Etap harmonogramu (załącznik nr 2) – do kontroli sumy.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KontrolaEtapow {
    pub suma_gr: i64,
    pub roznica_gr: i64,
    pub zgodna: bool,
}

pub fn kontrola_etapow(kwoty: &[i64], wartosc_gr: Option<i64>) -> KontrolaEtapow {
    let suma: i64 = kwoty.iter().sum();
    let w = wartosc_gr.unwrap_or(0);
    KontrolaEtapow {
        suma_gr: suma,
        roznica_gr: w - suma,
        zgodna: suma == w,
    }
}

#[cfg(test)]
mod testy {
    use super::*;

    fn d(r: i32, m: u32, dz: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(r, m, dz).unwrap()
    }

    fn wej(wariant: &str) -> WejsciePlanu {
        WejsciePlanu {
            wartosc_gr: Some(5_000_000),
            zadatek_gr: Some(1_500_000),
            data_zawarcia: Some(d(2026, 11, 20)), // piątek
            wariant: Some(wariant.into()),
            montaz_do: Some(d(2027, 2, 8)), // poniedziałek
            czas_montazu_dni: Some(3),
            ..Default::default()
        }
    }

    #[test]
    fn wariant_a() {
        let p = wylicz_plan(&wej("A"), &UstawieniaPlatnosci::default(), &[]);
        assert!(p.zgodny, "{:?}", p.bledy);
        assert_eq!(p.pozycje.len(), 2);
        // 20.11 + 3 dni = pon. 23.11
        assert_eq!(p.pozycje[0].termin.as_deref(), Some("2026-11-23"));
        assert_eq!(p.pozycje[1].kwota_gr, 3_500_000);
        // 3 dni robocze przed pon. 8.02 → śr. 3.02
        assert_eq!(p.pozycje[1].termin.as_deref(), Some("2027-02-03"));
        assert_eq!(p.suma_gr, 5_000_000);
    }

    #[test]
    fn zadatek_w_dzien_wolny_i_wlasna_data() {
        let mut w = wej("A");
        w.data_zawarcia = Some(d(2026, 11, 19)); // czw. + 3 = niedz. 22.11 → pon. 23.11
        let p = wylicz_plan(&w, &UstawieniaPlatnosci::default(), &[]);
        assert_eq!(p.pozycje[0].termin.as_deref(), Some("2026-11-23"));
        w.termin_zadatku = Some(d(2026, 11, 30));
        let p = wylicz_plan(&w, &UstawieniaPlatnosci::default(), &[]);
        assert_eq!(p.pozycje[0].termin.as_deref(), Some("2026-11-30"));
    }

    #[test]
    fn wariant_b_i_kontrola_sumy() {
        let mut w = wej("B");
        w.kwota_przed_montazem_gr = Some(2_500_000);
        let p = wylicz_plan(&w, &UstawieniaPlatnosci::default(), &[]);
        assert!(p.zgodny, "{:?}", p.bledy);
        assert_eq!(
            p.pozycje.iter().map(|x| x.kwota_gr).collect::<Vec<_>>(),
            vec![1_500_000, 2_500_000, 1_000_000]
        );
        assert_eq!(p.pozycje[1].termin.as_deref(), Some("2027-02-03"));
        // montaż pon. 8.02, 3 dni robocze → koniec śr. 10.02; +3 dni robocze → pon. 15.02
        assert_eq!(p.pozycje[2].termin.as_deref(), Some("2027-02-15"));
        assert!(p.pozycje[2].szacunkowy);
        // po odbiorze termin od daty protokołu
        w.data_odbioru = Some(d(2027, 2, 12)); // pt. → +3 robocze = śr. 17.02
        let p = wylicz_plan(&w, &UstawieniaPlatnosci::default(), &[]);
        assert_eq!(p.pozycje[2].termin.as_deref(), Some("2027-02-17"));
        assert!(!p.pozycje[2].szacunkowy);

        // B: nic nie zostaje po odbiorze → błąd
        w.kwota_przed_montazem_gr = Some(3_500_000);
        let p = wylicz_plan(&w, &UstawieniaPlatnosci::default(), &[]);
        assert!(!p.zgodny);
        assert!(p.bledy[0].contains("musi zostać"));
        // B bez kwoty przed montażem
        w.kwota_przed_montazem_gr = None;
        assert!(!wylicz_plan(&w, &UstawieniaPlatnosci::default(), &[]).zgodny);
    }

    #[test]
    fn bledy_danych() {
        let mut w = wej("A");
        w.zadatek_gr = Some(6_000_000);
        assert!(!wylicz_plan(&w, &UstawieniaPlatnosci::default(), &[]).zgodny);
        w.zadatek_gr = Some(5_000_000); // zadatek = całość → jedna pozycja
        let p = wylicz_plan(&w, &UstawieniaPlatnosci::default(), &[]);
        assert!(p.zgodny);
        assert_eq!(p.pozycje.len(), 1);
        w.wariant = None;
        assert!(!wylicz_plan(&w, &UstawieniaPlatnosci::default(), &[]).zgodny);
        w.wariant = Some("C".into());
        assert!(!wylicz_plan(&w, &UstawieniaPlatnosci::default(), &[]).zgodny);
        w.wartosc_gr = None;
        assert!(!wylicz_plan(&w, &UstawieniaPlatnosci::default(), &[]).zgodny);
    }

    #[test]
    fn dni_wolne_i_ustawienia() {
        let u = UstawieniaPlatnosci {
            a_dni_przed_montazem: 5,
            ..Default::default()
        };
        // dzień wolny użytkownika pt. 5.02 → 5 dni roboczych przed 8.02: 4,3,2,1.02, 29.01
        let p = wylicz_plan(&wej("A"), &u, &[d(2027, 2, 5)]);
        assert_eq!(p.pozycje[1].termin.as_deref(), Some("2027-01-29"));
        assert_eq!(opis_dni_roboczych(5), "5 dni roboczych");
        assert_eq!(opis_dni_roboczych(3), "3 dni robocze");
        assert_eq!(opis_dni_roboczych(1), "1 dzień roboczy");
    }

    #[test]
    fn etapy() {
        let k = kontrola_etapow(&[100, 200], Some(300));
        assert!(k.zgodna);
        let k = kontrola_etapow(&[100], Some(300));
        assert_eq!(k.roznica_gr, 200);
        assert!(!k.zgodna);
    }
}
