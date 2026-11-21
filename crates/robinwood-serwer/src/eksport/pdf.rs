//! Render oferty do PDF (genpdf). Czcionka Inter (SIL OFL) wkompilowana w program –
//! polskie znaki działają niezależnie od czcionek systemu.

use genpdf::elements::{
    Break, FrameCellDecorator, Image, LinearLayout, PaddedElement, Paragraph, TableLayout,
};
use genpdf::fonts::{FontData, FontFamily};
use genpdf::style::{Color, Style};
use genpdf::{Alignment, Element, Margins, SimplePageDecorator};

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

fn komorka(t: &str) -> PaddedElement<Paragraph> {
    Paragraph::new(t).padded(Margins::trbl(1, 1.5, 1, 1.5))
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

fn tabela(c: &CzescDok, agd: bool) -> Wynik<TableLayout> {
    let wagi = if agd {
        vec![17, 24, 31, 11, 16]
    } else {
        vec![19, 27, 42, 12]
    };
    let mut t = TableLayout::new(wagi);
    t.set_cell_decorator(FrameCellDecorator::new(true, true, false));
    let naglowek = Style::new().bold().with_font_size(8).with_color(AKCENT);
    let naglowki: &[&str] = if agd {
        &["Rodzaj", "Model / marka", "Opis", "Ilość", "Wartość"]
    } else {
        &["Rodzaj", "Materiał", "Opis", "Ilość"]
    };
    let mut r = t.row();
    for n in naglowki {
        r.push_element(komorka_styl(n, naglowek));
    }
    r.push().map_err(blad_renderowania)?;

    let dodaj = |t: &mut TableLayout, w: &WierszDok| -> Wynik<()> {
        let mut r = t.row();
        r.push_element(komorka(&w.rodzaj));
        r.push_element(komorka(&w.material));
        r.push_element(komorka(&w.opis));
        r.push_element(komorka(&w.ilosc));
        if agd {
            r.push_element(
                Paragraph::new(w.cena.as_deref().unwrap_or(""))
                    .aligned(Alignment::Right)
                    .padded(Margins::trbl(1, 1.5, 1, 1.5)),
            );
        }
        r.push().map_err(blad_renderowania)
    };
    let podnaglowek = |t: &mut TableLayout, tytul: &str| -> Wynik<()> {
        let mut r = t.row();
        r.push_element(komorka_styl(tytul, Style::new().bold().italic()));
        for _ in 1..(if agd { 5 } else { 4 }) {
            r.push_element(Paragraph::new(""));
        }
        r.push().map_err(blad_renderowania)
    };
    for w in &c.wiersze {
        dodaj(&mut t, w)?;
    }
    if !c.frezowanie.is_empty() {
        podnaglowek(&mut t, "Frezowanie")?;
        for w in &c.frezowanie {
            dodaj(&mut t, w)?;
        }
    }
    if !c.wyposazenie.is_empty() {
        podnaglowek(&mut t, "Wyposażenie")?;
        for w in &c.wyposazenie {
            dodaj(&mut t, w)?;
        }
    }
    Ok(t)
}

pub fn renderuj(dok: &DokumentOferty) -> Wynik<Vec<u8>> {
    let mut d = genpdf::Document::new(czcionki()?);
    d.set_title(format!("Oferta {}", dok.numer));
    d.set_font_size(9);
    d.set_line_spacing(1.2);
    d.set_paper_size(genpdf::PaperSize::A4);
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
            let tytul = match &c.cena {
                Some(cena) => format!("{} – {cena}", c.nazwa),
                None => c.nazwa.clone(),
            };
            d.push(Paragraph::new(tytul).styled(Style::new().bold().with_font_size(10)));
            d.push(tabela(c, s.agd)?);
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
