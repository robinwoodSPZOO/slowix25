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
        self.aliasy.insert(normalizuj(inna_nazwa), normalizuj(nazwa));
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
                c if (c as u32) < 0x20 && c != '\t' && c != '\n' => {}
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
    pub fn akapit(przebiegi: &str, wyrownanie: Option<&str>, odstep_po: Option<u32>, z_nastepnym: bool) -> String {
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
                s.push_str(&format!("<w:tc><w:tcPr><w:tcW w:w=\"{szer}\" w:type=\"dxa\"/>"));
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
                let Some(z) = x[od..].find("</w:t>") else { break };
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
            _ => i = if j > i + 1 && j < b.len() && b[j] == b'[' { j } else { i + 1 },
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
fn znajdz_znacznik(x: &str, od: usize, pasuje: &dyn Fn(&str) -> bool) -> Option<(usize, usize, String)> {
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

/// Usuwa znacznik: cały akapit, jeśli zawiera tylko znacznik, w przeciwnym razie sam tekst.
fn usun_znacznik(x: &str, od: usize, do_: usize, zn: &str) -> String {
    if let Some((ps, pe)) = akapit_wokol(x, od) {
        if tekst_zakresu(x, ps, pe).trim() == zn.trim() && !x[ps..pe].contains("<w:sectPr") {
            return format!("{}{}", &x[..ps], &x[pe..]);
        }
    }
    format!("{}{}", &x[..od], &x[do_..])
}

/// Krok 2: bloki warunkowe `[[nazwa]] … [[/nazwa]]`.
fn bloki_warunkowe(mut x: String, dane: &DaneSzablonu, r: &mut RaportSzablonu) -> String {
    let mut od = 0;
    for _ in 0..10_000 {
        let Some((s1, e1, zn1)) = znajdz_znacznik(&x, od, &|t| t.starts_with("[[")) else {
            break;
        };
        let Some((zamkniecie, nazwa)) = nazwa_warunku(&zn1) else {
            od = e1;
            continue;
        };
        if zamkniecie {
            r.ostrzezenia.push(format!(
                "znacznik końca „{zn1}” bez znacznika początku – usunięty"
            ));
            x = usun_znacznik(&x, s1, e1, &zn1);
            continue;
        }
        dodaj_raz(&mut r.bloki_warunkowe, &nazwa);
        let klucz = dane.klucz(&nazwa);
        let koniec = znajdz_znacznik(&x, e1, &|t| {
            nazwa_warunku(t).is_some_and(|(z, n)| z && dane.klucz(&n) == klucz)
        });
        let Some((s2, e2, zn2)) = koniec else {
            r.ostrzezenia.push(format!(
                "blok „{zn1}” nie ma znacznika końca „[[/{nazwa}]]” – znacznik usunięty, treść zostaje"
            ));
            x = usun_znacznik(&x, s1, e1, &zn1);
            continue;
        };
        let dolacz = match dane.warunki.get(&klucz) {
            Some(t) => *t,
            None => {
                r.ostrzezenia.push(format!(
                    "nieznany blok warunkowy „{nazwa}” – treść zostaje w dokumencie"
                ));
                true
            }
        };
        if !dolacz {
            let p1 = akapit_wokol(&x, s1);
            let p2 = akapit_wokol(&x, s2);
            if let (Some((ps, _)), Some((_, pe))) = (p1, p2) {
                if zrownowazony(&x[ps..pe]) {
                    x = format!("{}{}", &x[..ps], &x[pe..]);
                    od = ps;
                    continue;
                }
            }
            r.ostrzezenia.push(format!(
                "blok „{nazwa}” zaczyna się i kończy w różnych miejscach struktury dokumentu (np. w tabeli) – nie można go usunąć, usunięto tylko znaczniki"
            ));
        }
        x = usun_znacznik(&x, s2, e2, &zn2);
        x = usun_znacznik(&x, s1, e1, &zn1);
        od = s1.min(x.len());
    }
    x
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

/// Krok 3: pola blokowe – akapit z samym polem zastępujemy przygotowanym XML-em.
fn wstaw_bloki(mut x: String, dane: &DaneSzablonu, r: &mut RaportSzablonu, glowna: bool) -> String {
    let mut od = 0;
    for _ in 0..1000 {
        let Some((s, e, zn)) = znajdz_znacznik(&x, od, &|t| {
            !t.starts_with("[[") && dane.bloki.contains_key(&dane.klucz(&xml::unescape(&t[1..t.len() - 1])))
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
                let mut wstaw = blok.clone();
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

/// Wartość atrybutu `w:val` elementu `<w:{el} …/>` w fragmencie.
fn wartosc_atrybutu(f: &str, el: &str) -> Option<String> {
    let i = f.find(&format!("<w:{el} "))?;
    let reszta = &f[i..];
    let k = reszta.find("/>").or_else(|| reszta.find('>'))?;
    let tag = &reszta[..k];
    let a = tag.find("w:val=\"")? + 7;
    let b = tag[a..].find('"')?;
    Some(xml::unescape(&tag[a..a + b]))
}

/// Koniec elementu `<w:sdt>` zaczynającego się w `od` (z uwzględnieniem zagnieżdżeń).
fn koniec_sdt(x: &str, od: usize) -> Option<usize> {
    let mut glebokosc = 0i32;
    let mut i = od;
    while i < x.len() {
        let r = &x[i..];
        if r.starts_with("<w:sdt>") || r.starts_with("<w:sdt ") {
            glebokosc += 1;
            i += 6;
        } else if r.starts_with("</w:sdt>") {
            glebokosc -= 1;
            i += 8;
            if glebokosc == 0 {
                return Some(i);
            }
        } else {
            i += r.find('<').filter(|&k| k > 0).unwrap_or(1);
        }
    }
    None
}

fn przebiegi_wartosci(rpr: &str, wartosc: &str) -> String {
    let linie: Vec<String> = wartosc.split('\n').map(xml::escape).collect();
    format!(
        "<w:r>{rpr}<w:t xml:space=\"preserve\">{}</w:t></w:r>",
        linie.join("</w:t><w:br/><w:t xml:space=\"preserve\">")
    )
}

/// Krok 0: kontrolki treści Worda (`w:sdt`) z nazwą (`w:alias`) albo znacznikiem (`w:tag`)
/// odpowiadającym polu – cała kontrolka jest zastępowana wartością (formatowanie z pierwszego
/// przebiegu). Dzięki temu znika też powiązanie z danymi XML (`w:dataBinding`), które przy
/// otwarciu w Wordzie przywróciłoby treść z szablonu.
fn kontrolki_tresci(x: &str, dane: &DaneSzablonu, r: &mut RaportSzablonu, glowna: bool) -> String {
    let mut wynik = String::with_capacity(x.len());
    let mut poz = 0;
    loop {
        let a = x[poz..].find("<w:sdt>");
        let b = x[poz..].find("<w:sdt ");
        let start = match (a, b) {
            (Some(a), Some(b)) => poz + a.min(b),
            (Some(a), None) => poz + a,
            (None, Some(b)) => poz + b,
            (None, None) => break,
        };
        let Some(koniec) = koniec_sdt(x, start) else { break };
        let sdt = &x[start..koniec];
        let pr_do = sdt.find("</w:sdtPr>").unwrap_or(0);
        let pr = &sdt[..pr_do];
        let mut klucz = None;
        let mut nazwa = String::new();
        for kandydat in [wartosc_atrybutu(pr, "alias"), wartosc_atrybutu(pr, "tag")]
            .into_iter()
            .flatten()
        {
            let k = dane.klucz(&kandydat);
            if dane.pola.contains_key(&k) || dane.bloki.contains_key(&k) {
                nazwa = kandydat;
                klucz = Some(k);
                break;
            }
        }
        let Some(klucz) = klucz else {
            // nieznana kontrolka – zostaje; szukamy dalej wewnątrz niej
            wynik.push_str(&x[poz..start + 6]);
            poz = start + 6;
            continue;
        };
        let tresc = sdt
            .find("<w:sdtContent>")
            .map(|i| {
                let od = i + 14;
                let do_ = sdt.rfind("</w:sdtContent>").unwrap_or(sdt.len());
                &sdt[od..do_.max(od)]
            })
            .unwrap_or("");
        let blokowa = tresc.contains("<w:p>") || tresc.contains("<w:p ") || tresc.contains("<w:tbl>");
        let rpr = tresc
            .find("<w:rPr>")
            .and_then(|i| tresc[i..].find("</w:rPr>").map(|k| &tresc[i..i + k + 8]))
            .unwrap_or("");
        let nowy = if let Some(blok) = dane.bloki.get(&klucz) {
            if !blokowa || !glowna {
                r.ostrzezenia.push(format!(
                    "kontrolka „{nazwa}” musi obejmować cały akapit, aby wstawić w niej tabelę – pominięto"
                ));
                wynik.push_str(&x[poz..koniec]);
                poz = koniec;
                continue;
            }
            blok.clone()
        } else {
            let w = &dane.pola[&klucz];
            let w = if w.trim().is_empty() {
                dodaj_raz(&mut r.puste, nazwa.trim());
                PUSTE_POLE.to_string()
            } else {
                w.clone()
            };
            let przebiegi = przebiegi_wartosci(rpr, &w);
            if blokowa {
                let ppr = tresc
                    .find("<w:pPr>")
                    .and_then(|i| tresc[i..].find("</w:pPr>").map(|k| &tresc[i..i + k + 8]))
                    .unwrap_or("");
                format!("<w:p>{ppr}{przebiegi}</w:p>")
            } else {
                przebiegi
            }
        };
        dodaj_raz(&mut r.pola, nazwa.trim());
        r.kontrolki += 1;
        wynik.push_str(&x[poz..start]);
        wynik.push_str(&nowy);
        poz = koniec;
    }
    wynik.push_str(&x[poz..]);
    wynik
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

/// Części pakietu (nazwa, treść) w kolejności z archiwum.
pub fn rozpakuj(docx: &[u8]) -> Wynik<Vec<(String, Vec<u8>)>> {
    let mut arch = zip::ZipArchive::new(Cursor::new(docx)).map_err(blad_pliku)?;
    let mut wynik = Vec::with_capacity(arch.len());
    let mut lacznie: u64 = 0;
    for i in 0..arch.len() {
        let mut p = arch.by_index(i).map_err(blad_pliku)?;
        if p.is_dir() {
            continue;
        }
        lacznie += p.size();
        if lacznie > 200 * 1024 * 1024 {
            return Err(BladApi::ZaDuzo("szablon po rozpakowaniu przekracza 200 MB".into()));
        }
        let mut b = Vec::with_capacity(p.size() as usize);
        p.read_to_end(&mut b).map_err(blad_pliku)?;
        wynik.push((p.name().to_string(), b));
    }
    if !wynik.iter().any(|(n, _)| n == "word/document.xml") {
        return Err(blad_pliku("brak word/document.xml"));
    }
    Ok(wynik)
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

/// Wypełnia szablon. `tylko` – wycięcie samego bloku warunkowego (np. „załącznik 5”).
pub fn wypelnij(
    szablon: &[u8],
    dane: &DaneSzablonu,
    tylko: Option<&str>,
) -> Wynik<(Vec<u8>, RaportSzablonu)> {
    let mut czesci = rozpakuj(szablon)?;
    let mut r = RaportSzablonu::default();
    if bez_makr(&mut czesci) {
        r.ostrzezenia
            .push("szablon zawierał makra (.docm) – w dokumencie zostały usunięte".into());
    }
    for (n, b) in czesci.iter_mut() {
        if !czesc_z_polami(n) {
            continue;
        }
        let tekst = String::from_utf8(std::mem::take(b))
            .map_err(|_| blad_pliku(format!("{n}: nieprawidłowe kodowanie")))?;
        let glowna = n == "word/document.xml";
        let mut x = kontrolki_tresci(&tekst, dane, &mut r, glowna);
        x = scal_pola(&x);
        if glowna {
            if let Some(blok) = tylko {
                x = tylko_blok(&x, blok)?;
            }
        }
        x = bloki_warunkowe(x, dane, &mut r);
        x = wstaw_bloki(x, dane, &mut r, glowna);
        x = zamien_pola(&x, dane, &mut r);
        *b = x.into_bytes();
    }
    Ok((spakuj(&czesci)?, r))
}

/// Analiza szablonu bez danych: znalezione pola i bloki (do raportu po wgraniu pliku).
/// `znane` – dane z kompletem kluczy (wartości bez znaczenia) do rozpoznania nieznanych pól.
pub fn analizuj(szablon: &[u8], znane: &DaneSzablonu) -> Wynik<RaportSzablonu> {
    let mut d = znane.clone();
    for w in d.warunki.values_mut() {
        *w = true;
    }
    let (_, mut r) = wypelnij(szablon, &d, None)?;
    r.puste.clear();
    Ok(r)
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
        d.blok("specyfikacja", xml::akapit_tekst("TABELA SPECYFIKACJI [x]", true));
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
        let body = "<w:p><w:r><w:t>[zupełnie nowe pole] [piętro] [1] [adres klienta]</w:t></w:r></w:p>";
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
                    <w:p><w:r><w:t>[[nieznany]]x[[/nieznany]] [[bez końca]]</w:t></w:r></w:p>";
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
        assert!(r.ostrzezenia.iter().any(|o| o.contains("bez końca")));
        assert!(zrownowazony(&doc));
        // makra usunięte
        let typy = czesc(&wynik, "[Content_Types].xml");
        assert!(!typy.contains("macroEnabled") && !typy.contains("vbaProject"));
        assert!(rozpakuj(&wynik).unwrap().iter().all(|(n, _)| !n.contains("vba")));
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
        assert!(doc.contains("<w:color w:val=\"FF3333\"/></w:rPr><w:t xml:space=\"preserve\">7/2027"));
        assert_eq!(r.kontrolki, 2);
        assert_eq!(r.do_uzupelnienia, 1);
        assert!(r.nieznane.is_empty(), "{:?}", r.nieznane);
        assert!(doc.contains("<w:tag w:val=\"cos\"/>"), "nieznana kontrolka zostaje");
        assert!(zrownowazony(&doc));
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
        let r = analizuj(&dokument(body), &dane()).unwrap();
        assert_eq!(r.nieznane, vec!["coś innego".to_string()]);
        assert!(wypelnij(b"to nie jest zip", &dane(), None).is_err());
    }
}
