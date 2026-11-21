//! Porównanie dwóch specyfikacji (np. wersji oferty, oferty i umowy, umowy i aneksu).
//!
//! Elementy dopasowujemy po `ident`, który przechodzi przez kopie specyfikacji.

use std::collections::{HashMap, HashSet};

use serde::Serialize;

use crate::formaty;
use crate::specyfikacje::{Czesc, Sekcja, Specyfikacja, Wiersz};

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ZmianaPola {
    pub pole: String,
    pub przed: Option<String>,
    pub po: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Roznica {
    /// `naglowek`, `wycena`, `sekcja`, `czesc`, `wiersz`, `zdjecie`
    pub poziom: String,
    /// `dodano`, `usunieto`, `zmieniono`, `przeniesiono`
    pub zmiana: String,
    pub sciezka: String,
    pub pola: Vec<ZmianaPola>,
}

pub fn pole(nazwa: &str, przed: Option<String>, po: Option<String>) -> Option<ZmianaPola> {
    (przed != po).then(|| ZmianaPola {
        pole: nazwa.into(),
        przed,
        po,
    })
}

fn etykieta_sekcji(s: &Sekcja) -> String {
    match &s.numer {
        Some(n) => format!("{n} {}", s.nazwa),
        None => s.nazwa.clone(),
    }
}

fn etykieta_wiersza(w: &Wiersz) -> String {
    let podgrupa = match w.podgrupa.as_str() {
        "wyposazenie" => "Wyposażenie › ",
        "frezowanie" => "Frezowanie › ",
        _ => "",
    };
    format!(
        "{podgrupa}{}",
        w.rodzaj.clone().unwrap_or_else(|| "(bez rodzaju)".into())
    )
}

pub fn status_wiersza(s: &str) -> &'static str {
    match s {
        "do_potwierdzenia" => "do potwierdzenia",
        "po_stronie_klienta" => "po stronie klienta",
        "poza_wycena" => "poza wyceną",
        _ => "ustalone",
    }
}

fn klucz(ident: &Option<String>, id: i64) -> String {
    ident.clone().unwrap_or_else(|| format!("#{id}"))
}

fn pola_sekcji(a: &Sekcja, b: &Sekcja) -> Vec<ZmianaPola> {
    [
        // Numer nie jest porównywany: wynika z kolejności i typu (przenumerowanie).
        pole("nazwa", Some(a.nazwa.clone()), Some(b.nazwa.clone())),
        pole("wariant", a.wariant.clone(), b.wariant.clone()),
        pole("typ", Some(a.typ.clone()), Some(b.typ.clone())),
        pole(
            "cena sekcji",
            a.cena_gr.map(formaty::kwota),
            b.cena_gr.map(formaty::kwota),
        ),
        pole(
            "stawka VAT",
            a.stawka_vat.map(|v| format!("{v}%")),
            b.stawka_vat.map(|v| format!("{v}%")),
        ),
    ]
    .into_iter()
    .flatten()
    .collect()
}

fn pola_czesci(a: &Czesc, b: &Czesc) -> Vec<ZmianaPola> {
    [
        pole("nazwa", Some(a.nazwa.clone()), Some(b.nazwa.clone())),
        pole(
            "cena części",
            a.cena_gr.map(formaty::kwota),
            b.cena_gr.map(formaty::kwota),
        ),
    ]
    .into_iter()
    .flatten()
    .collect()
}

fn pola_wiersza(a: &Wiersz, b: &Wiersz) -> Vec<ZmianaPola> {
    let il = |w: &Wiersz| {
        w.ilosc.map(|i| {
            format!(
                "{} {}",
                formaty::ilosc(i),
                w.jednostka.clone().unwrap_or_default()
            )
            .trim()
            .to_string()
        })
    };
    [
        pole("rodzaj", a.rodzaj.clone(), b.rodzaj.clone()),
        pole("materiał", a.material.clone(), b.material.clone()),
        pole("opis", a.opis.clone(), b.opis.clone()),
        pole("ilość", il(a), il(b)),
        pole(
            "status",
            Some(status_wiersza(&a.status).into()),
            Some(status_wiersza(&b.status).into()),
        ),
        pole(
            "podgrupa",
            Some(a.podgrupa.clone()),
            Some(b.podgrupa.clone()),
        ),
        pole(
            "cena",
            a.cena_gr.map(formaty::kwota),
            b.cena_gr.map(formaty::kwota),
        ),
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// Elementy, które faktycznie zmieniły kolejność względem pozostałych (dopełnienie
/// najdłuższego podciągu rosnącego). `pozycje[i]` = pozycja w starszej wersji i-tego elementu
/// w kolejności nowszej wersji. Przenumerowanie (np. po usunięciu sekcji) nie jest przesunięciem.
fn przesuniete(pozycje: &[usize]) -> Vec<bool> {
    let n = pozycje.len();
    if n == 0 {
        return vec![];
    }
    let mut dl = vec![1usize; n];
    let mut poprz = vec![usize::MAX; n];
    for i in 0..n {
        for j in 0..i {
            if pozycje[j] < pozycje[i] && dl[j] + 1 > dl[i] {
                dl[i] = dl[j] + 1;
                poprz[i] = j;
            }
        }
    }
    let mut koniec = 0;
    for i in 0..n {
        if dl[i] > dl[koniec] {
            koniec = i;
        }
    }
    let mut w_lis = vec![false; n];
    let mut i = koniec;
    loop {
        w_lis[i] = true;
        if poprz[i] == usize::MAX {
            break;
        }
        i = poprz[i];
    }
    w_lis.into_iter().map(|b| !b).collect()
}

struct ElementSekcji<'a> {
    klucz: String,
    etykieta: String,
    s: &'a Sekcja,
}

struct ElementCzesci<'a> {
    klucz: String,
    rodzic: String,
    etykieta: String,
    c: &'a Czesc,
}

struct ElementWiersza<'a> {
    klucz: String,
    rodzic: String,
    etykieta_rodzica: String,
    etykieta: String,
    w: &'a Wiersz,
}

struct Plaskie<'a> {
    sekcje: Vec<ElementSekcji<'a>>,
    czesci: Vec<ElementCzesci<'a>>,
    wiersze: Vec<ElementWiersza<'a>>,
}

fn splaszcz(s: &Specyfikacja) -> Plaskie<'_> {
    let mut p = Plaskie {
        sekcje: vec![],
        czesci: vec![],
        wiersze: vec![],
    };
    for sek in &s.sekcje {
        let es = etykieta_sekcji(sek);
        let ks = klucz(&sek.ident, sek.id);
        p.sekcje.push(ElementSekcji {
            klucz: ks.clone(),
            etykieta: es.clone(),
            s: sek,
        });
        for c in &sek.czesci {
            let ec = format!("{es} › {}", c.nazwa);
            let kc = klucz(&c.ident, c.id);
            p.czesci.push(ElementCzesci {
                klucz: kc.clone(),
                rodzic: ks.clone(),
                etykieta: ec.clone(),
                c,
            });
            for w in &c.wiersze {
                p.wiersze.push(ElementWiersza {
                    klucz: klucz(&w.ident, w.id),
                    rodzic: format!("{kc}/{}", w.podgrupa),
                    etykieta_rodzica: ec.clone(),
                    etykieta: format!("{ec} › {}", etykieta_wiersza(w)),
                    w,
                });
            }
        }
    }
    p
}

/// Przesunięcia w obrębie rodzica: dla elementów obecnych w obu wersjach i w tym samym rodzicu.
fn przesuniecia(a: &[(String, String)], b: &[(String, String)]) -> HashSet<String> {
    // (klucz, rodzic) w kolejności drzewa
    let mut wynik = HashSet::new();
    let rodzice: HashSet<&String> = b.iter().map(|(_, r)| r).collect();
    for r in rodzice {
        let w_a: Vec<&String> = a.iter().filter(|(_, x)| x == r).map(|(k, _)| k).collect();
        let w_b: Vec<&String> = b.iter().filter(|(_, x)| x == r).map(|(k, _)| k).collect();
        let wspolne: Vec<&String> = w_b.iter().copied().filter(|k| w_a.contains(k)).collect();
        let pozycje: Vec<usize> = wspolne
            .iter()
            .map(|k| w_a.iter().position(|x| x == k).unwrap_or(0))
            .collect();
        for (k, ruch) in wspolne.iter().zip(przesuniete(&pozycje)) {
            if ruch {
                wynik.insert((*k).clone());
            }
        }
    }
    wynik
}

fn opis_wiersza(w: &Wiersz) -> Vec<ZmianaPola> {
    [
        ("materiał", w.material.clone()),
        ("opis", w.opis.clone()),
        (
            "ilość",
            w.ilosc.map(|i| {
                format!(
                    "{} {}",
                    formaty::ilosc(i),
                    w.jednostka.clone().unwrap_or_default()
                )
                .trim()
                .to_string()
            }),
        ),
    ]
    .into_iter()
    .filter_map(|(n, v)| {
        v.map(|v| ZmianaPola {
            pole: n.into(),
            przed: None,
            po: Some(v),
        })
    })
    .collect()
}

fn polozenie_zdjecia(
    s: &Specyfikacja,
    sekcja: Option<i64>,
    czesc: Option<i64>,
) -> (String, String) {
    for sek in &s.sekcje {
        if let Some(cid) = czesc {
            if let Some(c) = sek.czesci.iter().find(|c| c.id == cid) {
                return (
                    format!("c:{}", klucz(&c.ident, c.id)),
                    format!("{} › {}", etykieta_sekcji(sek), c.nazwa),
                );
            }
        } else if sekcja == Some(sek.id) {
            return (
                format!("s:{}", klucz(&sek.ident, sek.id)),
                etykieta_sekcji(sek),
            );
        }
    }
    ("oferta".into(), "Cała oferta".into())
}

/// Różnice zdjęć: dodane, usunięte, przeniesione i ze zmienionym podpisem (po pliku).
fn roznice_zdjec(a: &Specyfikacja, b: &Specyfikacja) -> Vec<Roznica> {
    let mut wynik = Vec::new();
    let mut pozostale_a: Vec<&crate::specyfikacje::Zdjecie> = a.zdjecia.iter().collect();
    for zb in &b.zdjecia {
        let (kb, eb) = polozenie_zdjecia(b, zb.sekcja_id, zb.czesc_id);
        // najpierw to samo zdjęcie w tym samym miejscu, potem w dowolnym
        let idx = pozostale_a
            .iter()
            .position(|za| {
                za.plik_id == zb.plik_id && polozenie_zdjecia(a, za.sekcja_id, za.czesc_id).0 == kb
            })
            .or_else(|| pozostale_a.iter().position(|za| za.plik_id == zb.plik_id));
        match idx {
            None => wynik.push(Roznica {
                poziom: "zdjecie".into(),
                zmiana: "dodano".into(),
                sciezka: format!("{eb} › {}", zb.nazwa),
                pola: zb
                    .podpis
                    .clone()
                    .map(|p| {
                        vec![ZmianaPola {
                            pole: "podpis".into(),
                            przed: None,
                            po: Some(p),
                        }]
                    })
                    .unwrap_or_default(),
            }),
            Some(i) => {
                let za = pozostale_a.remove(i);
                let (ka, ea) = polozenie_zdjecia(a, za.sekcja_id, za.czesc_id);
                let mut pola: Vec<ZmianaPola> =
                    pole("podpis", za.podpis.clone(), zb.podpis.clone())
                        .into_iter()
                        .collect();
                let przeniesione = ka != kb;
                if przeniesione {
                    pola.push(ZmianaPola {
                        pole: "przypięte do".into(),
                        przed: Some(ea),
                        po: Some(eb.clone()),
                    });
                }
                if !pola.is_empty() {
                    wynik.push(Roznica {
                        poziom: "zdjecie".into(),
                        zmiana: if przeniesione && pola.len() == 1 {
                            "przeniesiono"
                        } else {
                            "zmieniono"
                        }
                        .into(),
                        sciezka: format!("{eb} › {}", zb.nazwa),
                        pola,
                    });
                }
            }
        }
    }
    for za in pozostale_a {
        let (_, ea) = polozenie_zdjecia(a, za.sekcja_id, za.czesc_id);
        wynik.push(Roznica {
            poziom: "zdjecie".into(),
            zmiana: "usunieto".into(),
            sciezka: format!("{ea} › {}", za.nazwa),
            pola: vec![],
        });
    }
    wynik
}

fn kolejnosc_pole() -> ZmianaPola {
    ZmianaPola {
        pole: "kolejność".into(),
        przed: Some("wcześniej".into()),
        po: Some("przesunięto".into()),
    }
}

/// `zmieniono`, a gdy jedyną zmianą jest położenie – `przeniesiono`.
fn rodzaj_zmiany(pola: &[ZmianaPola]) -> &'static str {
    if pola.iter().all(|p| {
        matches!(
            p.pole.as_str(),
            "kolejność" | "sekcja" | "część" | "przypięte do"
        )
    }) {
        "przeniesiono"
    } else {
        "zmieniono"
    }
}

/// Różnice od specyfikacji `a` (starszej) do `b` (nowszej), w kolejności drzewa.
///
/// Numer sekcji i położenie wynikające tylko z przenumerowania (np. po usunięciu sekcji)
/// nie są różnicą; przesunięcie elementu względem pozostałych i przeniesienie do innego
/// rodzica są zgłaszane jako `przeniesiono`.
pub fn porownaj_specyfikacje(a: &Specyfikacja, b: &Specyfikacja) -> Vec<Roznica> {
    let pa = splaszcz(a);
    let pb = splaszcz(b);
    let mut wynik = Vec::new();

    let sek_a: HashMap<&str, &ElementSekcji> =
        pa.sekcje.iter().map(|e| (e.klucz.as_str(), e)).collect();
    let sek_b: HashMap<&str, &ElementSekcji> =
        pb.sekcje.iter().map(|e| (e.klucz.as_str(), e)).collect();
    let cz_a: HashMap<&str, &ElementCzesci> =
        pa.czesci.iter().map(|e| (e.klucz.as_str(), e)).collect();
    let cz_b: HashMap<&str, &ElementCzesci> =
        pb.czesci.iter().map(|e| (e.klucz.as_str(), e)).collect();
    let w_a: HashMap<&str, &ElementWiersza> =
        pa.wiersze.iter().map(|e| (e.klucz.as_str(), e)).collect();
    let w_b: HashMap<&str, &ElementWiersza> =
        pb.wiersze.iter().map(|e| (e.klucz.as_str(), e)).collect();

    let ruch_sekcji = przesuniecia(
        &pa.sekcje
            .iter()
            .map(|e| (e.klucz.clone(), String::new()))
            .collect::<Vec<_>>(),
        &pb.sekcje
            .iter()
            .map(|e| (e.klucz.clone(), String::new()))
            .collect::<Vec<_>>(),
    );
    let ruch_czesci = przesuniecia(
        &pa.czesci
            .iter()
            .map(|e| (e.klucz.clone(), e.rodzic.clone()))
            .collect::<Vec<_>>(),
        &pb.czesci
            .iter()
            .map(|e| (e.klucz.clone(), e.rodzic.clone()))
            .collect::<Vec<_>>(),
    );
    let ruch_wierszy = przesuniecia(
        &pa.wiersze
            .iter()
            .map(|e| (e.klucz.clone(), e.rodzic.clone()))
            .collect::<Vec<_>>(),
        &pb.wiersze
            .iter()
            .map(|e| (e.klucz.clone(), e.rodzic.clone()))
            .collect::<Vec<_>>(),
    );

    let mut dodaj = |poziom: &str, zmiana: &str, sciezka: String, pola: Vec<ZmianaPola>| {
        wynik.push(Roznica {
            poziom: poziom.into(),
            zmiana: zmiana.into(),
            sciezka,
            pola,
        })
    };

    for e in &pb.sekcje {
        match sek_a.get(e.klucz.as_str()) {
            None => dodaj("sekcja", "dodano", e.etykieta.clone(), vec![]),
            Some(stara) => {
                let mut pola = pola_sekcji(stara.s, e.s);
                if ruch_sekcji.contains(&e.klucz) {
                    pola.push(kolejnosc_pole());
                }
                if !pola.is_empty() {
                    let z = rodzaj_zmiany(&pola);
                    dodaj("sekcja", z, e.etykieta.clone(), pola);
                }
            }
        }
    }
    for e in &pa.sekcje {
        if !sek_b.contains_key(e.klucz.as_str()) {
            dodaj("sekcja", "usunieto", e.etykieta.clone(), vec![]);
        }
    }
    for e in &pb.czesci {
        match cz_a.get(e.klucz.as_str()) {
            None => dodaj("czesc", "dodano", e.etykieta.clone(), vec![]),
            Some(stara) => {
                let mut pola = pola_czesci(stara.c, e.c);
                if stara.rodzic != e.rodzic {
                    let przed = sek_a.get(stara.rodzic.as_str()).map(|s| s.etykieta.clone());
                    let po = sek_b.get(e.rodzic.as_str()).map(|s| s.etykieta.clone());
                    pola.push(ZmianaPola {
                        pole: "sekcja".into(),
                        przed,
                        po,
                    });
                } else if ruch_czesci.contains(&e.klucz) {
                    pola.push(kolejnosc_pole());
                }
                if !pola.is_empty() {
                    let z = rodzaj_zmiany(&pola);
                    dodaj("czesc", z, e.etykieta.clone(), pola);
                }
            }
        }
    }
    for e in &pa.czesci {
        if !cz_b.contains_key(e.klucz.as_str()) {
            dodaj("czesc", "usunieto", e.etykieta.clone(), vec![]);
        }
    }
    for e in &pb.wiersze {
        match w_a.get(e.klucz.as_str()) {
            None => dodaj("wiersz", "dodano", e.etykieta.clone(), opis_wiersza(e.w)),
            Some(stary) => {
                let mut pola = pola_wiersza(stary.w, e.w);
                let czesc_a = stary.rodzic.split('/').next().unwrap_or_default();
                let czesc_b = e.rodzic.split('/').next().unwrap_or_default();
                if czesc_a != czesc_b {
                    pola.push(ZmianaPola {
                        pole: "część".into(),
                        przed: Some(stary.etykieta_rodzica.clone()),
                        po: Some(e.etykieta_rodzica.clone()),
                    });
                } else if stary.rodzic == e.rodzic && ruch_wierszy.contains(&e.klucz) {
                    pola.push(kolejnosc_pole());
                }
                if !pola.is_empty() {
                    let z = rodzaj_zmiany(&pola);
                    dodaj("wiersz", z, e.etykieta.clone(), pola);
                }
            }
        }
    }
    for e in &pa.wiersze {
        if !w_b.contains_key(e.klucz.as_str()) {
            dodaj("wiersz", "usunieto", e.etykieta.clone(), opis_wiersza(e.w));
        }
    }
    wynik.extend(roznice_zdjec(a, b));
    wynik
}

#[cfg(test)]
mod testy {
    use super::*;

    #[test]
    fn przesuniete_to_dopelnienie_lis() {
        assert_eq!(przesuniete(&[0, 1, 2]), vec![false, false, false]);
        // A: [1,2,3,4], B: [2,3,4,1] → przesunięto tylko 1
        assert_eq!(przesuniete(&[1, 2, 3, 0]), vec![false, false, false, true]);
        assert_eq!(przesuniete(&[]), Vec::<bool>::new());
    }
}
