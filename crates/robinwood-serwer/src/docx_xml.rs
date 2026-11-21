//! Drzewo XML z pozycjami bajtów (do bezpiecznej edycji części dokumentu Word) oraz walidator
//! struktury WordprocessingML uruchamiany po KAŻDYM wygenerowaniu dokumentu.
//!
//! Silnik szablonów (`szablon_docx`) edytuje XML tekstowo – zakresami bajtów. Żeby edycje nie
//! psuły struktury (np. usunięcie fragmentu z połową tabeli), operacje „usuń zakres treści”
//! i „zamień kontrolkę” liczą się na tym drzewie, a wynik jest sprawdzany walidatorem:
//! poprawność XML (domknięte elementy, encje, znaki dozwolone w XML 1.0), dozwolone dzieci
//! `w:body`/`w:tbl`/`w:tr`/`w:tc`/`w:p`, każda komórka kończy się akapitem, odwołania `r:id`
//! do istniejących relacji. Błąd = czytelny komunikat zamiast zepsutego pliku.

use std::collections::BTreeSet;

use quick_xml::events::Event;
use quick_xml::Reader;

#[derive(Debug, Clone)]
pub struct Wezel {
    /// Nazwa z prefiksem (`w:p`); pusta dla węzła tekstu.
    pub nazwa: String,
    /// Początek znacznika otwierającego (dla tekstu – początek tekstu).
    pub od: usize,
    /// Koniec znacznika otwierającego.
    pub otw_do: usize,
    /// Początek znacznika zamykającego (dla elementu pustego i tekstu = `do_`).
    pub zam_od: usize,
    /// Koniec elementu (wyłącznie).
    pub do_: usize,
    pub rodzic: Option<usize>,
    pub dzieci: Vec<usize>,
}

impl Wezel {
    pub fn tekst(&self) -> bool {
        self.nazwa.is_empty()
    }
}

/// Drzewo części XML; `wezly[0]` to wirtualny korzeń obejmujący cały dokument.
#[derive(Debug, Clone)]
pub struct Drzewo {
    pub wezly: Vec<Wezel>,
}

/// Znak dozwolony w XML 1.0 (`#x9 | #xA | #xD | [#x20-#xD7FF] | [#xE000-#xFFFD] | [#x10000-#x10FFFF]`).
pub fn znak_dozwolony(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\r')
        || ('\u{20}'..='\u{D7FF}').contains(&c)
        || ('\u{E000}'..='\u{FFFD}').contains(&c)
        || c >= '\u{10000}'
}

fn opis_znaku(c: char) -> String {
    format!("U+{:04X}", c as u32)
}

fn sprawdz_odwolanie(nazwa: &str) -> Result<(), String> {
    if let Some(n) = nazwa.strip_prefix('#') {
        let v = match n.strip_prefix('x') {
            Some(h) => u32::from_str_radix(h, 16).ok(),
            None => n.parse::<u32>().ok(),
        };
        match v.and_then(char::from_u32) {
            Some(c) if znak_dozwolony(c) => Ok(()),
            _ => Err(format!("odwołanie do znaku „&{nazwa};” niedozwolone w XML")),
        }
    } else if matches!(nazwa, "lt" | "gt" | "amp" | "apos" | "quot") {
        Ok(())
    } else {
        Err(format!("nieznana encja „&{nazwa};”"))
    }
}

/// Sprawdza surowy fragment (tekst lub znacznik) – niedozwolone znaki i odwołania `&…;`.
fn sprawdz_surowy(s: &str) -> Result<(), String> {
    if let Some(c) = s.chars().find(|c| !znak_dozwolony(*c)) {
        return Err(format!("niedozwolony znak {} w XML", opis_znaku(c)));
    }
    let mut reszta = s;
    while let Some(i) = reszta.find('&') {
        let r = &reszta[i + 1..];
        let k = r
            .find(';')
            .filter(|k| *k > 0 && *k < 40)
            .ok_or_else(|| "znak „&” bez encji (powinien być zapisany jako &amp;)".to_string())?;
        sprawdz_odwolanie(&r[..k])?;
        reszta = &r[k + 1..];
    }
    Ok(())
}

/// Parsuje część XML do drzewa; błąd = XML niepoprawny (z opisem).
pub fn parsuj(x: &str) -> Result<Drzewo, String> {
    let mut r = Reader::from_str(x);
    r.config_mut().check_end_names = true;
    let mut wezly = vec![Wezel {
        nazwa: "#dokument".into(),
        od: 0,
        otw_do: 0,
        zam_od: x.len(),
        do_: x.len(),
        rodzic: None,
        dzieci: vec![],
    }];
    let mut stos = vec![0usize];
    let mut korzenie = 0usize;
    loop {
        let przed = r.buffer_position() as usize;
        let ev = r
            .read_event()
            .map_err(|e| format!("{e} (bajt {})", r.error_position()))?;
        let po = r.buffer_position() as usize;
        let rodzic = *stos.last().unwrap_or(&0);
        let dodaj = |wezly: &mut Vec<Wezel>, w: Wezel| -> usize {
            let id = wezly.len();
            wezly.push(w);
            wezly[rodzic].dzieci.push(id);
            id
        };
        match ev {
            Event::Start(e) => {
                sprawdz_surowy(&x[przed..po])?;
                if rodzic == 0 {
                    korzenie += 1;
                }
                let nazwa = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                let id = dodaj(
                    &mut wezly,
                    Wezel {
                        nazwa,
                        od: przed,
                        otw_do: po,
                        zam_od: po,
                        do_: po,
                        rodzic: Some(rodzic),
                        dzieci: vec![],
                    },
                );
                stos.push(id);
            }
            Event::Empty(e) => {
                sprawdz_surowy(&x[przed..po])?;
                if rodzic == 0 {
                    korzenie += 1;
                }
                let nazwa = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                dodaj(
                    &mut wezly,
                    Wezel {
                        nazwa,
                        od: przed,
                        otw_do: po,
                        zam_od: po,
                        do_: po,
                        rodzic: Some(rodzic),
                        dzieci: vec![],
                    },
                );
            }
            Event::End(_) => {
                let id = stos.pop().unwrap_or(0);
                if id == 0 {
                    return Err(format!(
                        "znacznik zamykający bez otwierającego (bajt {przed})"
                    ));
                }
                wezly[id].zam_od = przed;
                wezly[id].do_ = po;
            }
            Event::Text(_) | Event::CData(_) | Event::GeneralRef(_) => {
                if matches!(ev, Event::Text(_) | Event::GeneralRef(_)) {
                    sprawdz_surowy(&x[przed..po])?;
                }
                if rodzic == 0 && !x[przed..po].trim().is_empty() {
                    return Err("tekst poza elementem głównym".into());
                }
                // sąsiednie fragmenty tekstu (tekst + encja + tekst) to jeden węzeł
                let ostatni = wezly[rodzic].dzieci.last().copied();
                match ostatni {
                    Some(o) if wezly[o].tekst() && wezly[o].do_ == przed => {
                        wezly[o].do_ = po;
                        wezly[o].zam_od = po;
                        wezly[o].otw_do = wezly[o].od;
                    }
                    _ => {
                        dodaj(
                            &mut wezly,
                            Wezel {
                                nazwa: String::new(),
                                od: przed,
                                otw_do: przed,
                                zam_od: po,
                                do_: po,
                                rodzic: Some(rodzic),
                                dzieci: vec![],
                            },
                        );
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if stos.len() != 1 {
        let id = stos[stos.len() - 1];
        return Err(format!(
            "element <{}> nie został zamknięty",
            wezly[id].nazwa
        ));
    }
    if korzenie != 1 {
        return Err("dokument XML musi mieć dokładnie jeden element główny".into());
    }
    Ok(Drzewo { wezly })
}

impl Drzewo {
    pub fn w(&self, i: usize) -> &Wezel {
        &self.wezly[i]
    }

    /// Przodkowie od korzenia do węzła (włącznie z nim).
    pub fn sciezka(&self, i: usize) -> Vec<usize> {
        let mut s = vec![i];
        let mut x = i;
        while let Some(r) = self.wezly[x].rodzic {
            s.push(r);
            x = r;
        }
        s.reverse();
        s
    }

    /// Najbliższy przodek (lub sam węzeł) o danej nazwie.
    pub fn przodek(&self, i: usize, nazwa: &str) -> Option<usize> {
        let mut x = Some(i);
        while let Some(v) = x {
            if self.wezly[v].nazwa == nazwa {
                return Some(v);
            }
            x = self.wezly[v].rodzic;
        }
        None
    }

    pub fn dzieci_el(&self, i: usize) -> impl Iterator<Item = usize> + '_ {
        self.wezly[i]
            .dzieci
            .iter()
            .copied()
            .filter(|d| !self.wezly[*d].tekst())
    }

    pub fn dziecko(&self, i: usize, nazwa: &str) -> Option<usize> {
        self.dzieci_el(i).find(|d| self.wezly[*d].nazwa == nazwa)
    }

    /// Wszystkie elementy o nazwie (w kolejności dokumentu).
    pub fn wszystkie(&self, nazwa: &str) -> Vec<usize> {
        (0..self.wezly.len())
            .filter(|i| self.wezly[*i].nazwa == nazwa)
            .collect()
    }

    /// Wartość atrybutu z surowego znacznika otwierającego.
    pub fn atrybut<'a>(&self, x: &'a str, i: usize, nazwa: &str) -> Option<&'a str> {
        let w = &self.wezly[i];
        atrybut_znacznika(&x[w.od..w.otw_do], nazwa)
    }
}

/// Wartość atrybutu `nazwa="…"` w tekście znacznika.
pub fn atrybut_znacznika<'a>(tag: &'a str, nazwa: &str) -> Option<&'a str> {
    let mut poz = 0;
    while let Some(i) = tag[poz..].find(nazwa) {
        let a = poz + i;
        let przed_ok = a > 0 && tag[..a].chars().last().is_some_and(|c| c.is_whitespace());
        let reszta = &tag[a + nazwa.len()..];
        let reszta_t = reszta.trim_start();
        if przed_ok && reszta_t.starts_with('=') {
            let r = reszta_t[1..].trim_start();
            let cudz = r.chars().next()?;
            if cudz == '"' || cudz == '\'' {
                let k = r[1..].find(cudz)?;
                return Some(&r[1..1 + k]);
            }
        }
        poz = a + nazwa.len();
    }
    None
}

// ---------------------------------------------------------------------------
// Edycje zakresami
// ---------------------------------------------------------------------------

/// Zamiana fragmentu `[od, do_)` na tekst.
#[derive(Debug, Clone, PartialEq)]
pub struct Edycja {
    pub od: usize,
    pub do_: usize,
    pub nowe: String,
}

impl Edycja {
    pub fn usun(od: usize, do_: usize) -> Self {
        Self {
            od,
            do_,
            nowe: String::new(),
        }
    }
}

/// Stosuje edycje (nie mogą się nakładać).
pub fn zastosuj(x: &str, mut edycje: Vec<Edycja>) -> Result<String, String> {
    edycje.sort_by_key(|e| (e.od, e.do_));
    edycje.dedup();
    let mut s = String::with_capacity(x.len());
    let mut poz = 0;
    for e in edycje {
        if e.od < poz {
            return Err("wewnętrzny błąd edycji szablonu (nakładające się zmiany)".into());
        }
        s.push_str(&x[poz..e.od]);
        s.push_str(&e.nowe);
        poz = e.do_;
    }
    s.push_str(&x[poz..]);
    Ok(s)
}

/// Kontenery treści blokowej (akapity i tabele).
pub fn kontener_blokowy(nazwa: &str) -> bool {
    matches!(
        nazwa,
        "w:body"
            | "w:tc"
            | "w:sdtContent"
            | "w:txbxContent"
            | "w:hdr"
            | "w:ftr"
            | "w:footnote"
            | "w:endnote"
            | "w:comment"
            | "w:docPartBody"
    )
}

/// Rodzaj „atomu” treści akapitu (do wykrywania pustych akapitów i podziałów wiersza).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Atom {
    Tekst,
    /// `w:br` bez typu (Shift+Enter) albo `w:cr`.
    Podzial,
    /// Inna widoczna treść: tabulator, rysunek, pole, podział strony…
    Inny,
}

/// Atomy treści akapitu w kolejności dokumentu: (od, do_, rodzaj). Tekst – tylko z `w:t`,
/// niebiałe znaki; elementy właściwości (`w:pPr`, `w:rPr`) i usunięte wersje są pomijane.
pub fn atomy(d: &Drzewo, x: &str, akapit: usize) -> Vec<(usize, usize, Atom)> {
    let mut wynik = Vec::new();
    fn idz(d: &Drzewo, x: &str, i: usize, w_t: bool, wynik: &mut Vec<(usize, usize, Atom)>) {
        let w = d.w(i);
        if w.tekst() {
            if w_t {
                let t = &x[w.od..w.do_];
                // pozycje pierwszego i ostatniego niebiałego znaku
                if let (Some(a), Some(b)) = (
                    t.find(|c: char| !c.is_whitespace()),
                    t.rfind(|c: char| !c.is_whitespace()),
                ) {
                    let dl = t[b..].chars().next().map(char::len_utf8).unwrap_or(1);
                    wynik.push((w.od + a, w.od + b + dl, Atom::Tekst));
                }
            }
            return;
        }
        match w.nazwa.as_str() {
            "w:pPr"
            | "w:rPr"
            | "w:del"
            | "w:delText"
            | "w:proofErr"
            | "w:bookmarkStart"
            | "w:bookmarkEnd"
            | "w:lastRenderedPageBreak"
            | "w:commentRangeStart"
            | "w:commentRangeEnd"
            | "w:permStart"
            | "w:permEnd"
            | "w:sdtPr"
            | "w:sdtEndPr" => {}
            "w:br" | "w:cr" => {
                let typ = d.atrybut(x, i, "w:type");
                let rodzaj = if typ.is_none() || typ == Some("textWrapping") {
                    Atom::Podzial
                } else {
                    Atom::Inny
                };
                wynik.push((w.od, w.do_, rodzaj));
            }
            "w:tab"
            | "w:drawing"
            | "w:pict"
            | "w:object"
            | "w:sym"
            | "w:fldChar"
            | "w:instrText"
            | "w:footnoteReference"
            | "w:endnoteReference"
            | "w:noBreakHyphen"
            | "w:softHyphen"
            | "w:ptab"
            | "w:fldSimple"
            | "w:commentReference" => {
                wynik.push((w.od, w.do_, Atom::Inny));
            }
            n => {
                let t = n == "w:t";
                for &c in &w.dzieci {
                    idz(d, x, c, t, wynik);
                }
            }
        }
    }
    idz(d, x, akapit, false, &mut wynik);
    wynik
}

/// Treść widoczna akapitu przed pozycją `poz` (z uwzględnieniem tekstu do `poz`).
fn tresc_przed(x: &str, at: &[(usize, usize, Atom)], poz: usize) -> Vec<(usize, usize, Atom)> {
    let mut w = Vec::new();
    for &(a, b, r) in at {
        if b <= poz {
            w.push((a, b, r));
        } else if a < poz && r == Atom::Tekst && !x[a..poz].trim().is_empty() {
            w.push((a, poz, r));
        }
    }
    w
}

fn tresc_po(x: &str, at: &[(usize, usize, Atom)], poz: usize) -> Vec<(usize, usize, Atom)> {
    let mut w = Vec::new();
    for &(a, b, r) in at {
        if a >= poz {
            w.push((a, b, r));
        } else if b > poz && r == Atom::Tekst && !x[poz..b].trim().is_empty() {
            w.push((poz, b, r));
        }
    }
    w
}

/// Ustawienia sekcji (`w:sectPr`) w akapicie – nie wolno ich usunąć razem z akapitem.
pub fn ustawienia_sekcji<'a>(d: &Drzewo, x: &'a str, akapit: usize) -> Option<&'a str> {
    let ppr = d.dziecko(akapit, "w:pPr")?;
    let s = d.dziecko(ppr, "w:sectPr")?;
    Some(&x[d.w(s).od..d.w(s).do_])
}

/// Usunięcie całego akapitu (z zachowaniem ustawień sekcji w pustym akapicie).
pub fn usun_akapit(d: &Drzewo, x: &str, akapit: usize) -> Edycja {
    let w = d.w(akapit);
    match ustawienia_sekcji(d, x, akapit) {
        Some(s) => Edycja {
            od: w.od,
            do_: w.do_,
            nowe: format!("<w:p><w:pPr>{s}</w:pPr></w:p>"),
        },
        None => Edycja::usun(w.od, w.do_),
    }
}

/// Usunięcie elementu blokowego (akapit z zachowaniem sekcji, tabela, kontrolka…).
fn usun_blok(d: &Drzewo, x: &str, i: usize) -> Edycja {
    if d.w(i).nazwa == "w:p" {
        usun_akapit(d, x, i)
    } else {
        Edycja::usun(d.w(i).od, d.w(i).do_)
    }
}

/// Liczba otwarć i zamknięć pól Worda (`w:fldChar`) we fragmencie.
fn bilans_pol(f: &str) -> i64 {
    let a = f.matches("w:fldCharType=\"begin\"").count() as i64;
    let b = f.matches("w:fldCharType=\"end\"").count() as i64;
    a - b
}

/// Usuwa treść między punktem `(t1, p1)` a `(t2, p2)` (pozycje bajtów wewnątrz węzłów tekstu
/// `w:t`, `p1 ≤ p2`) – jak „zaznacz i usuń” w edytorze, ale bez łączenia akapitów: tekst przed
/// `p1` i po `p2` zostaje w swoich akapitach, akapity pomiędzy znikają w całości, akapit, w którym
/// nie zostaje żadna treść, znika (ustawienia sekcji zostają). Podział wiersza (Shift+Enter)
/// przylegający do usuniętego fragmentu jest usuwany razem z nim, żeby nie zostawała pusta linia.
///
/// Odmowa (`Err`), gdy usunięcie nie jest bezpieczne: punkty w różnych komórkach/tabelach,
/// jeden w tabeli a drugi poza nią, fragment przecina pole Worda.
pub fn usun_zakres(
    d: &Drzewo,
    x: &str,
    (t1, p1): (usize, usize),
    (t2, p2): (usize, usize),
) -> Result<Vec<Edycja>, String> {
    let s1 = d.sciezka(t1);
    let s2 = d.sciezka(t2);
    let mut k = 0;
    while k + 1 < s1.len() && k + 1 < s2.len() && s1[k + 1] == s2[k + 1] {
        k += 1;
    }
    let wspolny = s1[k];
    let mut edycje: Vec<Edycja> = Vec::new();
    let akapit_wspolny = d.przodek(wspolny, "w:p");

    if t1 == t2 {
        edycje.push(Edycja::usun(p1, p2));
    } else {
        edycje.push(Edycja::usun(p1, d.w(t1).do_));
        edycje.push(Edycja::usun(d.w(t2).od, p2));
        // strona początku: następne rodzeństwo na każdym poziomie poniżej wspólnego przodka
        for i in (k + 2..s1.len()).rev() {
            let wezel = s1[i];
            let rodzic = s1[i - 1];
            let dz = &d.w(rodzic).dzieci;
            let poz = dz.iter().position(|c| *c == wezel).unwrap_or(0);
            for &c in &dz[poz + 1..] {
                edycje.push(usun_blok(d, x, c));
            }
        }
        // poziom wspólnego przodka: rodzeństwo pomiędzy
        if s1.len() > k + 1 && s2.len() > k + 1 {
            let a = s1[k + 1];
            let b = s2[k + 1];
            let dz = &d.w(wspolny).dzieci;
            let pa = dz.iter().position(|c| *c == a).unwrap_or(0);
            let pb = dz.iter().position(|c| *c == b).unwrap_or(0);
            for &c in &dz[pa + 1..pb] {
                edycje.push(usun_blok(d, x, c));
            }
        }
        // strona końca: wcześniejsze rodzeństwo
        for i in k + 2..s2.len() {
            let wezel = s2[i];
            let rodzic = s2[i - 1];
            let dz = &d.w(rodzic).dzieci;
            let poz = dz.iter().position(|c| *c == wezel).unwrap_or(0);
            for &c in &dz[..poz] {
                edycje.push(usun_blok(d, x, c));
            }
        }
    }

    // pola Worda nie mogą zostać przecięte
    let usuwane: String = edycje.iter().map(|e| &x[e.od..e.do_]).collect();
    if bilans_pol(&usuwane) != 0 {
        return Err(
            "fragment przecina pole Worda (np. numer strony, spis treści) – przesuń znacznik poza pole"
                .into(),
        );
    }

    if let Some(p) = akapit_wspolny {
        // oba punkty w tym samym akapicie
        let at = atomy(d, x, p);
        let przed = tresc_przed(x, &at, p1);
        let po = tresc_po(x, &at, p2);
        if przed.is_empty() && po.is_empty() {
            return Ok(vec![usun_akapit(d, x, p)]);
        }
        let ost = przed.last().map(|a| a.2);
        let pierw = po.first().map(|a| a.2);
        match (ost, pierw) {
            (Some(Atom::Podzial), Some(Atom::Podzial)) | (None, Some(Atom::Podzial)) => {
                let (a, b, _) = po[0];
                edycje.push(Edycja::usun(a, b));
            }
            (Some(Atom::Podzial), None) => {
                let (a, b, _) = przed[przed.len() - 1];
                edycje.push(Edycja::usun(a, b));
            }
            _ => {}
        }
        return Ok(edycje);
    }

    // różne akapity: muszą leżeć w tym samym kontenerze blokowym
    let a = s1.get(k + 1).copied();
    let b = s2.get(k + 1).copied();
    let nazwa_w = d.w(wspolny).nazwa.as_str();
    let ok = kontener_blokowy(nazwa_w)
        && a.is_some_and(|a| d.w(a).nazwa == "w:p")
        && b.is_some_and(|b| d.w(b).nazwa == "w:p");
    if !ok {
        let opis = match nazwa_w {
            "w:tbl" | "w:tr" => "znaczniki stoją w różnych wierszach lub komórkach tej samej tabeli",
            _ if [a, b].iter().flatten().any(|i| d.w(*i).nazwa == "w:tbl") => {
                "jeden znacznik stoi w tabeli, a drugi poza nią"
            }
            _ => "znaczniki stoją na różnych poziomach dokumentu (np. w kontrolce treści albo polu tekstowym)",
        };
        return Err(format!(
            "{opis} – umieść oba znaczniki bloku na tym samym poziomie (oba poza tabelą albo oba w tej samej komórce)"
        ));
    }
    let (a, b) = (a.unwrap_or(0), b.unwrap_or(0));
    // akapit początkowy: zostaje treść przed znacznikiem
    let at1 = atomy(d, x, a);
    let przed = tresc_przed(x, &at1, p1);
    let wa = d.w(a);
    if przed.iter().all(|z| z.2 == Atom::Podzial) {
        edycje.retain(|e| e.do_ <= wa.od || e.od >= wa.do_);
        edycje.push(usun_akapit(d, x, a));
    } else if let Some(&(o, p, Atom::Podzial)) = przed.last() {
        edycje.push(Edycja::usun(o, p));
    }
    // akapit końcowy: zostaje treść po znaczniku
    let at2 = atomy(d, x, b);
    let po = tresc_po(x, &at2, p2);
    let wb = d.w(b);
    if po.iter().all(|z| z.2 == Atom::Podzial) {
        edycje.retain(|e| e.do_ <= wb.od || e.od >= wb.do_);
        edycje.push(usun_akapit(d, x, b));
    } else if let Some(&(o, p, Atom::Podzial)) = po.first() {
        edycje.push(Edycja::usun(o, p));
    }
    Ok(edycje)
}

// ---------------------------------------------------------------------------
// Walidator struktury
// ---------------------------------------------------------------------------

/// Elementy, które nie mogą być bezpośrednimi dziećmi danego rodzica.
fn zakazane_dzieci(rodzic: &str) -> &'static [&'static str] {
    match rodzic {
        "w:body" | "w:hdr" | "w:ftr" | "w:footnote" | "w:endnote" | "w:txbxContent" => {
            &["w:r", "w:t", "w:tr", "w:tc", "w:body"]
        }
        "w:tbl" => &["w:p", "w:tbl", "w:r", "w:t", "w:tc"],
        "w:tr" => &["w:p", "w:tbl", "w:r", "w:t", "w:tr"],
        "w:tc" => &["w:r", "w:t", "w:tr", "w:tc"],
        "w:p" => &["w:p", "w:tbl", "w:tr", "w:tc", "w:t"],
        "w:r" => &["w:p", "w:r", "w:tbl", "w:tr", "w:tc"],
        _ => &[],
    }
}

fn opis_miejsca(d: &Drzewo, x: &str, i: usize) -> String {
    // tekst najbliższego akapitu – żeby użytkownik znalazł miejsce w Wordzie
    let p = d.przodek(i, "w:p").or_else(|| {
        let mut n = i;
        loop {
            if let Some(c) = d.w(n).dzieci.iter().find(|c| d.w(**c).nazwa == "w:p") {
                return Some(*c);
            }
            n = *d.w(n).dzieci.first()?;
        }
    });
    let t = p.map(|p| tekst_elementu(d, x, p)).unwrap_or_default();
    let t: String = t.chars().take(60).collect();
    if t.trim().is_empty() {
        String::new()
    } else {
        format!(" (w okolicy tekstu „{}”)", t.trim())
    }
}

/// Tekst elementu (zawartość `w:t`, bez rozkodowania encji poza podstawowymi).
pub fn tekst_elementu(d: &Drzewo, x: &str, i: usize) -> String {
    let mut s = String::new();
    fn idz(d: &Drzewo, x: &str, i: usize, w_t: bool, s: &mut String) {
        let w = d.w(i);
        if w.tekst() {
            if w_t {
                s.push_str(&x[w.od..w.do_]);
            }
            return;
        }
        let t = w.nazwa == "w:t";
        for &c in &w.dzieci {
            idz(d, x, c, t, s);
        }
    }
    idz(d, x, i, false, &mut s);
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&#91;", "[")
        .replace("&#93;", "]")
        .replace("&amp;", "&")
}

/// Walidacja części dokumentu: poprawność XML, dozwolone dzieci, komórki zakończone akapitem,
/// odwołania `r:id`/`r:embed`/`r:link` do istniejących relacji (gdy `relacje` podane).
pub fn sprawdz_czesc(
    nazwa: &str,
    x: &str,
    relacje: Option<&BTreeSet<String>>,
) -> Result<(), String> {
    let d = parsuj(x).map_err(|e| format!("{nazwa}: niepoprawny XML – {e}"))?;
    for (i, w) in d.wezly.iter().enumerate().skip(1) {
        if w.tekst() {
            continue;
        }
        let rodzic = w.rodzic.map(|r| d.w(r).nazwa.as_str()).unwrap_or("");
        if zakazane_dzieci(rodzic).contains(&w.nazwa.as_str()) {
            return Err(format!(
                "{nazwa}: element <{}> nie może stać bezpośrednio w <{rodzic}>{}",
                w.nazwa,
                opis_miejsca(&d, x, i)
            ));
        }
        if w.nazwa == "w:tc" {
            let ostatni = d
                .dzieci_el(i)
                .filter(|c| {
                    matches!(
                        d.w(*c).nazwa.as_str(),
                        "w:p" | "w:tbl" | "w:sdt" | "w:customXml"
                    )
                })
                .last();
            match ostatni {
                Some(c) if d.w(c).nazwa == "w:tbl" => {
                    return Err(format!(
                        "{nazwa}: komórka tabeli kończy się tabelą zamiast akapitu{}",
                        opis_miejsca(&d, x, i)
                    ))
                }
                None => {
                    return Err(format!(
                        "{nazwa}: pusta komórka tabeli (bez akapitu){}",
                        opis_miejsca(&d, x, i)
                    ))
                }
                _ => {}
            }
        }
        if let Some(rel) = relacje {
            let tag = &x[w.od..w.otw_do];
            for a in ["r:id", "r:embed", "r:link", "r:pict", "r:href"] {
                if let Some(v) = atrybut_znacznika(tag, a) {
                    if !rel.contains(v) {
                        return Err(format!(
                            "{nazwa}: odwołanie {a}=\"{v}\" do nieistniejącej relacji{}",
                            opis_miejsca(&d, x, i)
                        ));
                    }
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod testy {
    use super::*;

    const NS: &str = "xmlns:w=\"w\" xmlns:r=\"r\"";

    fn dok(body: &str) -> String {
        format!("<w:document {NS}><w:body>{body}</w:body></w:document>")
    }

    #[test]
    fn parsowanie_i_bledy_xml() {
        assert!(parsuj(&dok("<w:p><w:r><w:t>a &amp; b</w:t></w:r></w:p>")).is_ok());
        assert!(parsuj(&dok("<w:p><w:r><w:t>a</w:r></w:p>")).is_err());
        assert!(parsuj(&dok("<w:p>")).is_err());
        assert!(parsuj(&dok("<w:p><w:r><w:t>a & b</w:t></w:r></w:p>")).is_err());
        assert!(parsuj(&dok("<w:p><w:r><w:t>a &#xFFFE; b</w:t></w:r></w:p>")).is_err());
        assert!(parsuj(&dok("<w:p><w:r><w:t>a \u{FFFF} b</w:t></w:r></w:p>")).is_err());
        assert!(parsuj(&dok("<w:p><w:r><w:t>a \u{1} b</w:t></w:r></w:p>")).is_err());
        assert!(parsuj(&dok("<w:p><w:r><w:t>&nbsp;</w:t></w:r></w:p>")).is_err());
        let d = parsuj(&dok("<w:p><w:r><w:t>x &lt; y</w:t></w:r></w:p>")).unwrap();
        let t = d.wszystkie("w:t")[0];
        assert_eq!(d.w(t).dzieci.len(), 1, "tekst z encją to jeden węzeł");
    }

    #[test]
    fn walidator_struktury() {
        let ok = dok("<w:tbl><w:tr><w:tc><w:p/></w:tc></w:tr></w:tbl><w:p/>");
        assert!(sprawdz_czesc("d", &ok, None).is_ok());
        let zly = dok("<w:tbl><w:tr><w:tc><w:p/><w:tbl><w:tr><w:tc><w:p/></w:tc></w:tr></w:tbl></w:tc></w:tr></w:tbl>");
        assert!(sprawdz_czesc("d", &zly, None)
            .unwrap_err()
            .contains("kończy się tabelą"));
        let zly = dok("<w:tbl><w:p/></w:tbl>");
        assert!(sprawdz_czesc("d", &zly, None).is_err());
        let zly = dok("<w:tbl><w:tr><w:p/></w:tr></w:tbl>");
        assert!(sprawdz_czesc("d", &zly, None).is_err());
        let zly = dok("<w:tbl><w:tr><w:tc></w:tc></w:tr></w:tbl>");
        assert!(sprawdz_czesc("d", &zly, None).is_err());
        let rel: BTreeSet<String> = ["rId1".to_string()].into();
        let r = dok("<w:p><w:hyperlink r:id=\"rId2\"/></w:p>");
        assert!(sprawdz_czesc("d", &r, Some(&rel)).is_err());
        let r = dok("<w:p><w:hyperlink r:id=\"rId1\"/></w:p>");
        assert!(sprawdz_czesc("d", &r, Some(&rel)).is_ok());
    }

    fn punkt(d: &Drzewo, x: &str, znak: &str) -> (usize, usize) {
        let poz = x.find(znak).unwrap();
        let t = (0..d.wezly.len())
            .find(|i| d.w(*i).tekst() && d.w(*i).od <= poz && poz < d.w(*i).do_)
            .unwrap();
        (t, poz)
    }

    fn usun(x: &str, a: &str, b: &str) -> Result<String, String> {
        let d = parsuj(x)?;
        let (t1, p1) = punkt(&d, x, a);
        let (t2, mut p2) = punkt(&d, x, b);
        p2 += b.len();
        let e = usun_zakres(&d, x, (t1, p1), (t2, p2))?;
        let w = zastosuj(x, e)?;
        sprawdz_czesc("t", &w, None)?;
        Ok(w)
    }

    fn tekst(x: &str) -> String {
        let d = parsuj(x).unwrap();
        d.wszystkie("w:p")
            .into_iter()
            .map(|p| tekst_elementu(&d, x, p))
            .collect::<Vec<_>>()
            .join("|")
    }

    #[test]
    fn usuwanie_w_akapicie_z_podzialem_wiersza() {
        // Shift+Enter: znaczniki w jednym akapicie z innym tekstem
        let x = dok("<w:p><w:r><w:t>Przed</w:t><w:br/><w:t>[[a]]</w:t><w:br/><w:t>Środek</w:t><w:br/><w:t>[[/a]]</w:t><w:br/><w:t>Po</w:t></w:r></w:p>");
        let w = usun(&x, "[[a]]", "[[/a]]").unwrap();
        assert_eq!(tekst(&w), "PrzedPo");
        assert_eq!(w.matches("<w:br/>").count(), 1, "{w}");
    }

    #[test]
    fn usuwanie_miedzy_akapitami_zachowuje_sasiednie() {
        let x = dok("<w:p><w:r><w:t>§1 treść. [[a]]ukryte</w:t></w:r></w:p>\
                     <w:p><w:r><w:t>środek</w:t></w:r></w:p>\
                     <w:tbl><w:tr><w:tc><w:p><w:r><w:t>komórka</w:t></w:r></w:p></w:tc></w:tr></w:tbl>\
                     <w:p><w:r><w:t>ukryte[[/a]] §2 treść.</w:t></w:r></w:p><w:p><w:r><w:t>§3</w:t></w:r></w:p>");
        let w = usun(&x, "[[a]]", "[[/a]]").unwrap();
        assert_eq!(tekst(&w), "§1 treść. | §2 treść.|§3");
        assert!(!w.contains("<w:tbl>"));
    }

    #[test]
    fn akapity_z_samymi_znacznikami_znikaja_sekcja_zostaje() {
        let x = dok("<w:p><w:r><w:t>A</w:t></w:r></w:p><w:p><w:r><w:t>[[a]]</w:t></w:r></w:p>\
                     <w:p><w:pPr><w:sectPr><w:pgSz w:w=\"1\"/></w:sectPr></w:pPr><w:r><w:t>x</w:t></w:r></w:p>\
                     <w:p><w:r><w:t>[[/a]]</w:t></w:r></w:p><w:p><w:r><w:t>B</w:t></w:r></w:p>");
        let w = usun(&x, "[[a]]", "[[/a]]").unwrap();
        assert_eq!(tekst(&w), "A||B");
        assert!(w.contains("<w:sectPr>"), "ustawienia sekcji zostają");
    }

    #[test]
    fn odmowa_dla_tabeli_i_pola() {
        let x = dok("<w:p><w:r><w:t>[[a]]</w:t></w:r></w:p><w:tbl><w:tr><w:tc><w:p><w:r><w:t>[[/a]]</w:t></w:r></w:p></w:tc></w:tr></w:tbl>");
        assert!(usun(&x, "[[a]]", "[[/a]]")
            .unwrap_err()
            .contains("w tabeli"));
        let x = dok("<w:tbl><w:tr><w:tc><w:p><w:r><w:t>[[a]]</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>[[/a]]</w:t></w:r></w:p></w:tc></w:tr></w:tbl>");
        assert!(usun(&x, "[[a]]", "[[/a]]")
            .unwrap_err()
            .contains("komórkach"));
        let x = dok("<w:p><w:r><w:t>[[a]]</w:t></w:r><w:r><w:fldChar w:fldCharType=\"begin\"/></w:r></w:p><w:p><w:r><w:t>[[/a]]</w:t></w:r><w:r><w:fldChar w:fldCharType=\"end\"/></w:r></w:p>");
        assert!(usun(&x, "[[a]]", "[[/a]]").unwrap_err().contains("pole"));
    }

    #[test]
    fn znacznik_w_srodku_zdania() {
        let x = dok("<w:p><w:r><w:t xml:space=\"preserve\">Zamawiający </w:t></w:r><w:r><w:rPr><w:b/></w:rPr><w:t>[[a]]wariant[[/a]]</w:t></w:r><w:r><w:t xml:space=\"preserve\"> płaci.</w:t></w:r></w:p>");
        let w = usun(&x, "[[a]]", "[[/a]]").unwrap();
        assert_eq!(tekst(&w), "Zamawiający  płaci.");
    }
}
