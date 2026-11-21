//! Render oferty do PDF (genpdf). Czcionka Inter (SIL OFL) wkompilowana w program –
//! polskie znaki działają niezależnie od czcionek systemu. Pliki czcionek w `zasoby/czcionki/`
//! to PODZBIÓR Inter (łacina z rozszerzeniami A/B, interpunkcja, €) przygotowany narzędziem
//! fontTools (`pyftsubset … --no-hinting`) – genpdf/printpdf 0.3 osadzają czcionkę w całości,
//! więc mniejszy plik czcionki = mniejszy PDF (≈ 4 × 60 KB zamiast 4 × 420 KB).

use genpdf::elements::{
    Break, FrameCellDecorator, Image, LinearLayout, PaddedElement, Paragraph, TableLayout,
};
use genpdf::fonts::{FontData, FontFamily};
use genpdf::style::{Color, Style};
use genpdf::{Alignment, Element, Margins, Mm, SimplePageDecorator};

use super::{blad_renderowania, CzescDok, DokumentOferty, ObrazDok, WierszDok};
use crate::bledy::Wynik;

const REGULAR: &[u8] = include_bytes!("../../zasoby/czcionki/Inter-Regular.ttf");
const BOLD: &[u8] = include_bytes!("../../zasoby/czcionki/Inter-Bold.ttf");
const ITALIC: &[u8] = include_bytes!("../../zasoby/czcionki/Inter-Italic.ttf");
const BOLD_ITALIC: &[u8] = include_bytes!("../../zasoby/czcionki/Inter-BoldItalic.ttf");

const AKCENT: Color = Color::Rgb(0x2D, 0x5E, 0x50);
const SZARY: Color = Color::Rgb(0x66, 0x66, 0x66);

fn czcionki() -> Wynik<FontFamily<FontData>> {
    let f = |b: &[u8]| FontData::new(b.to_vec(), None).map_err(blad_renderowania);
    Ok(FontFamily {
        regular: f(REGULAR)?,
        bold: f(BOLD)?,
        italic: f(ITALIC)?,
        bold_italic: f(BOLD_ITALIC)?,
    })
}

fn komorka_styl(t: &str, styl: Style) -> PaddedElement<genpdf::elements::StyledElement<Paragraph>> {
    Paragraph::new(t)
        .styled(styl)
        .padded(Margins::trbl(1, 1.5, 1, 1.5))
}

fn obraz(o: &ObrazDok) -> Option<LinearLayout> {
    let (w, h) = (o.obraz.width() as f64, o.obraz.height() as f64);
    // maks. 120 × 90 mm: dpi tak dobrane, żeby obraz zmieścił się w ramce
    let dpi = (w * 25.4 / 120.0).max(h * 25.4 / 90.0).max(1.0);
    let img = Image::from_dynamic_image(image023::DynamicImage::ImageRgb8(o.obraz.clone()))
        .ok()?
        .with_dpi(dpi);
    let mut l = LinearLayout::vertical();
    l.push(img);
    if let Some(p) = &o.podpis {
        l.push(Paragraph::new(p.as_str()).styled(Style::new().italic().with_font_size(8)));
    }
    l.push(Break::new(0.5));
    Some(l)
}

// ---------------------------------------------------------------------------
// Tabela części z łamaniem stron: wiersz nie dzieli się między strony, nagłówek tabeli
// powtarza się na każdej stronie, a tytuł części / nagłówek podgrupy nie zostaje sam na
// dole strony (trzyma się z pierwszym wierszem danych). genpdf 0.2 tego nie umie
// (TableLayout dzieli komórki), stąd własny element z pomiarem wysokości wiersza.
// ---------------------------------------------------------------------------

enum WierszTabeli {
    Dane(Vec<String>),
    Podgrupa(String),
}

struct TabelaCzesci {
    tytul: String,
    naglowki: Vec<&'static str>,
    wagi: Vec<f64>,
    /// Kolumny wyrównane do prawej (kwoty).
    do_prawej: Vec<bool>,
    wiersze: Vec<WierszTabeli>,
    nastepny: usize,
    /// Poprzednie wywołanie nie zmieściło niczego – teraz jesteśmy na świeżej stronie.
    odlozony: bool,
}

const PAD_X: f64 = 1.5;
const PAD_Y: f64 = 1.0;

fn mm(x: f64) -> Mm {
    Mm::from(x)
}

fn f(x: Mm) -> f64 {
    f64::from(x)
}

/// Łamanie tekstu na linie mieszczące się w szerokości (słowa; za długie słowo – po znakach).
fn zawin(t: &str, szer: f64, styl: Style, fc: &genpdf::fonts::FontCache) -> Vec<String> {
    let mut linie = Vec::new();
    for akapit in t.split('\n') {
        let mut linia = String::new();
        for slowo in akapit.split_whitespace() {
            let proba = if linia.is_empty() {
                slowo.to_string()
            } else {
                format!("{linia} {slowo}")
            };
            if f(styl.str_width(fc, &proba)) <= szer {
                linia = proba;
                continue;
            }
            if !linia.is_empty() {
                linie.push(std::mem::take(&mut linia));
            }
            if f(styl.str_width(fc, slowo)) <= szer {
                linia = slowo.to_string();
            } else {
                for ch in slowo.chars() {
                    let mut p = linia.clone();
                    p.push(ch);
                    if f(styl.str_width(fc, &p)) > szer && !linia.is_empty() {
                        linie.push(std::mem::take(&mut linia));
                        linia.push(ch);
                    } else {
                        linia = p;
                    }
                }
            }
        }
        linie.push(linia);
    }
    if linie.is_empty() {
        linie.push(String::new());
    }
    linie
}

impl TabelaCzesci {
    fn szerokosci(&self, calosc: f64) -> Vec<f64> {
        let suma: f64 = self.wagi.iter().sum();
        self.wagi.iter().map(|w| calosc * w / suma).collect()
    }

    fn style(&self, styl: Style) -> (Style, Style, Style, Style) {
        (
            styl,
            styl.and(Style::new().bold().with_font_size(8).with_color(AKCENT)),
            styl.and(Style::new().bold().italic()),
            styl.and(Style::new().bold().with_font_size(10)),
        )
    }

    fn wys_komorek(
        &self,
        komorki: &[String],
        szer: &[f64],
        styl: Style,
        fc: &genpdf::fonts::FontCache,
    ) -> f64 {
        let lh = f(styl.line_height(fc));
        komorki
            .iter()
            .zip(szer)
            .map(|(t, w)| zawin(t, w - 2.0 * PAD_X, styl, fc).len() as f64 * lh)
            .fold(0.0, f64::max)
            + 2.0 * PAD_Y
    }

    fn wys_wiersza(
        &self,
        i: usize,
        szer: &[f64],
        styl: Style,
        fc: &genpdf::fonts::FontCache,
    ) -> f64 {
        let (zwykly, _, podgrupa, _) = self.style(styl);
        match &self.wiersze[i] {
            WierszTabeli::Dane(k) => self.wys_komorek(k, szer, zwykly, fc),
            WierszTabeli::Podgrupa(_) => f(podgrupa.line_height(fc)) + 2.0 * PAD_Y,
        }
    }

    /// Wysokość „bloku nierozerwalnego” od wiersza `i`: nagłówek podgrupy razem z kolejnym.
    fn wys_bloku(&self, i: usize, szer: &[f64], styl: Style, fc: &genpdf::fonts::FontCache) -> f64 {
        let mut h = self.wys_wiersza(i, szer, styl, fc);
        if matches!(self.wiersze[i], WierszTabeli::Podgrupa(_)) && i + 1 < self.wiersze.len() {
            h += self.wys_wiersza(i + 1, szer, styl, fc);
        }
        h
    }

    fn rysuj_ramke(area: &genpdf::render::Area<'_>, y0: f64, y1: f64, szer: &[f64], pelna: bool) {
        let linia = Style::new().with_color(Color::Rgb(0xC8, 0xC3, 0xB8));
        let calosc: f64 = szer.iter().sum();
        area.draw_line(
            vec![
                genpdf::Position::new(mm(0.0), mm(y1)),
                genpdf::Position::new(mm(calosc), mm(y1)),
            ],
            linia,
        );
        let mut x = 0.0;
        let granice: Vec<f64> = if pelna {
            std::iter::once(0.0)
                .chain(szer.iter().map(|w| {
                    x += w;
                    x
                }))
                .collect()
        } else {
            vec![0.0, calosc]
        };
        for gx in granice {
            area.draw_line(
                vec![
                    genpdf::Position::new(mm(gx), mm(y0)),
                    genpdf::Position::new(mm(gx), mm(y1)),
                ],
                linia,
            );
        }
    }

    fn rysuj_komorki(
        &self,
        area: &genpdf::render::Area<'_>,
        y: f64,
        komorki: &[String],
        szer: &[f64],
        styl: Style,
        fc: &genpdf::fonts::FontCache,
    ) -> Result<f64, genpdf::error::Error> {
        let h = self.wys_komorek(komorki, szer, styl, fc);
        let lh = f(styl.line_height(fc));
        let mut x = 0.0;
        for (k, (t, w)) in komorki.iter().zip(szer).enumerate() {
            for (n, l) in zawin(t, w - 2.0 * PAD_X, styl, fc).iter().enumerate() {
                let dx = if self.do_prawej.get(k).copied().unwrap_or(false) {
                    w - PAD_X - f(styl.str_width(fc, l))
                } else {
                    PAD_X
                };
                area.print_str(
                    fc,
                    genpdf::Position::new(mm(x + dx), mm(y + PAD_Y + n as f64 * lh)),
                    styl,
                    l,
                )?;
            }
            x += w;
        }
        Self::rysuj_ramke(area, y, y + h, szer, true);
        Ok(h)
    }
}

impl Element for TabelaCzesci {
    fn render(
        &mut self,
        context: &genpdf::Context,
        area: genpdf::render::Area<'_>,
        styl: Style,
    ) -> Result<genpdf::RenderResult, genpdf::error::Error> {
        let fc = &context.font_cache;
        let mut wynik = genpdf::RenderResult::default();
        let szer = self.szerokosci(f(area.size().width));
        let wys = f(area.size().height);
        let (zwykly, naglowek, podgrupa, tytulowy) = self.style(styl);
        let naglowki: Vec<String> = self.naglowki.iter().map(|s| s.to_string()).collect();
        let tytul = if self.nastepny == 0 {
            self.tytul.clone()
        } else {
            format!("{} (cd.)", self.tytul)
        };
        let h_tytulu = f(tytulowy.line_height(fc)) + 1.0;
        let h_naglowka = self.wys_komorek(&naglowki, &szer, naglowek, fc);
        let h_pierwszego = if self.nastepny < self.wiersze.len() {
            self.wys_bloku(self.nastepny, &szer, zwykly, fc)
        } else {
            0.0
        };
        // Tytuł + nagłówek + pierwszy blok muszą się zmieścić razem – inaczej nowa strona.
        if h_tytulu + h_naglowka + h_pierwszego > wys && !self.odlozony {
            self.odlozony = true;
            wynik.has_more = true;
            return Ok(wynik);
        }
        self.odlozony = false;
        let mut y = 0.0;
        area.print_str(
            fc,
            genpdf::Position::new(mm(0.0), mm(0.0)),
            tytulowy,
            &tytul,
        )?;
        y += h_tytulu;
        let gora = y;
        let lin = Style::new().with_color(Color::Rgb(0xC8, 0xC3, 0xB8));
        area.draw_line(
            vec![
                genpdf::Position::new(mm(0.0), mm(gora)),
                genpdf::Position::new(mm(szer.iter().sum::<f64>()), mm(gora)),
            ],
            lin,
        );
        y += self.rysuj_komorki(&area, y, &naglowki, &szer, naglowek, fc)?;
        let mut na_stronie = 0;
        while self.nastepny < self.wiersze.len() {
            let blok = self.wys_bloku(self.nastepny, &szer, zwykly, fc);
            if y + blok > wys && na_stronie > 0 {
                break;
            }
            match &self.wiersze[self.nastepny] {
                WierszTabeli::Dane(k) => {
                    let k = k.clone();
                    y += self.rysuj_komorki(&area, y, &k, &szer, zwykly, fc)?;
                }
                WierszTabeli::Podgrupa(t) => {
                    let h = f(podgrupa.line_height(fc)) + 2.0 * PAD_Y;
                    area.print_str(
                        fc,
                        genpdf::Position::new(mm(PAD_X), mm(y + PAD_Y)),
                        podgrupa,
                        t,
                    )?;
                    Self::rysuj_ramke(&area, y, y + h, &szer, false);
                    y += h;
                    // nagłówek podgrupy zawsze razem z kolejnym wierszem
                    if let Some(WierszTabeli::Dane(k)) = self.wiersze.get(self.nastepny + 1) {
                        let k = k.clone();
                        self.nastepny += 1;
                        y += self.rysuj_komorki(&area, y, &k, &szer, zwykly, fc)?;
                    }
                }
            }
            self.nastepny += 1;
            na_stronie += 1;
        }
        wynik.size = genpdf::Size::new(area.size().width, mm(y.min(wys)));
        wynik.has_more = self.nastepny < self.wiersze.len();
        Ok(wynik)
    }
}

fn tabela(c: &CzescDok, agd: bool) -> TabelaCzesci {
    let (wagi, naglowki): (Vec<f64>, Vec<&'static str>) = if agd {
        (
            vec![17.0, 24.0, 31.0, 11.0, 16.0],
            vec!["Rodzaj", "Model / marka", "Opis", "Ilość", "Wartość"],
        )
    } else {
        (
            vec![19.0, 27.0, 42.0, 12.0],
            vec!["Rodzaj", "Materiał", "Opis", "Ilość"],
        )
    };
    let dane = |w: &WierszDok| {
        let mut k = vec![
            w.rodzaj.clone(),
            w.material.clone(),
            w.opis.clone(),
            w.ilosc.clone(),
        ];
        if agd {
            k.push(w.cena.clone().unwrap_or_default());
        }
        WierszTabeli::Dane(k)
    };
    let mut wiersze: Vec<WierszTabeli> = c.wiersze.iter().map(dane).collect();
    if !c.frezowanie.is_empty() {
        wiersze.push(WierszTabeli::Podgrupa("Frezowanie".into()));
        wiersze.extend(c.frezowanie.iter().map(dane));
    }
    if !c.wyposazenie.is_empty() {
        wiersze.push(WierszTabeli::Podgrupa("Wyposażenie".into()));
        wiersze.extend(c.wyposazenie.iter().map(dane));
    }
    TabelaCzesci {
        tytul: match &c.cena {
            Some(cena) => format!("{} – {cena}", c.nazwa),
            None => c.nazwa.clone(),
        },
        do_prawej: if agd {
            vec![false, false, false, false, true]
        } else {
            vec![false; 4]
        },
        naglowki,
        wagi,
        wiersze,
        nastepny: 0,
        odlozony: false,
    }
}

pub fn renderuj(dok: &DokumentOferty) -> Wynik<Vec<u8>> {
    let mut d = genpdf::Document::new(czcionki()?);
    d.set_title(format!("Oferta {}", dok.numer));
    d.set_font_size(9);
    d.set_line_spacing(1.2);
    d.set_paper_size(genpdf::PaperSize::A4);
    // Bez profilu kolorów ICC (PDF/X-3 dokłada ~650 KB) – zwykły PDF wystarcza do druku biurowego.
    d.set_minimal_conformance();
    let mut dek = SimplePageDecorator::new();
    dek.set_margins(Margins::trbl(14, 16, 14, 16));
    dek.set_header(|strona| {
        Paragraph::new(if strona > 1 {
            format!("Strona {strona}")
        } else {
            String::new()
        })
        .aligned(Alignment::Right)
        .styled(Style::new().with_font_size(7).with_color(SZARY))
    });
    d.set_page_decorator(dek);

    d.push(
        Paragraph::new("OFERTA")
            .aligned(Alignment::Center)
            .styled(Style::new().bold().with_font_size(20).with_color(AKCENT)),
    );
    d.push(
        Paragraph::new(dok.numer.as_str())
            .aligned(Alignment::Center)
            .styled(Style::new().with_font_size(8).with_color(SZARY)),
    );
    d.push(Break::new(1));

    let mut strony = TableLayout::new(vec![1, 1]);
    let blok = |tytul: &str, linie: &[String]| {
        let mut l = LinearLayout::vertical();
        l.push(Paragraph::new(tytul).styled(Style::new().bold().with_color(AKCENT)));
        for x in linie {
            l.push(Paragraph::new(x.as_str()));
        }
        l
    };
    wiersz_stron(
        &mut strony,
        blok("Inwestor", &dok.inwestor),
        blok("Wykonawca", &dok.wykonawca),
    )?;
    d.push(strony);
    d.push(Break::new(0.8));

    let mut meta = vec![("Data", dok.data.clone())];
    if let Some(t) = &dok.termin_realizacji {
        meta.push(("Termin realizacji", t.clone()));
    }
    if let Some(i) = &dok.inwestycja {
        meta.push(("Inwestycja", i.clone()));
    }
    for (k, v) in meta {
        let mut p = Paragraph::default();
        p.push_styled(format!("{k}: "), Style::new().bold());
        p.push(v);
        d.push(p);
    }
    if let Some(z) = &dok.zdanie_waznosci {
        d.push(Paragraph::new(z.as_str()).styled(Style::new().italic()));
    }

    for s in &dok.sekcje {
        d.push(Break::new(1.2));
        d.push(
            Paragraph::new(s.tytul.as_str())
                .styled(Style::new().bold().with_font_size(13).with_color(AKCENT)),
        );
        d.push(Break::new(0.3));
        for c in &s.czesci {
            d.push(tabela(c, s.agd));
            for o in &c.zdjecia {
                if let Some(e) = obraz(o) {
                    d.push(e);
                }
            }
            d.push(Break::new(0.5));
        }
        if let Some(suma) = &s.suma {
            d.push(
                Paragraph::new(suma.as_str())
                    .aligned(Alignment::Right)
                    .styled(Style::new().bold().with_font_size(10)),
            );
        }
        for o in &s.zdjecia {
            if let Some(e) = obraz(o) {
                d.push(e);
            }
        }
    }

    d.push(Break::new(1.5));
    d.push(
        Paragraph::new("Podsumowanie")
            .styled(Style::new().bold().with_font_size(13).with_color(AKCENT)),
    );
    let mut t = TableLayout::new(vec![3, 2]);
    t.set_cell_decorator(FrameCellDecorator::new(false, true, false));
    for (etykieta, kwota, wyr) in &dok.podsumowanie {
        let styl = if *wyr {
            Style::new().bold().with_font_size(12)
        } else {
            Style::new()
        };
        let mut r = t.row();
        r.push_element(komorka_styl(etykieta, styl));
        r.push_element(
            Paragraph::new(kwota.as_str())
                .aligned(Alignment::Right)
                .styled(styl)
                .padded(Margins::trbl(1, 1.5, 1, 1.5)),
        );
        r.push().map_err(blad_renderowania)?;
    }
    d.push(t);
    d.push(Paragraph::new(dok.stawka_vat.as_str()).styled(Style::new().with_font_size(8)));

    if !dok.klauzule.is_empty() {
        d.push(Break::new(1));
        for k in &dok.klauzule {
            d.push(Paragraph::new(format!("• {k}")).styled(Style::new().with_font_size(8)));
        }
    }
    if !dok.zdjecia.is_empty() {
        d.push(genpdf::elements::PageBreak::new());
        d.push(
            Paragraph::new("Zdjęcia i wizualizacje")
                .styled(Style::new().bold().with_font_size(13).with_color(AKCENT)),
        );
        for o in &dok.zdjecia {
            if let Some(e) = obraz(o) {
                d.push(e);
            }
        }
    }

    let mut bufor = Vec::new();
    d.render(&mut bufor).map_err(blad_renderowania)?;
    Ok(bufor)
}

fn wiersz_stron(t: &mut TableLayout, a: LinearLayout, b: LinearLayout) -> Wynik<()> {
    t.row()
        .element(a.padded(Margins::trbl(0, 4, 0, 0)))
        .element(b)
        .push()
        .map_err(blad_renderowania)
}
