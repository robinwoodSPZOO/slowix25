//! Szablony mebli (typ zabudowy → typowe części i wiersze) oraz import słowników i szablonów
//! z plików użytkownika (`slowniki.json`, `szablony-mebli.json`).
//!
//! Formaty importu opisuje docs/ARCHITEKTURA.md (rozdz. „Import słowników i szablonów”).
//! Import jest addytywny: istniejące pozycje nie są usuwane ani dublowane.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{FromRow, SqliteConnection, SqlitePool};

use crate::bledy::{BladApi, Wynik};
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
    let mut tx = pula.begin().await?;
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
    let mut tx = pula.begin().await?;
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

async fn kategoria_id(
    db: &mut SqliteConnection,
    nazwa: &str,
    raport: &mut RaportImportu,
) -> Wynik<i64> {
    let id: Option<i64> =
        sqlx::query_scalar("SELECT id FROM kategorie_materialow WHERE lower(nazwa) = lower(?1)")
            .bind(nazwa)
            .fetch_optional(&mut *db)
            .await?;
    if let Some(id) = id {
        return Ok(id);
    }
    raport.nowe_kategorie.push(nazwa.to_string());
    Ok(
        sqlx::query("INSERT INTO kategorie_materialow (nazwa) VALUES (?1)")
            .bind(nazwa)
            .execute(&mut *db)
            .await?
            .last_insert_rowid(),
    )
}

/// Import słowników z pliku użytkownika (JSON jako wartość).
pub async fn importuj_slowniki(pula: &SqlitePool, j: &Value) -> Wynik<RaportImportu> {
    let pozycje = pozycje_z_pliku(j)?;
    let mut raport = RaportImportu::default();
    let mut tx = pula.begin().await?;
    let znane: Vec<String> = sqlx::query_scalar("SELECT kod FROM slowniki")
        .fetch_all(&mut *tx)
        .await?;
    let mut kolejnosc: BTreeMap<String, i64> = BTreeMap::new();
    for (kod, p) in pozycje {
        if !znane.contains(&kod) {
            raport.pominiete += 1;
            if !raport.ostrzezenia.iter().any(|o| o.contains(&kod)) {
                raport
                    .ostrzezenia
                    .push(format!("nieznany słownik „{kod}” – pominięto"));
            }
            continue;
        }
        let podgrupa = p
            .podgrupa
            .filter(|g| specyfikacje::PODGRUPY.contains(&g.as_str()));
        let istnieje: Option<i64> = sqlx::query_scalar(
            "SELECT id FROM pozycje_slownika WHERE slownik_kod = ?1 AND lower(wartosc) = lower(?2) \
             AND COALESCE(producent, '') = COALESCE(?3, '') AND COALESCE(dekor, '') = COALESCE(?4, '')",
        )
        .bind(&kod)
        .bind(&p.wartosc)
        .bind(&p.producent)
        .bind(&p.dekor)
        .fetch_optional(&mut *tx)
        .await?;
        if istnieje.is_some() {
            raport.pominiete += 1;
            continue;
        }
        let kat = match &p.kategoria {
            Some(k) => Some(kategoria_id(&mut tx, k, &mut raport).await?),
            None => None,
        };
        let k = match kolejnosc.get(&kod) {
            Some(k) => *k,
            None => {
                let max: Option<i64> = sqlx::query_scalar(
                    "SELECT MAX(kolejnosc) FROM pozycje_slownika WHERE slownik_kod = ?1",
                )
                .bind(&kod)
                .fetch_one(&mut *tx)
                .await?;
                max.unwrap_or(0)
            }
        } + 1;
        kolejnosc.insert(kod.clone(), k);
        sqlx::query(
            "INSERT INTO pozycje_slownika (slownik_kod, wartosc, producent, dekor, kategoria_materialu_id, kolejnosc, podgrupa, zrodlo) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'import')",
        )
        .bind(&kod)
        .bind(&p.wartosc)
        .bind(&p.producent)
        .bind(&p.dekor)
        .bind(kat)
        .bind(k)
        .bind(podgrupa)
        .execute(&mut *tx)
        .await?;
        *raport.dodane.entry(kod).or_default() += 1;
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
            wiersze: serde_json::from_value(v.get("wiersze").cloned().unwrap_or(Value::Null))
                .unwrap_or_default(),
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

/// Import szablonów mebli; szablon o tym samym kodzie jest nadpisywany.
pub async fn importuj_szablony(pula: &SqlitePool, j: &Value) -> Wynik<RaportImportu> {
    let lista = j
        .get("szablony")
        .and_then(Value::as_array)
        .or_else(|| j.as_array())
        .ok_or_else(|| {
            BladApi::NieprawidloweDane("plik szablonów musi zawierać listę „szablony”".into())
        })?;
    let mut raport = RaportImportu::default();
    let mut tx = pula.begin().await?;
    for (i, v) in lista.iter().enumerate() {
        let Some((kod, nazwa, tytul, dane)) = szablon_z_pliku(v) else {
            raport.pominiete += 1;
            raport
                .ostrzezenia
                .push(format!("pozycja {} bez nazwy – pominięto", i + 1));
            continue;
        };
        if kod.is_empty() {
            raport.pominiete += 1;
            continue;
        }
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

    const SLOWNIKI: &str = include_str!("../tests/dane/slowniki-przyklad.json");
    const SZABLONY: &str = include_str!("../tests/dane/szablony-mebli-przyklad.json");

    #[test]
    fn kody_szablonow() {
        assert_eq!(
            kod_z_nazwy("łazienka / WC (Geberit)"),
            "lazienka-wc-geberit"
        );
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
