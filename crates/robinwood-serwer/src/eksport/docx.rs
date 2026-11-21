//! Render oferty do DOCX (docx-rs).

use docx_rs::{
    AlignmentType, BreakType, Docx, LineSpacing, PageMargin, Paragraph, Pic, Run, RunFonts,
    Shading, ShdType, Table, TableBorders, TableCell, TableRow, WidthType,
};

use super::{blad_renderowania, CzescDok, DokumentOferty, ObrazDok, WierszDok};
use crate::bledy::Wynik;

/// Szerokość obszaru tekstu A4 przy marginesach 1,8 cm (twipy).
const SZEROKOSC: usize = 9_866;
const KOLUMNY: [usize; 4] = [1_900, 2_700, 4_066, 1_200];
const KOLUMNY_AGD: [usize; 5] = [1_700, 2_400, 3_066, 1_100, 1_600];
const AKCENT: &str = "2D5E50";
const TLO_NAGLOWKA: &str = "E8EFEC";
const TLO_CZESCI: &str = "F4F3F0";

fn tekst(t: &str) -> Run {
    Run::new().add_text(t)
}

fn akapit(t: &str) -> Paragraph {
    Paragraph::new().add_run(tekst(t))
}

fn pogrubiony(t: &str, rozmiar: usize) -> Paragraph {
    Paragraph::new().add_run(Run::new().add_text(t).bold().size(rozmiar))
}

fn komorka(t: &str, szer: usize) -> TableCell {
    TableCell::new()
        .width(szer, WidthType::Dxa)
        .add_paragraph(Paragraph::new().add_run(Run::new().add_text(t).size(18)))
}

fn wiersz_tabeli(w: &WierszDok, agd: bool) -> TableRow {
    let mut k = vec![
        komorka(&w.rodzaj, KOLUMNY[0]),
        komorka(&w.material, KOLUMNY[1]),
        komorka(&w.opis, KOLUMNY[2]),
        komorka(&w.ilosc, KOLUMNY[3]),
    ];
    if agd {
        k = vec![
            komorka(&w.rodzaj, KOLUMNY_AGD[0]),
            komorka(&w.material, KOLUMNY_AGD[1]),
            komorka(&w.opis, KOLUMNY_AGD[2]),
            komorka(&w.ilosc, KOLUMNY_AGD[3]),
            TableCell::new()
                .width(KOLUMNY_AGD[4], WidthType::Dxa)
                .add_paragraph(
                    Paragraph::new().align(AlignmentType::Right).add_run(
                        Run::new()
                            .add_text(w.cena.as_deref().unwrap_or(""))
                            .size(18),
                    ),
                ),
        ];
    }
    TableRow::new(k).cant_split()
}

fn wiersz_scalony(t: &str, kolumn: usize, tlo: &str, pogrub: bool, kursywa: bool) -> TableRow {
    let mut r = Run::new().add_text(t).size(18);
    if pogrub {
        r = r.bold();
    }
    if kursywa {
        r = r.italic();
    }
    TableRow::new(vec![TableCell::new()
        .grid_span(kolumn)
        .shading(Shading::new().shd_type(ShdType::Clear).fill(tlo))
        .add_paragraph(Paragraph::new().keep_next(true).add_run(r))])
    .cant_split()
}

fn obrazy(mut d: Docx, obrazy: &[ObrazDok]) -> Docx {
    for o in obrazy {
        let (w, h) = (o.obraz.width(), o.obraz.height());
        // maks. 12 cm szerokości i 9 cm wysokości (1 cm = 360 000 EMU)
        let skala = (12.0 * 360_000.0 / w as f64).min(9.0 * 360_000.0 / h as f64);
        let pic = Pic::new_with_dimensions(o.png.clone(), w, h)
            .size((w as f64 * skala) as u32, (h as f64 * skala) as u32);
        d = d.add_paragraph(Paragraph::new().add_run(Run::new().add_image(pic)));
        if let Some(p) = &o.podpis {
            d = d.add_paragraph(Paragraph::new().add_run(Run::new().add_text(p).italic().size(18)));
        }
    }
    d
}

fn tabela_czesci(c: &CzescDok, agd: bool) -> Table {
    let kolumn = if agd { 5 } else { 4 };
    let naglowki: &[&str] = if agd {
        &["Rodzaj", "Model / marka", "Opis", "Ilość", "Wartość"]
    } else {
        &["Rodzaj", "Materiał", "Opis", "Ilość"]
    };
    let szer: &[usize] = if agd { &KOLUMNY_AGD } else { &KOLUMNY };
    let mut wiersze = vec![TableRow::new(
        naglowki
            .iter()
            .zip(szer)
            .map(|(n, s)| {
                TableCell::new()
                    .width(*s, WidthType::Dxa)
                    .shading(Shading::new().shd_type(ShdType::Clear).fill(TLO_NAGLOWKA))
                    .add_paragraph(
                        Paragraph::new().add_run(Run::new().add_text(*n).bold().size(18)),
                    )
            })
            .collect(),
    )];
    let tytul = match &c.cena {
        Some(cena) => format!("{} – {cena}", c.nazwa),
        None => c.nazwa.clone(),
    };
    wiersze.push(wiersz_scalony(&tytul, kolumn, TLO_CZESCI, true, false));
    wiersze.extend(c.wiersze.iter().map(|w| wiersz_tabeli(w, agd)));
    if !c.frezowanie.is_empty() {
        wiersze.push(wiersz_scalony("Frezowanie", kolumn, "FFFFFF", true, true));
        wiersze.extend(c.frezowanie.iter().map(|w| wiersz_tabeli(w, agd)));
    }
    if !c.wyposazenie.is_empty() {
        wiersze.push(wiersz_scalony("Wyposażenie", kolumn, "FFFFFF", true, true));
        wiersze.extend(c.wyposazenie.iter().map(|w| wiersz_tabeli(w, agd)));
    }
    Table::new(wiersze)
        .set_grid(szer.to_vec())
        .width(SZEROKOSC, WidthType::Dxa)
}

pub fn renderuj(dok: &DokumentOferty) -> Wynik<Vec<u8>> {
    let mut d = Docx::new()
        .page_size(11_906, 16_838)
        .page_margin(
            PageMargin::new()
                .top(1_000)
                .bottom(1_000)
                .left(1_020)
                .right(1_020),
        )
        // Calibri jest na każdym Windows z Office (i w Word Online); LibreOffice podstawia
        // metrycznie zgodne Carlito. Inter nie jest osadzany w DOCX, więc nie może być domyślny.
        .default_fonts(
            RunFonts::new()
                .ascii("Calibri")
                .hi_ansi("Calibri")
                .cs("Arial")
                .east_asia("Arial"),
        )
        .default_size(20)
        .default_line_spacing(LineSpacing::new().after(60));

    d = d.add_paragraph(
        Paragraph::new()
            .align(AlignmentType::Center)
            .add_run(Run::new().add_text("OFERTA").bold().size(40).color(AKCENT)),
    );
    d = d.add_paragraph(
        Paragraph::new()
            .align(AlignmentType::Center)
            .add_run(Run::new().add_text(&dok.numer).size(18).color("666666")),
    );

    // Inwestor | Wykonawca
    let blok = |tytul: &str, linie: &[String]| {
        let mut c = TableCell::new()
            .width(SZEROKOSC / 2, WidthType::Dxa)
            .add_paragraph(
                Paragraph::new().add_run(Run::new().add_text(tytul).bold().size(18).color(AKCENT)),
            );
        for l in linie {
            c = c.add_paragraph(akapit(l));
        }
        c
    };
    d = d.add_table(
        Table::new(vec![TableRow::new(vec![
            blok("Inwestor", &dok.inwestor),
            blok("Wykonawca", &dok.wykonawca),
        ])])
        .set_grid(vec![SZEROKOSC / 2, SZEROKOSC / 2])
        .set_borders(TableBorders::new().clear_all()),
    );
    d = d.add_paragraph(Paragraph::new());
    let mut meta = vec![("Data", dok.data.clone())];
    if let Some(t) = &dok.termin_realizacji {
        meta.push(("Termin realizacji", t.clone()));
    }
    if let Some(i) = &dok.inwestycja {
        meta.push(("Inwestycja", i.clone()));
    }
    for (k, v) in meta {
        d = d.add_paragraph(
            Paragraph::new()
                .add_run(Run::new().add_text(format!("{k}: ")).bold())
                .add_run(tekst(&v)),
        );
    }
    if let Some(z) = &dok.zdanie_waznosci {
        d = d.add_paragraph(Paragraph::new().add_run(Run::new().add_text(z).italic()));
    }

    for s in &dok.sekcje {
        d = d.add_paragraph(Paragraph::new());
        d = d.add_paragraph(
            Paragraph::new()
                .keep_next(true)
                .add_run(Run::new().add_text(&s.tytul).bold().size(28).color(AKCENT)),
        );
        for c in &s.czesci {
            d = d.add_table(tabela_czesci(c, s.agd));
            d = obrazy(d, &c.zdjecia);
            d = d.add_paragraph(Paragraph::new());
        }
        if let Some(suma) = &s.suma {
            d = d.add_paragraph(
                Paragraph::new()
                    .align(AlignmentType::Right)
                    .add_run(Run::new().add_text(suma).bold()),
            );
        }
        d = obrazy(d, &s.zdjecia);
    }

    d = d.add_paragraph(Paragraph::new());
    d = d.add_paragraph(pogrubiony("Podsumowanie", 28));
    let mut rows = Vec::new();
    for (etykieta, kwota, wyroznij) in &dok.podsumowanie {
        let mk = |t: &str, wyr: bool, align: AlignmentType, szer: usize| {
            let mut r = Run::new().add_text(t);
            if wyr {
                r = r.bold().size(24);
            }
            let mut c = TableCell::new()
                .width(szer, WidthType::Dxa)
                .add_paragraph(Paragraph::new().align(align).add_run(r));
            if wyr {
                c = c.shading(Shading::new().shd_type(ShdType::Clear).fill(TLO_NAGLOWKA));
            }
            c
        };
        rows.push(TableRow::new(vec![
            mk(etykieta, *wyroznij, AlignmentType::Left, 6_000),
            mk(kwota, *wyroznij, AlignmentType::Right, 3_866),
        ]));
    }
    d = d.add_table(
        Table::new(rows)
            .set_grid(vec![6_000, 3_866])
            .width(SZEROKOSC, WidthType::Dxa),
    );
    d = d.add_paragraph(Paragraph::new().add_run(Run::new().add_text(&dok.stawka_vat).size(18)));

    if !dok.klauzule.is_empty() {
        d = d.add_paragraph(Paragraph::new());
        for k in &dok.klauzule {
            d = d.add_paragraph(
                Paragraph::new().add_run(Run::new().add_text(format!("• {k}")).size(18)),
            );
        }
    }
    if !dok.zdjecia.is_empty() {
        d = d.add_paragraph(Paragraph::new().add_run(Run::new().add_break(BreakType::Page)));
        d = d.add_paragraph(pogrubiony("Zdjęcia i wizualizacje", 28));
        d = obrazy(d, &dok.zdjecia);
    }

    let mut bufor = std::io::Cursor::new(Vec::new());
    d.build().pack(&mut bufor).map_err(blad_renderowania)?;
    Ok(bufor.into_inner())
}
