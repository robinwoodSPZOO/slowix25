//! Wycena oferty – czysta funkcja bez bazy danych, testowana jednostkowo.
//!
//! Zasady (decyzje opisane w docs/ARCHITEKTURA.md):
//! * Kwoty w groszach (`i64`), zaokrąglenie „połówki od zera” (0,5 gr → 1 gr).
//! * Cenę mebli wpisuje się na jednym poziomie wybranym w ofercie: część (moduł), sekcja
//!   (pomieszczenie) albo całość. Ceny z innych poziomów są ignorowane.
//! * „Suma za meble” sekcji = suma cen części (poziom „część”) albo cena sekcji (poziom „sekcja”).
//! * Rabat za całość (procentowy, w setnych procenta) liczony jest od sumy za meble PRZED
//!   rabatem za termin; oba rabaty odejmowane są od sumy za meble. Wynik nie spada poniżej zera.
//! * Sprzęt AGD (sekcje typu `agd`) liczony osobno: suma pozycji cena × ilość, bez rabatów,
//!   ze stawką VAT sekcji (gdy ustawiona) albo stawką oferty. Pozycje „po stronie klienta”
//!   i „poza wyceną” nie wchodzą do sumy.
//! * Tryb cen „brutto” (domyślny): wpisane kwoty są brutto, netto = brutto·100/(100+VAT).
//!   Tryb „netto”: VAT = netto·stawka/100 doliczany do kwoty.
//! * Marża (dane wewnętrzne) = meble netto po rabatach − koszt własny; AGD nie wchodzi do marży.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PoziomWyceny {
    Czesc,
    Sekcja,
    Calosc,
}

impl PoziomWyceny {
    pub fn z_tekstu(s: &str) -> Option<Self> {
        match s {
            "czesc" => Some(Self::Czesc),
            "sekcja" => Some(Self::Sekcja),
            "calosc" => Some(Self::Calosc),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrybCen {
    Brutto,
    Netto,
}

impl TrybCen {
    pub fn z_tekstu(s: &str) -> Option<Self> {
        match s {
            "brutto" => Some(Self::Brutto),
            "netto" => Some(Self::Netto),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ParametryWyceny {
    pub poziom: PoziomWyceny,
    pub cena_calosc_gr: Option<i64>,
    pub rabat_termin_gr: i64,
    pub rabat_calosc_bp: i64,
    pub stawka_vat: i64,
    pub tryb_cen: TrybCen,
    pub koszt_wlasny_gr: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct CzescDoWyceny {
    pub id: i64,
    pub nazwa: String,
    pub cena_gr: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct PozycjaAgd {
    pub cena_gr: Option<i64>,
    pub ilosc: Option<f64>,
    /// Status wiersza; „po_stronie_klienta” i „poza_wycena” nie są liczone.
    pub status: String,
}

#[derive(Debug, Clone)]
pub struct SekcjaDoWyceny {
    pub id: i64,
    pub numer: Option<String>,
    pub nazwa: String,
    pub agd: bool,
    pub cena_gr: Option<i64>,
    pub stawka_vat: Option<i64>,
    pub czesci: Vec<CzescDoWyceny>,
    pub pozycje_agd: Vec<PozycjaAgd>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct WycenaCzesci {
    pub czesc_id: i64,
    pub nazwa: String,
    pub cena_gr: Option<i64>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct WycenaSekcji {
    pub sekcja_id: i64,
    pub numer: Option<String>,
    pub nazwa: String,
    pub agd: bool,
    /// „Suma za meble” (albo suma AGD); `None` przy wycenie całościowej.
    pub suma_gr: Option<i64>,
    pub stawka_vat: i64,
    pub czesci: Vec<WycenaCzesci>,
    /// Brakuje ceny wymaganej na wybranym poziomie.
    pub brak_ceny: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct KwotyVat {
    pub stawka: i64,
    pub netto_gr: i64,
    pub vat_gr: i64,
    pub brutto_gr: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct DaneWewnetrzne {
    pub koszt_wlasny_gr: Option<i64>,
    pub marza_gr: Option<i64>,
    /// Marża w setnych procenta przychodu netto mebli (np. 2500 = 25%).
    pub marza_bp: Option<i64>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Wycena {
    pub poziom: PoziomWyceny,
    pub tryb_cen: TrybCen,
    pub sekcje: Vec<WycenaSekcji>,
    /// Suma za meble przed rabatami (w trybie wpisywania cen).
    pub suma_meble_gr: i64,
    pub rabat_termin_gr: i64,
    pub rabat_calosc_bp: i64,
    pub rabat_calosc_gr: i64,
    pub meble_po_rabatach_gr: i64,
    pub meble: KwotyVat,
    /// Suma AGD w trybie wpisywania cen.
    pub suma_agd_gr: i64,
    /// Rozbicie AGD wg stawek VAT (zwykle jedna).
    pub agd: Vec<KwotyVat>,
    pub suma_netto_gr: i64,
    pub suma_vat_gr: i64,
    pub suma_brutto_gr: i64,
    pub wewnetrzne: DaneWewnetrzne,
    pub ostrzezenia: Vec<String>,
}

/// Dzielenie całkowite z zaokrągleniem połówek od zera.
pub fn dziel_zaokraglij(licznik: i128, mianownik: i128) -> i64 {
    assert!(mianownik != 0, "dzielenie przez zero");
    let (l, m) = if mianownik < 0 {
        (-licznik, -mianownik)
    } else {
        (licznik, mianownik)
    };
    let wynik = if l >= 0 {
        (2 * l + m) / (2 * m)
    } else {
        -((2 * -l + m) / (2 * m))
    };
    wynik as i64
}

/// Kwota × ilość (ilość może być ułamkowa, np. 2,5 mb), zaokrąglona do grosza.
pub fn razy_ilosc(cena_gr: i64, ilosc: f64) -> i64 {
    // Ilość z bazy ma najwyżej kilka miejsc po przecinku – liczymy w tysięcznych, żeby uniknąć
    // błędów zmiennoprzecinkowych przy zaokrąglaniu połówek.
    let tysieczne = (ilosc * 1000.0).round() as i128;
    dziel_zaokraglij(cena_gr as i128 * tysieczne, 1000)
}

/// Rozbija kwotę na netto/VAT/brutto zależnie od trybu wpisywania cen.
pub fn kwoty_vat(kwota_gr: i64, stawka: i64, tryb: TrybCen) -> KwotyVat {
    match tryb {
        TrybCen::Brutto => {
            let netto = dziel_zaokraglij(kwota_gr as i128 * 100, 100 + stawka as i128);
            KwotyVat {
                stawka,
                netto_gr: netto,
                vat_gr: kwota_gr - netto,
                brutto_gr: kwota_gr,
            }
        }
        TrybCen::Netto => {
            let vat = dziel_zaokraglij(kwota_gr as i128 * stawka as i128, 100);
            KwotyVat {
                stawka,
                netto_gr: kwota_gr,
                vat_gr: vat,
                brutto_gr: kwota_gr + vat,
            }
        }
    }
}

fn etykieta(s: &SekcjaDoWyceny) -> String {
    match &s.numer {
        Some(n) => format!("{n} {}", s.nazwa),
        None => s.nazwa.clone(),
    }
}

/// Główna funkcja wyceny.
pub fn wylicz(p: &ParametryWyceny, sekcje: &[SekcjaDoWyceny]) -> Wycena {
    let mut ostrzezenia = Vec::new();
    let mut wynik_sekcji = Vec::with_capacity(sekcje.len());
    let mut suma_meble: i64 = 0;
    let mut agd_wg_stawek: Vec<(i64, i64)> = Vec::new();
    let mut suma_agd: i64 = 0;

    for s in sekcje {
        let stawka = s.stawka_vat.unwrap_or(p.stawka_vat);
        let czesci: Vec<WycenaCzesci> = s
            .czesci
            .iter()
            .map(|c| WycenaCzesci {
                czesc_id: c.id,
                nazwa: c.nazwa.clone(),
                cena_gr: if p.poziom == PoziomWyceny::Czesc && !s.agd {
                    c.cena_gr
                } else {
                    None
                },
            })
            .collect();

        if s.agd {
            let mut suma = 0i64;
            let mut brak = false;
            for poz in &s.pozycje_agd {
                if poz.status == "po_stronie_klienta" || poz.status == "poza_wycena" {
                    continue;
                }
                match poz.cena_gr {
                    Some(c) => suma += razy_ilosc(c, poz.ilosc.unwrap_or(1.0)),
                    None => brak = true,
                }
            }
            if brak {
                ostrzezenia.push(format!("{}: pozycje AGD bez ceny", etykieta(s)));
            }
            suma_agd += suma;
            match agd_wg_stawek.iter_mut().find(|(st, _)| *st == stawka) {
                Some((_, k)) => *k += suma,
                None => agd_wg_stawek.push((stawka, suma)),
            }
            wynik_sekcji.push(WycenaSekcji {
                sekcja_id: s.id,
                numer: s.numer.clone(),
                nazwa: s.nazwa.clone(),
                agd: true,
                suma_gr: Some(suma),
                stawka_vat: stawka,
                czesci,
                brak_ceny: brak,
            });
            continue;
        }

        let (suma, brak) = match p.poziom {
            PoziomWyceny::Czesc => {
                let brak = s.czesci.iter().any(|c| c.cena_gr.is_none());
                let suma: i64 = s.czesci.iter().filter_map(|c| c.cena_gr).sum();
                (Some(suma), brak || s.czesci.is_empty())
            }
            PoziomWyceny::Sekcja => (Some(s.cena_gr.unwrap_or(0)), s.cena_gr.is_none()),
            PoziomWyceny::Calosc => (None, false),
        };
        if brak {
            ostrzezenia.push(match p.poziom {
                PoziomWyceny::Czesc => format!("{}: nie wszystkie części mają cenę", etykieta(s)),
                _ => format!("{}: brak ceny sekcji", etykieta(s)),
            });
        }
        if let Some(k) = suma {
            suma_meble += k;
        }
        wynik_sekcji.push(WycenaSekcji {
            sekcja_id: s.id,
            numer: s.numer.clone(),
            nazwa: s.nazwa.clone(),
            agd: false,
            suma_gr: suma,
            stawka_vat: p.stawka_vat,
            czesci,
            brak_ceny: brak,
        });
    }

    if p.poziom == PoziomWyceny::Calosc {
        match p.cena_calosc_gr {
            Some(c) => suma_meble = c,
            None => ostrzezenia.push("Brak ceny za całość".into()),
        }
    }

    let rabat_calosc_gr = dziel_zaokraglij(suma_meble as i128 * p.rabat_calosc_bp as i128, 10_000);
    let mut po_rabatach = suma_meble - p.rabat_termin_gr - rabat_calosc_gr;
    if po_rabatach < 0 {
        ostrzezenia.push("Rabaty przekraczają sumę za meble – przyjęto 0 zł".into());
        po_rabatach = 0;
    }

    let meble = kwoty_vat(po_rabatach, p.stawka_vat, p.tryb_cen);
    agd_wg_stawek.sort_by_key(|(s, _)| *s);
    let agd: Vec<KwotyVat> = agd_wg_stawek
        .into_iter()
        .map(|(s, k)| kwoty_vat(k, s, p.tryb_cen))
        .collect();

    let suma_netto = meble.netto_gr + agd.iter().map(|k| k.netto_gr).sum::<i64>();
    let suma_vat = meble.vat_gr + agd.iter().map(|k| k.vat_gr).sum::<i64>();
    let suma_brutto = meble.brutto_gr + agd.iter().map(|k| k.brutto_gr).sum::<i64>();

    let wewnetrzne = match p.koszt_wlasny_gr {
        Some(koszt) => {
            let marza = meble.netto_gr - koszt;
            DaneWewnetrzne {
                koszt_wlasny_gr: Some(koszt),
                marza_gr: Some(marza),
                marza_bp: (meble.netto_gr != 0)
                    .then(|| dziel_zaokraglij(marza as i128 * 10_000, meble.netto_gr as i128)),
            }
        }
        None => DaneWewnetrzne {
            koszt_wlasny_gr: None,
            marza_gr: None,
            marza_bp: None,
        },
    };

    Wycena {
        poziom: p.poziom,
        tryb_cen: p.tryb_cen,
        sekcje: wynik_sekcji,
        suma_meble_gr: suma_meble,
        rabat_termin_gr: p.rabat_termin_gr,
        rabat_calosc_bp: p.rabat_calosc_bp,
        rabat_calosc_gr,
        meble_po_rabatach_gr: po_rabatach,
        meble,
        suma_agd_gr: suma_agd,
        agd,
        suma_netto_gr: suma_netto,
        suma_vat_gr: suma_vat,
        suma_brutto_gr: suma_brutto,
        wewnetrzne,
        ostrzezenia,
    }
}

#[cfg(test)]
mod testy {
    use super::*;

    fn parametry(poziom: PoziomWyceny) -> ParametryWyceny {
        ParametryWyceny {
            poziom,
            cena_calosc_gr: None,
            rabat_termin_gr: 0,
            rabat_calosc_bp: 0,
            stawka_vat: 23,
            tryb_cen: TrybCen::Brutto,
            koszt_wlasny_gr: None,
        }
    }

    fn sekcja(id: i64, cena: Option<i64>, czesci: &[Option<i64>]) -> SekcjaDoWyceny {
        SekcjaDoWyceny {
            id,
            numer: Some(format!("{id:02}")),
            nazwa: format!("Sekcja {id}"),
            agd: false,
            cena_gr: cena,
            stawka_vat: None,
            czesci: czesci
                .iter()
                .enumerate()
                .map(|(i, c)| CzescDoWyceny {
                    id: id * 100 + i as i64,
                    nazwa: format!("Część {i}"),
                    cena_gr: *c,
                })
                .collect(),
            pozycje_agd: vec![],
        }
    }

    fn agd(id: i64, stawka: Option<i64>, pozycje: &[(Option<i64>, f64, &str)]) -> SekcjaDoWyceny {
        SekcjaDoWyceny {
            id,
            numer: None,
            nazwa: "Sprzęt AGD".into(),
            agd: true,
            cena_gr: None,
            stawka_vat: stawka,
            czesci: vec![],
            pozycje_agd: pozycje
                .iter()
                .map(|(c, i, s)| PozycjaAgd {
                    cena_gr: *c,
                    ilosc: Some(*i),
                    status: s.to_string(),
                })
                .collect(),
        }
    }

    #[test]
    fn zaokraglanie_polowek_od_zera() {
        assert_eq!(dziel_zaokraglij(5, 10), 1);
        assert_eq!(dziel_zaokraglij(4, 10), 0);
        assert_eq!(dziel_zaokraglij(15, 10), 2);
        assert_eq!(dziel_zaokraglij(-5, 10), -1);
        assert_eq!(dziel_zaokraglij(-4, 10), 0);
        assert_eq!(dziel_zaokraglij(7, -2), -4);
        assert_eq!(razy_ilosc(333, 1.5), 500); // 499,5 → 500
        assert_eq!(razy_ilosc(1000, 0.333), 333);
        assert_eq!(razy_ilosc(12345, 2.0), 24690);
    }

    #[test]
    fn vat_z_brutto_i_z_netto() {
        let k = kwoty_vat(12_300, 23, TrybCen::Brutto);
        assert_eq!((k.netto_gr, k.vat_gr, k.brutto_gr), (10_000, 2_300, 12_300));
        let k = kwoty_vat(10_800, 8, TrybCen::Brutto);
        assert_eq!((k.netto_gr, k.vat_gr), (10_000, 800));
        // 1 gr brutto przy 23%: netto 0,81 → 1 gr, VAT 0
        let k = kwoty_vat(1, 23, TrybCen::Brutto);
        assert_eq!((k.netto_gr, k.vat_gr), (1, 0));
        // 100,01 zł brutto przy 23% → netto 81,31 (8130,89 zaokr. 8131)
        let k = kwoty_vat(10_001, 23, TrybCen::Brutto);
        assert_eq!((k.netto_gr, k.vat_gr), (8_131, 1_870));
        let k = kwoty_vat(10_000, 23, TrybCen::Netto);
        assert_eq!((k.vat_gr, k.brutto_gr), (2_300, 12_300));
        // VAT 8% od 0,07 zł = 0,0056 → 1 gr
        let k = kwoty_vat(7, 8, TrybCen::Netto);
        assert_eq!(k.vat_gr, 1);
    }

    #[test]
    fn wycena_na_poziomie_czesci() {
        let p = parametry(PoziomWyceny::Czesc);
        let w = wylicz(
            &p,
            &[
                sekcja(1, Some(999_999), &[Some(1_000_000), Some(550_050)]),
                sekcja(2, None, &[Some(200_000)]),
            ],
        );
        // cena sekcji ignorowana na poziomie części
        assert_eq!(w.sekcje[0].suma_gr, Some(1_550_050));
        assert_eq!(w.sekcje[1].suma_gr, Some(200_000));
        assert_eq!(w.suma_meble_gr, 1_750_050);
        assert_eq!(w.suma_brutto_gr, 1_750_050);
        assert!(w.ostrzezenia.is_empty());
        assert_eq!(w.sekcje[0].czesci[1].cena_gr, Some(550_050));
    }

    #[test]
    fn brak_ceny_czesci_daje_ostrzezenie() {
        let w = wylicz(
            &parametry(PoziomWyceny::Czesc),
            &[sekcja(1, None, &[Some(100), None])],
        );
        assert_eq!(w.sekcje[0].suma_gr, Some(100));
        assert!(w.sekcje[0].brak_ceny);
        assert_eq!(w.ostrzezenia.len(), 1);
    }

    #[test]
    fn wycena_na_poziomie_sekcji_i_calosci() {
        let s = [
            sekcja(1, Some(3_000_000), &[Some(1)]),
            sekcja(2, Some(1_000_000), &[]),
        ];
        let w = wylicz(&parametry(PoziomWyceny::Sekcja), &s);
        assert_eq!(w.suma_meble_gr, 4_000_000);
        assert_eq!(w.sekcje[0].czesci[0].cena_gr, None);

        let mut p = parametry(PoziomWyceny::Calosc);
        p.cena_calosc_gr = Some(3_500_000);
        let w = wylicz(&p, &s);
        assert_eq!(w.suma_meble_gr, 3_500_000);
        assert_eq!(w.sekcje[0].suma_gr, None);

        p.cena_calosc_gr = None;
        let w = wylicz(&p, &s);
        assert_eq!(w.suma_meble_gr, 0);
        assert_eq!(w.ostrzezenia, vec!["Brak ceny za całość".to_string()]);
    }

    #[test]
    fn rabaty_kwotowy_i_procentowy() {
        let mut p = parametry(PoziomWyceny::Sekcja);
        p.rabat_termin_gr = 50_000; // 500 zł
        p.rabat_calosc_bp = 500; // 5%
        let w = wylicz(&p, &[sekcja(1, Some(2_000_000), &[])]);
        // 5% od 20 000 zł = 1 000 zł (liczone przed rabatem kwotowym)
        assert_eq!(w.rabat_calosc_gr, 100_000);
        assert_eq!(w.meble_po_rabatach_gr, 2_000_000 - 50_000 - 100_000);
        assert_eq!(w.suma_brutto_gr, 1_850_000);

        // zaokrąglenie rabatu procentowego: 12,5% od 0,99 zł = 0,12375 → 12 gr
        p.rabat_termin_gr = 0;
        p.rabat_calosc_bp = 1250;
        let w = wylicz(&p, &[sekcja(1, Some(99), &[])]);
        assert_eq!(w.rabat_calosc_gr, 12);
        // 2,5% od 0,20 zł = 0,005 → 1 gr (połówka w górę)
        p.rabat_calosc_bp = 250;
        let w = wylicz(&p, &[sekcja(1, Some(20), &[])]);
        assert_eq!(w.rabat_calosc_gr, 1);
    }

    #[test]
    fn rabat_wiekszy_niz_suma() {
        let mut p = parametry(PoziomWyceny::Sekcja);
        p.rabat_termin_gr = 10_000;
        let w = wylicz(&p, &[sekcja(1, Some(5_000), &[])]);
        assert_eq!(w.meble_po_rabatach_gr, 0);
        assert_eq!(w.suma_brutto_gr, 0);
        assert_eq!(w.ostrzezenia.len(), 1);
    }

    #[test]
    fn agd_liczone_osobno_bez_rabatow() {
        let mut p = parametry(PoziomWyceny::Sekcja);
        p.stawka_vat = 8;
        p.rabat_calosc_bp = 1000; // 10% – nie dotyczy AGD
        let w = wylicz(
            &p,
            &[
                sekcja(1, Some(1_080_000), &[]),
                agd(
                    9,
                    Some(23),
                    &[
                        (Some(246_000), 1.0, "ustalone"),
                        (Some(12_300), 2.0, "ustalone"),
                        (Some(500_000), 1.0, "po_stronie_klienta"),
                        (Some(100_000), 1.0, "poza_wycena"),
                    ],
                ),
            ],
        );
        assert_eq!(w.suma_meble_gr, 1_080_000);
        assert_eq!(w.rabat_calosc_gr, 108_000);
        assert_eq!(w.meble.brutto_gr, 972_000);
        assert_eq!(w.meble.netto_gr, 900_000);
        assert_eq!(w.meble.vat_gr, 72_000);
        assert_eq!(w.suma_agd_gr, 270_600);
        assert_eq!(w.agd.len(), 1);
        assert_eq!(w.agd[0].stawka, 23);
        assert_eq!(w.agd[0].netto_gr, 220_000);
        assert_eq!(w.suma_brutto_gr, 972_000 + 270_600);
        assert_eq!(w.suma_netto_gr, 900_000 + 220_000);
        assert_eq!(w.suma_vat_gr, 72_000 + 50_600);
        assert_eq!(w.sekcje[1].suma_gr, Some(270_600));
        assert_eq!(w.sekcje[1].stawka_vat, 23);
    }

    #[test]
    fn agd_bez_stawki_bierze_stawke_oferty_i_tryb_netto() {
        let mut p = parametry(PoziomWyceny::Sekcja);
        p.tryb_cen = TrybCen::Netto;
        p.stawka_vat = 8;
        let w = wylicz(
            &p,
            &[
                sekcja(1, Some(100_000), &[]),
                agd(
                    2,
                    None,
                    &[(Some(10_000), 1.0, "ustalone"), (None, 1.0, "ustalone")],
                ),
            ],
        );
        assert_eq!(w.meble.brutto_gr, 108_000);
        assert_eq!(w.agd[0].stawka, 8);
        assert_eq!(w.agd[0].brutto_gr, 10_800);
        assert_eq!(w.suma_brutto_gr, 118_800);
        assert!(w.sekcje[1].brak_ceny);
    }

    #[test]
    fn marza_z_kosztu_wlasnego() {
        let mut p = parametry(PoziomWyceny::Sekcja);
        p.koszt_wlasny_gr = Some(600_000);
        let w = wylicz(&p, &[sekcja(1, Some(1_230_000), &[])]);
        // netto 10 000 zł, koszt 6 000 zł → marża 4 000 zł = 40%
        assert_eq!(w.wewnetrzne.marza_gr, Some(400_000));
        assert_eq!(w.wewnetrzne.marza_bp, Some(4000));

        p.koszt_wlasny_gr = None;
        let w = wylicz(&p, &[sekcja(1, Some(1_230_000), &[])]);
        assert_eq!(w.wewnetrzne.marza_gr, None);

        p.koszt_wlasny_gr = Some(100);
        let w = wylicz(&p, &[]);
        assert_eq!(w.wewnetrzne.marza_bp, None);
        assert_eq!(w.wewnetrzne.marza_gr, Some(-100));
    }
}
