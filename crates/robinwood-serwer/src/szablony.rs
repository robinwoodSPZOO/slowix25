//! Szablony mebli (typ zabudowy → typowe części i wiersze) oraz import słowników i szablonów
//! z plików użytkownika (`slowniki.json`, `szablony-mebli.json`).
//!
//! Formaty importu opisuje docs/ARCHITEKTURA.md (rozdz. „Import słowników i szablonów”).
//! Import jest addytywny: istniejące pozycje nie są usuwane ani dublowane.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{FromRow, SqliteConnection, SqlitePool};

use crate::bledy::{BladApi, BladPozycji, Wynik};
use crate::specyfikacje::{self, dziedziczy_material_korpusu, WierszSzablonu, WynikZmiany};

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default)]
pub struct DaneSzablonu {
    pub czesci: Vec<String>,
    pub wiersze: Vec<WierszSzablonu>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Szablon {
    pub id: i64,
    pub kod: String,
    pub nazwa: String,
    pub tytul_sekcji: String,
    pub zrodlo: Option<String>,
    pub czesci: Vec<String>,
    pub wiersze: Vec<WierszSzablonu>,
}

#[derive(FromRow)]
struct SzablonBaza {
    id: i64,
    kod: String,
    nazwa: String,
    tytul_sekcji: String,
    zrodlo: Option<String>,
    dane: String,
}

impl SzablonBaza {
    fn rozpakuj(self) -> Szablon {
        let d: DaneSzablonu = serde_json::from_str(&self.dane).unwrap_or_default();
        Szablon {
            id: self.id,
            kod: self.kod,
            nazwa: self.nazwa,
            tytul_sekcji: self.tytul_sekcji,
            zrodlo: self.zrodlo,
            czesci: d.czesci,
            wiersze: d.wiersze,
        }
    }
}

pub async fn lista(pula: &SqlitePool) -> Wynik<Vec<Szablon>> {
    let w: Vec<SzablonBaza> = sqlx::query_as(
        "SELECT id, kod, nazwa, tytul_sekcji, zrodlo, dane FROM szablony_mebli WHERE aktywny = 1 \
         ORDER BY kolejnosc, nazwa",
    )
    .fetch_all(pula)
    .await?;
    Ok(w.into_iter().map(SzablonBaza::rozpakuj).collect())
}

async fn pobierz_w(db: &mut SqliteConnection, id: i64) -> Wynik<Szablon> {
    let w: Option<SzablonBaza> = sqlx::query_as(
        "SELECT id, kod, nazwa, tytul_sekcji, zrodlo, dane FROM szablony_mebli WHERE id = ?1",
    )
    .bind(id)
    .fetch_optional(&mut *db)
    .await?;
    w.map(SzablonBaza::rozpakuj)
        .ok_or_else(|| BladApi::NieZnaleziono(format!("szablon {id}")))
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct WstawCzesc {
    pub szablon_id: i64,
    /// Nazwa nowej części; domyślnie pierwsza typowa część szablonu.
    pub nazwa: Option<String>,
    pub po_czesci_id: Option<i64>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct WstawSekcje {
    pub szablon_id: i64,
    pub nazwa: Option<String>,
    /// Części do utworzenia; domyślnie pierwsza typowa część szablonu.
    pub czesci: Option<Vec<String>>,
}

/// Nowa część w sekcji z wierszami szablonu.
pub async fn wstaw_czesc(pula: &SqlitePool, sekcja_id: i64, d: WstawCzesc) -> Wynik<WynikZmiany> {
    let mut tx = crate::baza::transakcja_zapisu(pula).await?;
    let spec_id = specyfikacje::spec_sekcji_pub(&mut tx, sekcja_id).await?;
    specyfikacje::sprawdz_zapis(&mut tx, spec_id).await?;
    let sz = pobierz_w(&mut tx, d.szablon_id).await?;
    let nazwa = d
        .nazwa
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty())
        .or_else(|| sz.czesci.first().cloned())
        .unwrap_or_else(|| sz.nazwa.clone());
    let id = specyfikacje::wstaw_czesc_z_wierszami(
        &mut tx,
        sekcja_id,
        &nazwa,
        &sz.wiersze,
        d.po_czesci_id,
    )
    .await?;
    specyfikacje::dziennik(&mut tx, "czesci_specyfikacji", id, "dodanie").await?;
    let w = specyfikacje::pobierz_w_wynik(&mut tx, id, spec_id).await?;
    tx.commit().await?;
    Ok(w)
}

/// Nowa sekcja (pomieszczenie) z częściami z szablonu.
pub async fn wstaw_sekcje(pula: &SqlitePool, spec_id: i64, d: WstawSekcje) -> Wynik<WynikZmiany> {
    let mut tx = crate::baza::transakcja_zapisu(pula).await?;
    specyfikacje::sprawdz_zapis(&mut tx, spec_id).await?;
    let sz = pobierz_w(&mut tx, d.szablon_id).await?;
    let nazwa = d
        .nazwa
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| sz.tytul_sekcji.clone());
    let sekcja = specyfikacje::dodaj_sekcje_w(&mut tx, spec_id, &nazwa, "meble").await?;
    let czesci = d.czesci.unwrap_or_else(|| {
        sz.czesci
            .first()
            .cloned()
            .map(|c| vec![c])
            .unwrap_or_else(|| vec![sz.nazwa.clone()])
    });
    for c in czesci.iter().filter(|c| !c.trim().is_empty()) {
        specyfikacje::wstaw_czesc_z_wierszami(&mut tx, sekcja, c.trim(), &sz.wiersze, None).await?;
    }
    specyfikacje::dziennik(&mut tx, "sekcje_specyfikacji", sekcja, "dodanie").await?;
    let w = specyfikacje::pobierz_w_wynik(&mut tx, sekcja, spec_id).await?;
    tx.commit().await?;
    Ok(w)
}

// ---------------------------------------------------------------------------
// Import
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Serialize, PartialEq)]
pub struct RaportImportu {
    /// Liczba nowych pozycji wg kodu słownika (albo „szablony”).
    pub dodane: BTreeMap<String, u32>,
    pub zaktualizowane: u32,
    pub pominiete: u32,
    pub nowe_kategorie: Vec<String>,
    pub ostrzezenia: Vec<String>,
}

#[derive(Debug, Default, Clone)]
struct NowaPozycja {
    wartosc: String,
    producent: Option<String>,
    dekor: Option<String>,
    kategoria: Option<String>,
    podgrupa: Option<String>,
}

fn tekst(v: &Value, klucz: &str) -> Option<String> {
    v.get(klucz)
        .and_then(Value::as_str)
        .map(oczysc_wartosc)
        .filter(|s| !s.is_empty())
}

fn tablica<'a>(v: &'a Value, sciezka: &[&str]) -> &'a [Value] {
    let mut biezacy = v;
    for k in sciezka {
        match biezacy.get(k) {
            Some(n) => biezacy = n,
            None => return &[],
        }
    }
    biezacy.as_array().map(Vec::as_slice).unwrap_or(&[])
}

/// Przycina spacje i końcowe separatory „|” pozostałe po eksporcie tabel.
fn oczysc_wartosc(s: &str) -> String {
    let t = s.trim().trim_end_matches('|').trim();
    t.chars().take(200).collect()
}

fn poz(wartosc: Option<String>) -> Option<NowaPozycja> {
    wartosc.map(|w| NowaPozycja {
        wartosc: w,
        ..Default::default()
    })
}

/// Rozpoznaje format pliku słowników i zwraca pozycje pogrupowane wg kodu słownika.
fn pozycje_z_pliku(j: &Value) -> Wynik<Vec<(String, NowaPozycja)>> {
    if !j.is_object() {
        return Err(BladApi::NieprawidloweDane(
            "plik słowników musi zawierać obiekt JSON".into(),
        ));
    }
    let mut wynik: Vec<(String, NowaPozycja)> = Vec::new();
    let mut dodaj = |kod: &str, p: Option<NowaPozycja>| {
        if let Some(p) = p {
            wynik.push((kod.to_string(), p));
        }
    };

    // Format prosty: {"slowniki": {"kod": ["wartość", {"wartosc": ..., "producent": ...}]}}
    if let Some(sl) = j.get("slowniki").and_then(Value::as_object) {
        for (kod, lista) in sl {
            for e in lista.as_array().map(Vec::as_slice).unwrap_or(&[]) {
                let p = match e {
                    Value::String(s) => poz(Some(oczysc_wartosc(s)).filter(|s| !s.is_empty())),
                    Value::Object(_) => tekst(e, "wartosc").map(|w| NowaPozycja {
                        wartosc: w,
                        producent: tekst(e, "producent"),
                        dekor: tekst(e, "dekor"),
                        kategoria: tekst(e, "kategoria"),
                        podgrupa: tekst(e, "podgrupa"),
                    }),
                    _ => None,
                };
                dodaj(kod, p);
            }
        }
    }

    // Format analityczny (słowniki wyprowadzone z dotychczasowych dokumentów).
    for r in tablica(j, &["rodzaje_elementow"]) {
        dodaj(
            "rodzaj_elementu",
            tekst(r, "nazwa").map(|w| NowaPozycja {
                wartosc: w,
                podgrupa: Some(
                    if tekst(r, "grupa_dominujaca").as_deref() == Some("wyposazenie") {
                        "wyposazenie".into()
                    } else {
                        "glowna".into()
                    },
                ),
                ..Default::default()
            }),
        );
    }
    for m in tablica(j, &["materialy"]) {
        let kategoria = tekst(m, "kategoria");
        for o in tablica(m, &["typowe_opisy_materialu"]) {
            dodaj(
                "material",
                tekst(o, "zapis").map(|w| NowaPozycja {
                    wartosc: w,
                    kategoria: kategoria.clone(),
                    ..Default::default()
                }),
            );
        }
        for d in tablica(m, &["dekory_kolory"]) {
            dodaj(
                "dekor",
                tekst(d, "nazwa_najpelniejsza").map(|w| NowaPozycja {
                    wartosc: w,
                    producent: tekst(d, "producent"),
                    dekor: tekst(d, "kod"),
                    kategoria: kategoria.clone(),
                    ..Default::default()
                }),
            );
        }
    }
    for p in tablica(j, &["okucia_wyposazenie", "wg_producenta"]) {
        let producent = tekst(p, "producent");
        for s in tablica(p, &["systemy"]) {
            for z in tablica(s, &["przykladowe_zapisy"]) {
                dodaj(
                    "okucie",
                    z.as_str()
                        .map(oczysc_wartosc)
                        .filter(|s| !s.is_empty())
                        .map(|w| NowaPozycja {
                            wartosc: w,
                            producent: producent.clone(),
                            podgrupa: Some("wyposazenie".into()),
                            ..Default::default()
                        }),
                );
            }
        }
    }
    for b in tablica(j, &["okucia_wyposazenie", "bez_producenta"])
        .iter()
        .chain(tablica(j, &["okucia_wyposazenie", "uchwyty_zapisy"]))
    {
        dodaj(
            "okucie",
            tekst(b, "zapis").map(|w| NowaPozycja {
                wartosc: w,
                podgrupa: Some("wyposazenie".into()),
                ..Default::default()
            }),
        );
    }
    for z in tablica(j, &["oswietlenie_wlaczniki", "zapisy"]) {
        dodaj(
            "oswietlenie",
            tekst(z, "wartosc").map(|w| NowaPozycja {
                wartosc: w,
                podgrupa: Some("wyposazenie".into()),
                ..Default::default()
            }),
        );
    }
    for o in tablica(j, &["opis_i_statusy", "typowe_wartosci_opisu"]) {
        dodaj("opis", poz(tekst(o, "wzorzec")));
    }
    if let Some(agd) = j.get("sprzet_agd") {
        for (klucz, kod) in [("rodzaje", "agd"), ("marki", "agd_marka")] {
            if let Some(m) = agd.get(klucz).and_then(Value::as_object) {
                for k in m.keys() {
                    dodaj(kod, poz(Some(oczysc_wartosc(k)).filter(|s| !s.is_empty())));
                }
            }
        }
    }
    Ok(wynik)
}

/// Największa liczba pozycji w jednym pliku słowników.
pub const MAKS_POZYCJI_IMPORTU: usize = 20_000;
/// Największa liczba szablonów w jednym pliku.
pub const MAKS_SZABLONOW_IMPORTU: usize = 500;
/// Limit rozmiaru treści żądań importu (bajty).
pub const MAKS_PLIK_IMPORTU: usize = 8 * 1024 * 1024;

fn klucz_pozycji(wartosc: &str, producent: Option<&str>, dekor: Option<&str>) -> String {
    format!(
        "{}\u{1f}{}\u{1f}{}",
        wartosc.to_lowercase(),
        producent.unwrap_or_default(),
        dekor.unwrap_or_default()
    )
}

/// Import słowników z pliku użytkownika (JSON jako wartość).
///
/// Istniejące klucze (wartość bez wielkości liter + producent + dekor) i kategorie są
/// wczytywane raz do pamięci, a nowe pozycje wstawiane partiami – import tysięcy pozycji
/// trwa ułamek sekundy i nie blokuje zapisu z innych stanowisk.
pub async fn importuj_slowniki(pula: &SqlitePool, j: &Value) -> Wynik<RaportImportu> {
    use std::collections::{HashMap, HashSet};
    let pozycje = pozycje_z_pliku(j)?;
    if pozycje.len() > MAKS_POZYCJI_IMPORTU {
        return Err(BladApi::NieprawidloweDane(format!(
            "plik zawiera {} pozycji – w jednym imporcie najwyżej {MAKS_POZYCJI_IMPORTU}; podziel plik",
            pozycje.len()
        )));
    }
    let mut raport = RaportImportu::default();
    let mut tx = crate::baza::transakcja_zapisu(pula).await?;
    let znane: HashSet<String> = sqlx::query_scalar("SELECT kod FROM slowniki")
        .fetch_all(&mut *tx)
        .await?
        .into_iter()
        .collect();
    let istniejace: Vec<(String, String, Option<String>, Option<String>)> =
        sqlx::query_as("SELECT slownik_kod, wartosc, producent, dekor FROM pozycje_slownika")
            .fetch_all(&mut *tx)
            .await?;
    let mut klucze: HashSet<(String, String)> = istniejace
        .iter()
        .map(|(k, w, p, d)| (k.clone(), klucz_pozycji(w, p.as_deref(), d.as_deref())))
        .collect();
    let mut kategorie: HashMap<String, i64> =
        sqlx::query_as::<_, (i64, String)>("SELECT id, nazwa FROM kategorie_materialow")
            .fetch_all(&mut *tx)
            .await?
            .into_iter()
            .map(|(id, n)| (crate::nazwy::klucz(&n), id))
            .collect();
    let mut kolejnosc: HashMap<String, i64> = sqlx::query_as::<_, (String, i64)>(
        "SELECT slownik_kod, COALESCE(MAX(kolejnosc), 0) FROM pozycje_slownika GROUP BY slownik_kod",
    )
    .fetch_all(&mut *tx)
    .await?
    .into_iter()
    .collect();

    struct DoWstawienia {
        kod: String,
        p: NowaPozycja,
        kategoria: Option<i64>,
        kolejnosc: i64,
        podgrupa: Option<String>,
    }
    let mut nowe: Vec<DoWstawienia> = Vec::new();
    let mut nieznane: Vec<String> = Vec::new();
    for (kod, p) in pozycje {
        if !znane.contains(&kod) {
            raport.pominiete += 1;
            if !nieznane.contains(&kod) {
                nieznane.push(kod.clone());
            }
            continue;
        }
        if !klucze.insert((
            kod.clone(),
            klucz_pozycji(&p.wartosc, p.producent.as_deref(), p.dekor.as_deref()),
        )) {
            raport.pominiete += 1;
            continue;
        }
        let kat = match &p.kategoria {
            Some(k) => Some(match kategorie.get(&crate::nazwy::klucz(k)) {
                Some(id) => *id,
                None => {
                    let id = sqlx::query("INSERT INTO kategorie_materialow (nazwa) VALUES (?1)")
                        .bind(crate::nazwy::oczysc(k))
                        .execute(&mut *tx)
                        .await?
                        .last_insert_rowid();
                    raport.nowe_kategorie.push(k.clone());
                    kategorie.insert(crate::nazwy::klucz(k), id);
                    id
                }
            }),
            None => None,
        };
        let k = kolejnosc.entry(kod.clone()).or_insert(0);
        *k += 1;
        let podgrupa = p
            .podgrupa
            .clone()
            .filter(|g| specyfikacje::PODGRUPY.contains(&g.as_str()));
        *raport.dodane.entry(kod.clone()).or_default() += 1;
        nowe.push(DoWstawienia {
            kolejnosc: *k,
            kod,
            p,
            kategoria: kat,
            podgrupa,
        });
    }
    for kod in nieznane {
        raport
            .ostrzezenia
            .push(format!("nieznany słownik „{kod}” – pominięto"));
    }
    // Partiami po 100 wierszy (8 parametrów na wiersz, limit SQLite 32 766).
    for partia in nowe.chunks(100) {
        let mut sql = String::from(
            "INSERT INTO pozycje_slownika (slownik_kod, wartosc, producent, dekor, kategoria_materialu_id, kolejnosc, podgrupa, zrodlo) VALUES ",
        );
        for i in 0..partia.len() {
            if i > 0 {
                sql.push(',');
            }
            sql.push_str("(?, ?, ?, ?, ?, ?, ?, 'import')");
        }
        let mut q = sqlx::query(&sql);
        for n in partia {
            q = q
                .bind(&n.kod)
                .bind(&n.p.wartosc)
                .bind(&n.p.producent)
                .bind(&n.p.dekor)
                .bind(n.kategoria)
                .bind(n.kolejnosc)
                .bind(&n.podgrupa);
        }
        q.execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(raport)
}

/// Kod szablonu z nazwy: małe litery bez polskich znaków, separator „-”.
pub fn kod_z_nazwy(nazwa: &str) -> String {
    let mut wynik = String::new();
    for ch in nazwa.to_lowercase().chars() {
        let c = match ch {
            'ą' => 'a',
            'ć' => 'c',
            'ę' => 'e',
            'ł' => 'l',
            'ń' => 'n',
            'ó' => 'o',
            'ś' => 's',
            'ź' | 'ż' => 'z',
            c if c.is_ascii_alphanumeric() => c,
            _ => '-',
        };
        if c == '-' && (wynik.is_empty() || wynik.ends_with('-')) {
            continue;
        }
        wynik.push(c);
    }
    wynik.trim_end_matches('-').to_string()
}

fn wielka_litera(s: &str) -> String {
    let mut z = s.chars();
    match z.next() {
        Some(p) => p.to_uppercase().collect::<String>() + z.as_str(),
        None => String::new(),
    }
}

/// Minimalny udział sekcji, od którego wiersz z pliku analitycznego trafia do szablonu.
pub const PROG_UDZIALU: f64 = 0.3;

/// Szablon z elementu pliku: format wewnętrzny albo analityczny (typ_zabudowy, typowe_czesci…).
fn szablon_z_pliku(v: &Value) -> Option<(String, String, String, DaneSzablonu)> {
    if let Some(nazwa) = tekst(v, "nazwa") {
        // format wewnętrzny
        let kod = tekst(v, "kod").unwrap_or_else(|| kod_z_nazwy(&nazwa));
        let tytul = tekst(v, "tytul_sekcji").unwrap_or_else(|| nazwa.clone());
        let dane = DaneSzablonu {
            czesci: tablica(v, &["czesci"])
                .iter()
                .filter_map(|c| c.as_str().map(oczysc_wartosc))
                .filter(|c| !c.is_empty())
                .collect(),
            // Poprawność wierszy sprawdza wcześniej `sprawdz_szablon` – tu już się nie gubią.
            wiersze: tablica(v, &["wiersze"])
                .iter()
                .filter_map(|w| serde_json::from_value(w.clone()).ok())
                .collect(),
        };
        return Some((kod, nazwa, tytul, dane));
    }
    let typ = tekst(v, "typ_zabudowy")?;
    let nazwa = wielka_litera(&typ);
    let tytul = tablica(v, &["przykladowe_tytuly_sekcji"])
        .iter()
        .find_map(|t| t.as_str().map(oczysc_wartosc).filter(|t| !t.is_empty()))
        .unwrap_or_else(|| nazwa.clone());
    let czesci = tablica(v, &["typowe_czesci"])
        .iter()
        .filter_map(|c| tekst(c, "czesc"))
        .take(12)
        .collect();
    let udzial = |w: &Value| w.get("udzial_sekcji").and_then(Value::as_f64);
    let mut wiersze = Vec::new();
    for w in tablica(v, &["wiersze_materialowe"]) {
        if udzial(w).unwrap_or(1.0) < PROG_UDZIALU {
            continue;
        }
        let rodzaj = tekst(w, "rodzaj");
        // Wiersze dziedziczące materiał korpusu zostawiamy puste – wypełni je korpus.
        let material = if dziedziczy_material_korpusu(rodzaj.as_deref()) {
            None
        } else {
            tekst(w, "najczestszy_material")
        };
        wiersze.push(WierszSzablonu {
            podgrupa: Some("glowna".into()),
            rodzaj,
            material,
            opis: tekst(w, "typowy_opis"),
            kategoria: tekst(w, "najczestsza_kategoria"),
            udzial: udzial(w),
        });
    }
    for w in tablica(v, &["wyposazenie"]) {
        if udzial(w).unwrap_or(1.0) < PROG_UDZIALU {
            continue;
        }
        wiersze.push(WierszSzablonu {
            podgrupa: Some("wyposazenie".into()),
            rodzaj: tekst(w, "rodzaj"),
            material: tablica(w, &["typowe_wyposazenie"])
                .iter()
                .find_map(|t| t.as_str().map(oczysc_wartosc).filter(|t| !t.is_empty())),
            opis: tekst(w, "typowy_opis"),
            kategoria: None,
            udzial: udzial(w),
        });
    }
    Some((
        kod_z_nazwy(&typ),
        nazwa,
        tytul,
        DaneSzablonu { czesci, wiersze },
    ))
}

const POLA_WIERSZA_SZABLONU: &[&str] = &[
    "podgrupa",
    "rodzaj",
    "material",
    "opis",
    "kategoria",
    "udzial",
];

/// Sprawdza jeden szablon z pliku; dopisuje błędy (pozycja, pole, powód) do `bledy`.
fn sprawdz_szablon(i: usize, v: &Value, bledy: &mut Vec<BladPozycji>) {
    let nazwa_szablonu = tekst(v, "nazwa")
        .or_else(|| tekst(v, "typ_zabudowy"))
        .unwrap_or_default();
    let poz = |dodatek: &str| {
        let n = if nazwa_szablonu.is_empty() {
            String::new()
        } else {
            format!(" („{nazwa_szablonu}”)")
        };
        format!("szablon {}{n}{dodatek}", i + 1)
    };
    let mut blad = |pozycja: String, pole: &str, powod: String| {
        bledy.push(BladPozycji {
            pozycja,
            pole: pole.into(),
            powod,
        })
    };
    let Some(obj) = v.as_object() else {
        blad(poz(""), "", "oczekiwano obiektu szablonu".into());
        return;
    };
    let tekst_lub_null =
        |w: Option<&Value>| matches!(w, None | Some(Value::Null) | Some(Value::String(_)));
    if obj.contains_key("nazwa") || !obj.contains_key("typ_zabudowy") {
        // format wewnętrzny
        match obj.get("nazwa") {
            Some(Value::String(n)) if !n.trim().is_empty() => {
                if n.chars().count() > crate::limity::MAKS_NAZWA {
                    blad(poz(""), "nazwa", "najwyżej 200 znaków".into());
                }
            }
            Some(Value::String(_)) => blad(poz(""), "nazwa", "nazwa nie może być pusta".into()),
            Some(_) => blad(poz(""), "nazwa", "oczekiwano tekstu".into()),
            None => blad(
                poz(""),
                "nazwa",
                "brak nazwy szablonu (pole „nazwa” albo „typ_zabudowy”)".into(),
            ),
        }
        for pole in ["kod", "tytul_sekcji"] {
            if !tekst_lub_null(obj.get(pole)) {
                blad(poz(""), pole, "oczekiwano tekstu".into());
            }
        }
        match obj.get("czesci") {
            None | Some(Value::Null) => {}
            Some(Value::Array(c)) => {
                for (k, x) in c.iter().enumerate() {
                    if !x.is_string() {
                        blad(
                            poz(&format!(", część {}", k + 1)),
                            "czesci",
                            "oczekiwano tekstu (nazwy części)".into(),
                        );
                    }
                }
            }
            Some(_) => blad(poz(""), "czesci", "oczekiwano listy nazw części".into()),
        }
        match obj.get("wiersze") {
            None | Some(Value::Null) => {}
            Some(Value::Array(w)) => {
                for (k, x) in w.iter().enumerate() {
                    let pw = poz(&format!(", wiersz {}", k + 1));
                    let Some(o) = x.as_object() else {
                        blad(pw, "", "oczekiwano obiektu wiersza".into());
                        continue;
                    };
                    for klucz in o.keys() {
                        if !POLA_WIERSZA_SZABLONU.contains(&klucz.as_str()) {
                            blad(
                                pw.clone(),
                                klucz,
                                format!(
                                    "nieznane pole (dozwolone: {})",
                                    POLA_WIERSZA_SZABLONU.join(", ")
                                ),
                            );
                        }
                    }
                    for pole in ["rodzaj", "material", "opis", "kategoria"] {
                        match o.get(pole) {
                            None | Some(Value::Null) => {}
                            Some(Value::String(t))
                                if t.chars().count() > crate::limity::MAKS_TEKST =>
                            {
                                blad(
                                    pw.clone(),
                                    pole,
                                    "za długi tekst (najwyżej 1000 znaków)".into(),
                                )
                            }
                            Some(Value::String(_)) => {}
                            Some(_) => blad(pw.clone(), pole, "oczekiwano tekstu".into()),
                        }
                    }
                    match o.get("podgrupa") {
                        None | Some(Value::Null) => {}
                        Some(Value::String(g)) if specyfikacje::PODGRUPY.contains(&g.as_str()) => {}
                        Some(Value::String(g)) => blad(
                            pw.clone(),
                            "podgrupa",
                            format!(
                                "nieznana podgrupa „{g}” (dozwolone: {})",
                                specyfikacje::PODGRUPY.join(", ")
                            ),
                        ),
                        Some(_) => blad(pw.clone(), "podgrupa", "oczekiwano tekstu".into()),
                    }
                    if !matches!(
                        o.get("udzial"),
                        None | Some(Value::Null) | Some(Value::Number(_))
                    ) {
                        blad(pw.clone(), "udzial", "oczekiwano liczby od 0 do 1".into());
                    }
                }
            }
            Some(_) => blad(poz(""), "wiersze", "oczekiwano listy wierszy".into()),
        }
    } else {
        // format analityczny
        if !matches!(obj.get("typ_zabudowy"), Some(Value::String(t)) if !t.trim().is_empty()) {
            blad(
                poz(""),
                "typ_zabudowy",
                "oczekiwano niepustego tekstu".into(),
            );
        }
        for (klucz, nazwa_listy) in [
            ("wiersze_materialowe", "wiersz"),
            ("wyposazenie", "wyposażenie"),
        ] {
            match obj.get(klucz) {
                None | Some(Value::Null) => {}
                Some(Value::Array(w)) => {
                    for (k, x) in w.iter().enumerate() {
                        let pw = poz(&format!(", {nazwa_listy} {}", k + 1));
                        let Some(o) = x.as_object() else {
                            blad(pw, klucz, "oczekiwano obiektu".into());
                            continue;
                        };
                        for pole in [
                            "rodzaj",
                            "najczestszy_material",
                            "typowy_opis",
                            "najczestsza_kategoria",
                        ] {
                            if !tekst_lub_null(o.get(pole)) {
                                blad(pw.clone(), pole, "oczekiwano tekstu".into());
                            }
                        }
                        if !matches!(
                            o.get("udzial_sekcji"),
                            None | Some(Value::Null) | Some(Value::Number(_))
                        ) {
                            blad(
                                pw.clone(),
                                "udzial_sekcji",
                                "oczekiwano liczby od 0 do 1".into(),
                            );
                        }
                    }
                }
                Some(_) => blad(poz(""), klucz, "oczekiwano listy".into()),
            }
        }
        if !matches!(
            obj.get("typowe_czesci"),
            None | Some(Value::Null) | Some(Value::Array(_))
        ) {
            blad(poz(""), "typowe_czesci", "oczekiwano listy".into());
        }
    }
}

/// Import szablonów mebli; szablon o tym samym kodzie jest nadpisywany.
///
/// Decyzja: plik jest sprawdzany w całości PRZED zapisem i przy jakimkolwiek błędzie
/// odrzucany (422 z listą: pozycja, pole, powód). Częściowy import szablonów byłby mylący –
/// szablon z pominiętymi wierszami nadpisałby wbudowany o tym samym kodzie i „zgubił”
/// elementy bez widocznego śladu. Pominięcia wynikające z zasad formatu (udział < 0,3) są
/// opisane w raporcie (`ostrzezenia`).
pub async fn importuj_szablony(pula: &SqlitePool, j: &Value) -> Wynik<RaportImportu> {
    let lista = j
        .get("szablony")
        .and_then(Value::as_array)
        .or_else(|| j.as_array())
        .ok_or_else(|| {
            BladApi::NieprawidloweDane("plik szablonów musi zawierać listę „szablony”".into())
        })?;
    if lista.len() > MAKS_SZABLONOW_IMPORTU {
        return Err(BladApi::NieprawidloweDane(format!(
            "plik zawiera {} szablonów – najwyżej {MAKS_SZABLONOW_IMPORTU}",
            lista.len()
        )));
    }
    let mut bledy = Vec::new();
    for (i, v) in lista.iter().enumerate() {
        sprawdz_szablon(i, v, &mut bledy);
    }
    let mut odczytane = Vec::new();
    let mut kody: Vec<String> = Vec::new();
    let mut raport = RaportImportu::default();
    if bledy.is_empty() {
        for (i, v) in lista.iter().enumerate() {
            match szablon_z_pliku(v) {
                Some((kod, nazwa, tytul, dane)) if !kod.is_empty() => {
                    if kody.contains(&kod) {
                        bledy.push(BladPozycji {
                            pozycja: format!("szablon {} („{nazwa}”)", i + 1),
                            pole: "kod".into(),
                            powod: format!("kod „{kod}” powtarza się w pliku"),
                        });
                    }
                    let pominiete = tablica(v, &["wiersze_materialowe"])
                        .iter()
                        .chain(tablica(v, &["wyposazenie"]))
                        .filter(|w| {
                            w.get("udzial_sekcji")
                                .and_then(Value::as_f64)
                                .is_some_and(|u| u < PROG_UDZIALU)
                        })
                        .count();
                    if pominiete > 0 {
                        raport.ostrzezenia.push(format!(
                            "szablon „{nazwa}”: pominięto {pominiete} rzadkich wierszy (udział w sekcjach < 30%)"
                        ));
                    }
                    kody.push(kod.clone());
                    odczytane.push((i, kod, nazwa, tytul, dane));
                }
                _ => bledy.push(BladPozycji {
                    pozycja: format!("szablon {}", i + 1),
                    pole: "nazwa".into(),
                    powod: "nie da się ustalić kodu szablonu z nazwy".into(),
                }),
            }
        }
    }
    if !bledy.is_empty() {
        let n = bledy.len();
        bledy.truncate(200);
        return Err(BladApi::BledyPozycji {
            komunikat: format!(
                "Plik szablonów zawiera błędy ({n}) – nic nie zaimportowano. Popraw plik i spróbuj ponownie."
            ),
            bledy,
        });
    }
    let mut tx = crate::baza::transakcja_zapisu(pula).await?;
    for (i, kod, nazwa, tytul, dane) in odczytane {
        let json = serde_json::to_string(&dane).unwrap_or_else(|_| "{}".into());
        let istnieje: Option<i64> =
            sqlx::query_scalar("SELECT id FROM szablony_mebli WHERE kod = ?1")
                .bind(&kod)
                .fetch_optional(&mut *tx)
                .await?;
        match istnieje {
            Some(id) => {
                sqlx::query(
                    "UPDATE szablony_mebli SET nazwa = ?1, tytul_sekcji = ?2, dane = ?3, zrodlo = 'import', aktywny = 1 WHERE id = ?4",
                )
                .bind(&nazwa)
                .bind(&tytul)
                .bind(&json)
                .bind(id)
                .execute(&mut *tx)
                .await?;
                raport.zaktualizowane += 1;
            }
            None => {
                sqlx::query(
                    "INSERT INTO szablony_mebli (kod, nazwa, tytul_sekcji, dane, zrodlo, kolejnosc) \
                     VALUES (?1, ?2, ?3, ?4, 'import', 100 + ?5)",
                )
                .bind(&kod)
                .bind(&nazwa)
                .bind(&tytul)
                .bind(&json)
                .bind(i as i64)
                .execute(&mut *tx)
                .await?;
                *raport.dodane.entry("szablony".into()).or_default() += 1;
            }
        }
    }
    tx.commit().await?;
    Ok(raport)
}

#[cfg(test)]
mod testy {
    use super::*;
    use crate::baza::otworz_w_pamieci;

    const SLOWNIKI: &str = include_str!("../tests/przyklady/slowniki-przyklad.json");
    const SZABLONY: &str = include_str!("../tests/przyklady/szablony-mebli-przyklad.json");

    #[test]
    fn kody_szablonow() {
        assert_eq!(kod_z_nazwy("łazienka / WC (stelaż)"), "lazienka-wc-stelaz");
        assert_eq!(kod_z_nazwy("  Szafa / wnęka "), "szafa-wneka");
    }

    #[tokio::test]
    async fn import_slownikow_bez_duplikatow() {
        let pula = otworz_w_pamieci().await.unwrap();
        let j: Value = serde_json::from_str(SLOWNIKI).unwrap();
        let r = importuj_slowniki(&pula, &j).await.unwrap();
        assert_eq!(r.dodane.get("dekor"), Some(&2));
        assert_eq!(r.dodane.get("material"), Some(&1)); // druga pozycja już jest w migracji
        assert!(r.dodane.get("okucie").copied().unwrap_or(0) >= 3);
        assert_eq!(r.dodane.get("agd_marka"), Some(&2));
        assert_eq!(
            r.nowe_kategorie,
            vec!["Płyta testowa laminowana".to_string()]
        );
        // Rodzaje wyposażeniowe dostają podgrupę „wyposazenie”.
        let pod: Option<String> = sqlx::query_scalar(
            "SELECT podgrupa FROM pozycje_slownika WHERE slownik_kod = 'rodzaj_elementu' AND wartosc = 'Zawiasy testowe'",
        )
        .fetch_one(&pula)
        .await
        .unwrap();
        assert_eq!(pod.as_deref(), Some("wyposazenie"));
        // Ponowny import niczego nie dodaje
        let r2 = importuj_slowniki(&pula, &j).await.unwrap();
        assert!(r2.dodane.is_empty());
        assert!(r2.pominiete > 0);
        // końcowe „|” obcięte
        let n: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pozycje_slownika WHERE slownik_kod = 'oswietlenie' AND wartosc = 'Taśma LED testowa w profilu'",
        )
        .fetch_one(&pula)
        .await
        .unwrap();
        assert_eq!(n, 1);
    }

    #[tokio::test]
    async fn import_szablonow_odrzuca_bledny_plik_w_calosci() {
        let pula = otworz_w_pamieci().await.unwrap();
        let j = serde_json::json!({ "szablony": [
            { "nazwa": "Dobry", "wiersze": [{ "rodzaj": "Korpus" }] },
            { "nazwa": "Zły", "wiersze": [
                { "rodzaj": "Korpus", "podgrupa": "nieznana" },
                "tekst zamiast wiersza",
                { "rodzaj": 5, "kolor": "x" }
            ] },
            { "czesci": ["A"] }
        ]});
        let Err(BladApi::BledyPozycji { bledy, .. }) = importuj_szablony(&pula, &j).await else {
            panic!("oczekiwano listy błędów");
        };
        let pola: Vec<(&str, &str)> = bledy
            .iter()
            .map(|b| (b.pozycja.as_str(), b.pole.as_str()))
            .collect();
        assert!(
            pola.contains(&("szablon 2 („Zły”), wiersz 1", "podgrupa")),
            "{pola:?}"
        );
        assert!(pola
            .iter()
            .any(|(p, _)| p.starts_with("szablon 2 („Zły”), wiersz 2")));
        assert!(pola.contains(&("szablon 2 („Zły”), wiersz 3", "rodzaj")));
        assert!(pola.contains(&("szablon 2 („Zły”), wiersz 3", "kolor")));
        assert!(pola.contains(&("szablon 3", "nazwa")));
        // nic nie zapisano – także poprawny szablon 1
        assert!(lista(&pula)
            .await
            .unwrap()
            .iter()
            .all(|s| s.nazwa != "Dobry"));
    }

    #[tokio::test]
    async fn import_slownikow_tysiecy_pozycji_jest_szybki() {
        let pula = otworz_w_pamieci().await.unwrap();
        let wartosci: Vec<String> = (0..4000).map(|i| format!("Dekor testowy {i}")).collect();
        let j = serde_json::json!({ "slowniki": { "dekor": wartosci } });
        let start = std::time::Instant::now();
        let r = importuj_slowniki(&pula, &j).await.unwrap();
        let czas = start.elapsed();
        assert_eq!(r.dodane.get("dekor"), Some(&4000));
        assert!(
            czas.as_secs_f64() < 3.0,
            "import 4000 pozycji trwał {czas:?}"
        );
        let r2 = importuj_slowniki(&pula, &j).await.unwrap();
        assert!(r2.dodane.is_empty());
        assert_eq!(r2.pominiete, 4000);
        let za_duzo: Vec<String> = (0..MAKS_POZYCJI_IMPORTU + 1)
            .map(|i| format!("x{i}"))
            .collect();
        assert!(importuj_slowniki(
            &pula,
            &serde_json::json!({ "slowniki": { "dekor": za_duzo } })
        )
        .await
        .is_err());
    }

    #[tokio::test]
    async fn import_szablonow_i_wstawienie() {
        let pula = otworz_w_pamieci().await.unwrap();
        let j: Value = serde_json::from_str(SZABLONY).unwrap();
        let r = importuj_szablony(&pula, &j).await.unwrap();
        assert_eq!(r.dodane.get("szablony"), Some(&1));
        assert_eq!(r.zaktualizowane, 1); // „kuchnia” nadpisuje wbudowany
        let lista = lista(&pula).await.unwrap();
        let k = lista.iter().find(|s| s.kod == "kuchnia").unwrap();
        assert_eq!(k.tytul_sekcji, "Zabudowa Kuchni Testowej");
        // wiersz o udziale 0,1 pominięty; półki bez materiału (dziedziczą z korpusu)
        assert!(!k
            .wiersze
            .iter()
            .any(|w| w.rodzaj.as_deref() == Some("Rzadki element")));
        let polki = k
            .wiersze
            .iter()
            .find(|w| w.rodzaj.as_deref() == Some("Półki"))
            .unwrap();
        assert_eq!(polki.material, None);

        let spec = {
            let mut db = pula.acquire().await.unwrap();
            specyfikacje::utworz_w(&mut db).await.unwrap()
        };
        let w = wstaw_sekcje(
            &pula,
            spec,
            WstawSekcje {
                szablon_id: k.id,
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let s = &w.specyfikacja.sekcje[0];
        assert_eq!(s.nazwa, "Zabudowa Kuchni Testowej");
        assert_eq!(s.czesci[0].nazwa, "Szafki dolne");
        let ws = &s.czesci[0].wiersze;
        let korpus = ws
            .iter()
            .find(|w| w.rodzaj.as_deref() == Some("Korpus"))
            .unwrap();
        let polki = ws
            .iter()
            .find(|w| w.rodzaj.as_deref() == Some("Półki"))
            .unwrap();
        assert_eq!(polki.material, korpus.material);
        assert!(polki.material_z_korpusu);
        assert!(ws.iter().any(|w| w.podgrupa == "wyposazenie"));

        let w = wstaw_czesc(
            &pula,
            s.id,
            WstawCzesc {
                szablon_id: k.id,
                nazwa: Some("Wyspa".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(w.specyfikacja.sekcje[0].czesci[1].nazwa, "Wyspa");
    }
}
