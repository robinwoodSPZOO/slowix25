//! Silnik szablonów DOCX: wypełnianie pól w nawiasach kwadratowych w pliku Word użytkownika.
//!
//! Szablon to zwykły dokument Word (`.docx`, także `.docm` – makra są usuwane przy
//! generowaniu) z polami zapisanymi w tekście:
//!
//! * `[nazwa pola]` – pole tekstowe (np. `[numer umowy]`, `[wartość]`); wielkość liter,
//!   polskie znaki i wielokrotne spacje nie mają znaczenia (`[Wartosc]` = `[wartość]`);
//! * `[specyfikacja]`, `[harmonogram płatności]`, `[harmonogram etapów]` … – pola blokowe:
//!   akapit zawierający samo pole jest zastępowany tabelą / listą akapitów;
//! * `[[nazwa]] … [[/nazwa]]` – blok warunkowy (np. `[[załącznik 3]]`, `[[wariant B]]`):
//!   treść między znacznikami zostaje albo jest usuwana; akapity z samymi znacznikami znikają.
//!
//! Word często dzieli tekst jednego pola na kilka fragmentów (`<w:r>`/`<w:t>`), np. po
//! poprawce pisowni albo zmianie formatowania. Silnik najpierw scala każde pole do pierwszego
//! fragmentu (w obrębie akapitu), a dopiero potem je zamienia – formatowanie pola przejmuje
//! pierwszy znak. Pola są szukane w treści dokumentu, nagłówkach, stopkach i przypisach.
//! Nieznane pola zostają w dokumencie bez zmian i trafiają do raportu (nie są błędem).

use std::collections::BTreeMap;
use std::io::{Cursor, Read, Write};

use serde::Serialize;

use crate::bledy::{BladApi, Wynik};
use crate::docx_xml;

/// Napis wstawiany w miejsce pola bez wartości (do uzupełnienia ręcznie).
pub const PUSTE_POLE: &str = "………………";

#[derive(Debug, Clone, Default)]
pub struct DaneSzablonu {
    /// Klucz znormalizowany ([`normalizuj`]) → wartość. Pusty napis = pole bez wartości.
    pub pola: BTreeMap<String, String>,
    /// Klucz → gotowy XML elementów blokowych (akapity, tabele) – patrz [`xml`].
    pub bloki: BTreeMap<String, String>,
    /// Klucz bloku warunkowego → czy dołączyć treść.
    pub warunki: BTreeMap<String, bool>,
    /// Inne nazwy pól (np. znacznik `w:tag` kontrolki treści) → klucz kanoniczny.
    pub aliasy: BTreeMap<String, String>,
    /// Pola celowo zostawiane w dokumencie bez zmian (np. `[uzupełnij]` do wpisania ręcznie).
    pub pomijane: Vec<String>,
}

impl DaneSzablonu {
    pub fn pole(&mut self, nazwa: &str, wartosc: impl Into<String>) {
        self.pola.insert(normalizuj(nazwa), wartosc.into());
    }
    pub fn blok(&mut self, nazwa: &str, xml: String) {
        self.bloki.insert(normalizuj(nazwa), xml);
    }
    pub fn warunek(&mut self, nazwa: &str, tak: bool) {
        self.warunki.insert(normalizuj(nazwa), tak);
    }
    pub fn alias(&mut self, inna_nazwa: &str, nazwa: &str) {
        self.aliasy
            .insert(normalizuj(inna_nazwa), normalizuj(nazwa));
    }
    pub fn pomin(&mut self, nazwa: &str) {
        self.pomijane.push(normalizuj(nazwa));
    }
    /// Klucz kanoniczny dla nazwy z szablonu (z uwzględnieniem aliasów).
    pub fn klucz(&self, nazwa: &str) -> String {
        let n = normalizuj(nazwa);
        self.aliasy.get(&n).cloned().unwrap_or(n)
    }
}

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct RaportSzablonu {
    /// Pola znalezione w szablonie (nazwy jak w szablonie, bez powtórzeń).
    pub pola: Vec<String>,
    /// Pola, których program nie zna (zostają w dokumencie bez zmian).
    pub nieznane: Vec<String>,
    /// Pola bez wartości (w dokumencie: „………”).
    pub puste: Vec<String>,
    /// Bloki warunkowe znalezione w szablonie.
    pub bloki_warunkowe: Vec<String>,
    /// Pola zostawione celowo do ręcznego uzupełnienia (np. `[uzupełnij]`) i ich liczba.
    pub do_uzupelnienia: usize,
    /// Kontrolki treści Worda rozpoznane po nazwie lub znaczniku i zastąpione wartością.
    pub kontrolki: usize,
    pub ostrzezenia: Vec<String>,
    /// Błędy szablonu, przy których dokument NIE powstanie (np. niezamknięty blok, blok
    /// przecinający tabelę, kontrolka obejmująca wiersz tabeli) – z opisem miejsca.
    pub bledy: Vec<String>,
    /// Pola wymagane w tym rodzaju dokumentu, których szablon nie zawiera (np. `[specyfikacja]`).
    pub brak_wymaganych: Vec<String>,
}

fn dodaj_raz(v: &mut Vec<String>, s: &str) {
    if !v.iter().any(|x| x == s) {
        v.push(s.to_string());
    }
}

/// Nazwa pola bez znaczenia wielkości liter, polskich znaków i odstępów:
/// „ Wartość  słownie” → „wartosc slownie”.
pub fn normalizuj(nazwa: &str) -> String {
    let mut s = String::with_capacity(nazwa.len());
    let mut spacja = false;
    for c in nazwa.chars().flat_map(char::to_lowercase) {
        let c = match c {
            'ą' => 'a',
            'ć' => 'c',
            'ę' => 'e',
            'ł' => 'l',
            'ń' => 'n',
            'ó' => 'o',
            'ś' => 's',
            'ź' | 'ż' => 'z',
            '\u{a0}' | '\t' | '\n' | '\u{2009}' | '\u{202f}' => ' ',
            '–' | '—' => '-',
            c => c,
        };
        if c == ' ' {
            spacja = true;
            continue;
        }
        if spacja && !s.is_empty() {
            s.push(' ');
        }
        spacja = false;
        s.push(c);
    }
    s
}

// ---------------------------------------------------------------------------
// XML pomocniczo
// ---------------------------------------------------------------------------

/// Budowanie fragmentów WordprocessingML (akapity, tabele) wstawianych w miejsce pól blokowych.
/// Nawias `[` w treści jest kodowany jako `&#91;`, żeby wstawiony tekst nigdy nie był brany
/// za pole szablonu.
pub mod xml {
    pub fn escape(t: &str) -> String {
        let mut s = String::with_capacity(t.len());
        for c in t.chars() {
            match c {
                '&' => s.push_str("&amp;"),
                '<' => s.push_str("&lt;"),
                '>' => s.push_str("&gt;"),
                '"' => s.push_str("&quot;"),
                '[' => s.push_str("&#91;"),
                ']' => s.push_str("&#93;"),
                // znaki niedozwolone w XML 1.0 (sterujące, U+FFFE, U+FFFF) są pomijane
                c if !crate::docx_xml::znak_dozwolony(c) || c == '\r' => {}
                c => s.push(c),
            }
        }
        s
    }

    /// Przebieg tekstu (`w:r`) z opcjonalnym pogrubieniem i rozmiarem (w połówkach punktu).
    pub fn przebieg(t: &str, pogrub: bool, rozmiar: Option<u32>) -> String {
        let mut rpr = String::new();
        if pogrub {
            rpr.push_str("<w:b/>");
        }
        if let Some(r) = rozmiar {
            rpr.push_str(&format!("<w:sz w:val=\"{r}\"/><w:szCs w:val=\"{r}\"/>"));
        }
        let rpr = if rpr.is_empty() {
            String::new()
        } else {
            format!("<w:rPr>{rpr}</w:rPr>")
        };
        let linie: Vec<String> = t
            .split('\n')
            .map(|l| format!("<w:t xml:space=\"preserve\">{}</w:t>", escape(l)))
            .collect();
        format!("<w:r>{rpr}{}</w:r>", linie.join("<w:br/>"))
    }

    /// Akapit; `wyrownanie`: `left`/`center`/`right`/`both`; `odstep_po` w twipach.
    pub fn akapit(
        przebiegi: &str,
        wyrownanie: Option<&str>,
        odstep_po: Option<u32>,
        z_nastepnym: bool,
    ) -> String {
        let mut ppr = String::new();
        if z_nastepnym {
            ppr.push_str("<w:keepNext/>");
        }
        if let Some(o) = odstep_po {
            ppr.push_str(&format!("<w:spacing w:before=\"0\" w:after=\"{o}\"/>"));
        }
        if let Some(w) = wyrownanie {
            ppr.push_str(&format!("<w:jc w:val=\"{w}\"/>"));
        }
        let ppr = if ppr.is_empty() {
            String::new()
        } else {
            format!("<w:pPr>{ppr}</w:pPr>")
        };
        format!("<w:p>{ppr}{przebiegi}</w:p>")
    }

    pub fn akapit_tekst(t: &str, pogrub: bool) -> String {
        akapit(&przebieg(t, pogrub, None), None, Some(80), pogrub)
    }

    /// Komórka tabeli.
    #[derive(Clone, Debug, Default)]
    pub struct Komorka {
        pub tekst: String,
        pub pogrub: bool,
        pub tlo: Option<&'static str>,
        pub scal: u32,
        pub wyrownanie: Option<&'static str>,
    }

    impl Komorka {
        pub fn t(t: impl Into<String>) -> Self {
            Self {
                tekst: t.into(),
                scal: 1,
                ..Default::default()
            }
        }
        pub fn naglowek(t: impl Into<String>) -> Self {
            Self {
                tekst: t.into(),
                pogrub: true,
                tlo: Some("EDEAE4"),
                scal: 1,
                wyrownanie: None,
            }
        }
        pub fn pogrubiona(mut self) -> Self {
            self.pogrub = true;
            self
        }
        pub fn do_prawej(mut self) -> Self {
            self.wyrownanie = Some("right");
            self
        }
        pub fn scalona(mut self, n: u32) -> Self {
            self.scal = n.max(1);
            self
        }
        pub fn tlo(mut self, t: &'static str) -> Self {
            self.tlo = Some(t);
            self
        }
    }

    /// Tabela z obramowaniem; `szerokosci` w twipach (suma ≈ 9600 dla A4 z marginesami 2 cm).
    /// Wiersze z `naglowek = true` powtarzają się na kolejnych stronach; wiersz nie dzieli się
    /// między strony. Rozmiar tekstu 9 pt.
    pub fn tabela(szerokosci: &[u32], wiersze: &[(bool, Vec<Komorka>)]) -> String {
        let suma: u32 = szerokosci.iter().sum();
        let mut s = format!(
            "<w:tbl><w:tblPr><w:tblW w:w=\"{suma}\" w:type=\"dxa\"/><w:tblBorders>\
             <w:top w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"8C8C8C\"/>\
             <w:left w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"8C8C8C\"/>\
             <w:bottom w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"8C8C8C\"/>\
             <w:right w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"8C8C8C\"/>\
             <w:insideH w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"8C8C8C\"/>\
             <w:insideV w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"8C8C8C\"/>\
             </w:tblBorders><w:tblLayout w:type=\"fixed\"/><w:tblCellMar>\
             <w:left w:w=\"80\" w:type=\"dxa\"/><w:right w:w=\"80\" w:type=\"dxa\"/>\
             </w:tblCellMar><w:tblLook w:val=\"04A0\" w:firstRow=\"1\" w:lastRow=\"0\" w:firstColumn=\"0\" w:lastColumn=\"0\" w:noHBand=\"0\" w:noVBand=\"1\"/></w:tblPr><w:tblGrid>"
        );
        for w in szerokosci {
            s.push_str(&format!("<w:gridCol w:w=\"{w}\"/>"));
        }
        s.push_str("</w:tblGrid>");
        for (naglowek, komorki) in wiersze {
            s.push_str("<w:tr><w:trPr><w:cantSplit/>");
            if *naglowek {
                s.push_str("<w:tblHeader/>");
            }
            s.push_str("</w:trPr>");
            let mut kol = 0usize;
            for k in komorki {
                let n = k.scal.max(1) as usize;
                let szer: u32 = szerokosci.iter().skip(kol).take(n).sum();
                kol += n;
                s.push_str(&format!(
                    "<w:tc><w:tcPr><w:tcW w:w=\"{szer}\" w:type=\"dxa\"/>"
                ));
                if n > 1 {
                    s.push_str(&format!("<w:gridSpan w:val=\"{n}\"/>"));
                }
                if let Some(t) = k.tlo {
                    s.push_str(&format!(
                        "<w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"{t}\"/>"
                    ));
                }
                s.push_str("</w:tcPr>");
                s.push_str(&akapit(
                    &przebieg(&k.tekst, k.pogrub, Some(18)),
                    k.wyrownanie,
                    Some(0),
                    false,
                ));
                s.push_str("</w:tc>");
            }
            s.push_str("</w:tr>");
        }
        s.push_str("</w:tbl>");
        s
    }

    /// Unescape encji XML w nazwie pola.
    pub fn unescape(t: &str) -> String {
        t.replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&apos;", "'")
            .replace("&#91;", "[")
            .replace("&#93;", "]")
            .replace("&amp;", "&")
    }
}

// ---------------------------------------------------------------------------
// Węzły tekstu w części XML
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
struct WezelT {
    /// Początek znacznika otwierającego `<w:t …>`.
    otwarcie: usize,
    /// Treść: `[od, do)`.
    od: usize,
    do_: usize,
    akapit: usize,
}

fn wezly_tekstu(x: &str) -> Vec<WezelT> {
    let b = x.as_bytes();
    let mut wynik = Vec::new();
    let mut akapit = 0usize;
    let mut i = 0;
    while i < b.len() {
        if b[i] != b'<' {
            i += 1;
            continue;
        }
        let reszta = &x[i..];
        if reszta.starts_with("</w:p>") {
            akapit += 1;
            i += 6;
            continue;
        }
        if reszta.starts_with("<w:t") {
            let nast = b.get(i + 4).copied();
            if matches!(nast, Some(b'>') | Some(b' ')) {
                let Some(kon) = reszta.find('>') else { break };
                if reszta.as_bytes()[kon - 1] == b'/' {
                    i += kon + 1;
                    continue;
                }
                let od = i + kon + 1;
                let Some(z) = x[od..].find("</w:t>") else {
                    break;
                };
                wynik.push(WezelT {
                    otwarcie: i,
                    od,
                    do_: od + z,
                    akapit,
                });
                i = od + z + 6;
                continue;
            }
        }
        i += 1;
    }
    wynik
}

/// Zakresy pól w tekście: `[...]` i `[[...]]` (bajty, koniec wyłączny).
fn zakresy_pol(c: &str) -> Vec<(usize, usize)> {
    let b = c.as_bytes();
    let mut wynik = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] != b'[' {
            i += 1;
            continue;
        }
        let podwojny = b.get(i + 1) == Some(&b'[');
        let start_nazwy = if podwojny { i + 2 } else { i + 1 };
        let mut j = start_nazwy;
        let mut koniec = None;
        while j < b.len() && j - start_nazwy <= 300 {
            if b[j] == b'[' {
                break;
            }
            if b[j] == b']' {
                if podwojny {
                    if b.get(j + 1) == Some(&b']') {
                        koniec = Some(j + 2);
                    }
                } else {
                    koniec = Some(j + 1);
                }
                break;
            }
            j += 1;
        }
        match koniec {
            Some(k) if k > start_nazwy + 1 => {
                wynik.push((i, k));
                i = k;
            }
            _ => {
                i = if j > i + 1 && j < b.len() && b[j] == b'[' {
                    j
                } else {
                    i + 1
                }
            }
        }
    }
    wynik
}

/// Ustawia `xml:space="preserve"` w znaczniku `<w:t …>`.
fn otwarcie_preserve(tag: &str) -> String {
    if tag.contains("xml:space") {
        tag.to_string()
    } else {
        "<w:t xml:space=\"preserve\">".to_string()
    }
}

/// Krok 1: każde pole rozbite na kilka `w:t` w akapicie trafia w całości do pierwszego z nich.
fn scal_pola(x: &str) -> String {
    let wezly = wezly_tekstu(x);
    // (zakres w xml, nowy tekst)
    let mut zamiany: Vec<(usize, usize, String)> = Vec::new();
    let mut i = 0;
    while i < wezly.len() {
        let mut j = i;
        while j < wezly.len() && wezly[j].akapit == wezly[i].akapit {
            j += 1;
        }
        let grupa = &wezly[i..j];
        i = j;
        if grupa.len() < 2 {
            continue;
        }
        let mut c = String::new();
        let mut poczatki = Vec::with_capacity(grupa.len());
        for w in grupa {
            poczatki.push(c.len());
            c.push_str(&x[w.od..w.do_]);
        }
        let wezel_dla = |pos: usize| -> usize {
            match poczatki.binary_search(&pos) {
                Ok(mut k) => {
                    // puste węzły mają ten sam początek – bierzemy ostatni z nich
                    while k + 1 < poczatki.len() && poczatki[k + 1] == pos {
                        k += 1;
                    }
                    k
                }
                Err(k) => k - 1,
            }
        };
        let mut wlasciciel: Vec<usize> = (0..c.len()).map(wezel_dla).collect();
        let mut zmienione = false;
        for (s, e) in zakresy_pol(&c) {
            let a = wlasciciel[s];
            let z = wlasciciel[e - 1];
            if a != z {
                zmienione = true;
                for w in wlasciciel.iter_mut().take(e).skip(s) {
                    *w = a;
                }
            }
        }
        if !zmienione {
            continue;
        }
        for (k, w) in grupa.iter().enumerate() {
            let nowy: String = {
                let mut od = None;
                let mut do_ = 0;
                for (p, &wl) in wlasciciel.iter().enumerate() {
                    if wl == k {
                        if od.is_none() {
                            od = Some(p);
                        }
                        do_ = p + 1;
                    }
                }
                od.map(|o| c[o..do_].to_string()).unwrap_or_default()
            };
            if nowy != x[w.od..w.do_] {
                let tag = otwarcie_preserve(&x[w.otwarcie..w.od]);
                zamiany.push((w.otwarcie, w.do_, format!("{tag}{nowy}")));
            }
        }
    }
    zastosuj(x, zamiany)
}

fn zastosuj(x: &str, mut zamiany: Vec<(usize, usize, String)>) -> String {
    if zamiany.is_empty() {
        return x.to_string();
    }
    zamiany.sort_by_key(|z| z.0);
    let mut s = String::with_capacity(x.len());
    let mut poz = 0;
    for (od, do_, t) in zamiany {
        s.push_str(&x[poz..od]);
        s.push_str(&t);
        poz = do_;
    }
    s.push_str(&x[poz..]);
    s
}

/// Zakres akapitu `<w:p …>…</w:p>` zawierającego pozycję.
fn akapit_wokol(x: &str, pos: usize) -> Option<(usize, usize)> {
    let przed = &x[..pos];
    let a = przed.rfind("<w:p>");
    let b = przed.rfind("<w:p ");
    let start = match (a, b) {
        (Some(a), Some(b)) => a.max(b),
        (Some(a), None) => a,
        (None, Some(b)) => b,
        (None, None) => return None,
    };
    let koniec = x[pos..].find("</w:p>")? + pos + 6;
    Some((start, koniec))
}

/// Tekst akapitu (zawartość wszystkich `w:t`).
fn tekst_zakresu(x: &str, od: usize, do_: usize) -> String {
    wezly_tekstu(&x[od..do_])
        .iter()
        .map(|w| &x[od + w.od..od + w.do_])
        .collect()
}

fn zrownowazony(f: &str) -> bool {
    let licz = |a: &str| f.matches(a).count();
    let pary = [
        (licz("<w:tbl>") + licz("<w:tbl "), licz("</w:tbl>")),
        (licz("<w:tr>") + licz("<w:tr "), licz("</w:tr>")),
        (licz("<w:tc>") + licz("<w:tc "), licz("</w:tc>")),
        (licz("<w:sdt>") + licz("<w:sdt "), licz("</w:sdt>")),
        (licz("<w:sdtContent>"), licz("</w:sdtContent>")),
        (licz("<w:txbxContent>"), licz("</w:txbxContent>")),
        (licz("<w:p>") + licz("<w:p "), licz("</w:p>")),
    ];
    pary.iter().all(|(a, b)| a == b)
}

/// Pozycje znacznika (tekst) w xml – tylko wewnątrz treści `w:t`.
fn znajdz_znacznik(
    x: &str,
    od: usize,
    pasuje: &dyn Fn(&str) -> bool,
) -> Option<(usize, usize, String)> {
    for w in wezly_tekstu(x) {
        if w.do_ <= od {
            continue;
        }
        let c = &x[w.od..w.do_];
        for (s, e) in zakresy_pol(c) {
            let abs = w.od + s;
            if abs < od {
                continue;
            }
            let t = &c[s..e];
            if pasuje(t) {
                return Some((abs, w.od + e, t.to_string()));
            }
        }
    }
    None
}

fn nazwa_warunku(znacznik: &str) -> Option<(bool, String)> {
    let w = znacznik.strip_prefix("[[")?.strip_suffix("]]")?;
    let w = xml::unescape(w);
    match w.trim().strip_prefix('/') {
        Some(n) => Some((true, n.trim().to_string())),
        None => Some((false, w.trim().to_string())),
    }
}

/// Znacznik bloku warunkowego w treści (`[[nazwa]]` / `[[/nazwa]]`).
#[derive(Debug, Clone)]
struct Znacznik {
    /// Węzeł tekstu (dziecko `w:t`), w którym stoi znacznik.
    wezel: usize,
    od: usize,
    do_: usize,
    nazwa: String,
    zamkniecie: bool,
}

fn znaczniki(d: &docx_xml::Drzewo, x: &str) -> Vec<Znacznik> {
    let mut w = Vec::new();
    for t in d.wszystkie("w:t") {
        for &c in &d.w(t).dzieci {
            let n = d.w(c);
            if !n.tekst() {
                continue;
            }
            let s = &x[n.od..n.do_];
            for (a, b) in zakresy_pol(s) {
                if !s[a..b].starts_with("[[") {
                    continue;
                }
                if let Some((z, nazwa)) = nazwa_warunku(&s[a..b]) {
                    w.push(Znacznik {
                        wezel: c,
                        od: n.od + a,
                        do_: n.od + b,
                        nazwa,
                        zamkniecie: z,
                    });
                }
            }
        }
    }
    w
}

/// „ (akapit „…”)” – miejsce w dokumencie, żeby użytkownik znalazł je w Wordzie.
fn miejsce(d: &docx_xml::Drzewo, x: &str, wezel: usize) -> String {
    let t = d
        .przodek(wezel, "w:p")
        .map(|p| docx_xml::tekst_elementu(d, x, p))
        .unwrap_or_default();
    let t: String = t.trim().chars().take(70).collect();
    if t.is_empty() {
        String::new()
    } else {
        format!(" (akapit „{t}”)")
    }
}

/// Pary znaczników (indeksy w liście) albo opis błędu struktury bloków.
fn pary_znacznikow(
    d: &docx_xml::Drzewo,
    x: &str,
    zn: &[Znacznik],
    dane: &DaneSzablonu,
) -> Result<Vec<(usize, usize)>, String> {
    let mut stos: Vec<usize> = Vec::new();
    let mut pary = Vec::new();
    for (i, z) in zn.iter().enumerate() {
        if !z.zamkniecie {
            stos.push(i);
            continue;
        }
        let k = dane.klucz(&z.nazwa);
        match stos.iter().rposition(|&o| dane.klucz(&zn[o].nazwa) == k) {
            Some(p) if p + 1 == stos.len() => {
                pary.push((stos.pop().unwrap_or(0), i));
            }
            Some(_) => {
                let wewn = &zn[stos[stos.len() - 1]];
                return Err(format!(
                    "bloki [[{}]] i [[{}]] zachodzą na siebie – najpierw zamknij [[/{}]]{}",
                    z.nazwa,
                    wewn.nazwa,
                    wewn.nazwa,
                    miejsce(d, x, z.wezel)
                ));
            }
            None => return Err(format!(
                "znacznik końca [[/{}]] bez znacznika początku [[{}]] (albo w złej kolejności){}",
                z.nazwa,
                z.nazwa,
                miejsce(d, x, z.wezel)
            )),
        }
    }
    if let Some(&o) = stos.first() {
        return Err(format!(
            "blok [[{}]] nie ma znacznika końca [[/{}]]{}",
            zn[o].nazwa,
            zn[o].nazwa,
            miejsce(d, x, zn[o].wezel)
        ));
    }
    Ok(pary)
}

/// Krok 2: bloki warunkowe `[[nazwa]] … [[/nazwa]]`. Gdy warunek jest fałszywy, usuwana jest
/// WYŁĄCZNIE treść między znacznikami ([`docx_xml::usun_zakres`]) – tekst przed znacznikiem
/// początku i po znaczniku końca w tych samych akapitach zostaje. Gdy bezpieczne usunięcie
/// nie jest możliwe (blok niezamknięty, przeplatany, przecinający tabelę lub pole), tryb ścisły
/// zwraca błąd z miejscem, a tryb analizy zapisuje go w `r.bledy`.
fn bloki_warunkowe(
    mut x: String,
    dane: &DaneSzablonu,
    r: &mut RaportSzablonu,
    o: &Opcje,
) -> Wynik<String> {
    let blad = |m: String| BladApi::NieprawidloweDane(format!("szablon: {m}"));
    for _ in 0..2_000 {
        let d = docx_xml::parsuj(&x).map_err(|e| blad(format!("niepoprawny XML – {e}")))?;
        let zn = znaczniki(&d, &x);
        if zn.is_empty() {
            break;
        }
        let pary = match pary_znacznikow(&d, &x, &zn, dane) {
            Ok(p) => p,
            Err(m) => {
                if o.scisly {
                    return Err(blad(m));
                }
                dodaj_raz(&mut r.bledy, &m);
                // analiza: zdejmujemy same znaczniki, żeby zebrać pola z reszty
                let e = zn
                    .iter()
                    .map(|z| docx_xml::Edycja::usun(z.od, z.do_))
                    .collect();
                x = docx_xml::zastosuj(&x, e).map_err(blad)?;
                break;
            }
        };
        if o.analiza {
            // ostrzeżenia o znacznikach dzielących akapit z innym tekstem (Shift+Enter)
            for z in &zn {
                if let Some(p) = d.przodek(z.wezel, "w:p") {
                    let t = docx_xml::tekst_elementu(&d, &x, p);
                    let znak = xml::unescape(&x[z.od..z.do_]);
                    if t.trim() != znak.trim() {
                        dodaj_raz(
                            &mut r.ostrzezenia,
                            &format!(
                                "znacznik {znak} dzieli akapit z innym tekstem – zalecany osobny akapit (Enter, nie Shift+Enter){}",
                                miejsce(&d, &x, z.wezel)
                            ),
                        );
                    }
                }
            }
            // bezpieczeństwo usunięcia sprawdzamy dla każdej pary (bez zmiany dokumentu)
            for &(a, b) in &pary {
                let (za, zb) = (&zn[a], &zn[b]);
                if let Err(m) = docx_xml::usun_zakres(&d, &x, (za.wezel, za.od), (zb.wezel, zb.do_))
                {
                    dodaj_raz(
                        &mut r.bledy,
                        &format!("blok [[{}]]: {m}{}", za.nazwa, miejsce(&d, &x, za.wezel)),
                    );
                }
            }
        }
        // najbardziej wewnętrzna para: znaczniki sąsiadują na liście
        let Some(&(a, b)) = pary.iter().find(|(a, b)| b == &(a + 1)) else {
            break;
        };
        let (za, zb) = (zn[a].clone(), zn[b].clone());
        dodaj_raz(&mut r.bloki_warunkowe, &za.nazwa);
        let klucz = dane.klucz(&za.nazwa);
        let dolacz = match dane.warunki.get(&klucz) {
            Some(t) => *t,
            None => {
                dodaj_raz(
                    &mut r.ostrzezenia,
                    &format!(
                        "nieznany blok warunkowy „{}” – treść zostaje w dokumencie",
                        za.nazwa
                    ),
                );
                true
            }
        };
        let wynik = if dolacz {
            // zostaje treść, znikają same znaczniki (najpierw końcowy – pozycje początku bez zmian)
            docx_xml::usun_zakres(&d, &x, (zb.wezel, zb.od), (zb.wezel, zb.do_))
                .and_then(|e| docx_xml::zastosuj(&x, e))
                .and_then(|x2| {
                    let d2 = docx_xml::parsuj(&x2)?;
                    let z2 = znaczniki(&d2, &x2);
                    let p = z2
                        .iter()
                        .find(|z| z.od == za.od)
                        .ok_or("wewnętrzny błąd – zgubiony znacznik")?;
                    let e = docx_xml::usun_zakres(&d2, &x2, (p.wezel, p.od), (p.wezel, p.do_))?;
                    docx_xml::zastosuj(&x2, e)
                })
        } else {
            docx_xml::usun_zakres(&d, &x, (za.wezel, za.od), (zb.wezel, zb.do_))
                .and_then(|e| docx_xml::zastosuj(&x, e))
        };
        match wynik {
            Ok(n) => x = n,
            Err(m) => {
                let m = format!(
                    "blok [[{}]] nie może zostać bezpiecznie {}: {m}{}",
                    za.nazwa,
                    if dolacz {
                        "oczyszczony ze znaczników"
                    } else {
                        "usunięty"
                    },
                    miejsce(&d, &x, za.wezel)
                );
                if o.scisly {
                    return Err(blad(m));
                }
                dodaj_raz(&mut r.bledy, &m);
                let e = vec![
                    docx_xml::Edycja::usun(za.od, za.do_),
                    docx_xml::Edycja::usun(zb.od, zb.do_),
                ];
                x = docx_xml::zastosuj(&x, e).map_err(blad)?;
            }
        }
    }
    Ok(x)
}

/// Wycina z treści dokumentu tylko zawartość bloku `[[nazwa]]…[[/nazwa]]` (np. protokół
/// odbioru z załącznika nr 5), zachowując ustawienia strony.
fn tylko_blok(x: &str, nazwa: &str) -> Wynik<String> {
    let klucz = normalizuj(nazwa);
    let brak = || {
        BladApi::NieprawidloweDane(format!(
            "szablon nie zawiera bloku [[{nazwa}]] … [[/{nazwa}]]"
        ))
    };
    let (s1, ..) = znajdz_znacznik(x, 0, &|t| {
        nazwa_warunku(t).is_some_and(|(z, n)| !z && normalizuj(&n) == klucz)
    })
    .ok_or_else(brak)?;
    let (s2, ..) = znajdz_znacznik(x, s1, &|t| {
        nazwa_warunku(t).is_some_and(|(z, n)| z && normalizuj(&n) == klucz)
    })
    .ok_or_else(brak)?;
    let (ps1, pe1) = akapit_wokol(x, s1).ok_or_else(brak)?;
    let (ps2, _) = akapit_wokol(x, s2).ok_or_else(brak)?;
    let srodek = &x[pe1..ps2];
    let body_od = x.find("<w:body>").ok_or_else(brak)? + 8;
    let body_do = x.rfind("</w:body>").ok_or_else(brak)?;
    let sect = x[..body_do]
        .rfind("<w:sectPr")
        .filter(|&p| x[..body_do][p..].find("</w:p>").is_none())
        .map(|p| &x[p..body_do])
        .unwrap_or("");
    let _ = ps1;
    Ok(format!(
        "{}{}{}{}",
        &x[..body_od],
        srodek,
        sect,
        &x[body_do..]
    ))
}

/// Wcięcie lewe (twipy) pierwszego akapitu fragmentu – `w:ind w:left` / `w:start`.
fn wciecie(f: &str) -> Option<u32> {
    let i = f.find("<w:ind ")?;
    let tag = &f[i..i + f[i..].find('>')?];
    for atr in ["w:left=\"", "w:start=\""] {
        if let Some(a) = tag.find(atr) {
            let od = a + atr.len();
            let do_ = tag[od..].find('"')? + od;
            return tag[od..do_]
                .parse::<i64>()
                .ok()
                .filter(|v| *v > 0 && *v < 6000)
                .map(|v| v as u32);
        }
    }
    None
}

/// Wstawiony blok dostaje wcięcie akapitu, który zastępuje (np. lista w paragrafie umowy):
/// tabele – `w:tblInd` i proporcjonalnie węższe kolumny, akapity – `w:ind`.
fn z_wcieciem(blok: &str, n: u32) -> String {
    const SZEROKOSC: u32 = 9600;
    let skala = |v: u32| {
        (u64::from(v) * u64::from(SZEROKOSC.saturating_sub(n).max(4000)) / u64::from(SZEROKOSC))
            as u32
    };
    let mut wynik = String::with_capacity(blok.len() + 256);
    let mut reszta = blok;
    // szerokości w tabelach
    while let Some(i) = ["<w:gridCol w:w=\"", "<w:tcW w:w=\"", "<w:tblW w:w=\""]
        .iter()
        .filter_map(|t| reszta.find(t).map(|p| (p, t.len())))
        .min()
        .map(|(p, l)| p + l)
    {
        wynik.push_str(&reszta[..i]);
        let k = reszta[i..].find('"').unwrap_or(0);
        let v: u32 = reszta[i..i + k].parse().unwrap_or(0);
        wynik.push_str(&skala(v).to_string());
        reszta = &reszta[i + k..];
    }
    wynik.push_str(reszta);
    let wynik = wynik.replace(
        "<w:tblBorders>",
        &format!("<w:tblInd w:w=\"{n}\" w:type=\"dxa\"/><w:tblBorders>"),
    );
    // akapity spoza tabel: w:ind przed w:jc (kolejność elementów w:pPr)
    let mut out = String::with_capacity(wynik.len() + 128);
    let mut glebokosc = 0i32;
    let mut i = 0;
    while i < wynik.len() {
        let r = &wynik[i..];
        if r.starts_with("<w:tbl>") {
            glebokosc += 1;
        } else if r.starts_with("</w:tbl>") {
            glebokosc -= 1;
        } else if glebokosc == 0 && r.starts_with("<w:pPr>") {
            let k = r.find("</w:pPr>").unwrap_or(0);
            let ppr = &r[..k];
            out.push_str(&match ppr.find("<w:jc ") {
                Some(j) => format!("{}<w:ind w:left=\"{n}\"/>{}", &ppr[..j], &ppr[j..]),
                None => format!("{ppr}<w:ind w:left=\"{n}\"/>"),
            });
            i += k;
            continue;
        }
        let c = r.chars().next().unwrap_or(' ');
        out.push(c);
        i += c.len_utf8();
    }
    out
}

/// Krok 3: pola blokowe – akapit z samym polem zastępujemy przygotowanym XML-em.
fn wstaw_bloki(mut x: String, dane: &DaneSzablonu, r: &mut RaportSzablonu, glowna: bool) -> String {
    let mut od = 0;
    for _ in 0..1000 {
        let Some((s, e, zn)) = znajdz_znacznik(&x, od, &|t| {
            !t.starts_with("[[")
                && dane
                    .bloki
                    .contains_key(&dane.klucz(&xml::unescape(&t[1..t.len() - 1])))
        }) else {
            break;
        };
        let nazwa = xml::unescape(&zn[1..zn.len() - 1]);
        dodaj_raz(&mut r.pola, nazwa.trim());
        let blok = &dane.bloki[&dane.klucz(&nazwa)];
        if !glowna {
            r.ostrzezenia.push(format!(
                "pole blokowe {zn} w nagłówku/stopce – pominięte (wstaw je w treści dokumentu)"
            ));
            x = format!("{}{}", &x[..s], &x[e..]);
            od = s;
            continue;
        }
        match akapit_wokol(&x, s) {
            Some((ps, pe)) if tekst_zakresu(&x, ps, pe).trim() == zn.trim() => {
                // akapit w komórce tabeli musi zostać zakończony akapitem
                let w_komorce = {
                    let przed = &x[..ps];
                    let otw = przed.matches("<w:tc>").count() + przed.matches("<w:tc ").count();
                    otw > przed.matches("</w:tc>").count()
                };
                let mut wstaw = match wciecie(&x[ps..pe]) {
                    Some(n) if !w_komorce => z_wcieciem(blok, n),
                    _ => blok.clone(),
                };
                if w_komorce && !wstaw.ends_with("</w:p>") {
                    wstaw.push_str("<w:p/>");
                }
                if wstaw.is_empty() {
                    wstaw = "<w:p/>".into();
                }
                // ustawienia sekcji w akapicie (koniec sekcji) przenosimy do pustego akapitu
                if let Some(sp) = x[ps..pe].find("<w:sectPr") {
                    if let Some(k) = x[ps + sp..pe].find("</w:sectPr>") {
                        let sect = &x[ps + sp..ps + sp + k + 11];
                        wstaw.push_str(&format!("<w:p><w:pPr>{sect}</w:pPr></w:p>"));
                    }
                }
                let dl = wstaw.len();
                x = format!("{}{}{}", &x[..ps], wstaw, &x[pe..]);
                od = ps + dl;
            }
            Some((_, pe)) => {
                r.ostrzezenia.push(format!(
                    "pole blokowe {zn} powinno stać w osobnym akapicie – wstawiono je pod akapitem"
                ));
                let bez = format!("{}{}", &x[..s], &x[e..]);
                let pe = pe - (e - s);
                x = format!("{}{}{}", &bez[..pe], blok, &bez[pe..]);
                od = pe + blok.len();
            }
            None => {
                x = format!("{}{}", &x[..s], &x[e..]);
                od = s;
            }
        }
    }
    x
}

fn przebiegi_wartosci(rpr: &str, wartosc: &str) -> String {
    let linie: Vec<String> = wartosc.split('\n').map(xml::escape).collect();
    format!(
        "<w:r>{rpr}<w:t xml:space=\"preserve\">{}</w:t></w:r>",
        linie.join("</w:t><w:br/><w:t xml:space=\"preserve\">")
    )
}

/// Poziom kontrolki treści – wynika z rodzica elementu `w:sdt`.
#[derive(Debug, Clone, Copy, PartialEq)]
enum PoziomKontrolki {
    /// W akapicie (zawiera przebiegi).
    Wiersza,
    /// Akapity / tabele w treści, komórce, nagłówku.
    Blokowa,
    /// Obejmuje komórki (rodzic `w:tr`).
    Komorkowa,
    /// Obejmuje wiersze tabeli (rodzic `w:tbl`).
    Wierszowa,
}

/// Krok 0: kontrolki treści Worda (`w:sdt`) z nazwą (`w:alias`) albo znacznikiem (`w:tag`)
/// odpowiadającym polu – cała kontrolka jest zastępowana wartością (formatowanie z pierwszego
/// przebiegu). Dzięki temu znika też powiązanie z danymi XML (`w:dataBinding`).
///
/// Poziomy: w akapicie → przebiegi; blokowa → akapit / tabela (w komórce zawsze zakończona
/// akapitem); obejmująca komórki lub wiersze tabeli → odmowa z komunikatem (wartości nie da się
/// tam wstawić bez zepsucia tabeli). Nieznane kontrolki zostają bez zmian.
fn kontrolki_tresci(
    x: &str,
    dane: &DaneSzablonu,
    r: &mut RaportSzablonu,
    glowna: bool,
    o: &Opcje,
) -> Wynik<String> {
    let blad = |m: String| BladApi::NieprawidloweDane(format!("szablon: {m}"));
    let d = docx_xml::parsuj(x).map_err(|e| blad(format!("niepoprawny XML – {e}")))?;
    let mut edycje = Vec::new();
    let mut zajete: Vec<(usize, usize)> = Vec::new();
    for i in d.wszystkie("w:sdt") {
        let w = d.w(i);
        if zajete.iter().any(|(a, b)| w.od >= *a && w.do_ <= *b) {
            continue;
        }
        let Some(pr) = d.dziecko(i, "w:sdtPr") else {
            continue;
        };
        let mut klucz = None;
        let mut nazwa = String::new();
        for el in ["w:alias", "w:tag"] {
            let Some(v) = d
                .dziecko(pr, el)
                .and_then(|e| d.atrybut(x, e, "w:val"))
                .map(xml::unescape)
            else {
                continue;
            };
            let k = dane.klucz(&v);
            if dane.pola.contains_key(&k) || dane.bloki.contains_key(&k) {
                nazwa = v;
                klucz = Some(k);
                break;
            }
        }
        let Some(klucz) = klucz else { continue };
        let rodzic = w.rodzic.unwrap_or(0);
        let poziom = match d.w(rodzic).nazwa.as_str() {
            "w:tbl" => PoziomKontrolki::Wierszowa,
            "w:tr" => PoziomKontrolki::Komorkowa,
            _ if d.przodek(rodzic, "w:p").is_some() => PoziomKontrolki::Wiersza,
            _ => PoziomKontrolki::Blokowa,
        };
        if matches!(
            poziom,
            PoziomKontrolki::Wierszowa | PoziomKontrolki::Komorkowa
        ) {
            let m = format!(
                "kontrolka treści „{nazwa}” obejmuje {} tabeli – wartości nie można tam wstawić. Usuń kontrolkę (Deweloper → Właściwości → usuń) i wpisz w komórce pole [{nazwa}] albo wstaw kontrolkę wewnątrz akapitu komórki{}",
                if poziom == PoziomKontrolki::Wierszowa { "cały wiersz" } else { "całą komórkę" },
                miejsce(&d, x, i)
            );
            if o.scisly {
                return Err(blad(m));
            }
            dodaj_raz(&mut r.bledy, &m);
            continue;
        }
        let tresc = d
            .dziecko(i, "w:sdtContent")
            .map(|c| &x[d.w(c).otw_do..d.w(c).zam_od.max(d.w(c).otw_do)])
            .unwrap_or("");
        let rpr = tresc
            .find("<w:rPr>")
            .and_then(|a| tresc[a..].find("</w:rPr>").map(|k| &tresc[a..a + k + 8]))
            .unwrap_or("");
        let nowy = if let Some(blok) = dane.bloki.get(&klucz) {
            if poziom == PoziomKontrolki::Wiersza || !glowna {
                dodaj_raz(
                    &mut r.ostrzezenia,
                    &format!("kontrolka „{nazwa}” musi obejmować cały akapit, aby wstawić w niej tabelę – pominięto"),
                );
                continue;
            }
            match wciecie(tresc) {
                Some(n) => z_wcieciem(blok, n),
                None => blok.clone(),
            }
        } else {
            let wart = &dane.pola[&klucz];
            let wart = if wart.trim().is_empty() {
                dodaj_raz(&mut r.puste, nazwa.trim());
                PUSTE_POLE.to_string()
            } else {
                wart.clone()
            };
            let przebiegi = przebiegi_wartosci(rpr, &wart);
            if poziom == PoziomKontrolki::Blokowa {
                let ppr = tresc
                    .find("<w:pPr>")
                    .and_then(|a| tresc[a..].find("</w:pPr>").map(|k| &tresc[a..a + k + 8]))
                    .unwrap_or("");
                format!("<w:p>{ppr}{przebiegi}</w:p>")
            } else {
                przebiegi
            }
        };
        let mut nowy = nowy;
        // komórka tabeli musi kończyć się akapitem
        if poziom == PoziomKontrolki::Blokowa && d.w(rodzic).nazwa == "w:tc" {
            let dz = &d.w(rodzic).dzieci;
            let poz = dz.iter().position(|c| *c == i).unwrap_or(0);
            let ostatni = !dz[poz + 1..]
                .iter()
                .any(|c| matches!(d.w(*c).nazwa.as_str(), "w:p" | "w:tbl" | "w:sdt"));
            if ostatni
                && !nowy.trim_end().ends_with("</w:p>")
                && !nowy.trim_end().ends_with("<w:p/>")
            {
                nowy.push_str("<w:p/>");
            }
        }
        dodaj_raz(&mut r.pola, nazwa.trim());
        r.kontrolki += 1;
        zajete.push((w.od, w.do_));
        edycje.push(docx_xml::Edycja {
            od: w.od,
            do_: w.do_,
            nowe: nowy,
        });
    }
    docx_xml::zastosuj(x, edycje).map_err(blad)
}

/// Krok 4: pola tekstowe.
fn zamien_pola(x: &str, dane: &DaneSzablonu, r: &mut RaportSzablonu) -> String {
    let mut zamiany = Vec::new();
    for w in wezly_tekstu(x) {
        let c = &x[w.od..w.do_];
        let zakresy = zakresy_pol(c);
        if zakresy.is_empty() {
            continue;
        }
        let mut nowy = String::with_capacity(c.len());
        let mut poz = 0;
        let mut zmieniono = false;
        for (s, e) in zakresy {
            let zn = &c[s..e];
            if zn.starts_with("[[") {
                continue;
            }
            let nazwa = xml::unescape(&zn[1..zn.len() - 1]);
            let klucz = dane.klucz(&nazwa);
            if klucz.is_empty() {
                continue;
            }
            if dane.pomijane.contains(&klucz) {
                r.do_uzupelnienia += 1;
                continue;
            }
            let Some(wartosc) = dane.pola.get(&klucz) else {
                if !dane.bloki.contains_key(&klucz) && klucz.chars().any(char::is_alphabetic) {
                    dodaj_raz(&mut r.nieznane, nazwa.trim());
                }
                continue;
            };
            dodaj_raz(&mut r.pola, nazwa.trim());
            nowy.push_str(&c[poz..s]);
            let t = if wartosc.trim().is_empty() {
                dodaj_raz(&mut r.puste, nazwa.trim());
                PUSTE_POLE.to_string()
            } else {
                wartosc.clone()
            };
            let linie: Vec<String> = t.split('\n').map(xml::escape).collect();
            nowy.push_str(&linie.join("</w:t><w:br/><w:t xml:space=\"preserve\">"));
            poz = e;
            zmieniono = true;
        }
        if zmieniono {
            nowy.push_str(&c[poz..]);
            let tag = otwarcie_preserve(&x[w.otwarcie..w.od]);
            zamiany.push((w.otwarcie, w.do_, format!("{tag}{nowy}")));
        }
    }
    zastosuj(x, zamiany)
}

// ---------------------------------------------------------------------------
// Pakiet DOCX
// ---------------------------------------------------------------------------

fn czesc_z_polami(nazwa: &str) -> bool {
    nazwa == "word/document.xml"
        || nazwa == "word/footnotes.xml"
        || nazwa == "word/endnotes.xml"
        || (nazwa.starts_with("word/header") && nazwa.ends_with(".xml"))
        || (nazwa.starts_with("word/footer") && nazwa.ends_with(".xml"))
}

fn blad_pliku(e: impl std::fmt::Display) -> BladApi {
    BladApi::NieprawidloweDane(format!(
        "plik szablonu nie jest poprawnym dokumentem Word (.docx/.docm): {e}"
    ))
}

/// Limit danych po rozpakowaniu szablonu (liczony z faktycznie odczytanych bajtów).
pub const MAKS_ROZPAKOWANE: u64 = 200 * 1024 * 1024;
/// Limit liczby plików w pakiecie.
pub const MAKS_WPISOW: usize = 2_000;

fn bezpieczna_nazwa(n: &str) -> bool {
    !n.is_empty()
        && !n.starts_with('/')
        && !n.starts_with('\\')
        && !n.contains('\\')
        && !n.contains(':')
        && !n.split('/').any(|c| c == ".." || c == ".")
        && !n.contains('\0')
}

/// Części pakietu (nazwa, treść) w kolejności z archiwum. Rozmiar liczony z faktycznie
/// odczytanych bajtów (strumieniowo, z twardym limitem – zadeklarowany rozmiar w archiwum
/// można sfałszować), limit liczby wpisów, odrzucane ścieżki z `..` i bezwzględne.
pub fn rozpakuj(docx: &[u8]) -> Wynik<Vec<(String, Vec<u8>)>> {
    let mut arch = zip::ZipArchive::new(Cursor::new(docx)).map_err(blad_pliku)?;
    if arch.len() > MAKS_WPISOW {
        return Err(BladApi::ZaDuzo(format!(
            "szablon zawiera zbyt wiele plików ({}; najwyżej {MAKS_WPISOW})",
            arch.len()
        )));
    }
    let mut wynik = Vec::with_capacity(arch.len());
    let mut lacznie: u64 = 0;
    for i in 0..arch.len() {
        let p = arch.by_index(i).map_err(blad_pliku)?;
        let nazwa = p.name().to_string();
        if !bezpieczna_nazwa(&nazwa) {
            return Err(blad_pliku(format!(
                "niedozwolona ścieżka w archiwum „{nazwa}”"
            )));
        }
        if p.is_dir() {
            continue;
        }
        let zostalo = MAKS_ROZPAKOWANE - lacznie;
        let mut b = Vec::new();
        let n = p
            .take(zostalo + 1)
            .read_to_end(&mut b)
            .map_err(blad_pliku)? as u64;
        lacznie += n;
        if lacznie > MAKS_ROZPAKOWANE {
            return Err(BladApi::ZaDuzo(
                "szablon po rozpakowaniu przekracza 200 MB".into(),
            ));
        }
        wynik.push((nazwa, b));
    }
    if !wynik.iter().any(|(n, _)| n == "word/document.xml") {
        return Err(blad_pliku("brak word/document.xml"));
    }
    Ok(wynik)
}

/// Plik relacji danej części (`word/document.xml` → `word/_rels/document.xml.rels`).
pub fn plik_relacji(czesc: &str) -> String {
    match czesc.rsplit_once('/') {
        Some((k, n)) => format!("{k}/_rels/{n}.rels"),
        None => format!("_rels/{czesc}.rels"),
    }
}

/// Relacje z pliku `.rels`: (od, do_, Id, Type, Target, zewnętrzna).
fn relacje(x: &str) -> Vec<(usize, usize, String, String, String, bool)> {
    let Ok(d) = docx_xml::parsuj(x) else {
        return vec![];
    };
    d.wszystkie("Relationship")
        .into_iter()
        .map(|i| {
            let a = |n: &str| d.atrybut(x, i, n).unwrap_or("").to_string();
            (
                d.w(i).od,
                d.w(i).do_,
                a("Id"),
                a("Type"),
                xml::unescape(&a("Target")),
                a("TargetMode") == "External",
            )
        })
        .collect()
}

fn tekst_czesci(czesci: &[(String, Vec<u8>)], nazwa: &str) -> Option<String> {
    czesci
        .iter()
        .find(|(n, _)| n == nazwa)
        .map(|(_, b)| String::from_utf8_lossy(b).into_owned())
}

fn ustaw_czesc(czesci: &mut [(String, Vec<u8>)], nazwa: &str, t: String) {
    if let Some(c) = czesci.iter_mut().find(|(n, _)| n == nazwa) {
        c.1 = t.into_bytes();
    }
}

/// Oczyszczenie pakietu: makra (`.docm`), kontrolki ActiveX, dołączony szablon
/// (`attachedTemplate`) i odnośniki do plików lokalnych/sieciowych. Wynik to czysty `.docx`.
/// Odwołania zewnętrzne inne niż odnośniki WWW/e-mail (połączone obrazy, obiekty, ramki) →
/// odmowa z instrukcją.
fn oczysc_pakiet(czesci: &mut Vec<(String, Vec<u8>)>, r: &mut RaportSzablonu) -> Wynik<()> {
    if bez_makr(czesci) {
        dodaj_raz(
            &mut r.ostrzezenia,
            "szablon zawierał makra (.docm) – w dokumencie zostały usunięte",
        );
    }
    // ActiveX: części, relacje, typy i odwołania w treści
    let przed = czesci.len();
    czesci.retain(|(n, _)| !n.starts_with("word/activeX/"));
    let mut activex = przed - czesci.len();
    let nazwy: Vec<String> = czesci.iter().map(|(n, _)| n.clone()).collect();
    for n in &nazwy {
        if n.ends_with(".rels") {
            let t = tekst_czesci(czesci, n).unwrap_or_default();
            let mut e = Vec::new();
            for (od, do_, _, typ, cel, zewn) in relacje(&t) {
                let ax = typ.ends_with("/control")
                    || typ.contains("activeX")
                    || cel.contains("activeX/");
                let szablon = typ.ends_with("/attachedTemplate");
                if ax || szablon {
                    e.push(docx_xml::Edycja::usun(od, do_));
                    if szablon {
                        dodaj_raz(
                            &mut r.ostrzezenia,
                            "usunięto powiązanie z szablonem Worda (attachedTemplate) – dokument nie odwołuje się do plików na dysku",
                        );
                    }
                    continue;
                }
                if zewn {
                    let www = ["http://", "https://", "mailto:"]
                        .iter()
                        .any(|p| cel.to_lowercase().starts_with(p));
                    if !(www && typ.ends_with("/hyperlink")) {
                        return Err(BladApi::NieprawidloweDane(format!(
                            "szablon zawiera odwołanie do pliku zewnętrznego „{cel}” ({}) – w Wordzie zamień połączony obraz/obiekt/odnośnik na osadzony (Plik → Informacje → Edytuj łącza do plików → Przerwij łącze) i wgraj szablon ponownie",
                            typ.rsplit('/').next().unwrap_or("relacja")
                        )));
                    }
                }
            }
            if !e.is_empty() {
                let t2 = docx_xml::zastosuj(&t, e).map_err(BladApi::Wewnetrzny)?;
                ustaw_czesc(czesci, n, t2);
            }
        } else if n == "[Content_Types].xml" {
            let t = tekst_czesci(czesci, n).unwrap_or_default();
            let t2 = usun_elementy(&t, "Override", &["/word/activeX/"]);
            let t2 = usun_elementy(&t2, "Default", &["activeX"]);
            if t2 != t {
                ustaw_czesc(czesci, n, t2);
            }
        } else if n == "word/settings.xml" {
            let t = tekst_czesci(czesci, n).unwrap_or_default();
            let t2 = usun_elementy(&t, "w:attachedTemplate", &[""]);
            if t2 != t {
                ustaw_czesc(czesci, n, t2);
            }
        }
    }
    for n in &nazwy {
        if !czesc_z_polami(n) {
            continue;
        }
        let t = tekst_czesci(czesci, n).unwrap_or_default();
        if !t.contains("<w:control") {
            continue;
        }
        let d = docx_xml::parsuj(&t).map_err(|e| {
            BladApi::NieprawidloweDane(format!("szablon: {n}: niepoprawny XML – {e}"))
        })?;
        let mut e = Vec::new();
        let mut zajete: Vec<(usize, usize)> = Vec::new();
        for c in d.wszystkie("w:control") {
            let cel = d.przodek(c, "w:object").unwrap_or(c);
            let w = d.w(cel);
            if zajete.iter().any(|(a, b)| w.od >= *a && w.do_ <= *b) {
                continue;
            }
            zajete.push((w.od, w.do_));
            e.push(docx_xml::Edycja::usun(w.od, w.do_));
        }
        activex += e.len();
        let t2 = docx_xml::zastosuj(&t, e).map_err(BladApi::Wewnetrzny)?;
        ustaw_czesc(czesci, n, t2);
    }
    if activex > 0 {
        dodaj_raz(
            &mut r.ostrzezenia,
            "szablon zawierał kontrolki ActiveX (np. przycisk formularza) – zostały usunięte z dokumentu",
        );
    }
    // odnośniki w treści do usuniętych relacji (np. hiperłącze do pliku) – bez r:id
    Ok(())
}

/// Identyfikatory relacji części.
fn id_relacji(czesci: &[(String, Vec<u8>)], czesc: &str) -> std::collections::BTreeSet<String> {
    tekst_czesci(czesci, &plik_relacji(czesc))
        .map(|t| relacje(&t).into_iter().map(|r| r.2).collect())
        .unwrap_or_default()
}

/// Walidator wyniku: każda część XML poprawna, części z treścią – poprawna struktura
/// (komórki zakończone akapitem, dozwolone dzieci, istniejące relacje). Błąd = komunikat.
pub fn sprawdz_pakiet(czesci: &[(String, Vec<u8>)]) -> Result<(), String> {
    for (n, b) in czesci {
        if !(n.ends_with(".xml") || n.ends_with(".rels")) {
            continue;
        }
        let t = std::str::from_utf8(b).map_err(|_| format!("{n}: nieprawidłowe kodowanie"))?;
        if czesc_z_polami(n) {
            let rel = id_relacji(czesci, n);
            docx_xml::sprawdz_czesc(n, t, Some(&rel))?;
        } else {
            docx_xml::parsuj(t).map_err(|e| format!("{n}: niepoprawny XML – {e}"))?;
        }
    }
    if !czesci.iter().any(|(n, _)| n == "[Content_Types].xml") {
        return Err("brak [Content_Types].xml".into());
    }
    Ok(())
}

/// Akapit informacyjny (czerwony, pogrubiony, wyśrodkowany).
fn akapit_informacyjny(t: &str, rozmiar: u32) -> String {
    format!(
        "<w:p><w:pPr><w:jc w:val=\"center\"/><w:spacing w:before=\"0\" w:after=\"80\"/></w:pPr><w:r><w:rPr><w:b/><w:color w:val=\"C00000\"/><w:sz w:val=\"{rozmiar}\"/><w:szCs w:val=\"{rozmiar}\"/></w:rPr><w:t xml:space=\"preserve\">{}</w:t></w:r></w:p>",
        xml::escape(t)
    )
}

const ID_NAGLOWKA: &str = "rIdRobinwoodNaglowek";

/// Oznaczenie dokumentu (np. „PROJEKT – NIE DO PODPISU”, „TEKST JEDNOLITY…”): akapity na
/// początku treści i wiersz w nagłówku każdej strony (istniejące nagłówki dostają akapit na
/// początku; dokument bez nagłówka dostaje nowy).
fn oznacz_dokument(czesci: &mut Vec<(String, Vec<u8>)>, n: &Naglowek) -> Wynik<()> {
    let blad = |e: String| BladApi::Wewnetrzny(format!("oznaczenie dokumentu: {e}"));
    let doc = tekst_czesci(czesci, "word/document.xml").unwrap_or_default();
    let d = docx_xml::parsuj(&doc).map_err(blad)?;
    let body = d
        .wszystkie("w:body")
        .first()
        .copied()
        .ok_or_else(|| blad("brak w:body".into()))?;
    let mut e = vec![docx_xml::Edycja {
        od: d.w(body).otw_do,
        do_: d.w(body).otw_do,
        nowe: n.tresc.iter().map(|t| akapit_informacyjny(t, 26)).collect(),
    }];
    let naglowki: Vec<String> = czesci
        .iter()
        .map(|(x, _)| x.clone())
        .filter(|x| x.starts_with("word/header") && x.ends_with(".xml"))
        .collect();
    let mut nowy_naglowek = false;
    for s in d.wszystkie("w:sectPr") {
        if d.dziecko(s, "w:headerReference").is_none() {
            nowy_naglowek = true;
            e.push(docx_xml::Edycja {
                od: d.w(s).otw_do,
                do_: d.w(s).otw_do,
                nowe: format!("<w:headerReference w:type=\"default\" r:id=\"{ID_NAGLOWKA}\"/>"),
            });
        }
    }
    if nowy_naglowek {
        let korzen = d
            .dzieci_el(0)
            .next()
            .ok_or_else(|| blad("brak elementu głównego".into()))?;
        let tag = &doc[d.w(korzen).od..d.w(korzen).otw_do];
        if !tag.contains("xmlns:r=") {
            e.push(docx_xml::Edycja {
                od: d.w(korzen).otw_do - 1,
                do_: d.w(korzen).otw_do - 1,
                nowe: " xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\"".into(),
            });
        }
    }
    let doc2 = docx_xml::zastosuj(&doc, e).map_err(blad)?;
    ustaw_czesc(czesci, "word/document.xml", doc2);
    for h in naglowki {
        let t = tekst_czesci(czesci, &h).unwrap_or_default();
        let dh = docx_xml::parsuj(&t).map_err(blad)?;
        let korzen = dh.dzieci_el(0).next();
        if let Some(k) = korzen {
            let t2 = docx_xml::zastosuj(
                &t,
                vec![docx_xml::Edycja {
                    od: dh.w(k).otw_do,
                    do_: dh.w(k).otw_do,
                    nowe: akapit_informacyjny(&n.naglowek, 18),
                }],
            )
            .map_err(blad)?;
            ustaw_czesc(czesci, &h, t2);
        }
    }
    if nowy_naglowek {
        czesci.push((
            "word/header_robinwood.xml".into(),
            format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><w:hdr xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\">{}</w:hdr>",
                akapit_informacyjny(&n.naglowek, 18)
            )
            .into_bytes(),
        ));
        let rels = "word/_rels/document.xml.rels";
        let wpis = format!("<Relationship Id=\"{ID_NAGLOWKA}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/header\" Target=\"header_robinwood.xml\"/>");
        match tekst_czesci(czesci, rels) {
            Some(t) => {
                let k = t.rfind("</Relationships>").ok_or_else(|| blad("relacje".into()))?;
                ustaw_czesc(czesci, rels, format!("{}{wpis}{}", &t[..k], &t[k..]));
            }
            None => czesci.push((
                rels.into(),
                format!("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">{wpis}</Relationships>").into_bytes(),
            )),
        }
        let typy = tekst_czesci(czesci, "[Content_Types].xml").unwrap_or_default();
        if let Some(k) = typy.rfind("</Types>") {
            let wpis = "<Override PartName=\"/word/header_robinwood.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml\"/>";
            ustaw_czesc(
                czesci,
                "[Content_Types].xml",
                format!("{}{wpis}{}", &typy[..k], &typy[k..]),
            );
        }
    }
    Ok(())
}

/// Oznaczenie dokumentu: wiersz nagłówka strony i akapity na początku treści.
#[derive(Debug, Clone, Default)]
pub struct Naglowek {
    pub naglowek: String,
    pub tresc: Vec<String>,
}

/// Opcje wypełniania.
#[derive(Debug, Clone, Default)]
pub struct Opcje {
    /// Wycięcie samego bloku warunkowego (np. „załącznik 5”).
    pub tylko: Option<String>,
    /// Tryb ścisły (generowanie): błąd szablonu = odmowa. Wyłączony – analiza (raport).
    pub scisly: bool,
    /// Analiza szablonu: dodatkowe ostrzeżenia i kontrole bezpieczeństwa wszystkich bloków.
    pub analiza: bool,
    /// Pola wymagane w szablonie (brak → `brak_wymaganych`).
    pub wymagane: Vec<String>,
    /// Oznaczenie dokumentu (projekt / tekst jednolity).
    pub naglowek: Option<Naglowek>,
}

/// Wypełnia szablon w trybie ścisłym (wynik przechodzi walidator struktury).
/// `tylko` – wycięcie samego bloku warunkowego (np. „załącznik 5”).
pub fn wypelnij(
    szablon: &[u8],
    dane: &DaneSzablonu,
    tylko: Option<&str>,
) -> Wynik<(Vec<u8>, RaportSzablonu)> {
    wypelnij_z(
        szablon,
        dane,
        &Opcje {
            tylko: tylko.map(String::from),
            scisly: true,
            ..Default::default()
        },
    )
}

pub fn wypelnij_z(
    szablon: &[u8],
    dane: &DaneSzablonu,
    o: &Opcje,
) -> Wynik<(Vec<u8>, RaportSzablonu)> {
    let mut czesci = rozpakuj(szablon)?;
    let mut r = RaportSzablonu::default();
    // każda część XML musi być poprawna – zepsuty szablon jest odrzucany
    for (n, b) in &czesci {
        if n.ends_with(".xml") || n.ends_with(".rels") {
            let t = std::str::from_utf8(b)
                .map_err(|_| blad_pliku(format!("{n}: nieprawidłowe kodowanie")))?;
            docx_xml::parsuj(t).map_err(|e| blad_pliku(format!("{n}: niepoprawny XML – {e}")))?;
        }
    }
    oczysc_pakiet(&mut czesci, &mut r)?;
    for (n, b) in czesci.iter_mut() {
        if !czesc_z_polami(n) {
            continue;
        }
        let tekst = String::from_utf8(std::mem::take(b))
            .map_err(|_| blad_pliku(format!("{n}: nieprawidłowe kodowanie")))?;
        let glowna = n == "word/document.xml";
        let mut x = kontrolki_tresci(&tekst, dane, &mut r, glowna, o)?;
        x = scal_pola(&x);
        if glowna {
            if let Some(blok) = &o.tylko {
                x = tylko_blok(&x, blok)?;
            }
        }
        x = bloki_warunkowe(x, dane, &mut r, o)?;
        x = wstaw_bloki(x, dane, &mut r, glowna);
        x = zamien_pola(&x, dane, &mut r);
        *b = x.into_bytes();
    }
    for w in &o.wymagane {
        let k = normalizuj(w);
        if !r.pola.iter().any(|p| dane.klucz(p) == k) {
            dodaj_raz(&mut r.brak_wymaganych, w);
        }
    }
    if let Some(n) = &o.naglowek {
        oznacz_dokument(&mut czesci, n)?;
    }
    if let Err(e) = sprawdz_pakiet(&czesci) {
        let m = format!(
            "wygenerowany dokument miałby niepoprawną strukturę ({e}) – dokumentu nie utworzono; popraw szablon w tym miejscu albo zgłoś problem"
        );
        if o.scisly {
            return Err(BladApi::NieprawidloweDane(m));
        }
        dodaj_raz(&mut r.bledy, &m);
    }
    Ok((spakuj(&czesci)?, r))
}

/// Analiza szablonu bez danych (raport po wgraniu pliku): znalezione pola i bloki, pola
/// nieznane, `[uzupełnij]`, pola wymagane nieobecne, znaczniki dzielące akapit z tekstem,
/// błędy bloków (niezamknięte, w złej kolejności, przecinające tabelę), kontrolki w tabelach.
/// Niepoprawny XML → błąd (szablon odrzucony).
pub fn analizuj(szablon: &[u8], znane: &DaneSzablonu, wymagane: &[&str]) -> Wynik<RaportSzablonu> {
    let mut d = znane.clone();
    for w in d.warunki.values_mut() {
        *w = true;
    }
    let (_, mut r) = wypelnij_z(
        szablon,
        &d,
        &Opcje {
            analiza: true,
            wymagane: wymagane.iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        },
    )?;
    r.puste.clear();
    Ok(r)
}

pub fn spakuj(czesci: &[(String, Vec<u8>)]) -> Wynik<Vec<u8>> {
    let mut z = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let opcje = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    // [Content_Types].xml na początku (część narzędzi tego wymaga)
    let mut kolejnosc: Vec<&(String, Vec<u8>)> = czesci.iter().collect();
    kolejnosc.sort_by_key(|(n, _)| n != "[Content_Types].xml");
    for (n, b) in kolejnosc {
        z.start_file(n.as_str(), opcje)
            .map_err(|e| BladApi::Wewnetrzny(format!("zip: {e}")))?;
        z.write_all(b)
            .map_err(|e| BladApi::Wewnetrzny(format!("zip: {e}")))?;
    }
    let c = z
        .finish()
        .map_err(|e| BladApi::Wewnetrzny(format!("zip: {e}")))?;
    Ok(c.into_inner())
}

/// Usuwa elementy XML `<znacznik … />`, których tekst zawiera którąś z fraz.
fn usun_elementy(x: &str, znacznik: &str, frazy: &[&str]) -> String {
    let otw = format!("<{znacznik} ");
    let mut s = String::with_capacity(x.len());
    let mut poz = 0;
    while let Some(i) = x[poz..].find(&otw) {
        let a = poz + i;
        let Some(k) = x[a..].find("/>") else { break };
        let b = a + k + 2;
        s.push_str(&x[poz..a]);
        if !frazy.iter().any(|f| x[a..b].contains(f)) {
            s.push_str(&x[a..b]);
        }
        poz = b;
    }
    s.push_str(&x[poz..]);
    s
}

/// `.docm` → `.docx`: typ części głównej i usunięcie projektu makr.
fn bez_makr(czesci: &mut Vec<(String, Vec<u8>)>) -> bool {
    let mut zmieniono = false;
    czesci.retain(|(n, _)| {
        let makro = n.ends_with("vbaProject.bin")
            || n.ends_with("vbaData.xml")
            || n.ends_with("vbaProject.bin.rels")
            || n.starts_with("customUI/");
        zmieniono |= makro;
        !makro
    });
    for (n, b) in czesci.iter_mut() {
        if n == "[Content_Types].xml" {
            let t = String::from_utf8_lossy(b).to_string();
            let mut t2 = t.replace(
                "application/vnd.ms-word.document.macroEnabled.main+xml",
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml",
            );
            t2 = usun_elementy(&t2, "Override", &["vbaProject", "vbaData"]);
            t2 = usun_elementy(&t2, "Default", &["vbaProject"]);
            t2 = usun_elementy(&t2, "Override", &["customUI"]);
            if t2 != t {
                zmieniono = true;
                *b = t2.into_bytes();
            }
        } else if n == "_rels/.rels" {
            let t = String::from_utf8_lossy(b).to_string();
            let t2 = usun_elementy(&t, "Relationship", &["customUI", "ui/extensibility"]);
            if t2 != t {
                zmieniono = true;
                *b = t2.into_bytes();
            }
        } else if n == "word/_rels/document.xml.rels" {
            let t = String::from_utf8_lossy(b).to_string();
            let t2 = usun_elementy(&t, "Relationship", &["vbaProject", "vbaData"]);
            if t2 != t {
                zmieniono = true;
                *b = t2.into_bytes();
            }
        }
    }
    zmieniono
}

#[cfg(test)]
mod testy {
    use super::*;

    fn dokument(body: &str) -> Vec<u8> {
        let doc = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
             <w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:body>{body}\
             <w:sectPr><w:pgSz w:w=\"11906\" w:h=\"16838\"/></w:sectPr></w:body></w:document>"
        );
        let czesci = vec![
            (
                "[Content_Types].xml".to_string(),
                b"<?xml version=\"1.0\"?><Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Override PartName=\"/word/document.xml\" ContentType=\"application/vnd.ms-word.document.macroEnabled.main+xml\"/><Override PartName=\"/word/vbaProject.bin\" ContentType=\"application/vnd.ms-office.vbaProject\"/></Types>".to_vec(),
            ),
            ("word/document.xml".to_string(), doc.into_bytes()),
            (
                "word/header1.xml".to_string(),
                b"<w:hdr xmlns:w=\"x\"><w:p><w:r><w:t>Umowa nr [numer </w:t></w:r><w:r><w:t>umowy]</w:t></w:r></w:p></w:hdr>".to_vec(),
            ),
            ("word/vbaProject.bin".to_string(), vec![1, 2, 3]),
        ];
        spakuj(&czesci).unwrap()
    }

    fn czesc(docx: &[u8], nazwa: &str) -> String {
        let c = rozpakuj(docx).unwrap();
        String::from_utf8(c.into_iter().find(|(n, _)| n == nazwa).unwrap().1).unwrap()
    }

    fn tekst(xml: &str) -> String {
        wezly_tekstu(xml)
            .iter()
            .map(|w| xml::unescape(&xml[w.od..w.do_]))
            .collect::<Vec<_>>()
            .join("")
    }

    fn dane() -> DaneSzablonu {
        let mut d = DaneSzablonu::default();
        d.pole("numer umowy", "7/2027");
        d.pole("wartość", "1 000,00 zł");
        d.pole("klient", "Jan <Testowy> & Syn");
        d.pole("piętro", "");
        d.pole("adres klienta", "ul. Testowa 1\n00-001 Miasto");
        d.blok(
            "specyfikacja",
            xml::akapit_tekst("TABELA SPECYFIKACJI [x]", true),
        );
        d.warunek("załącznik 3", false);
        d.warunek("załącznik 6", true);
        d
    }

    #[test]
    fn normalizacja_nazw() {
        assert_eq!(normalizuj("  Wartość\u{a0} SŁOWNIE "), "wartosc slownie");
        assert_eq!(normalizuj("e-mail"), "e-mail");
        assert_eq!(normalizuj("Kod  domofonu"), "kod domofonu");
    }

    #[test]
    fn pola_rozbite_na_fragmenty() {
        // Word: „[war” + „tość]” w dwóch przebiegach z innym formatowaniem, „[numer umowy]”
        // w trzech, i pole w nagłówku.
        let body = "<w:p><w:r><w:t xml:space=\"preserve\">Wartość: </w:t></w:r><w:r><w:rPr><w:b/></w:rPr><w:t>[war</w:t></w:r>\
                    <w:proofErr w:type=\"spellStart\"/><w:r><w:t>tość] brutto</w:t></w:r></w:p>\
                    <w:p><w:r><w:t>Nr [</w:t></w:r><w:r><w:t>numer </w:t></w:r><w:r><w:t>umowy</w:t></w:r><w:r><w:t>], [Klient]</w:t></w:r></w:p>";
        let (wynik, r) = wypelnij(&dokument(body), &dane(), None).unwrap();
        let doc = czesc(&wynik, "word/document.xml");
        let t = tekst(&doc);
        assert!(t.contains("Wartość: 1 000,00 zł brutto"), "{t}");
        assert!(t.contains("Nr 7/2027, Jan <Testowy> & Syn"), "{t}");
        assert!(doc.contains("Jan &lt;Testowy&gt; &amp; Syn"));
        // pogrubienie zostaje na pierwszym fragmencie pola
        assert!(doc.contains("<w:b/></w:rPr><w:t xml:space=\"preserve\">1 000,00 zł</w:t>"));
        let hdr = czesc(&wynik, "word/header1.xml");
        assert!(tekst(&hdr).contains("Umowa nr 7/2027"), "{hdr}");
        assert!(r.nieznane.is_empty(), "{:?}", r.nieznane);
        assert!(r.pola.contains(&"numer umowy".to_string()));
    }

    #[test]
    fn nieznane_puste_i_wieloliniowe() {
        let body =
            "<w:p><w:r><w:t>[zupełnie nowe pole] [piętro] [1] [adres klienta]</w:t></w:r></w:p>";
        let (wynik, r) = wypelnij(&dokument(body), &dane(), None).unwrap();
        let doc = czesc(&wynik, "word/document.xml");
        assert!(doc.contains("[zupełnie nowe pole]"));
        assert_eq!(r.nieznane, vec!["zupełnie nowe pole".to_string()]);
        assert_eq!(r.puste, vec!["piętro".to_string()]);
        assert!(doc.contains(PUSTE_POLE));
        assert!(doc.contains("[1]"), "liczby w nawiasach to nie pola");
        assert!(doc.contains("ul. Testowa 1</w:t><w:br/><w:t xml:space=\"preserve\">00-001 Miasto"));
    }

    #[test]
    fn bloki_warunkowe_i_pole_blokowe() {
        let body = "<w:p><w:r><w:t>Początek</w:t></w:r></w:p>\
                    <w:p><w:r><w:t>[[za</w:t></w:r><w:r><w:t>łącznik 3]]</w:t></w:r></w:p>\
                    <w:p><w:r><w:t>Cennik AGD</w:t></w:r></w:p>\
                    <w:tbl><w:tr><w:tc><w:p><w:r><w:t>komórka</w:t></w:r></w:p></w:tc></w:tr></w:tbl>\
                    <w:p><w:r><w:t>[[/załącznik 3]]</w:t></w:r></w:p>\
                    <w:p><w:r><w:t>[[Załącznik 6]]</w:t></w:r></w:p>\
                    <w:p><w:r><w:t>Zgoda na zdjęcia</w:t></w:r></w:p>\
                    <w:p><w:r><w:t>[[/załącznik 6]]</w:t></w:r></w:p>\
                    <w:p><w:pPr><w:jc w:val=\"left\"/></w:pPr><w:r><w:t>[specyfikacja]</w:t></w:r></w:p>\
                    <w:p><w:r><w:t>[[nieznany]]x[[/nieznany]]</w:t></w:r></w:p>";
        let (wynik, r) = wypelnij(&dokument(body), &dane(), None).unwrap();
        let doc = czesc(&wynik, "word/document.xml");
        let t = tekst(&doc);
        assert!(!t.contains("Cennik AGD") && !t.contains("komórka"), "{t}");
        assert!(t.contains("Zgoda na zdjęcia"));
        assert!(!t.contains("[[") && !t.contains("]]"), "{t}");
        assert!(t.contains("TABELA SPECYFIKACJI [x]"));
        // nawias w treści bloku zakodowany – nie jest polem
        assert!(doc.contains("&#91;x&#93;"));
        assert!(r.nieznane.is_empty(), "{:?}", r.nieznane);
        assert!(r.ostrzezenia.iter().any(|o| o.contains("nieznany")));
        assert!(zrownowazony(&doc));
        // makra usunięte
        let typy = czesc(&wynik, "[Content_Types].xml");
        assert!(!typy.contains("macroEnabled") && !typy.contains("vbaProject"));
        assert!(rozpakuj(&wynik)
            .unwrap()
            .iter()
            .all(|(n, _)| !n.contains("vba")));
    }

    #[test]
    fn kontrolki_tresci_worda() {
        let mut d = dane();
        d.alias("number", "numer umowy");
        d.pomin("uzupełnij");
        let body = "<w:p><w:r><w:t>Umowa nr </w:t></w:r><w:sdt><w:sdtPr><w:alias w:val=\"numer umowy\"/><w:tag w:val=\"number\"/>\
                    <w:dataBinding w:xpath=\"/rw:contract/rw:number\"/><w:text/></w:sdtPr><w:sdtContent><w:r><w:rPr><w:color w:val=\"FF3333\"/></w:rPr>\
                    <w:t>[numer umowy]</w:t></w:r></w:sdtContent></w:sdt><w:r><w:t> [uzupełnij]</w:t></w:r></w:p>\
                    <w:sdt><w:sdtPr><w:alias w:val=\"Spec\"/><w:tag w:val=\"specyfikacja\"/></w:sdtPr><w:sdtContent><w:p><w:r><w:t>[Uruchom makro]</w:t></w:r></w:p></w:sdtContent></w:sdt>\
                    <w:sdt><w:sdtPr><w:tag w:val=\"cos\"/></w:sdtPr><w:sdtContent><w:p><w:r><w:t>x</w:t></w:r></w:p></w:sdtContent></w:sdt>";
        let (wynik, r) = wypelnij(&dokument(body), &d, None).unwrap();
        let doc = czesc(&wynik, "word/document.xml");
        let t = tekst(&doc);
        assert!(t.contains("Umowa nr 7/2027 [uzupełnij]"), "{t}");
        assert!(t.contains("TABELA SPECYFIKACJI"), "{t}");
        assert!(!doc.contains("dataBinding") && !t.contains("Uruchom"));
        assert!(
            doc.contains("<w:color w:val=\"FF3333\"/></w:rPr><w:t xml:space=\"preserve\">7/2027")
        );
        assert_eq!(r.kontrolki, 2);
        assert_eq!(r.do_uzupelnienia, 1);
        assert!(r.nieznane.is_empty(), "{:?}", r.nieznane);
        assert!(
            doc.contains("<w:tag w:val=\"cos\"/>"),
            "nieznana kontrolka zostaje"
        );
        assert!(zrownowazony(&doc));
    }

    #[test]
    fn wciecie_bloku() {
        let blok = format!(
            "{}{}",
            xml::akapit(
                &xml::przebieg("Tytuł", true, None),
                Some("left"),
                Some(0),
                false
            ),
            xml::tabela(
                &[4800, 4800],
                &[(
                    true,
                    vec![xml::Komorka::naglowek("a"), xml::Komorka::naglowek("b")]
                )]
            )
        );
        let w = z_wcieciem(&blok, 960);
        assert!(w.contains("<w:tblInd w:w=\"960\" w:type=\"dxa\"/><w:tblBorders>"));
        assert!(w.contains("<w:gridCol w:w=\"4320\"/>"), "{w}");
        assert!(w.contains("<w:tblW w:w=\"8640\""));
        assert!(
            w.contains("<w:ind w:left=\"960\"/><w:jc w:val=\"left\"/>"),
            "{w}"
        );
        assert_eq!(
            wciecie("<w:pPr><w:ind w:left=\"851\" w:hanging=\"1\"/></w:pPr>"),
            Some(851)
        );
        assert_eq!(wciecie("<w:pPr/>"), None);
    }

    #[test]
    fn wyciecie_bloku() {
        let body = "<w:p><w:r><w:t>Umowa [numer umowy]</w:t></w:r></w:p>\
                    <w:p><w:r><w:t>[[załącznik 5]]</w:t></w:r></w:p>\
                    <w:p><w:r><w:t>Protokół do umowy [numer umowy]</w:t></w:r></w:p>\
                    <w:p><w:r><w:t>[[/załącznik 5]]</w:t></w:r></w:p>";
        let (wynik, _) = wypelnij(&dokument(body), &dane(), Some("załącznik 5")).unwrap();
        let doc = czesc(&wynik, "word/document.xml");
        let t = tekst(&doc);
        assert_eq!(t, "Protokół do umowy 7/2027");
        assert!(doc.contains("<w:sectPr>"));
        assert!(wypelnij(&dokument(body), &dane(), Some("załącznik 9")).is_err());
    }

    #[test]
    fn analiza_i_bledny_plik() {
        let body = "<w:p><w:r><w:t>[numer umowy] [coś innego]</w:t></w:r></w:p>";
        let r = analizuj(&dokument(body), &dane(), &[]).unwrap();
        assert_eq!(r.nieznane, vec!["coś innego".to_string()]);
        assert!(wypelnij(b"to nie jest zip", &dane(), None).is_err());
    }

    fn dokument_z(body: &str, czesci_dod: Vec<(String, Vec<u8>)>, rels: &str) -> Vec<u8> {
        let doc = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?><w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\"><w:body>{body}<w:sectPr/></w:body></w:document>"
        );
        let mut c = vec![
            (
                "[Content_Types].xml".to_string(),
                b"<?xml version=\"1.0\"?><Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Override PartName=\"/word/document.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml\"/><Override PartName=\"/word/activeX/activeX1.xml\" ContentType=\"application/vnd.ms-office.activeX+xml\"/></Types>".to_vec(),
            ),
            ("word/document.xml".to_string(), doc.into_bytes()),
            (
                "word/_rels/document.xml.rels".to_string(),
                format!("<?xml version=\"1.0\"?><Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">{rels}</Relationships>").into_bytes(),
            ),
        ];
        c.extend(czesci_dod);
        spakuj(&c).unwrap()
    }

    fn akapity(xml: &str) -> Vec<String> {
        let d = docx_xml::parsuj(xml).unwrap();
        d.wszystkie("w:p")
            .into_iter()
            .map(|p| docx_xml::tekst_elementu(&d, xml, p))
            .collect()
    }

    #[test]
    fn znaczniki_blokow_dzielace_akapit_nie_usuwaja_sasiednich_paragrafow() {
        // Shift+Enter: znacznik w tym samym akapicie co § 4 i § 6
        let body = "<w:p><w:r><w:t>§ 4. Płatność zadatku.</w:t><w:br/><w:t>[[załącznik 3]]</w:t></w:r></w:p>\
                    <w:p><w:r><w:t>Cennik AGD</w:t></w:r></w:p>\
                    <w:p><w:r><w:t>[[/załącznik 3]]</w:t><w:br/><w:t>§ 6. Odbiór.</w:t></w:r></w:p>\
                    <w:p><w:r><w:t>§ 7. Wariant: [[wariant B]]po odbiorze[[/wariant B]] koniec.</w:t></w:r></w:p>";
        let mut d = dane();
        d.warunek("wariant B", false);
        let (wynik, _) = wypelnij(&dokument(body), &d, None).unwrap();
        let doc = czesc(&wynik, "word/document.xml");
        let a = akapity(&doc);
        assert_eq!(
            a,
            vec![
                "§ 4. Płatność zadatku.",
                "§ 6. Odbiór.",
                "§ 7. Wariant:  koniec."
            ],
            "{doc}"
        );
        assert!(
            !doc.contains("<w:br/>"),
            "podziały wiersza przy znacznikach usunięte: {doc}"
        );
        // warunek prawdziwy – zostaje treść, znikają znaczniki i ich linie
        d.warunek("załącznik 3", true);
        let (wynik, _) = wypelnij(&dokument(body), &d, None).unwrap();
        let a = akapity(&czesc(&wynik, "word/document.xml"));
        assert_eq!(
            a,
            vec![
                "§ 4. Płatność zadatku.",
                "Cennik AGD",
                "§ 6. Odbiór.",
                "§ 7. Wariant:  koniec."
            ]
        );
        // raport przy wgraniu ostrzega
        let r = analizuj(&dokument(body), &dane(), &[]).unwrap();
        assert!(
            r.ostrzezenia.iter().any(|o| o.contains("dzieli akapit")),
            "{:?}",
            r.ostrzezenia
        );
        assert!(r.bledy.is_empty(), "{:?}", r.bledy);
    }

    #[test]
    fn bledy_struktury_blokow_odmowa_i_raport() {
        let przypadki = [
            ("<w:p><w:r><w:t>[[załącznik 3]]</w:t></w:r></w:p><w:p><w:r><w:t>x</w:t></w:r></w:p>", "nie ma znacznika końca"),
            ("<w:p><w:r><w:t>[[/załącznik 3]]</w:t></w:r></w:p>", "bez znacznika początku"),
            ("<w:p><w:r><w:t>[[załącznik 3]]</w:t></w:r></w:p><w:p><w:r><w:t>[[załącznik 6]]</w:t></w:r></w:p><w:p><w:r><w:t>[[/załącznik 3]]</w:t></w:r></w:p><w:p><w:r><w:t>[[/załącznik 6]]</w:t></w:r></w:p>", "zachodzą"),
            ("<w:p><w:r><w:t>[[załącznik 3]]</w:t></w:r></w:p><w:tbl><w:tr><w:tc><w:p><w:r><w:t>[[/załącznik 3]]</w:t></w:r></w:p></w:tc></w:tr></w:tbl>", "w tabeli"),
            ("<w:tbl><w:tr><w:tc><w:p><w:r><w:t>[[załącznik 3]]</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>[[/załącznik 3]]</w:t></w:r></w:p></w:tc></w:tr></w:tbl>", "komórkach"),
        ];
        for (body, fraza) in przypadki {
            let e = wypelnij(&dokument(body), &dane(), None)
                .unwrap_err()
                .to_string();
            assert!(e.contains(fraza), "{fraza}: {e}");
            let r = analizuj(&dokument(body), &dane(), &[]).unwrap();
            assert!(
                r.bledy.iter().any(|b| b.contains(fraza)),
                "{fraza}: {:?}",
                r.bledy
            );
        }
    }

    #[test]
    fn raport_pol_wymaganych_i_uzupelnij() {
        let body = "<w:p><w:r><w:t>[numer umowy] [uzupełnij] [uzupelnij] [coś]</w:t></w:r></w:p>";
        let mut d = dane();
        d.pomin("uzupełnij");
        let r = analizuj(&dokument(body), &d, &["numer umowy", "specyfikacja"]).unwrap();
        assert_eq!(r.brak_wymaganych, vec!["specyfikacja".to_string()]);
        assert_eq!(r.do_uzupelnienia, 2);
        assert_eq!(r.nieznane, vec!["coś".to_string()]);
    }

    #[test]
    fn zepsuty_xml_odrzucony() {
        let body = "<w:p><w:r><w:t>niezamknięty</w:r></w:p>";
        let e = analizuj(&dokument(body), &dane(), &[])
            .unwrap_err()
            .to_string();
        assert!(e.contains("niepoprawny XML"), "{e}");
    }

    #[test]
    fn kontrolki_w_tabeli_komorce_i_wierszu() {
        let mut d = dane();
        d.pole("numer umowy", "7/2027");
        // blokowa kontrolka z tabelą (pole blokowe) jako ostatnia w komórce → akapit na końcu
        let body = "<w:tbl><w:tr><w:tc><w:sdt><w:sdtPr><w:tag w:val=\"specyfikacja\"/></w:sdtPr><w:sdtContent><w:p><w:r><w:t>x</w:t></w:r></w:p></w:sdtContent></w:sdt></w:tc></w:tr></w:tbl>";
        d.blok(
            "specyfikacja",
            xml::tabela(&[1000], &[(true, vec![xml::Komorka::t("a")])]),
        );
        let (w, _) = wypelnij(&dokument(body), &d, None).unwrap();
        let doc = czesc(&w, "word/document.xml");
        assert!(doc.contains("</w:tbl><w:p/></w:tc>"), "{doc}");
        // blokowa kontrolka tekstowa w komórce
        let body = "<w:tbl><w:tr><w:tc><w:sdt><w:sdtPr><w:alias w:val=\"numer umowy\"/></w:sdtPr><w:sdtContent><w:p><w:r><w:t>x</w:t></w:r></w:p></w:sdtContent></w:sdt></w:tc></w:tr></w:tbl>";
        let (w, _) = wypelnij(&dokument(body), &d, None).unwrap();
        assert!(czesc(&w, "word/document.xml").contains(
            "<w:tc><w:p><w:r><w:t xml:space=\"preserve\">7/2027</w:t></w:r></w:p></w:tc>"
        ));
        // kontrolka obejmująca komórkę i wiersz → odmowa z komunikatem
        for body in [
            "<w:tbl><w:tr><w:sdt><w:sdtPr><w:alias w:val=\"numer umowy\"/></w:sdtPr><w:sdtContent><w:tc><w:p><w:r><w:t>x</w:t></w:r></w:p></w:tc></w:sdtContent></w:sdt></w:tr></w:tbl>",
            "<w:tbl><w:sdt><w:sdtPr><w:alias w:val=\"numer umowy\"/></w:sdtPr><w:sdtContent><w:tr><w:tc><w:p><w:r><w:t>x</w:t></w:r></w:p></w:tc></w:tr></w:sdtContent></w:sdt></w:tbl>",
        ] {
            let e = wypelnij(&dokument(body), &d, None).unwrap_err().to_string();
            assert!(e.contains("tabeli – wartości nie można"), "{e}");
            let r = analizuj(&dokument(body), &d, &[]).unwrap();
            assert_eq!(r.bledy.len(), 1, "{:?}", r.bledy);
        }
        // nieznana kontrolka wierszowa zostaje (poprawny OOXML)
        let body = "<w:tbl><w:sdt><w:sdtPr><w:tag w:val=\"inne\"/></w:sdtPr><w:sdtContent><w:tr><w:tc><w:p/></w:tc></w:tr></w:sdtContent></w:sdt></w:tbl>";
        assert!(wypelnij(&dokument(body), &d, None).is_ok());
    }

    #[test]
    fn znaki_niedozwolone_w_xml_odfiltrowane() {
        let mut d = dane();
        d.pole("klient", "Jan\u{FFFE}\u{FFFF}\u{1}\u{8} Kowalski\r");
        let body = "<w:p><w:r><w:t>[klient]</w:t></w:r></w:p>";
        let (w, _) = wypelnij(&dokument(body), &d, None).unwrap();
        let doc = czesc(&w, "word/document.xml");
        assert!(doc.contains(">Jan Kowalski<"), "{doc}");
        assert!(docx_xml::parsuj(&doc).is_ok());
        assert_eq!(xml::escape("a\u{FFFE}b\u{0}c"), "abc");
    }

    #[test]
    fn archiwum_z_zafalszowanym_rozmiarem_odrzucone() {
        // 300 MB zer w jednym wpisie (kompresja ~1000:1) – rozmiar liczony z odczytanych bajtów
        let mut z = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let o = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .large_file(true);
        z.start_file("word/document.xml", o).unwrap();
        let mb = vec![0u8; 1024 * 1024];
        for _ in 0..300 {
            z.write_all(&mb).unwrap();
        }
        let mut b = z.finish().unwrap().into_inner();
        assert!(b.len() < 2 * 1024 * 1024, "{}", b.len());
        // fałszujemy zadeklarowany rozmiar (nagłówek lokalny i katalog centralny) na 1 KB
        let falsz = |b: &mut Vec<u8>, sygn: &[u8], przes: usize| {
            let mut i = 0;
            while i + 4 <= b.len() {
                if &b[i..i + 4] == sygn {
                    b[i + przes..i + przes + 4].copy_from_slice(&1024u32.to_le_bytes());
                }
                i += 1;
            }
        };
        falsz(&mut b, b"PK\x03\x04", 22);
        falsz(&mut b, b"PK\x01\x02", 24);
        let e = rozpakuj(&b);
        assert!(e.is_err(), "zafałszowany rozmiar nie może obejść limitu");
        // ścieżki z .. i bezwzględne
        for n in ["../x.xml", "/etc/x", "word\\..\\x", "C:/x"] {
            let c = vec![
                ("word/document.xml".to_string(), b"<a/>".to_vec()),
                (n.to_string(), b"x".to_vec()),
            ];
            assert!(rozpakuj(&spakuj(&c).unwrap()).is_err(), "{n}");
        }
    }

    #[test]
    fn activex_i_odwolania_zewnetrzne_usuwane() {
        let body = "<w:p><w:r><w:t>Przycisk:</w:t></w:r><w:r><w:object w:dxaOrig=\"1\"><w:control r:id=\"rId9\" w:name=\"CommandButton1\"/></w:object></w:r></w:p>";
        let rels = "<Relationship Id=\"rId9\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/control\" Target=\"activeX/activeX1.xml\"/>\
                    <Relationship Id=\"rId3\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink\" Target=\"https://example.com\" TargetMode=\"External\"/>";
        let dod = vec![
            ("word/activeX/activeX1.xml".to_string(), b"<ax:ocx xmlns:ax=\"x\"/>".to_vec()),
            ("word/activeX/activeX1.bin".to_string(), vec![1, 2, 3]),
            ("word/activeX/_rels/activeX1.xml.rels".to_string(), b"<Relationships xmlns=\"x\"/>".to_vec()),
            (
                "word/_rels/settings.xml.rels".to_string(),
                b"<Relationships xmlns=\"x\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/attachedTemplate\" Target=\"file:///C:/Szablony/Umowa.dotm\" TargetMode=\"External\"/></Relationships>".to_vec(),
            ),
            (
                "word/settings.xml".to_string(),
                b"<w:settings xmlns:w=\"w\" xmlns:r=\"r\"><w:attachedTemplate r:id=\"rId1\"/></w:settings>".to_vec(),
            ),
        ];
        let (w, r) = wypelnij(&dokument_z(body, dod, rels), &dane(), None).unwrap();
        let c = rozpakuj(&w).unwrap();
        assert!(
            c.iter().all(|(n, _)| !n.contains("activeX")),
            "części ActiveX usunięte"
        );
        let doc = czesc(&w, "word/document.xml");
        assert!(
            !doc.contains("w:control") && !doc.contains("w:object") && doc.contains("Przycisk:")
        );
        let rels = czesc(&w, "word/_rels/document.xml.rels");
        assert!(!rels.contains("control") && rels.contains("example.com"));
        assert!(!czesc(&w, "[Content_Types].xml").contains("activeX"));
        assert!(!czesc(&w, "word/settings.xml").contains("attachedTemplate"));
        assert!(!czesc(&w, "word/_rels/settings.xml.rels").contains("file:"));
        assert!(r.ostrzezenia.iter().any(|o| o.contains("ActiveX")));
        // połączony obraz z udziału sieciowego → odmowa
        let rels = "<Relationship Id=\"rId5\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/image\" Target=\"file://serwer/udzial/logo.png\" TargetMode=\"External\"/>";
        let e = wypelnij(&dokument_z("<w:p/>", vec![], rels), &dane(), None)
            .unwrap_err()
            .to_string();
        assert!(e.contains("pliku zewnętrznego"), "{e}");
    }

    #[test]
    fn oznaczenie_projektu_w_tresci_i_naglowku() {
        let o = Opcje {
            scisly: true,
            naglowek: Some(Naglowek {
                naglowek: "PROJEKT – NIE DO PODPISU".into(),
                tresc: vec!["PROJEKT – NIE DO PODPISU".into()],
            }),
            ..Default::default()
        };
        // dokument bez nagłówka → nowy nagłówek z relacją
        let (w, _) = wypelnij_z(
            &dokument_z("<w:p><w:r><w:t>A</w:t></w:r></w:p>", vec![], ""),
            &dane(),
            &o,
        )
        .unwrap();
        let doc = czesc(&w, "word/document.xml");
        assert!(doc.contains("<w:body><w:p><w:pPr><w:jc w:val=\"center\"/>"));
        assert!(doc.contains("rIdRobinwoodNaglowek"));
        assert!(czesc(&w, "word/header_robinwood.xml").contains("NIE DO PODPISU"));
        sprawdz_pakiet(&rozpakuj(&w).unwrap()).unwrap();
        // dokument z nagłówkiem – akapit dopisany w istniejącym
        let (w, _) = wypelnij_z(&dokument("<w:p/>"), &dane(), &o).unwrap();
        assert!(czesc(&w, "word/header1.xml").contains("NIE DO PODPISU"));
    }
}
