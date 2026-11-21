//! Scalanie nowo wyliczonych zadań z zadaniami zapisanymi w bazie (zamrażanie wykonanych).
//!
//! Zasady:
//! * zadanie wpisane ręcznie (`wyliczone = false`) – nigdy nie jest ruszane,
//! * zadanie wyliczone i **wykonane** – zamrożone: zostaje bez zmian, nowa wersja jest pomijana,
//! * zadanie wyliczone, niewykonane, z kluczem występującym w nowym wyliczeniu – aktualizowane
//!   (zachowuje id, więc odnośniki w interfejsie się nie psują),
//! * zadanie wyliczone, niewykonane, którego klucza już nie ma (np. usunięty materiał) – usuwane,
//! * nowy klucz – dodawane.

use std::collections::HashMap;

use super::harmonogram::ZadanieWyliczone;

/// Zadanie zapisane w bazie (tylko pola potrzebne do scalenia).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZadanieIstniejace {
    pub id: i64,
    pub klucz: Option<String>,
    pub wyliczone: bool,
    pub wykonane: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlanZapisu {
    pub do_dodania: Vec<ZadanieWyliczone>,
    pub do_zmiany: Vec<(i64, ZadanieWyliczone)>,
    pub do_usuniecia: Vec<i64>,
    /// Id zadań pozostawionych bez zmian (wykonane lub ręczne).
    pub zamrozone: Vec<i64>,
}

pub fn scal(wyliczone: Vec<ZadanieWyliczone>, istniejace: &[ZadanieIstniejace]) -> PlanZapisu {
    let mut plan = PlanZapisu::default();
    let mut wg_klucza: HashMap<&str, &ZadanieIstniejace> = HashMap::new();
    for z in istniejace {
        match (&z.klucz, z.wyliczone) {
            (Some(k), true) => {
                // duplikat klucza (nie powinien wystąpić – chroni go indeks unikalny): usuń nadmiar
                if let Some(poprzedni) = wg_klucza.insert(k.as_str(), z) {
                    if poprzedni.wykonane {
                        wg_klucza.insert(k.as_str(), poprzedni);
                        plan.do_usuniecia.push(z.id);
                    } else {
                        plan.do_usuniecia.push(poprzedni.id);
                    }
                }
            }
            _ => plan.zamrozone.push(z.id),
        }
    }
    let mut uzyte: Vec<&str> = Vec::new();
    for nowe in wyliczone {
        match wg_klucza.get(nowe.klucz.as_str()) {
            Some(ist) if ist.wykonane => {
                plan.zamrozone.push(ist.id);
                uzyte.push(ist.klucz.as_deref().unwrap_or_default());
            }
            Some(ist) => {
                uzyte.push(ist.klucz.as_deref().unwrap_or_default());
                plan.do_zmiany.push((ist.id, nowe));
            }
            None => plan.do_dodania.push(nowe),
        }
    }
    for (klucz, ist) in &wg_klucza {
        if uzyte.contains(klucz) {
            continue;
        }
        if ist.wykonane {
            // wykonane zadanie, którego już nie wyliczamy – zostaje jako historia
            plan.zamrozone.push(ist.id);
        } else {
            plan.do_usuniecia.push(ist.id);
        }
    }
    plan.do_usuniecia.sort_unstable();
    plan.zamrozone.sort_unstable();
    plan
}

#[cfg(test)]
mod testy {
    use super::*;
    use crate::kalendarz::harmonogram::*;
    use crate::kalendarz::LiczenieDni;
    use chrono::NaiveDate;

    fn d(r: i32, m: u32, dz: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(r, m, dz).unwrap()
    }

    fn ist(id: i64, klucz: &str, wykonane: bool) -> ZadanieIstniejace {
        ZadanieIstniejace {
            id,
            klucz: Some(klucz.into()),
            wyliczone: true,
            wykonane,
        }
    }

    /// Przeliczenie po zmianie terminu montażu: wykonane zadania (pomiar) zostają,
    /// niewykonane dostają nowe daty, nowy materiał dochodzi, usunięty znika.
    #[test]
    fn przeliczenie_po_zmianie_terminu_z_zamrozonymi() {
        let ust = UstawieniaTerminow {
            liczenie_dni: LiczenieDni::Kalendarzowe,
            ..Default::default()
        };
        let mut zl = ZlecenieWejscie {
            id: 1,
            nazwa: "Szafa testowa".into(),
            montaz_od: d(2027, 2, 8),
            czas_montazu_dni: 2,
            czas_warsztatu_dni: 14,
            odstep_materialy_dni: None,
            odstep_rysunki_dni: None,
            odstep_pomiar_dni: None,
            data_umowy: None,
            dni_na_brakujace_informacje: None,
            platnosci: vec![],
        };
        let plyta = MaterialWejscie {
            id: 10,
            kategoria_id: 1,
            kategoria_nazwa: "Płyta meblowa".into(),
            dostawca_id: None,
            opis: None,
            widelki: Some(Widelki { min: 5, max: 10 }),
            zrodlo_widelek: ZrodloWidelek::Kategoria,
            data_zamowienia: None,
            data_dostawy: None,
            wiersze_do_potwierdzenia: 0,
        };
        let h1 = policz_harmonogram(&zl, std::slice::from_ref(&plyta), &ust, &[]).unwrap();
        let pierwszy = scal(h1.zadania.clone(), &[]);
        assert_eq!(pierwszy.do_dodania.len(), h1.zadania.len());

        // „baza” po pierwszym zapisie: id = indeks + 1; pomiar wykonany; ręczne zadanie id 100
        let mut istniejace: Vec<ZadanieIstniejace> = h1
            .zadania
            .iter()
            .enumerate()
            .map(|(i, z)| ist(i as i64 + 1, &z.klucz, z.klucz == "pomiar"))
            .collect();
        istniejace.push(ZadanieIstniejace {
            id: 100,
            klucz: None,
            wyliczone: false,
            wykonane: false,
        });
        istniejace.push(ist(200, "zamowienie:99", false)); // materiał usunięty ze zlecenia
        istniejace.push(ist(201, "zamowienie:98", true)); // usunięty, ale zamówiony – historia
        let id_pomiaru = istniejace
            .iter()
            .find(|z| z.klucz.as_deref() == Some("pomiar"))
            .unwrap()
            .id;
        let id_montazu = istniejace
            .iter()
            .find(|z| z.klucz.as_deref() == Some("montaz"))
            .unwrap()
            .id;

        // montaż przesunięty o tydzień + nowy materiał
        zl.montaz_od = d(2027, 2, 15);
        let fronty = MaterialWejscie {
            id: 11,
            kategoria_nazwa: "Fronty MDF frezowane".into(),
            widelki: Some(Widelki { min: 18, max: 24 }),
            ..plyta.clone()
        };
        let h2 = policz_harmonogram(&zl, &[plyta, fronty], &ust, &[]).unwrap();
        let plan = scal(h2.zadania, &istniejace);

        assert!(plan.zamrozone.contains(&id_pomiaru));
        assert!(plan.zamrozone.contains(&100));
        assert!(plan.zamrozone.contains(&201));
        assert_eq!(plan.do_usuniecia, vec![200]);
        assert!(!plan.do_zmiany.iter().any(|(id, _)| *id == id_pomiaru));
        let (_, montaz) = plan
            .do_zmiany
            .iter()
            .find(|(id, _)| *id == id_montazu)
            .unwrap();
        assert_eq!(montaz.data_od, d(2027, 2, 15));
        assert_eq!(
            plan.do_dodania
                .iter()
                .map(|z| z.klucz.as_str())
                .collect::<Vec<_>>(),
            vec!["zamowienie:11"]
        );
        // każde istniejące zadanie trafia dokładnie do jednej grupy
        let mut wszystkie: Vec<i64> = plan
            .do_zmiany
            .iter()
            .map(|(id, _)| *id)
            .chain(plan.do_usuniecia.iter().copied())
            .chain(plan.zamrozone.iter().copied())
            .collect();
        wszystkie.sort_unstable();
        let mut oczekiwane: Vec<i64> = istniejace.iter().map(|z| z.id).collect();
        oczekiwane.sort_unstable();
        assert_eq!(wszystkie, oczekiwane);
    }
}
