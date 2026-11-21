//! Wyliczanie harmonogramu zlecenia wstecz od terminu rozpoczęcia montażu.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use super::dni::{KalendarzDni, LiczenieDni};

/// Typ zadania kalendarza (= kolor). Wartości jak w kolumnie `zadania_kalendarza.typ`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TypZadania {
    Montaz,
    Warsztat,
    Zamowienie,
    Rysunki,
    Pomiar,
    Platnosc,
    Klient,
    Inne,
}

impl TypZadania {
    pub const WSZYSTKIE: [TypZadania; 8] = [
        Self::Montaz,
        Self::Warsztat,
        Self::Zamowienie,
        Self::Rysunki,
        Self::Pomiar,
        Self::Platnosc,
        Self::Klient,
        Self::Inne,
    ];

    pub fn jako_tekst(self) -> &'static str {
        match self {
            Self::Montaz => "montaz",
            Self::Warsztat => "warsztat",
            Self::Zamowienie => "zamowienie",
            Self::Rysunki => "rysunki",
            Self::Pomiar => "pomiar",
            Self::Platnosc => "platnosc",
            Self::Klient => "klient",
            Self::Inne => "inne",
        }
    }

    pub fn z_tekstu(t: &str) -> Option<Self> {
        Self::WSZYSTKIE.into_iter().find(|x| x.jako_tekst() == t)
    }
}

/// Widełki czasu dostawy w dniach (tryb zgodnie z [`LiczenieDni`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Widelki {
    pub min: u32,
    pub max: u32,
}

impl Widelki {
    /// Średnia z widełek **zaokrąglona w dół**: 5–10 → 7, 18–24 → 21.
    /// Zaokrąglenie w dół daje późniejszą datę planową; bezpieczny margines zapewnia sweet spot.
    pub fn srednia(self) -> u32 {
        (self.min + self.max) / 2
    }
}

/// Skąd wzięto widełki materiału.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ZrodloWidelek {
    /// Para kategoria + dostawca (`czasy_dostaw`).
    KategoriaDostawca,
    /// Domyślne widełki kategorii (`kategorie_materialow`).
    Kategoria,
    /// Brak widełek – terminy zamówienia nie zostaną wyliczone.
    Brak,
}

/// Ustawienia odstępów i progów (tabela `ustawienia`, nadpisania w zleceniu).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct UstawieniaTerminow {
    pub liczenie_dni: LiczenieDni,
    pub odstep_materialy_dni: u32,
    pub odstep_rysunki_dni: u32,
    pub odstep_pomiar_dni: u32,
    pub prog_pomaranczowy_dni: u32,
    pub prog_zbliza_sie_dni: u32,
    pub odstep_potwierdzenie_montazu_dni: u32,
    pub odstep_pomieszczenie_gotowe_dni: u32,
}

impl Default for UstawieniaTerminow {
    fn default() -> Self {
        Self {
            liczenie_dni: LiczenieDni::Robocze,
            odstep_materialy_dni: 7,
            odstep_rysunki_dni: 7,
            odstep_pomiar_dni: 7,
            prog_pomaranczowy_dni: 2,
            prog_zbliza_sie_dni: 3,
            odstep_potwierdzenie_montazu_dni: 7,
            odstep_pomieszczenie_gotowe_dni: 3,
        }
    }
}

/// Planowana płatność z terminem (z umowy).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatnoscWejscie {
    pub id: i64,
    pub opis: String,
    pub termin: NaiveDate,
}

/// Dane zlecenia potrzebne do policzenia harmonogramu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZlecenieWejscie {
    pub id: i64,
    pub nazwa: String,
    /// Termin rozpoczęcia montażu z umowy.
    pub montaz_od: NaiveDate,
    pub czas_montazu_dni: u32,
    pub czas_warsztatu_dni: u32,
    /// Nadpisania odstępów z ustawień (None = wartość globalna).
    pub odstep_materialy_dni: Option<u32>,
    pub odstep_rysunki_dni: Option<u32>,
    pub odstep_pomiar_dni: Option<u32>,
    /// Data podpisania umowy i liczba dni na przekazanie brakujących informacji przez klienta.
    pub data_umowy: Option<NaiveDate>,
    pub dni_na_brakujace_informacje: Option<u32>,
    pub platnosci: Vec<PlatnoscWejscie>,
}

/// Materiał (pozycja zamówienia) zlecenia.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterialWejscie {
    /// `zamowienia_materialow.id`
    pub id: i64,
    pub kategoria_id: i64,
    pub kategoria_nazwa: String,
    pub dostawca_id: Option<i64>,
    pub opis: Option<String>,
    pub widelki: Option<Widelki>,
    pub zrodlo_widelek: ZrodloWidelek,
    /// Faktyczna data zamówienia (Some = zamówione, terminy zamrożone).
    pub data_zamowienia: Option<NaiveDate>,
    pub data_dostawy: Option<NaiveDate>,
    /// Liczba wierszy specyfikacji „do potwierdzenia” w tej kategorii (blokują zamówienie).
    pub wiersze_do_potwierdzenia: u32,
}

/// Wyliczone terminy zamówienia materiału.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TerminyMaterialu {
    pub material_id: i64,
    pub kategoria_id: i64,
    pub kategoria_nazwa: String,
    pub dostawca_id: Option<i64>,
    pub widelki: Option<Widelki>,
    pub dni_srednia: Option<u32>,
    pub zrodlo_widelek: ZrodloWidelek,
    pub data_wymaganej_dostawy: NaiveDate,
    pub data_sweet_spot: Option<NaiveDate>,
    pub data_planowa: Option<NaiveDate>,
    pub data_krytyczna: Option<NaiveDate>,
    /// Zamówienie wstrzymane przez wiersze „do potwierdzenia”.
    pub zablokowane: bool,
    /// Do kiedy klient musi potwierdzić, żeby zamówić w dacie planowej.
    pub potwierdzic_do: Option<NaiveDate>,
    pub zamowione: bool,
}

/// Zadanie wyliczone przez silnik (jeszcze bez id z bazy).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ZadanieWyliczone {
    /// Stały klucz zadania w obrębie zlecenia, np. `montaz`, `zamowienie:12`.
    pub klucz: String,
    pub typ: TypZadania,
    pub tytul: String,
    pub data_od: NaiveDate,
    pub data_do: Option<NaiveDate>,
    pub data_sweet_spot: Option<NaiveDate>,
    pub data_planowa: Option<NaiveDate>,
    pub data_krytyczna: Option<NaiveDate>,
    pub zamowienie_id: Option<i64>,
    /// Zadanie już wykonane wg danych wejściowych (np. materiał zamówiony).
    pub wykonano: Option<NaiveDate>,
}

impl ZadanieWyliczone {
    fn proste(
        klucz: impl Into<String>,
        typ: TypZadania,
        tytul: impl Into<String>,
        data: NaiveDate,
    ) -> Self {
        Self {
            klucz: klucz.into(),
            typ,
            tytul: tytul.into(),
            data_od: data,
            data_do: None,
            data_sweet_spot: None,
            data_planowa: None,
            data_krytyczna: None,
            zamowienie_id: None,
            wykonano: None,
        }
    }
}

/// Ostrzeżenie dla użytkownika (harmonogram policzony, ale coś wymaga uwagi).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Ostrzezenie {
    pub kod: &'static str,
    pub komunikat: String,
    pub material_id: Option<i64>,
}

/// Wynik liczenia.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Harmonogram {
    pub zlecenie_id: i64,
    pub liczenie_dni: LiczenieDni,
    pub montaz_od: NaiveDate,
    pub montaz_do: NaiveDate,
    pub warsztat_od: Option<NaiveDate>,
    pub warsztat_do: Option<NaiveDate>,
    /// Wszystkie materiały na warsztacie najpóźniej tego dnia.
    pub data_wymaganej_dostawy: NaiveDate,
    pub rysunki_do: NaiveDate,
    pub pomiar_do: NaiveDate,
    pub materialy: Vec<TerminyMaterialu>,
    pub zadania: Vec<ZadanieWyliczone>,
    pub ostrzezenia: Vec<Ostrzezenie>,
}

/// Błąd uniemożliwiający policzenie harmonogramu.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BladHarmonogramu {
    #[error("czas montażu musi wynosić co najmniej 1 dzień")]
    CzasMontazuZero,
}

/// Liczy harmonogram zlecenia wstecz od `zlecenie.montaz_od` (konwencja: [`super::dni`]).
///
/// Funkcja jest czysta: nie sięga do zegara ani bazy; `dni_wolne` to dni wolne użytkownika
/// (święta ustawowe liczy sama).
pub fn policz_harmonogram(
    zlecenie: &ZlecenieWejscie,
    materialy: &[MaterialWejscie],
    ustawienia: &UstawieniaTerminow,
    dni_wolne: &[NaiveDate],
) -> Result<Harmonogram, BladHarmonogramu> {
    let kal = KalendarzDni::nowy(ustawienia.liczenie_dni, dni_wolne.iter().copied());
    policz_z_kalendarzem(zlecenie, materialy, ustawienia, &kal)
}

/// Jak [`policz_harmonogram`], z gotowym kalendarzem (np. do przeliczania wielu zleceń).
pub fn policz_z_kalendarzem(
    zlecenie: &ZlecenieWejscie,
    materialy: &[MaterialWejscie],
    ustawienia: &UstawieniaTerminow,
    kal: &KalendarzDni,
) -> Result<Harmonogram, BladHarmonogramu> {
    if zlecenie.czas_montazu_dni == 0 {
        return Err(BladHarmonogramu::CzasMontazuZero);
    }
    let mut ostrzezenia = Vec::new();
    let mut zadania = Vec::new();

    let odstep_materialy = zlecenie
        .odstep_materialy_dni
        .unwrap_or(ustawienia.odstep_materialy_dni);
    let odstep_rysunki = zlecenie
        .odstep_rysunki_dni
        .unwrap_or(ustawienia.odstep_rysunki_dni);
    let odstep_pomiar = zlecenie
        .odstep_pomiar_dni
        .unwrap_or(ustawienia.odstep_pomiar_dni);

    // 1. Montaż
    let montaz_od = zlecenie.montaz_od;
    if !kal.czy_roboczy(montaz_od) {
        ostrzezenia.push(Ostrzezenie {
            kod: "montaz_w_dzien_wolny",
            komunikat: format!("Termin rozpoczęcia montażu {montaz_od} wypada w dzień wolny"),
            material_id: None,
        });
    }
    let montaz_do = kal.ostatni_dzien(montaz_od, zlecenie.czas_montazu_dni);
    zadania.push(ZadanieWyliczone {
        data_do: Some(montaz_do),
        ..ZadanieWyliczone::proste(
            "montaz",
            TypZadania::Montaz,
            format!("Montaż: {}", zlecenie.nazwa),
            montaz_od,
        )
    });

    // 2. Składanie na warsztacie: [warsztat_od, montaz_od)
    let (warsztat_od, warsztat_do, poczatek_skladania) = if zlecenie.czas_warsztatu_dni > 0 {
        let od = kal.wstecz(montaz_od, zlecenie.czas_warsztatu_dni);
        let do_ = kal.wstecz(montaz_od, 1).max(od);
        zadania.push(ZadanieWyliczone {
            data_do: Some(do_),
            ..ZadanieWyliczone::proste(
                "warsztat",
                TypZadania::Warsztat,
                format!("Składanie na warsztacie: {}", zlecenie.nazwa),
                od,
            )
        });
        (Some(od), Some(do_), od)
    } else {
        ostrzezenia.push(Ostrzezenie {
            kod: "brak_czasu_warsztatu",
            komunikat: "Czas składania na warsztacie wynosi 0 dni – materiały liczone od montażu"
                .into(),
            material_id: None,
        });
        (None, None, montaz_od)
    };

    // 3. Data wymaganej dostawy
    let wymagana = kal.wstecz(poczatek_skladania, odstep_materialy);

    // 4. Terminy zamówień
    let mut terminy = Vec::with_capacity(materialy.len());
    for m in materialy {
        let widelki = match m.widelki {
            Some(w) if w.min <= w.max => Some(w),
            Some(w) => {
                ostrzezenia.push(Ostrzezenie {
                    kod: "nieprawidlowe_widelki",
                    komunikat: format!(
                        "{}: minimalny czas dostawy ({}) większy niż maksymalny ({}) – popraw widełki",
                        m.kategoria_nazwa, w.min, w.max
                    ),
                    material_id: Some(m.id),
                });
                None
            }
            None => {
                ostrzezenia.push(Ostrzezenie {
                    kod: "brak_widelek",
                    komunikat: format!(
                        "{}: brak czasu dostawy (widełek) – uzupełnij w ustawieniach, inaczej termin zamówienia nie zostanie wyliczony",
                        m.kategoria_nazwa
                    ),
                    material_id: Some(m.id),
                });
                None
            }
        };
        let (sweet, planowa, krytyczna) = match widelki {
            Some(w) => (
                Some(kal.wstecz(wymagana, w.max)),
                Some(kal.wstecz(wymagana, w.srednia())),
                Some(kal.wstecz(wymagana, w.min)),
            ),
            None => (None, None, None),
        };
        let zamowione = m.data_zamowienia.is_some();
        let zablokowane = m.wiersze_do_potwierdzenia > 0 && !zamowione;
        let potwierdzic_do = if zablokowane { planowa } else { None };
        if zablokowane {
            ostrzezenia.push(Ostrzezenie {
                kod: "do_potwierdzenia",
                komunikat: format!(
                    "{}: {} poz. specyfikacji do potwierdzenia przez klienta blokuje zamówienie{}",
                    m.kategoria_nazwa,
                    m.wiersze_do_potwierdzenia,
                    potwierdzic_do
                        .map(|d| format!(" (potwierdzić do {d})"))
                        .unwrap_or_default()
                ),
                material_id: Some(m.id),
            });
        }

        let opis = m
            .opis
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .map(|s| format!(" – {s}"))
            .unwrap_or_default();
        if let (Some(s), Some(p), Some(k)) = (sweet, planowa, krytyczna) {
            zadania.push(ZadanieWyliczone {
                klucz: format!("zamowienie:{}", m.id),
                typ: TypZadania::Zamowienie,
                tytul: format!("Zamówić: {}{opis} ({})", m.kategoria_nazwa, zlecenie.nazwa),
                data_od: s,
                data_do: Some(k),
                data_sweet_spot: Some(s),
                data_planowa: Some(p),
                data_krytyczna: Some(k),
                zamowienie_id: Some(m.id),
                wykonano: m.data_zamowienia,
            });
            if let Some(termin) = potwierdzic_do {
                zadania.push(ZadanieWyliczone {
                    zamowienie_id: Some(m.id),
                    ..ZadanieWyliczone::proste(
                        format!("potwierdzenie:{}", m.id),
                        TypZadania::Klient,
                        format!(
                            "Klient potwierdza: {}{opis} ({})",
                            m.kategoria_nazwa, zlecenie.nazwa
                        ),
                        termin,
                    )
                });
            }
        }
        terminy.push(TerminyMaterialu {
            material_id: m.id,
            kategoria_id: m.kategoria_id,
            kategoria_nazwa: m.kategoria_nazwa.clone(),
            dostawca_id: m.dostawca_id,
            widelki,
            dni_srednia: widelki.map(Widelki::srednia),
            zrodlo_widelek: if widelki.is_some() {
                m.zrodlo_widelek
            } else {
                ZrodloWidelek::Brak
            },
            data_wymaganej_dostawy: wymagana,
            data_sweet_spot: sweet,
            data_planowa: planowa,
            data_krytyczna: krytyczna,
            zablokowane,
            potwierdzic_do,
            zamowione,
        });
    }

    // 5. Rysunki: przed najwcześniejszą datą planową (materiał, na który czeka się najdłużej)
    let najwczesniejsza_planowa = terminy.iter().filter_map(|t| t.data_planowa).min();
    let odniesienie_rysunkow = match najwczesniejsza_planowa {
        Some(d) => d,
        None => {
            ostrzezenia.push(Ostrzezenie {
                kod: "brak_materialow",
                komunikat: "Brak materiałów z czasem dostawy – termin rysunków liczony od daty wymaganej dostawy"
                    .into(),
                material_id: None,
            });
            wymagana
        }
    };
    let rysunki_do = kal.wstecz(odniesienie_rysunkow, odstep_rysunki);
    zadania.push(ZadanieWyliczone::proste(
        "rysunki",
        TypZadania::Rysunki,
        format!("Rysunki wykonawcze: {}", zlecenie.nazwa),
        rysunki_do,
    ));

    // 6. Pomiar końcowy
    let pomiar_do = kal.wstecz(rysunki_do, odstep_pomiar);
    zadania.push(ZadanieWyliczone::proste(
        "pomiar",
        TypZadania::Pomiar,
        format!("Pomiar końcowy: {}", zlecenie.nazwa),
        pomiar_do,
    ));

    // 7. Zadania pomocnicze
    zadania.push(ZadanieWyliczone::proste(
        "potwierdzenie_montazu",
        TypZadania::Klient,
        format!("Potwierdzić klientowi termin montażu: {}", zlecenie.nazwa),
        kal.wstecz(montaz_od, ustawienia.odstep_potwierdzenie_montazu_dni),
    ));
    zadania.push(ZadanieWyliczone::proste(
        "pomieszczenie_gotowe",
        TypZadania::Klient,
        format!("Pomieszczenie gotowe do montażu: {}", zlecenie.nazwa),
        kal.wstecz(montaz_od, ustawienia.odstep_pomieszczenie_gotowe_dni),
    ));
    if let (Some(data_umowy), Some(n)) = (zlecenie.data_umowy, zlecenie.dni_na_brakujace_informacje)
    {
        zadania.push(ZadanieWyliczone::proste(
            "brakujace_informacje",
            TypZadania::Klient,
            format!("Klient przekazuje brakujące informacje: {}", zlecenie.nazwa),
            kal.termin_od(data_umowy, n),
        ));
    }
    for p in &zlecenie.platnosci {
        zadania.push(ZadanieWyliczone::proste(
            format!("platnosc:{}", p.id),
            TypZadania::Platnosc,
            format!("Płatność: {} ({})", p.opis, zlecenie.nazwa),
            p.termin,
        ));
    }

    if let Some(data_umowy) = zlecenie.data_umowy {
        if pomiar_do < data_umowy {
            ostrzezenia.push(Ostrzezenie {
                kod: "harmonogram_przed_umowa",
                komunikat: format!(
                    "Termin pomiaru ({pomiar_do}) wypada przed datą umowy ({data_umowy}) – termin montażu może być nierealny"
                ),
                material_id: None,
            });
        }
    }

    zadania.sort_by(|a, b| {
        a.data_od
            .cmp(&b.data_od)
            .then_with(|| a.klucz.cmp(&b.klucz))
    });

    Ok(Harmonogram {
        zlecenie_id: zlecenie.id,
        liczenie_dni: kal.tryb(),
        montaz_od,
        montaz_do,
        warsztat_od,
        warsztat_do,
        data_wymaganej_dostawy: wymagana,
        rysunki_do,
        pomiar_do,
        materialy: terminy,
        zadania,
        ostrzezenia,
    })
}

#[cfg(test)]
mod testy {
    use super::*;

    fn d(r: i32, m: u32, dz: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(r, m, dz).unwrap()
    }

    fn zlecenie(montaz: NaiveDate, warsztat: u32) -> ZlecenieWejscie {
        ZlecenieWejscie {
            id: 1,
            nazwa: "Kuchnia testowa".into(),
            montaz_od: montaz,
            czas_montazu_dni: 3,
            czas_warsztatu_dni: warsztat,
            odstep_materialy_dni: None,
            odstep_rysunki_dni: None,
            odstep_pomiar_dni: None,
            data_umowy: None,
            dni_na_brakujace_informacje: None,
            platnosci: vec![],
        }
    }

    fn material(id: i64, nazwa: &str, min: u32, max: u32) -> MaterialWejscie {
        MaterialWejscie {
            id,
            kategoria_id: id,
            kategoria_nazwa: nazwa.into(),
            dostawca_id: None,
            opis: None,
            widelki: Some(Widelki { min, max }),
            zrodlo_widelek: ZrodloWidelek::Kategoria,
            data_zamowienia: None,
            data_dostawy: None,
            wiersze_do_potwierdzenia: 0,
        }
    }

    fn kalendarzowe() -> UstawieniaTerminow {
        UstawieniaTerminow {
            liczenie_dni: LiczenieDni::Kalendarzowe,
            ..Default::default()
        }
    }

    fn zadanie<'a>(h: &'a Harmonogram, klucz: &str) -> &'a ZadanieWyliczone {
        h.zadania.iter().find(|z| z.klucz == klucz).unwrap()
    }

    /// Przykład kontrolny od użytkownika (tryb kalendarzowy).
    #[test]
    fn przyklad_kontrolny_kalendarzowe() {
        let h = policz_harmonogram(
            &zlecenie(d(2027, 2, 8), 14),
            &[
                material(1, "Fronty MDF frezowane", 18, 24),
                material(2, "Płyta meblowa", 5, 10),
            ],
            &kalendarzowe(),
            &[],
        )
        .unwrap();
        assert_eq!(h.warsztat_od, Some(d(2027, 1, 25)));
        // 7.02 (niedziela) → cofnięte na pt. 5.02
        assert_eq!(h.warsztat_do, Some(d(2027, 2, 5)));
        assert_eq!(h.data_wymaganej_dostawy, d(2027, 1, 18));

        let mdf = &h.materialy[0];
        // 18.01 − 24 = 25.12.2026 (Boże Narodzenie) → 24.12 (Wigilia, wolna od 2025 r.)
        // → najbliższy wcześniejszy dzień roboczy: śr. 23.12.2026.
        // (W przykładzie użytkownika: 24.12 – różnica wynika wyłącznie z Wigilii jako dnia wolnego.)
        assert_eq!(mdf.data_sweet_spot, Some(d(2026, 12, 23)));
        assert_eq!(mdf.dni_srednia, Some(21));
        assert_eq!(mdf.data_planowa, Some(d(2026, 12, 28)));
        assert_eq!(mdf.data_krytyczna, Some(d(2026, 12, 31)));

        let plyta = &h.materialy[1];
        assert_eq!(plyta.data_sweet_spot, Some(d(2027, 1, 8)));
        // średnia 7,5 zaokrąglona w dół do 7 → 18.01 − 7 = 11.01
        assert_eq!(plyta.dni_srednia, Some(7));
        assert_eq!(plyta.data_planowa, Some(d(2027, 1, 11)));
        assert_eq!(plyta.data_krytyczna, Some(d(2027, 1, 13)));

        assert_eq!(h.rysunki_do, d(2026, 12, 21));
        assert_eq!(h.pomiar_do, d(2026, 12, 14));
        assert_eq!(h.montaz_do, d(2027, 2, 10));
        assert!(h.ostrzezenia.is_empty(), "{:?}", h.ostrzezenia);

        let z = zadanie(&h, "zamowienie:1");
        assert_eq!(z.typ, TypZadania::Zamowienie);
        assert_eq!(z.data_od, d(2026, 12, 23));
        assert_eq!(z.data_do, Some(d(2026, 12, 31)));
        assert_eq!(z.data_planowa, Some(d(2026, 12, 28)));
        assert_eq!(zadanie(&h, "potwierdzenie_montazu").data_od, d(2027, 2, 1));
        assert_eq!(zadanie(&h, "pomieszczenie_gotowe").data_od, d(2027, 2, 5));
        // zadania posortowane po dacie
        assert!(h.zadania.windows(2).all(|w| w[0].data_od <= w[1].data_od));
    }

    #[test]
    fn przyklad_kontrolny_robocze() {
        let h = policz_harmonogram(
            &zlecenie(d(2027, 2, 8), 14),
            &[
                material(1, "Fronty MDF frezowane", 18, 24),
                material(2, "Płyta meblowa", 5, 10),
            ],
            &UstawieniaTerminow::default(),
            &[],
        )
        .unwrap();
        // 14 dni roboczych przed pon. 8.02: 5,4,3,2,1.02, 29,28,27,26,25,22,21,20,19.01
        assert_eq!(h.warsztat_od, Some(d(2027, 1, 19)));
        assert_eq!(h.warsztat_do, Some(d(2027, 2, 5)));
        // 7 dni roboczych przed 19.01: 18,15,14,13,12,11,8.01
        assert_eq!(h.data_wymaganej_dostawy, d(2027, 1, 8));
        let plyta = &h.materialy[1];
        // 10 dni roboczych przed 8.01: 7,5,4.01 (6.01 święto), 31,30,29,28,23,22,21.12
        assert_eq!(plyta.data_sweet_spot, Some(d(2026, 12, 21)));
        // 7 dni roboczych: 7,5,4.01, 31,30,29,28.12
        assert_eq!(plyta.data_planowa, Some(d(2026, 12, 28)));
        // 5 dni roboczych: 7,5,4.01, 31,30.12
        assert_eq!(plyta.data_krytyczna, Some(d(2026, 12, 30)));
        // montaż 3 dni robocze: 8–10.02
        assert_eq!(h.montaz_do, d(2027, 2, 10));
        let mdf = &h.materialy[0];
        let kal = KalendarzDni::nowy(LiczenieDni::Robocze, []);
        assert_eq!(
            kal.roznica(mdf.data_planowa.unwrap(), h.data_wymaganej_dostawy),
            21
        );
        assert_eq!(h.rysunki_do, kal.wstecz(mdf.data_planowa.unwrap(), 7));
        assert_eq!(h.pomiar_do, kal.wstecz(h.rysunki_do, 7));
    }

    #[test]
    fn zlecenie_bez_materialow() {
        let h =
            policz_harmonogram(&zlecenie(d(2027, 2, 8), 14), &[], &kalendarzowe(), &[]).unwrap();
        assert!(h.materialy.is_empty());
        assert_eq!(h.rysunki_do, d(2027, 1, 11)); // 18.01 − 7
        assert_eq!(h.pomiar_do, d(2027, 1, 4));
        assert!(h.ostrzezenia.iter().any(|o| o.kod == "brak_materialow"));
        assert!(!h.zadania.iter().any(|z| z.typ == TypZadania::Zamowienie));
    }

    #[test]
    fn kategoria_bez_widelek_daje_ostrzezenie() {
        let mut bez = material(3, "Tapicerka", 0, 0);
        bez.widelki = None;
        bez.zrodlo_widelek = ZrodloWidelek::Brak;
        let mut zle = material(4, "Uchwyty", 9, 3);
        zle.zrodlo_widelek = ZrodloWidelek::KategoriaDostawca;
        let h = policz_harmonogram(
            &zlecenie(d(2027, 2, 8), 14),
            &[bez, zle, material(2, "Płyta meblowa", 5, 10)],
            &kalendarzowe(),
            &[],
        )
        .unwrap();
        assert_eq!(h.materialy[0].data_planowa, None);
        assert_eq!(h.materialy[1].zrodlo_widelek, ZrodloWidelek::Brak);
        assert!(h
            .ostrzezenia
            .iter()
            .any(|o| o.kod == "brak_widelek" && o.material_id == Some(3)));
        assert!(h
            .ostrzezenia
            .iter()
            .any(|o| o.kod == "nieprawidlowe_widelki" && o.material_id == Some(4)));
        // rysunki liczone od materiału z widełkami
        assert_eq!(h.rysunki_do, d(2027, 1, 4));
        assert_eq!(
            h.zadania
                .iter()
                .filter(|z| z.typ == TypZadania::Zamowienie)
                .count(),
            1
        );
    }

    #[test]
    fn min_rowne_max() {
        let h = policz_harmonogram(
            &zlecenie(d(2027, 2, 8), 14),
            &[material(1, "Okucia", 4, 4)],
            &kalendarzowe(),
            &[],
        )
        .unwrap();
        let m = &h.materialy[0];
        assert_eq!(m.data_sweet_spot, Some(d(2027, 1, 14)));
        assert_eq!(m.data_sweet_spot, m.data_planowa);
        assert_eq!(m.data_planowa, m.data_krytyczna);
    }

    #[test]
    fn widelki_zerowe_z_magazynu() {
        let h = policz_harmonogram(
            &zlecenie(d(2027, 2, 8), 14),
            &[material(1, "Okucia z magazynu", 0, 0)],
            &kalendarzowe(),
            &[],
        )
        .unwrap();
        assert_eq!(h.materialy[0].data_krytyczna, Some(d(2027, 1, 18)));
    }

    #[test]
    fn czas_montazu_zero_to_blad() {
        let mut z = zlecenie(d(2027, 2, 8), 14);
        z.czas_montazu_dni = 0;
        assert_eq!(
            policz_harmonogram(&z, &[], &kalendarzowe(), &[]),
            Err(BladHarmonogramu::CzasMontazuZero)
        );
    }

    #[test]
    fn bez_warsztatu_i_montaz_w_dzien_wolny() {
        let h = policz_harmonogram(&zlecenie(d(2027, 2, 6), 0), &[], &kalendarzowe(), &[]).unwrap();
        assert_eq!(h.warsztat_od, None);
        assert!(h
            .ostrzezenia
            .iter()
            .any(|o| o.kod == "montaz_w_dzien_wolny"));
        assert!(h
            .ostrzezenia
            .iter()
            .any(|o| o.kod == "brak_czasu_warsztatu"));
        assert_eq!(h.data_wymaganej_dostawy, d(2027, 1, 29)); // 6.02 − 7 = 30.01 (sob.) → 29.01
    }

    #[test]
    fn nadpisania_odstepow_i_dni_wolne_uzytkownika() {
        let mut z = zlecenie(d(2027, 2, 8), 14);
        z.odstep_materialy_dni = Some(3);
        z.odstep_rysunki_dni = Some(1);
        z.odstep_pomiar_dni = Some(2);
        // 22.01 (pt.) wolne u użytkownika
        let h = policz_harmonogram(&z, &[], &kalendarzowe(), &[d(2027, 1, 22)]).unwrap();
        assert_eq!(h.data_wymaganej_dostawy, d(2027, 1, 21)); // 25.01 − 3 = 22.01 (wolne) → 21.01
        assert_eq!(h.rysunki_do, d(2027, 1, 20));
        assert_eq!(h.pomiar_do, d(2027, 1, 18));
    }

    #[test]
    fn do_potwierdzenia_blokuje_zamowienie() {
        let mut m = material(1, "Fronty MDF frezowane", 18, 24);
        m.wiersze_do_potwierdzenia = 2;
        let h = policz_harmonogram(
            &zlecenie(d(2027, 2, 8), 14),
            &[m.clone()],
            &kalendarzowe(),
            &[],
        )
        .unwrap();
        assert!(h.materialy[0].zablokowane);
        assert_eq!(h.materialy[0].potwierdzic_do, Some(d(2026, 12, 28)));
        assert_eq!(zadanie(&h, "potwierdzenie:1").data_od, d(2026, 12, 28));
        assert!(h.ostrzezenia.iter().any(|o| o.kod == "do_potwierdzenia"));
        // po zamówieniu blokada nie ma znaczenia
        m.data_zamowienia = Some(d(2026, 12, 20));
        let h =
            policz_harmonogram(&zlecenie(d(2027, 2, 8), 14), &[m], &kalendarzowe(), &[]).unwrap();
        assert!(!h.materialy[0].zablokowane);
        assert_eq!(zadanie(&h, "zamowienie:1").wykonano, Some(d(2026, 12, 20)));
    }

    #[test]
    fn zadania_klienta_i_platnosci() {
        let mut z = zlecenie(d(2027, 2, 8), 14);
        z.data_umowy = Some(d(2026, 11, 30));
        z.dni_na_brakujace_informacje = Some(14);
        z.platnosci = vec![PlatnoscWejscie {
            id: 5,
            opis: "przed montażem".into(),
            termin: d(2027, 2, 1),
        }];
        let h = policz_harmonogram(&z, &[], &kalendarzowe(), &[]).unwrap();
        assert_eq!(zadanie(&h, "brakujace_informacje").data_od, d(2026, 12, 14));
        assert_eq!(zadanie(&h, "platnosc:5").typ, TypZadania::Platnosc);
        // pomiar 4.01.2027 jest po umowie – brak ostrzeżenia
        assert!(!h
            .ostrzezenia
            .iter()
            .any(|o| o.kod == "harmonogram_przed_umowa"));
        z.data_umowy = Some(d(2027, 1, 10));
        let h = policz_harmonogram(&z, &[], &kalendarzowe(), &[]).unwrap();
        assert!(h
            .ostrzezenia
            .iter()
            .any(|o| o.kod == "harmonogram_przed_umowa"));
    }

    /// Własność: sweet spot ≤ planowa ≤ krytyczna ≤ wymagana dostawa < warsztat ≤ montaż,
    /// pomiar < rysunki < najwcześniejsza planowa – dla wielu terminów i widełek, w obu trybach.
    #[test]
    fn wlasnosci_kolejnosci_terminow() {
        for tryb in [LiczenieDni::Robocze, LiczenieDni::Kalendarzowe] {
            let ust = UstawieniaTerminow {
                liczenie_dni: tryb,
                ..Default::default()
            };
            let mut montaz = d(2026, 6, 1);
            while montaz <= d(2028, 12, 31) {
                for (min, max) in [(0, 0), (1, 2), (5, 10), (18, 24), (3, 40), (7, 7)] {
                    let z = ZlecenieWejscie {
                        czas_warsztatu_dni: (min % 5) * 3,
                        ..zlecenie(montaz, 0)
                    };
                    let h =
                        policz_harmonogram(&z, &[material(1, "X", min, max)], &ust, &[]).unwrap();
                    let m = &h.materialy[0];
                    let (s, p, k) = (
                        m.data_sweet_spot.unwrap(),
                        m.data_planowa.unwrap(),
                        m.data_krytyczna.unwrap(),
                    );
                    assert!(
                        s <= p && p <= k && k <= h.data_wymaganej_dostawy,
                        "{montaz} {min}-{max}"
                    );
                    assert!(h.data_wymaganej_dostawy <= h.warsztat_od.unwrap_or(montaz));
                    assert!(h.pomiar_do < h.rysunki_do && h.rysunki_do < p);
                    assert!(h.montaz_do >= montaz);
                }
                montaz += chrono::Duration::days(3);
            }
        }
    }
    /// Wartości oczekiwane policzone ręcznie (niezależnie od kodu) – wykrywają jednolite
    /// przesunięcie o dzień, którego test kolejności nie zauważy.
    #[test]
    fn wartosci_oczekiwane_robocze_boze_cialo_2029() {
        let ust = UstawieniaTerminow::default(); // robocze, odstępy 7
        let z = ZlecenieWejscie {
            czas_montazu_dni: 2,
            ..zlecenie(d(2029, 6, 4), 5)
        };
        let h = policz_harmonogram(&z, &[material(1, "Płyta", 3, 6)], &ust, &[]).unwrap();
        // montaż pon. 4–5.06; warsztat 5 dni rob. wstecz (31.05 Boże Ciało): 1.06, 30, 29, 28, 25.05
        assert_eq!(h.montaz_do, d(2029, 6, 5));
        assert_eq!(h.warsztat_od, Some(d(2029, 5, 25)));
        assert_eq!(h.warsztat_do, Some(d(2029, 6, 1)));
        // dostawa: 7 dni rob. przed 25.05 → 16.05
        assert_eq!(h.data_wymaganej_dostawy, d(2029, 5, 16));
        let m = &h.materialy[0];
        assert_eq!(m.data_sweet_spot, Some(d(2029, 5, 8)));
        assert_eq!(m.data_planowa, Some(d(2029, 5, 10)));
        assert_eq!(m.data_krytyczna, Some(d(2029, 5, 11)));
        // rysunki: 7 dni rob. przed 10.05 (1 i 3.05 święta) → 27.04; pomiar → 18.04
        assert_eq!(h.rysunki_do, d(2029, 4, 27));
        assert_eq!(h.pomiar_do, d(2029, 4, 18));
        assert_eq!(zadanie(&h, "potwierdzenie_montazu").data_od, d(2029, 5, 23));
        assert_eq!(zadanie(&h, "pomieszczenie_gotowe").data_od, d(2029, 5, 29));
    }

    #[test]
    fn wartosci_oczekiwane_kalendarzowe_wielkanoc_2030() {
        let z = zlecenie(d(2030, 4, 23), 10);
        let h =
            policz_harmonogram(&z, &[material(1, "Płyta", 5, 10)], &kalendarzowe(), &[]).unwrap();
        // 23.04 − 10 = 13.04 (sob.) → 12.04; koniec warsztatu 22.04 (Pon. Wielkanocny) → 19.04
        assert_eq!(h.warsztat_od, Some(d(2030, 4, 12)));
        assert_eq!(h.warsztat_do, Some(d(2030, 4, 19)));
        assert_eq!(h.montaz_do, d(2030, 4, 25));
        assert_eq!(h.data_wymaganej_dostawy, d(2030, 4, 5));
        let m = &h.materialy[0];
        assert_eq!(m.data_sweet_spot, Some(d(2030, 3, 26)));
        assert_eq!(m.data_planowa, Some(d(2030, 3, 29)));
        // 5.04 − 5 = 31.03 (niedz.) → 29.03
        assert_eq!(m.data_krytyczna, Some(d(2030, 3, 29)));
        assert_eq!(h.rysunki_do, d(2030, 3, 22));
        assert_eq!(h.pomiar_do, d(2030, 3, 15));
        assert_eq!(zadanie(&h, "potwierdzenie_montazu").data_od, d(2030, 4, 16));
        // 23.04 − 3 = 20.04 (sob.) → 19.04
        assert_eq!(zadanie(&h, "pomieszczenie_gotowe").data_od, d(2030, 4, 19));
    }

    /// Kolizja montażu w trybie roboczym (montaż przeskakuje weekend), której nie ma
    /// w kalendarzowym.
    #[test]
    fn kolizje_w_trybie_roboczym() {
        use crate::kalendarz::{wykryj_kolizje, Kolizja, Okres};
        let okresy = |ust: &UstawieniaTerminow| {
            let a = ZlecenieWejscie {
                id: 1,
                czas_montazu_dni: 3,
                ..zlecenie(d(2027, 1, 7), 5)
            };
            let b = ZlecenieWejscie {
                id: 2,
                czas_montazu_dni: 1,
                ..zlecenie(d(2027, 1, 11), 2)
            };
            let mut v = Vec::new();
            for z in [a, b] {
                let h = policz_harmonogram(&z, &[], ust, &[]).unwrap();
                v.push(Okres {
                    zlecenie_id: z.id,
                    typ: TypZadania::Montaz,
                    od: h.montaz_od,
                    do_: h.montaz_do,
                });
                v.push(Okres {
                    zlecenie_id: z.id,
                    typ: TypZadania::Warsztat,
                    od: h.warsztat_od.unwrap(),
                    do_: h.warsztat_do.unwrap(),
                });
            }
            v
        };
        let rob = okresy(&UstawieniaTerminow::default());
        // A: montaż 7, 8, 11.01; warsztat 29.12–5.01 (1 i 6.01 święta); B: montaż 11.01, warsztat 7–8.01
        assert_eq!(rob[0].do_, d(2027, 1, 11));
        assert_eq!(rob[1].od, d(2026, 12, 29));
        assert_eq!(rob[1].do_, d(2027, 1, 5));
        assert_eq!(rob[3].od, d(2027, 1, 7));
        assert_eq!(rob[3].do_, d(2027, 1, 8));
        assert_eq!(
            wykryj_kolizje(&rob),
            vec![Kolizja {
                typ: TypZadania::Montaz,
                zlecenie_a: 1,
                zlecenie_b: 2,
                od: d(2027, 1, 11),
                do_: d(2027, 1, 11),
            }]
        );
        // kalendarzowo montaż A kończy się 9.01 – brak kolizji montażu
        assert!(wykryj_kolizje(&okresy(&kalendarzowe()))
            .iter()
            .all(|k| k.typ != TypZadania::Montaz));
    }
}
