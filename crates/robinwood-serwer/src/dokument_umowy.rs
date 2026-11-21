//! Dokumenty umowy i aneksu z szablonów DOCX użytkownika (`szablon_docx`).
//!
//! * Szablon wgrywa użytkownik w programie (*Umowy → Ustawienia → Szablony dokumentów*);
//!   plik leży na serwerze w `<folder danych>/szablony-dokumentow/<rodzaj>.docx` – nigdy w repozytorium.
//! * Bez wgranego szablonu dokument powstaje z wbudowanego, FIKCYJNEGO szablonu przykładowego
//!   (z wyraźnym znakiem „SZABLON PRZYKŁADOWY”).
//! * Lista pól ([`POLA`]) jest jedna dla dokumentu, podglądu pól i instrukcji w programie.
//! * PDF: z gotowego DOCX przez Microsoft Word (Windows) albo LibreOffice na komputerze-serwerze
//!   – tylko tak zachowuje się dokładny układ szablonu użytkownika (patrz [`pdf_z_docx`]).
//! * Dokument nigdy nie zawiera danych wewnętrznych (koszt własny, marża, czasy produkcji, uwagi).

use std::io::Cursor;
use std::path::{Path, PathBuf};

use chrono::NaiveDate;
use docx_rs::{BreakType, Docx, Footer, Header, PageMargin, Paragraph, Run, RunFonts};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::aneksy::PelnyAneks;
use crate::bledy::{BladApi, Wynik};
use crate::formaty;
use crate::slownie;
use crate::specyfikacje::Specyfikacja;
use crate::szablon_docx::xml::{self, Komorka};
use crate::szablon_docx::{self, DaneSzablonu, RaportSzablonu};
use crate::umowy::{self, MigawkaUmowy, PelnaUmowa};

pub const RODZAJE_SZABLONOW: &[&str] = &["umowa", "aneks"];
pub const ZNAK_PRZYKLADU: &str = "SZABLON PRZYKŁADOWY";

/// Opis pola szablonu (instrukcja w programie i w dokumentacji).
#[derive(Debug, Clone, Serialize)]
pub struct OpisPola {
    pub nazwa: &'static str,
    pub opis: &'static str,
    /// `tekst`, `blok` (tabela/lista – w osobnym akapicie), `warunek` (`[[…]] … [[/…]]`).
    pub rodzaj: &'static str,
    /// `umowa`, `aneks`, `oba`.
    pub dokument: &'static str,
}

const fn p(nazwa: &'static str, opis: &'static str) -> OpisPola {
    OpisPola { nazwa, opis, rodzaj: "tekst", dokument: "oba" }
}
const fn pu(nazwa: &'static str, opis: &'static str) -> OpisPola {
    OpisPola { nazwa, opis, rodzaj: "tekst", dokument: "umowa" }
}
const fn pa(nazwa: &'static str, opis: &'static str) -> OpisPola {
    OpisPola { nazwa, opis, rodzaj: "tekst", dokument: "aneks" }
}

pub const POLA: &[OpisPola] = &[
    p("numer umowy", "numer umowy, np. 07/2026"),
    p("miejscowość", "miejsce zawarcia umowy"),
    p("data umowy", "data zawarcia (DD.MM.RRRR)"),
    p("klient", "zamawiający – imię i nazwisko albo nazwa"),
    p("adres klienta", "adres zamawiającego w jednej linii"),
    p("e-mail", "e-mail zamawiającego"),
    p("telefon", "telefon zamawiającego"),
    p("nip klienta", "NIP zamawiającego (firma)"),
    p("osoby kontaktowe", "osoby kontaktowe zamawiającego (po przecinku)"),
    p("adres montażu", "adres miejsca montażu"),
    pu("piętro", "piętro"),
    pu("kod domofonu", "kod domofonu"),
    pu("parking", "informacja o parkingu"),
    pu("wniesienie", "winda i możliwość wniesienia"),
    pu("liczba dni", "dni kalendarzowe na przekazanie brakujących informacji"),
    pu("wartość", "wartość umowy brutto, np. 48 352,00 (bez „zł” – dopisz w szablonie)"),
    pu("kwota słownie", "wartość umowy słownie, np. „czterdzieści osiem tysięcy … złote 00/100”"),
    pu("stawka VAT", "8 albo 23 (bez znaku %)"),
    pu("zadatek", "kwota zadatku"),
    pu("zadatek słownie", "zadatek słownie"),
    pu("termin zadatku", "data, do której należy wpłacić zadatek"),
    pu("wariant A lub B", "wybrany wariant płatności: A albo B"),
    pu("kwota przed montażem", "wariant B: kwota płatna przed montażem"),
    pu("kwota przed montażem słownie", "wariant B: kwota przed montażem słownie"),
    pu("termin kwoty przed montażem", "wariant B: termin kwoty przed montażem"),
    pu("pozostała kwota", "ostatnia rata (A: przed montażem, B: po odbiorze)"),
    pu("pozostała kwota słownie", "ostatnia rata słownie"),
    pu("termin pozostałej kwoty", "termin ostatniej raty"),
    p("termin montażu", "planowany termin rozpoczęcia montażu („do”)"),
    pu("nazwa zabudowy", "nazwy sekcji specyfikacji, np. „Zabudowa kuchni, Szafa”"),
    pu("rodzaj nieruchomości", "§9 VAT: lokal mieszkalny / budynek mieszkalny jednorodzinny / inny"),
    pu("powierzchnia", "§9 VAT: powierzchnia użytkowa w m²"),
    pu("podstawa powierzchni", "§9 VAT: podstawa określenia powierzchni"),
    pu("VAT lokal", "pole wyboru ☒/☐ – lokal mieszkalny"),
    pu("VAT dom", "pole wyboru ☒/☐ – budynek mieszkalny jednorodzinny"),
    pu("VAT inny", "pole wyboru ☒/☐ – inny"),
    pu("data odbioru", "data protokołu odbioru"),
    pu("wpłacono", "suma wpłat"),
    pu("pozostało", "pozostało do zapłaty"),
    pu("termin płatności", "termin najbliższej niezapłaconej raty"),
    pu("odbiór bez uwag", "pole wyboru ☒/☐ – odbiór bez zastrzeżeń"),
    pu("odbiór z uwagami", "pole wyboru ☒/☐ – odbiór z uwagami"),
    pu("uwagi do odbioru", "uwagi z protokołu odbioru"),
    pu("data zgody", "data zgody na zdjęcia (= data umowy)"),
    p("wykonawca", "nazwa firmy (Ustawienia → Dane firmy)"),
    p("adres wykonawcy", "adres firmy"),
    p("nip wykonawcy", "NIP firmy"),
    p("telefon wykonawcy", "telefon firmy"),
    p("e-mail wykonawcy", "e-mail firmy"),
    p("bank", "nazwa banku firmy"),
    p("numer rachunku", "numer rachunku firmy"),
    p("swift", "kod SWIFT/BIC"),
    p("data wydruku", "dzisiejsza data"),
    pa("numer aneksu", "numer kolejny aneksu do umowy"),
    pa("data aneksu", "data aneksu"),
    pa("opis zmian", "opis zmian wpisany w aneksie"),
    pa("zmiana kwoty", "zmiana wynagrodzenia (np. +1 200,00 albo -500,00)"),
    pa("poprzednia wartość", "wartość umowy przed aneksem"),
    pa("nowa wartość", "wartość umowy po aneksie"),
    pa("nowa wartość słownie", "wartość po aneksie słownie"),
    pa("poprzedni termin montażu", "termin przed aneksem"),
    pa("nowy termin montażu", "termin po aneksie (albo dotychczasowy)"),
    OpisPola { nazwa: "specyfikacja", opis: "tabele specyfikacji (sekcje → części → Rodzaj | Materiał | Opis | Ilość)", rodzaj: "blok", dokument: "oba" },
    OpisPola { nazwa: "harmonogram płatności", opis: "tabela rat: zadatek, raty, terminy, suma", rodzaj: "blok", dokument: "umowa" },
    OpisPola { nazwa: "harmonogram etapów", opis: "tabela: Lp. | nazwa pracy | termin rozpoczęcia | kwota | suma", rodzaj: "blok", dokument: "umowa" },
    OpisPola { nazwa: "lista zmian", opis: "numerowana lista zmian aneksu (kwota, termin, specyfikacja)", rodzaj: "blok", dokument: "aneks" },
    OpisPola { nazwa: "załącznik 2", opis: "[[załącznik 2]] … [[/załącznik 2]] – harmonogram etapów / projekt (gdy zaznaczony)", rodzaj: "warunek", dokument: "umowa" },
    OpisPola { nazwa: "załącznik 3", opis: "[[załącznik 3]] … [[/załącznik 3]] – cennik montażu AGD", rodzaj: "warunek", dokument: "umowa" },
    OpisPola { nazwa: "załącznik 5", opis: "[[załącznik 5]] … [[/załącznik 5]] – protokół odbioru (z niego powstaje osobny protokół)", rodzaj: "warunek", dokument: "umowa" },
    OpisPola { nazwa: "załącznik 6", opis: "[[załącznik 6]] … [[/załącznik 6]] – zgoda na zdjęcia", rodzaj: "warunek", dokument: "umowa" },
    OpisPola { nazwa: "wariant A", opis: "[[wariant A]] … [[/wariant A]] – tekst tylko dla wariantu A (analogicznie [[wariant B]])", rodzaj: "warunek", dokument: "umowa" },
    OpisPola { nazwa: "stawka 8", opis: "[[stawka 8]] … [[/stawka 8]] – np. oświadczenie VAT tylko przy 8% (analogicznie [[stawka 23]])", rodzaj: "warunek", dokument: "umowa" },
];

/// Znaczniki (`w:tag`) kontrolek treści z dotychczasowego wzorca umowy → nazwy pól.
const ALIASY: &[(&str, &str)] = &[
    ("number", "numer umowy"),
    ("city", "miejscowość"),
    ("signed", "data umowy"),
    ("client", "klient"),
    ("address", "adres klienta"),
    ("email", "e-mail"),
    ("phone", "telefon"),
    ("installation_address", "adres montażu"),
    ("floor", "piętro"),
    ("entry_code", "kod domofonu"),
    ("parking", "parking"),
    ("access", "wniesienie"),
    ("details_days", "liczba dni"),
    ("total", "wartość"),
    ("total_words", "kwota słownie"),
    ("deposit", "zadatek"),
    ("deposit_words", "zadatek słownie"),
    ("deposit_due", "termin zadatku"),
    ("variant", "wariant A lub B"),
    ("payment_schedule", "harmonogram płatności"),
    ("planned_start", "termin montażu"),
    ("scope", "nazwa zabudowy"),
    ("accepted", "data odbioru"),
    ("paid", "wpłacono"),
    ("balance", "pozostało"),
    ("final_due", "termin płatności"),
    ("photo_date", "data zgody"),
    ("wartość brutto", "wartość"),
    ("wartosc umowy", "wartość"),
    ("wariant", "wariant A lub B"),
    ("wariant płatności", "wariant A lub B"),
    ("email klienta", "e-mail"),
    ("e-mail klienta", "e-mail"),
    ("telefon klienta", "telefon"),
    ("zamawiający", "klient"),
];

fn kwota_bez_zl(gr: i64) -> String {
    formaty::kwota(gr).trim_end_matches("\u{a0}zł").to_string()
}

fn kwota_opc(gr: Option<i64>) -> String {
    gr.map(kwota_bez_zl).unwrap_or_default()
}

fn data_opc(d: &Option<String>) -> String {
    d.as_deref().map(formaty::data).unwrap_or_default()
}

fn pole_wyboru(tak: bool) -> &'static str {
    if tak {
        "☒"
    } else {
        "☐"
    }
}

fn linia_adresu(ulica: &Option<String>, kod: &Option<String>, miasto: &Option<String>) -> String {
    let km = [kod.clone(), miasto.clone()]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" ");
    [ulica.clone().unwrap_or_default(), km]
        .into_iter()
        .filter(|s| !s.trim().is_empty())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Wspólne pola: strony (z migawki po wyjściu ze szkicu), wykonawca, aliasy, pomijane.
fn pola_stron(d: &mut DaneSzablonu, m: &MigawkaUmowy) {
    let z = &m.zamawiajacy;
    d.pole("klient", z.nazwa.clone());
    d.pole("adres klienta", linia_adresu(&z.adres_ulica, &z.adres_kod, &z.adres_miasto));
    d.pole("e-mail", z.email.clone().unwrap_or_default());
    d.pole("telefon", z.telefon.clone().unwrap_or_default());
    d.pole("nip klienta", z.nip.clone().unwrap_or_default());
    d.pole(
        "osoby kontaktowe",
        m.osoby_kontaktowe
            .iter()
            .map(|o| {
                let mut s = o.imie_nazwisko.clone();
                let kontakt: Vec<String> = [o.telefon.clone(), o.email.clone()].into_iter().flatten().collect();
                if !kontakt.is_empty() {
                    s.push_str(&format!(" ({})", kontakt.join(", ")));
                }
                s
            })
            .collect::<Vec<_>>()
            .join(", "),
    );
    let w = &m.wykonawca;
    d.pole("wykonawca", w.nazwa.clone());
    d.pole("adres wykonawcy", w.adres.replace('\n', ", "));
    d.pole("nip wykonawcy", w.nip.clone());
    d.pole("telefon wykonawcy", w.telefon.clone());
    d.pole("e-mail wykonawcy", w.email.clone());
    d.pole("bank", m.wykonawca_bank.clone());
    d.pole("numer rachunku", m.wykonawca_rachunek.clone());
    d.pole("swift", m.wykonawca_swift.clone());
    d.pole("data wydruku", formaty::data(&umowy::dzis().format("%Y-%m-%d").to_string()));
    for (a, n) in ALIASY {
        d.alias(a, n);
    }
    d.pomin("uzupełnij");
}

/// Tabele specyfikacji dla dokumentu (bez cen mebli, kosztów i marży; AGD z wartością pozycji).
pub fn blok_specyfikacji(s: &Specyfikacja) -> String {
    let mut x = String::new();
    if s.sekcje.is_empty() {
        return xml::akapit_tekst("(specyfikacja nie zawiera pozycji)", false);
    }
    for sek in &s.sekcje {
        let agd = sek.typ == "agd";
        let tytul = match (&sek.numer, &sek.wariant) {
            (Some(n), Some(w)) => format!("{n}  {} ({w})", sek.nazwa),
            (Some(n), None) => format!("{n}  {}", sek.nazwa),
            (None, Some(w)) => format!("{} ({w})", sek.nazwa),
            (None, None) => sek.nazwa.clone(),
        };
        x.push_str(&xml::akapit(&xml::przebieg(&tytul, true, Some(24)), None, Some(80), true));
        let szer: &[u32] = if agd {
            &[1700, 2300, 3100, 1000, 1500]
        } else {
            &[1900, 2900, 3800, 1000]
        };
        for c in &sek.czesci {
            if c.wiersze.is_empty() {
                continue;
            }
            x.push_str(&xml::akapit(&xml::przebieg(&c.nazwa, true, Some(20)), None, Some(40), true));
            let mut naglowek = vec![
                Komorka::naglowek("Rodzaj"),
                Komorka::naglowek("Materiał"),
                Komorka::naglowek("Opis"),
                Komorka::naglowek("Ilość"),
            ];
            if agd {
                naglowek.push(Komorka::naglowek("Wartość").do_prawej());
            }
            let mut wiersze = vec![(true, naglowek)];
            for (podgrupa, etykieta) in [("glowna", None), ("frezowanie", Some("Frezowanie")), ("wyposazenie", Some("Wyposażenie"))] {
                let ws: Vec<_> = c.wiersze.iter().filter(|w| w.podgrupa == podgrupa).collect();
                if ws.is_empty() {
                    continue;
                }
                if let Some(e) = etykieta {
                    wiersze.push((false, vec![Komorka::t(e).pogrubiona().tlo("F4F3F0").scalona(szer.len() as u32)]));
                }
                for w in ws {
                    let d = crate::eksport::wiersz_dok(w, agd);
                    let mut k = vec![Komorka::t(d.rodzaj), Komorka::t(d.material), Komorka::t(d.opis), Komorka::t(d.ilosc)];
                    if agd {
                        k.push(Komorka::t(d.cena.unwrap_or_default()).do_prawej());
                    }
                    wiersze.push((false, k));
                }
            }
            x.push_str(&xml::tabela(szer, &wiersze));
            x.push_str(&xml::akapit("", None, Some(80), false));
        }
    }
    x
}

fn blok_harmonogramu_platnosci(p: &PelnaUmowa) -> String {
    let mut wiersze = vec![(
        true,
        vec![
            Komorka::naglowek("Lp."),
            Komorka::naglowek("Płatność"),
            Komorka::naglowek("Kwota brutto").do_prawej(),
            Komorka::naglowek("Termin"),
        ],
    )];
    for (i, r) in p.plan.pozycje.iter().enumerate() {
        let termin = match &r.termin {
            Some(t) if !r.szacunkowy => format!("do {} ({})", formaty::data(t), r.termin_opis),
            _ => r.termin_opis.clone(),
        };
        wiersze.push((
            false,
            vec![
                Komorka::t(format!("{}.", i + 1)),
                Komorka::t(format!("{}\n(słownie: {})", r.opis, slownie::kwota(r.kwota_gr))),
                Komorka::t(formaty::kwota(r.kwota_gr)).do_prawej(),
                Komorka::t(termin),
            ],
        ));
    }
    wiersze.push((
        false,
        vec![
            Komorka::t("Razem").pogrubiona().scalona(2),
            Komorka::t(formaty::kwota(p.plan.suma_gr)).pogrubiona().do_prawej(),
            Komorka::t(""),
        ],
    ));
    if p.plan.pozycje.is_empty() {
        return xml::akapit_tekst("(plan płatności niekompletny – uzupełnij wartość, zadatek i wariant)", false);
    }
    xml::tabela(&[600, 4400, 1800, 2800], &wiersze)
}

fn blok_etapow(p: &PelnaUmowa) -> String {
    if p.etapy.is_empty() {
        return xml::akapit_tekst("(brak harmonogramu etapów)", false);
    }
    let mut wiersze = vec![(
        true,
        vec![
            Komorka::naglowek("Lp."),
            Komorka::naglowek("Nazwa pracy"),
            Komorka::naglowek("Termin rozpoczęcia"),
            Komorka::naglowek("Kwota do rozliczenia").do_prawej(),
        ],
    )];
    for e in &p.etapy {
        wiersze.push((
            false,
            vec![
                Komorka::t(format!("{}.", e.lp)),
                Komorka::t(e.nazwa.clone()),
                Komorka::t(data_opc(&e.termin)),
                Komorka::t(formaty::kwota(e.kwota_gr)).do_prawej(),
            ],
        ));
    }
    wiersze.push((
        false,
        vec![
            Komorka::t("Suma").pogrubiona().scalona(3),
            Komorka::t(formaty::kwota(p.kontrola_etapow.suma_gr)).pogrubiona().do_prawej(),
        ],
    ));
    xml::tabela(&[600, 5000, 2000, 2000], &wiersze)
}

/// Dane szablonu umowy. Po wyjściu ze szkicu dane stron pochodzą z migawki.
pub async fn dane_umowy(pula: &SqlitePool, p: &PelnaUmowa) -> Wynik<DaneSzablonu> {
    let u = &p.umowa;
    let m = match &u.migawka {
        Some(m) => m.clone(),
        None => {
            let mut db = pula.acquire().await?;
            umowy::migawka_w(&mut db, u).await?
        }
    };
    let dn = &u.dane;
    let mut d = DaneSzablonu::default();
    pola_stron(&mut d, &m);
    d.pole("numer umowy", u.numer());
    d.pole("miejscowość", dn.miejscowosc.clone().unwrap_or_default());
    d.pole("data umowy", data_opc(&dn.data_zawarcia));
    d.pole("adres montażu", linia_adresu(&dn.montaz_ulica, &dn.montaz_kod, &dn.montaz_miasto));
    d.pole("piętro", dn.montaz_pietro.clone().unwrap_or_default());
    d.pole("kod domofonu", dn.montaz_kod_domofonu.clone().unwrap_or_default());
    d.pole("parking", dn.montaz_parking.clone().unwrap_or_default());
    d.pole("wniesienie", dn.montaz_winda_wniesienie.clone().unwrap_or_default());
    d.pole(
        "liczba dni",
        dn.dni_na_brakujace_informacje.map(|n| n.to_string()).unwrap_or_default(),
    );
    d.pole("wartość", kwota_opc(dn.wartosc_brutto_gr));
    d.pole("kwota słownie", dn.wartosc_brutto_gr.map(slownie::kwota).unwrap_or_default());
    d.pole("stawka VAT", dn.stawka_vat.map(|s| s.to_string()).unwrap_or_default());
    d.pole("zadatek", kwota_opc(dn.zadatek_gr));
    d.pole("zadatek słownie", dn.zadatek_gr.map(slownie::kwota).unwrap_or_default());
    let zadatek = p.plan.pozycje.iter().find(|x| x.rodzaj == "zadatek");
    d.pole(
        "termin zadatku",
        zadatek.map(|z| data_opc(&z.termin)).unwrap_or_else(|| data_opc(&dn.termin_zadatku)),
    );
    d.pole("wariant A lub B", dn.wariant_platnosci.clone().unwrap_or_default());
    let przed = p.plan.pozycje.iter().find(|x| x.rodzaj == "przed_montazem");
    d.pole("kwota przed montażem", przed.map(|x| kwota_bez_zl(x.kwota_gr)).unwrap_or_default());
    d.pole("kwota przed montażem słownie", przed.map(|x| slownie::kwota(x.kwota_gr)).unwrap_or_default());
    d.pole("termin kwoty przed montażem", przed.map(|x| data_opc(&x.termin)).unwrap_or_default());
    let koncowa = p.plan.pozycje.iter().find(|x| x.rodzaj == "koncowa");
    d.pole("pozostała kwota", koncowa.map(|x| kwota_bez_zl(x.kwota_gr)).unwrap_or_default());
    d.pole("pozostała kwota słownie", koncowa.map(|x| slownie::kwota(x.kwota_gr)).unwrap_or_default());
    d.pole(
        "termin pozostałej kwoty",
        koncowa
            .map(|x| if x.szacunkowy { x.termin_opis.clone() } else { data_opc(&x.termin) })
            .unwrap_or_default(),
    );
    d.pole("termin montażu", data_opc(&dn.montaz_do));
    d.pole(
        "nazwa zabudowy",
        p.specyfikacja
            .sekcje
            .iter()
            .filter(|s| s.typ == "meble")
            .map(|s| s.nazwa.clone())
            .collect::<Vec<_>>()
            .join(", "),
    );
    let rodzaj = dn.vat_rodzaj_nieruchomosci.as_deref().unwrap_or("");
    d.pole("rodzaj nieruchomości", umowy::nazwa_rodzaju_nieruchomosci(rodzaj));
    d.pole("powierzchnia", dn.vat_powierzchnia_m2.map(formaty::ilosc).unwrap_or_default());
    d.pole("podstawa powierzchni", dn.vat_podstawa_powierzchni.clone().unwrap_or_default());
    d.pole("VAT lokal", pole_wyboru(rodzaj == "lokal_mieszkalny"));
    d.pole("VAT dom", pole_wyboru(rodzaj == "dom_jednorodzinny"));
    d.pole("VAT inny", pole_wyboru(rodzaj == "inny"));
    d.pole("data odbioru", p.protokol.as_ref().map(|x| formaty::data(&x.data)).unwrap_or_default());
    d.pole("wpłacono", kwota_bez_zl(p.saldo.wplacono_gr));
    d.pole("pozostało", kwota_bez_zl(p.saldo.pozostalo_gr.max(0)));
    d.pole(
        "termin płatności",
        p.saldo
            .nastepna
            .as_ref()
            .and_then(|n| n.termin.as_deref())
            .map(formaty::data)
            .unwrap_or_default(),
    );
    let bez_uwag = p.protokol.as_ref().map(|x| x.bez_zastrzezen);
    d.pole("odbiór bez uwag", pole_wyboru(bez_uwag == Some(true)));
    d.pole("odbiór z uwagami", pole_wyboru(bez_uwag == Some(false)));
    d.pole("uwagi do odbioru", p.protokol.as_ref().and_then(|x| x.uwagi.clone()).unwrap_or_default());
    d.pole("data zgody", data_opc(&dn.data_zawarcia));

    d.blok("specyfikacja", blok_specyfikacji(&p.specyfikacja));
    d.blok("harmonogram płatności", blok_harmonogramu_platnosci(p));
    d.blok("harmonogram etapów", blok_etapow(p));

    let zal = dn.zalaczniki.clone().unwrap_or_default();
    for n in 1..=6 {
        d.warunek(&format!("załącznik {n}"), zal.iter().any(|z| z == &n.to_string()));
    }
    let wariant = dn.wariant_platnosci.as_deref();
    d.warunek("wariant A", wariant == Some("A"));
    d.warunek("wariant B", wariant == Some("B"));
    d.warunek("stawka 8", dn.stawka_vat == Some(8));
    d.warunek("stawka 23", dn.stawka_vat == Some(23));
    Ok(d)
}

pub async fn dane_aneksu(pula: &SqlitePool, a: &PelnyAneks) -> Wynik<DaneSzablonu> {
    let p = umowy::pelna(pula, a.aneks.umowa_id).await?;
    let mut d = dane_umowy(pula, &p).await?;
    let an = &a.aneks;
    d.pole("numer aneksu", an.nr.to_string());
    d.pole("data aneksu", data_opc(&an.data));
    d.pole("opis zmian", an.opis_zmian.clone());
    let znak = if an.zmiana_ceny_gr > 0 { "+" } else { "" };
    d.pole("zmiana kwoty", format!("{znak}{}", kwota_bez_zl(an.zmiana_ceny_gr)));
    d.pole("poprzednia wartość", kwota_opc(an.poprzednia_wartosc_gr));
    d.pole("nowa wartość", kwota_opc(a.nowa_wartosc_gr));
    d.pole("nowa wartość słownie", a.nowa_wartosc_gr.map(slownie::kwota).unwrap_or_default());
    d.pole("poprzedni termin montażu", data_opc(&an.poprzedni_termin_montazu));
    d.pole(
        "nowy termin montażu",
        data_opc(&an.nowy_termin_montazu.clone().or(an.poprzedni_termin_montazu.clone())),
    );
    let lista: String = if a.lista_zmian.is_empty() {
        xml::akapit_tekst("(brak zmian)", false)
    } else {
        a.lista_zmian
            .iter()
            .enumerate()
            .map(|(i, z)| xml::akapit(&xml::przebieg(&format!("{}. {z}", i + 1), false, None), None, Some(60), false))
            .collect()
    };
    d.blok("lista zmian", lista);
    if let Some(s) = &a.specyfikacja {
        d.blok("specyfikacja", blok_specyfikacji(s));
    }
    Ok(d)
}

// ---------------------------------------------------------------------------
// Szablony przykładowe (fikcyjne) – generowane w kodzie
// ---------------------------------------------------------------------------

fn akapit_t(t: &str) -> Paragraph {
    Paragraph::new().add_run(Run::new().add_text(t))
}
fn naglowek_t(t: &str) -> Paragraph {
    Paragraph::new().add_run(Run::new().add_text(t).bold().size(24))
}
fn nowa_strona(t: &str) -> Paragraph {
    Paragraph::new()
        .add_run(Run::new().add_break(BreakType::Page))
        .add_run(Run::new().add_text(t).bold().size(26))
}

fn dokument_przykladowy(tresc: Vec<Paragraph>, stopka: &str) -> Wynik<Vec<u8>> {
    let znak = Paragraph::new().add_run(
        Run::new()
            .add_text(format!("{ZNAK_PRZYKLADU} – dokument wygenerowany z wbudowanego szablonu. Wgraj własny szablon: Umowy → Ustawienia → Szablony dokumentów."))
            .bold()
            .color("C00000")
            .size(18),
    );
    let mut d = Docx::new()
        .page_size(11_906, 16_838)
        .page_margin(PageMargin::new().top(1_000).bottom(1_000).left(1_134).right(1_134))
        .default_fonts(RunFonts::new().ascii("Calibri").hi_ansi("Calibri").cs("Arial").east_asia("Arial"))
        .default_size(21)
        .header(Header::new().add_paragraph(
            Paragraph::new().add_run(Run::new().add_text(format!("{ZNAK_PRZYKLADU} · umowa nr [numer umowy]")).color("C00000").size(16)),
        ))
        .footer(Footer::new().add_paragraph(
            Paragraph::new().add_run(Run::new().add_text(stopka).color("888888").size(16)),
        ))
        .add_paragraph(znak);
    for a in tresc {
        d = d.add_paragraph(a);
    }
    let mut bufor = Cursor::new(Vec::new());
    d.build()
        .pack(&mut bufor)
        .map_err(|e| BladApi::Wewnetrzny(format!("szablon przykładowy: {e}")))?;
    Ok(bufor.into_inner())
}

/// Fikcyjny szablon umowy ze wszystkimi polami (tekst ogólny, bez treści prawdziwej umowy).
pub fn szablon_przykladowy_umowy() -> Wynik<Vec<u8>> {
    let t = [
        ("h", "UMOWA NR [numer umowy]"),
        ("", "zawarta w dniu [data umowy] w miejscowości [miejscowość] pomiędzy:"),
        ("", "[wykonawca], [adres wykonawcy], NIP [nip wykonawcy], tel. [telefon wykonawcy], e-mail [e-mail wykonawcy] – Wykonawcą,"),
        ("", "a [klient], [adres klienta], tel. [telefon], e-mail [e-mail] – Zamawiającym. Osoby kontaktowe: [osoby kontaktowe]."),
        ("h", "§ 1. Przedmiot"),
        ("", "Wykonawca wykona i zamontuje zabudowę ([nazwa zabudowy]) zgodnie ze specyfikacją – załącznik nr 1."),
        ("h", "§ 2. Miejsce montażu"),
        ("", "Adres: [adres montażu]; piętro: [piętro]; kod domofonu: [kod domofonu]; parking: [parking]; winda i wniesienie: [wniesienie]."),
        ("h", "§ 3. Terminy"),
        ("", "Planowany termin rozpoczęcia montażu do: [termin montażu]. Brakujące informacje Zamawiający przekaże w ciągu [liczba dni] dni kalendarzowych od podpisania umowy."),
        ("h", "§ 4. Wynagrodzenie i płatności"),
        ("", "Wynagrodzenie: [wartość] zł brutto (słownie: [kwota słownie]), stawka VAT [stawka VAT]%."),
        ("", "Zadatek: [zadatek] zł (słownie: [zadatek słownie]), płatny do [termin zadatku]. Wariant płatności: [wariant A lub B]."),
        ("", "[[wariant A]]"),
        ("", "Wariant A – pozostała kwota [pozostała kwota] zł płatna do [termin pozostałej kwoty]."),
        ("", "[[/wariant A]]"),
        ("", "[[wariant B]]"),
        ("", "Wariant B – [kwota przed montażem] zł (słownie: [kwota przed montażem słownie]) do [termin kwoty przed montażem], pozostała kwota [pozostała kwota] zł (słownie: [pozostała kwota słownie]): [termin pozostałej kwoty]."),
        ("", "[[/wariant B]]"),
        ("", "Harmonogram płatności:"),
        ("", "[harmonogram płatności]"),
        ("", "Rachunek Wykonawcy: [bank], nr [numer rachunku], SWIFT [swift]."),
        ("h", "§ 5. Oświadczenie VAT"),
        ("", "[VAT lokal] lokal mieszkalny    [VAT dom] budynek mieszkalny jednorodzinny    [VAT inny] inny"),
        ("", "Rodzaj: [rodzaj nieruchomości]. Powierzchnia użytkowa: [powierzchnia] m². Podstawa określenia powierzchni: [podstawa powierzchni]."),
        ("h", "§ 6. Postanowienia końcowe"),
        ("", "Tekst przykładowy. W tym miejscu Twój szablon zawiera postanowienia umowy. Data wydruku: [data wydruku]. NIP Zamawiającego: [nip klienta]."),
        ("", ""),
        ("", "Wykonawca: ……………………………               Zamawiający: ……………………………"),
        ("p", "Załącznik nr 1 – Specyfikacja zamówienia do umowy nr [numer umowy]"),
        ("", "Termin rozpoczęcia do: [termin montażu]"),
        ("", "[specyfikacja]"),
        ("", "[[załącznik 2]]"),
        ("p", "Załącznik nr 2 – Harmonogram etapów do umowy nr [numer umowy]"),
        ("", "[harmonogram etapów]"),
        ("", "[[/załącznik 2]]"),
        ("", "[[załącznik 3]]"),
        ("p", "Załącznik nr 3 – Cennik montażu AGD"),
        ("", "Tekst przykładowy cennika."),
        ("", "[[/załącznik 3]]"),
        ("p", "Załącznik nr 4 – Standard wykonania i odbioru do umowy nr [numer umowy]"),
        ("", "Tekst przykładowy standardu wykonania."),
        ("", "[[załącznik 5]]"),
        ("p", "Załącznik nr 5 – Protokół odbioru"),
        ("", "Umowa nr [numer umowy]. Data odbioru: [data odbioru]. Adres realizacji: [adres montażu]. Zamawiający: [klient]."),
        ("", "Wartość umowy: [wartość] zł; wpłacono: [wpłacono] zł; pozostało do zapłaty: [pozostało] zł; termin płatności: [termin płatności]."),
        ("", "[odbiór bez uwag] bez uwag    [odbiór z uwagami] z uwagami: [uwagi do odbioru]"),
        ("", "Wykonawca: ……………………………               Zamawiający: ……………………………"),
        ("", "[[/załącznik 5]]"),
        ("", "[[załącznik 6]]"),
        ("p", "Załącznik nr 6 – Zgoda na zdjęcia"),
        ("", "Tekst przykładowy zgody. Data: [data zgody]."),
        ("", "[[/załącznik 6]]"),
    ];
    let tresc = t
        .iter()
        .map(|(r, s)| match *r {
            "h" => naglowek_t(s),
            "p" => nowa_strona(s),
            _ => akapit_t(s),
        })
        .collect();
    dokument_przykladowy(tresc, "Umowa nr [numer umowy] · SZABLON PRZYKŁADOWY")
}

pub fn szablon_przykladowy_aneksu() -> Wynik<Vec<u8>> {
    let t = [
        ("h", "ANEKS NR [numer aneksu] DO UMOWY NR [numer umowy] z dnia [data umowy]"),
        ("", "zawarty w dniu [data aneksu] pomiędzy [wykonawca] a [klient]."),
        ("h", "§ 1. Zmiany"),
        ("", "Strony zmieniają umowę w następującym zakresie:"),
        ("", "[lista zmian]"),
        ("h", "§ 2. Wynagrodzenie i termin"),
        ("", "Wynagrodzenie po zmianie: [nowa wartość] zł brutto (słownie: [nowa wartość słownie]); dotychczas [poprzednia wartość] zł."),
        ("", "Termin rozpoczęcia montażu: [nowy termin montażu]."),
        ("", "Pozostałe postanowienia umowy pozostają bez zmian."),
        ("", ""),
        ("", "Wykonawca: ……………………………               Zamawiający: ……………………………"),
        ("p", "Specyfikacja po zmianach"),
        ("", "[specyfikacja]"),
    ];
    let tresc = t
        .iter()
        .map(|(r, s)| match *r {
            "h" => naglowek_t(s),
            "p" => nowa_strona(s),
            _ => akapit_t(s),
        })
        .collect();
    dokument_przykladowy(tresc, "Aneks nr [numer aneksu] do umowy nr [numer umowy] · SZABLON PRZYKŁADOWY")
}

// ---------------------------------------------------------------------------
// Przechowywanie szablonów użytkownika
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MetaSzablonu {
    pub nazwa_pliku: String,
    pub wgrano: String,
    pub rozmiar: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct StanSzablonu {
    pub rodzaj: String,
    /// `wlasny` albo `przykladowy`.
    pub zrodlo: String,
    pub meta: Option<MetaSzablonu>,
    pub raport: RaportSzablonu,
}

fn katalog(folder: &Path) -> PathBuf {
    folder.join("szablony-dokumentow")
}

fn sprawdz_rodzaj(r: &str) -> Wynik<()> {
    if RODZAJE_SZABLONOW.contains(&r) {
        Ok(())
    } else {
        Err(BladApi::NieZnaleziono(format!("rodzaj szablonu „{r}” (umowa, aneks)")))
    }
}

/// Szablon do użycia: wgrany przez użytkownika albo przykładowy (`true` = przykładowy).
pub fn szablon(folder: &Path, rodzaj: &str) -> Wynik<(Vec<u8>, bool)> {
    sprawdz_rodzaj(rodzaj)?;
    let plik = katalog(folder).join(format!("{rodzaj}.docx"));
    match std::fs::read(&plik) {
        Ok(b) => Ok((b, false)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok((
            if rodzaj == "aneks" {
                szablon_przykladowy_aneksu()?
            } else {
                szablon_przykladowy_umowy()?
            },
            true,
        )),
        Err(e) => Err(BladApi::Wewnetrzny(format!("odczyt szablonu: {e}"))),
    }
}

/// Dane z kompletem kluczy (wartości przykładowe) – do rozpoznania pól przy analizie szablonu.
pub fn wszystkie_klucze() -> DaneSzablonu {
    let mut d = DaneSzablonu::default();
    for o in POLA {
        match o.rodzaj {
            "blok" => d.blok(o.nazwa, String::new()),
            "warunek" => {}
            _ => d.pole(o.nazwa, "x"),
        }
    }
    for n in 1..=6 {
        d.warunek(&format!("załącznik {n}"), true);
    }
    for w in ["wariant A", "wariant B", "stawka 8", "stawka 23"] {
        d.warunek(w, true);
    }
    for (a, n) in ALIASY {
        d.alias(a, n);
    }
    d.pomin("uzupełnij");
    d
}

pub fn stan(folder: &Path, rodzaj: &str) -> Wynik<StanSzablonu> {
    let (b, przykladowy) = szablon(folder, rodzaj)?;
    let meta = std::fs::read(katalog(folder).join(format!("{rodzaj}.json")))
        .ok()
        .and_then(|m| serde_json::from_slice(&m).ok());
    let raport = szablon_docx::analizuj(&b, &wszystkie_klucze())?;
    Ok(StanSzablonu {
        rodzaj: rodzaj.into(),
        zrodlo: if przykladowy { "przykladowy" } else { "wlasny" }.into(),
        meta: if przykladowy { None } else { meta },
        raport,
    })
}

/// Zapis szablonu (zapis atomowy: plik tymczasowy → zmiana nazwy). Plik jest najpierw sprawdzany.
pub fn zapisz(folder: &Path, rodzaj: &str, nazwa: &str, bajty: &[u8]) -> Wynik<StanSzablonu> {
    sprawdz_rodzaj(rodzaj)?;
    if bajty.len() > 50 * 1024 * 1024 {
        return Err(BladApi::ZaDuzo("szablon może mieć najwyżej 50 MB".into()));
    }
    szablon_docx::analizuj(bajty, &wszystkie_klucze())?;
    let k = katalog(folder);
    std::fs::create_dir_all(&k).map_err(|e| BladApi::Wewnetrzny(format!("folder szablonów: {e}")))?;
    let tmp = k.join(format!(".{rodzaj}.docx.tmp"));
    std::fs::write(&tmp, bajty).map_err(|e| BladApi::Wewnetrzny(format!("zapis szablonu: {e}")))?;
    std::fs::rename(&tmp, k.join(format!("{rodzaj}.docx")))
        .map_err(|e| BladApi::Wewnetrzny(format!("zapis szablonu: {e}")))?;
    let meta = MetaSzablonu {
        nazwa_pliku: nazwa.chars().take(200).collect(),
        wgrano: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        rozmiar: bajty.len() as u64,
    };
    let _ = std::fs::write(
        k.join(format!("{rodzaj}.json")),
        serde_json::to_vec(&meta).unwrap_or_default(),
    );
    stan(folder, rodzaj)
}

pub fn usun(folder: &Path, rodzaj: &str) -> Wynik<StanSzablonu> {
    sprawdz_rodzaj(rodzaj)?;
    let k = katalog(folder);
    for p in [k.join(format!("{rodzaj}.docx")), k.join(format!("{rodzaj}.json"))] {
        match std::fs::remove_file(&p) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(BladApi::Wewnetrzny(format!("usuwanie szablonu: {e}"))),
        }
    }
    stan(folder, rodzaj)
}

// ---------------------------------------------------------------------------
// Generowanie
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct WartoscPola {
    pub pole: String,
    pub wartosc: String,
    pub pusta: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct PodgladPol {
    pub zrodlo: String,
    pub nazwa_szablonu: Option<String>,
    /// Pola znalezione w szablonie z wartościami (do sprawdzenia przed generowaniem).
    pub pola: Vec<WartoscPola>,
    pub raport: RaportSzablonu,
}

pub struct Dokument {
    pub bajty: Vec<u8>,
    pub raport: RaportSzablonu,
    pub przykladowy: bool,
}

pub async fn generuj(pula: &SqlitePool, folder: &Path, rodzaj: &str, d: DaneSzablonu, tylko: Option<String>) -> Wynik<Dokument> {
    let (szablon, przykladowy) = szablon(folder, rodzaj)?;
    let _ = pula;
    let (bajty, raport) = tokio::task::spawn_blocking(move || szablon_docx::wypelnij(&szablon, &d, tylko.as_deref()))
        .await
        .map_err(|e| BladApi::Wewnetrzny(format!("generowanie dokumentu: {e}")))??;
    Ok(Dokument { bajty, raport, przykladowy })
}

pub fn podglad(d: &DaneSzablonu, dok: &Dokument, meta: Option<MetaSzablonu>) -> PodgladPol {
    let pola = dok
        .raport
        .pola
        .iter()
        .filter(|n| !d.bloki.contains_key(&d.klucz(n)))
        .map(|n| {
            let w = d.pola.get(&d.klucz(n)).cloned().unwrap_or_default();
            WartoscPola {
                pole: n.clone(),
                pusta: w.trim().is_empty(),
                wartosc: w,
            }
        })
        .collect();
    PodgladPol {
        zrodlo: if dok.przykladowy { "przykladowy" } else { "wlasny" }.into(),
        nazwa_szablonu: meta.map(|m| m.nazwa_pliku),
        pola,
        raport: dok.raport.clone(),
    }
}

pub fn meta(folder: &Path, rodzaj: &str) -> Option<MetaSzablonu> {
    std::fs::read(katalog(folder).join(format!("{rodzaj}.json")))
        .ok()
        .and_then(|m| serde_json::from_slice(&m).ok())
}

/// Nazwa pliku bez polskich znaków i ukośników, np. `Umowa_07-2026.docx`.
pub fn nazwa_pliku(prefiks: &str, numer: &str, rozszerzenie: &str) -> String {
    let n: String = numer
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    format!("{prefiks}_{n}.{rozszerzenie}")
}

// ---------------------------------------------------------------------------
// PDF
// ---------------------------------------------------------------------------

/// PDF z gotowego DOCX – przez program zgodny z Wordem na komputerze-serwerze.
///
/// Decyzja: układ szablonu użytkownika (style, nagłówki, tabele, podziały stron) potrafi
/// wiernie odtworzyć tylko silnik edytora tekstu, a nie własny generator PDF. Dlatego:
/// 1. Windows – Microsoft Word (automatyzacja COM przez PowerShell, `ExportAsFixedFormat`),
/// 2. LibreOffice (`soffice --headless --convert-to pdf`) – Windows i Linux,
/// 3. brak obu → 422 z instrukcją: pobierz DOCX i zapisz jako PDF w Wordzie.
///
/// Zmienna `ROBINWOOD_SOFFICE` może wskazać ścieżkę do `soffice`.
pub async fn pdf_z_docx(docx: Vec<u8>) -> Wynik<Vec<u8>> {
    let katalog = std::env::temp_dir().join(format!(
        "robinwood-pdf-{}-{}",
        std::process::id(),
        rand::random::<u64>()
    ));
    std::fs::create_dir_all(&katalog).map_err(|e| BladApi::Wewnetrzny(format!("PDF: {e}")))?;
    let wej = katalog.join("dokument.docx");
    let wyj = katalog.join("dokument.pdf");
    std::fs::write(&wej, &docx).map_err(|e| BladApi::Wewnetrzny(format!("PDF: {e}")))?;
    let wynik = konwertuj(&katalog, &wej, &wyj).await;
    let pdf = std::fs::read(&wyj).ok();
    let _ = std::fs::remove_dir_all(&katalog);
    match (wynik, pdf) {
        (_, Some(p)) if p.starts_with(b"%PDF") => Ok(p),
        (Err(proby), _) => Err(BladApi::NieprawidloweDane(format!(
            "nie można utworzyć PDF na komputerze-serwerze ({proby}). PDF powstaje z dokumentu Word: \
             zainstaluj na serwerze Microsoft Word albo LibreOffice, albo pobierz DOCX i zapisz go jako PDF w Wordzie (Plik → Zapisz jako → PDF)"
        ))),
        _ => Err(BladApi::NieprawidloweDane(
            "konwersja do PDF nie dała wyniku – pobierz DOCX i zapisz go jako PDF w Wordzie".into(),
        )),
    }
}

async fn uruchom(mut k: tokio::process::Command) -> Result<(), String> {
    k.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    let dziecko = k.spawn().map_err(|e| e.to_string())?;
    match tokio::time::timeout(std::time::Duration::from_secs(120), dziecko.wait_with_output()).await {
        Ok(Ok(o)) if o.status.success() => Ok(()),
        Ok(Ok(o)) => Err(String::from_utf8_lossy(&o.stderr).chars().take(200).collect()),
        Ok(Err(e)) => Err(e.to_string()),
        Err(_) => Err("przekroczony czas 120 s".into()),
    }
}

async fn konwertuj(katalog: &Path, wej: &Path, wyj: &Path) -> Result<(), String> {
    let mut bledy = Vec::new();
    if cfg!(windows) {
        let skrypt = format!(
            "$ErrorActionPreference='Stop'; $w = New-Object -ComObject Word.Application; $w.Visible = $false; \
             try {{ $d = $w.Documents.Open('{}', $false, $true); $d.ExportAsFixedFormat('{}', 17); $d.Close($false) }} finally {{ $w.Quit() }}",
            wej.display().to_string().replace('\'', "''"),
            wyj.display().to_string().replace('\'', "''")
        );
        let mut k = tokio::process::Command::new("powershell");
        k.args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", &skrypt]);
        match uruchom(k).await {
            Ok(()) if wyj.exists() => return Ok(()),
            Ok(()) => bledy.push("Word: brak pliku wynikowego".to_string()),
            Err(e) => bledy.push(format!("Word: {e}")),
        }
    }
    let mut kandydaci: Vec<String> = Vec::new();
    if let Ok(s) = std::env::var("ROBINWOOD_SOFFICE") {
        kandydaci.push(s);
    }
    if cfg!(windows) {
        kandydaci.push(r"C:\Program Files\LibreOffice\program\soffice.exe".into());
        kandydaci.push(r"C:\Program Files (x86)\LibreOffice\program\soffice.exe".into());
    }
    kandydaci.push("soffice".into());
    kandydaci.push("libreoffice".into());
    let profil = format!(
        "-env:UserInstallation=file:///{}",
        katalog.join("profil").display().to_string().replace('\\', "/").trim_start_matches('/')
    );
    for s in kandydaci {
        let mut k = tokio::process::Command::new(&s);
        k.arg(&profil)
            .args(["--headless", "--norestore", "--convert-to", "pdf", "--outdir"])
            .arg(katalog)
            .arg(wej);
        match uruchom(k).await {
            Ok(()) if wyj.exists() => return Ok(()),
            Ok(()) => bledy.push(format!("{s}: brak pliku wynikowego")),
            Err(e) => bledy.push(format!("{s}: {e}")),
        }
    }
    tracing::warn!("PDF umowy: {}", bledy.join("; "));
    Err(if cfg!(windows) {
        "brak Microsoft Word i LibreOffice".into()
    } else {
        "brak LibreOffice".into()
    })
}

/// Data z tekstu ISO (pomocniczo do testów).
pub fn data(t: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(t, "%Y-%m-%d").ok()
}

#[cfg(test)]
mod testy {
    use super::*;

    #[test]
    fn szablony_przykladowe_maja_wszystkie_pola() {
        let u = szablon_przykladowy_umowy().unwrap();
        let r = szablon_docx::analizuj(&u, &wszystkie_klucze()).unwrap();
        assert!(r.nieznane.is_empty(), "{:?}", r.nieznane);
        for o in POLA.iter().filter(|o| o.dokument == "umowa" && o.rodzaj != "warunek") {
            assert!(
                r.pola.iter().any(|x| szablon_docx::normalizuj(x) == szablon_docx::normalizuj(o.nazwa)),
                "brak pola {} w szablonie przykładowym",
                o.nazwa
            );
        }
        let a = szablon_przykladowy_aneksu().unwrap();
        let r = szablon_docx::analizuj(&a, &wszystkie_klucze()).unwrap();
        assert!(r.nieznane.is_empty(), "{:?}", r.nieznane);
        assert!(r.ostrzezenia.is_empty(), "{:?}", r.ostrzezenia);
    }

    #[test]
    fn kwoty_bez_waluty() {
        assert_eq!(kwota_bez_zl(4_835_200), "48\u{a0}352,00");
        assert_eq!(nazwa_pliku("Umowa", "07/2026", "docx"), "Umowa_07-2026.docx");
    }
}
