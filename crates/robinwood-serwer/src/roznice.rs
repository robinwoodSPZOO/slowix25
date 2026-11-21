//! Porównanie dwóch specyfikacji (np. wersji oferty, oferty i umowy, umowy i aneksu).
//!
//! Elementy dopasowujemy po `ident`, który przechodzi przez kopie specyfikacji.

use std::collections::HashMap;

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
    /// `naglowek`, `wycena`, `sekcja`, `czesc`, `wiersz`
    pub poziom: String,
    /// `dodano`, `usunieto`, `zmieniono`
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
        pole("nazwa", Some(a.nazwa.clone()), Some(b.nazwa.clone())),
        pole("numer", a.numer.clone(), b.numer.clone()),
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

struct Plaskie<'a> {
    sekcje: Vec<(String, &'a Sekcja)>,
    czesci: Vec<(String, String, &'a Czesc)>,
    wiersze: Vec<(String, String, &'a Wiersz)>,
}

fn splaszcz(s: &Specyfikacja) -> Plaskie<'_> {
    let mut p = Plaskie {
        sekcje: vec![],
        czesci: vec![],
        wiersze: vec![],
    };
    for sek in &s.sekcje {
        let es = etykieta_sekcji(sek);
        p.sekcje.push((klucz(&sek.ident, sek.id), sek));
        for c in &sek.czesci {
            let ec = format!("{es} › {}", c.nazwa);
            p.czesci.push((klucz(&c.ident, c.id), ec.clone(), c));
            for w in &c.wiersze {
                p.wiersze.push((
                    klucz(&w.ident, w.id),
                    format!("{ec} › {}", etykieta_wiersza(w)),
                    w,
                ));
            }
        }
    }
    p
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

/// Różnice od specyfikacji `a` (starszej) do `b` (nowszej), w kolejności drzewa.
pub fn porownaj_specyfikacje(a: &Specyfikacja, b: &Specyfikacja) -> Vec<Roznica> {
    let pa = splaszcz(a);
    let pb = splaszcz(b);
    let mut wynik = Vec::new();

    let sek_a: HashMap<&str, &Sekcja> = pa.sekcje.iter().map(|(k, s)| (k.as_str(), *s)).collect();
    let sek_b: HashMap<&str, &Sekcja> = pb.sekcje.iter().map(|(k, s)| (k.as_str(), *s)).collect();
    let cz_a: HashMap<&str, (&str, &Czesc)> = pa
        .czesci
        .iter()
        .map(|(k, e, c)| (k.as_str(), (e.as_str(), *c)))
        .collect();
    let cz_b: HashMap<&str, (&str, &Czesc)> = pb
        .czesci
        .iter()
        .map(|(k, e, c)| (k.as_str(), (e.as_str(), *c)))
        .collect();
    let w_a: HashMap<&str, (&str, &Wiersz)> = pa
        .wiersze
        .iter()
        .map(|(k, e, w)| (k.as_str(), (e.as_str(), *w)))
        .collect();
    let w_b: HashMap<&str, (&str, &Wiersz)> = pb
        .wiersze
        .iter()
        .map(|(k, e, w)| (k.as_str(), (e.as_str(), *w)))
        .collect();

    let mut dodaj = |poziom: &str, zmiana: &str, sciezka: String, pola: Vec<ZmianaPola>| {
        wynik.push(Roznica {
            poziom: poziom.into(),
            zmiana: zmiana.into(),
            sciezka,
            pola,
        })
    };

    for (k, s) in &pb.sekcje {
        match sek_a.get(k.as_str()) {
            None => dodaj("sekcja", "dodano", etykieta_sekcji(s), vec![]),
            Some(stara) => {
                let pola = pola_sekcji(stara, s);
                if !pola.is_empty() {
                    dodaj("sekcja", "zmieniono", etykieta_sekcji(s), pola);
                }
            }
        }
    }
    for (k, s) in &pa.sekcje {
        if !sek_b.contains_key(k.as_str()) {
            dodaj("sekcja", "usunieto", etykieta_sekcji(s), vec![]);
        }
    }
    for (k, e, c) in &pb.czesci {
        match cz_a.get(k.as_str()) {
            None => dodaj("czesc", "dodano", e.clone(), vec![]),
            Some((stara_e, stara)) => {
                let mut pola = pola_czesci(stara, c);
                if *stara_e != e.as_str() && stara.nazwa == c.nazwa {
                    pola.extend(pole(
                        "położenie",
                        Some(stara_e.to_string()),
                        Some(e.clone()),
                    ));
                }
                if !pola.is_empty() {
                    dodaj("czesc", "zmieniono", e.clone(), pola);
                }
            }
        }
    }
    for (k, e, _) in &pa.czesci {
        if !cz_b.contains_key(k.as_str()) {
            dodaj("czesc", "usunieto", e.clone(), vec![]);
        }
    }
    for (k, e, w) in &pb.wiersze {
        match w_a.get(k.as_str()) {
            None => dodaj("wiersz", "dodano", e.clone(), opis_wiersza(w)),
            Some((_, stary)) => {
                let pola = pola_wiersza(stary, w);
                if !pola.is_empty() {
                    dodaj("wiersz", "zmieniono", e.clone(), pola);
                }
            }
        }
    }
    for (k, e, w) in &pa.wiersze {
        if !w_b.contains_key(k.as_str()) {
            dodaj("wiersz", "usunieto", e.clone(), opis_wiersza(w));
        }
    }
    wynik
}
