//! Testy HTTP poprawek Generatora Ofert po recenzji (Etap I). Każdy test odtwarza zgłoszony
//! błąd, a potem sprawdza poprawne zachowanie. Wyłącznie dane fikcyjne.

use std::collections::HashMap;
use std::io::Read;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use robinwood_serwer::{api, baza, StanApi};
use serde_json::{json, Value};
use sqlx::SqlitePool;
use tower::ServiceExt;

const KLUCZ: &str = "klucz-testowy";

struct Srodowisko {
    app: axum::Router,
    pula: SqlitePool,
    folder: std::path::PathBuf,
}

impl Drop for Srodowisko {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.folder);
    }
}

fn unikalny() -> String {
    format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    )
}

async fn srodowisko() -> Srodowisko {
    let folder = std::env::temp_dir().join(format!("robinwood-popr-{}", unikalny()));
    let pula = baza::otworz_w_pamieci().await.unwrap();
    let stan =
        StanApi::nowy(pula.clone(), KLUCZ.into(), "Test".into()).z_folderem_danych(folder.clone());
    Srodowisko {
        app: api::router(stan),
        pula,
        folder,
    }
}

/// Prawdziwy plik bazy (WAL, kilka połączeń) – do testów równoległości.
async fn srodowisko_na_pliku() -> Srodowisko {
    let folder = std::env::temp_dir().join(format!("robinwood-plik-{}", unikalny()));
    let pula = baza::otworz(&folder.join("robinwood.db")).await.unwrap();
    let stan =
        StanApi::nowy(pula.clone(), KLUCZ.into(), "Test".into()).z_folderem_danych(folder.clone());
    Srodowisko {
        app: api::router(stan),
        pula,
        folder,
    }
}

fn zadanie(metoda: &str, uri: &str, cialo: Option<Value>) -> Request<Body> {
    let b = Request::builder()
        .method(metoda)
        .uri(uri)
        .header(header::AUTHORIZATION, format!("Bearer {KLUCZ}"))
        .header(header::CONTENT_TYPE, "application/json");
    match cialo {
        Some(v) => b.body(Body::from(v.to_string())).unwrap(),
        None => b.body(Body::empty()).unwrap(),
    }
}

async fn wyslij(
    app: &axum::Router,
    metoda: &str,
    uri: &str,
    cialo: Option<Value>,
) -> (StatusCode, Value) {
    let odp = app
        .clone()
        .oneshot(zadanie(metoda, uri, cialo))
        .await
        .unwrap();
    let kod = odp.status();
    let bajty = odp.into_body().collect().await.unwrap().to_bytes();
    (kod, serde_json::from_slice(&bajty).unwrap_or(Value::Null))
}

async fn ok(app: &axum::Router, metoda: &str, uri: &str, cialo: Option<Value>) -> Value {
    let (kod, v) = wyslij(app, metoda, uri, cialo).await;
    assert!(kod.is_success(), "{metoda} {uri} → {kod}: {v}");
    v
}

async fn surowe(
    app: &axum::Router,
    r: Request<Body>,
) -> (StatusCode, HashMap<String, String>, Vec<u8>) {
    let odp = app.clone().oneshot(r).await.unwrap();
    let kod = odp.status();
    let nag = odp
        .headers()
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or_default().to_string()))
        .collect();
    let b = odp.into_body().collect().await.unwrap().to_bytes().to_vec();
    (kod, nag, b)
}

async fn zdjecie(app: &axum::Router, spec: i64, bajty: Vec<u8>) -> (StatusCode, Value) {
    let (kod, _, b) = surowe(
        app,
        Request::post(format!(
            "/api/v1/specyfikacje/{spec}/zdjecia?nazwa=test.png"
        ))
        .header(header::AUTHORIZATION, format!("Bearer {KLUCZ}"))
        .body(Body::from(bajty))
        .unwrap(),
    )
    .await;
    (kod, serde_json::from_slice(&b).unwrap_or(Value::Null))
}

fn png(w: u32, h: u32, ziarno: u8) -> Vec<u8> {
    let img = image023::RgbImage::from_fn(w, h, |x, y| {
        image023::Rgb([(x as u8).wrapping_mul(7), (y as u8).wrapping_mul(5), ziarno])
    });
    let mut b = Vec::new();
    image023::png::PngEncoder::new(&mut b)
        .encode(img.as_raw(), w, h, image023::ColorType::Rgb8)
        .unwrap();
    b
}

/// Oferta z jedną sekcją (cena sekcji), częścią i wierszem; zwraca (oferta, wersja, spec, sekcja, część, wiersz).
async fn oferta(app: &axum::Router, nazwa_klienta: &str) -> (i64, i64, i64, i64, i64, i64) {
    let k = ok(
        app,
        "POST",
        "/api/v1/klienci",
        Some(json!({ "nazwa": nazwa_klienta, "adres_miasto": "Testowo" })),
    )
    .await;
    let p = ok(
        app,
        "POST",
        "/api/v1/oferty",
        Some(json!({ "klient_id": k["id"] })),
    )
    .await;
    let o = p["oferta"]["id"].as_i64().unwrap();
    let v = p["wersja"]["id"].as_i64().unwrap();
    let spec = p["wersja"]["specyfikacja_id"].as_i64().unwrap();
    let s = ok(
        app,
        "POST",
        &format!("/api/v1/specyfikacje/{spec}/sekcje"),
        Some(json!({ "nazwa": "Kuchnia", "cena_gr": 1_000_000 })),
    )
    .await["id"]
        .as_i64()
        .unwrap();
    let c = ok(
        app,
        "POST",
        &format!("/api/v1/sekcje/{s}/czesci"),
        Some(json!({ "nazwa": "Szafki" })),
    )
    .await["id"]
        .as_i64()
        .unwrap();
    let w = ok(
        app,
        "POST",
        &format!("/api/v1/czesci/{c}/wiersze"),
        Some(json!({ "rodzaj": "Korpus", "material": "Płyta A" })),
    )
    .await["id"]
        .as_i64()
        .unwrap();
    (o, v, spec, s, c, w)
}

fn tekst_docx(b: &[u8]) -> String {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(b)).unwrap();
    let mut s = String::new();
    z.by_name("word/document.xml")
        .unwrap()
        .read_to_string(&mut s)
        .unwrap();
    // Usuwamy znaczniki XML – zostaje sam tekst (kwoty mogą być w kilku <w:t>)
    let mut wynik = String::new();
    let mut w_znaczniku = false;
    for ch in s.chars() {
        match ch {
            '<' => w_znaczniku = true,
            '>' => w_znaczniku = false,
            c if !w_znaczniku => wynik.push(c),
            _ => {}
        }
    }
    wynik.replace("&amp;", "&")
}

/// Tekst PDF z genpdf/printpdf: czcionki Type0 (Identity-H) + mapa ToUnicode (bfchar).
fn tekst_pdf(b: &[u8]) -> String {
    let dok = lopdf::Document::load_mem(b).unwrap();
    let mut wynik = String::new();
    for (_, strona) in dok.get_pages() {
        let mut mapy: HashMap<Vec<u8>, HashMap<u16, char>> = HashMap::new();
        let deref = |o: &lopdf::Object| -> Option<lopdf::Object> {
            match o {
                lopdf::Object::Reference(r) => dok.get_object(*r).ok().cloned(),
                inny => Some(inny.clone()),
            }
        };
        let mut fonty: Vec<(Vec<u8>, lopdf::Dictionary)> = vec![];
        if let Ok(lopdf::Object::Dictionary(st)) = dok.get_object(strona) {
            if let Some(lopdf::Object::Dictionary(zas)) = st.get(b"Resources").ok().and_then(deref)
            {
                if let Some(lopdf::Object::Dictionary(f)) = zas.get(b"Font").ok().and_then(deref) {
                    for (n, o) in f.iter() {
                        if let Some(lopdf::Object::Dictionary(d)) = deref(o) {
                            fonty.push((n.clone(), d));
                        }
                    }
                }
            }
        }
        for (nazwa, font) in fonty {
            let mut mapa = HashMap::new();
            if let Ok(lopdf::Object::Reference(r)) = font.get(b"ToUnicode") {
                if let Ok(lopdf::Object::Stream(s)) = dok.get_object(*r) {
                    let tresc = s
                        .decompressed_content()
                        .unwrap_or_else(|_| s.content.clone());
                    for linia in String::from_utf8_lossy(&tresc).lines() {
                        let hex: Vec<&str> = linia
                            .split(['<', '>'])
                            .map(str::trim)
                            .filter(|x| !x.is_empty() && x.chars().all(|c| c.is_ascii_hexdigit()))
                            .collect();
                        if hex.len() == 2 {
                            if let (Ok(g), Ok(u)) = (
                                u16::from_str_radix(hex[0], 16),
                                u32::from_str_radix(hex[1], 16),
                            ) {
                                if let Some(c) = char::from_u32(u) {
                                    mapa.insert(g, c);
                                }
                            }
                        }
                    }
                }
            }
            mapy.insert(nazwa, mapa);
        }
        // Strumień treści printpdf: linie „/F0 9.00 Tf” i „[<0091><0160>…] TJ” (parser lopdf 0.26
        // nie czyta sklejonych łańcuchów szesnastkowych, więc czytamy linie sami).
        let tresc = dok.get_page_content(strona).unwrap();
        let tresc = String::from_utf8_lossy(&tresc);
        let mut biezaca: Vec<u8> = vec![];
        for linia in tresc.lines() {
            let l = linia.trim();
            if l.ends_with(" Tf") {
                if let Some(n) = l.split_whitespace().next() {
                    biezaca = n.trim_start_matches('/').as_bytes().to_vec();
                }
            } else if l.ends_with("TJ") || l.ends_with("Tj") {
                let mapa = mapy.get(&biezaca);
                for kawalek in l.split('<').skip(1) {
                    let hex = kawalek.split('>').next().unwrap_or_default();
                    for i in (0..hex.len()).step_by(4) {
                        if let Some(g) = hex
                            .get(i..i + 4)
                            .and_then(|h| u16::from_str_radix(h, 16).ok())
                        {
                            if let Some(c) = mapa.and_then(|m| m.get(&g)) {
                                wynik.push(*c);
                            }
                        }
                    }
                }
            } else if l == "ET" || l.ends_with(" Td") || l == "T*" {
                wynik.push('\n');
            }
        }
    }
    wynik
}

// ---------------------------------------------------------------------------
// 1. Równoczesne zapisy
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn rownolegle_zapisy_bez_database_is_locked() {
    let s = srodowisko_na_pliku().await;
    let mut wiersze = vec![];
    for i in 0..4 {
        wiersze.push(oferta(&s.app, &format!("Klient Testowy {i}")).await.5);
    }
    // 4 wątki × 25 PUT /wiersze/{id} w różnych ofertach
    let mut zadania = vec![];
    for (i, w) in wiersze.into_iter().enumerate() {
        let app = s.app.clone();
        zadania.push(tokio::spawn(async move {
            let mut kody = vec![];
            for n in 0..25 {
                let (kod, _) = wyslij(
                    &app,
                    "PUT",
                    &format!("/api/v1/wiersze/{w}"),
                    Some(json!({ "opis": format!("opis {i}-{n}") })),
                )
                .await;
                kody.push(kod);
            }
            kody
        }));
    }
    // 20 równoległych POST /oferty (+ klienci, słowniki – wszystkie funkcje zapisu)
    let klient = ok(
        &s.app,
        "POST",
        "/api/v1/klienci",
        Some(json!({ "nazwa": "Klient Równoległy" })),
    )
    .await["id"]
        .clone();
    for _ in 0..20 {
        let app = s.app.clone();
        let k = klient.clone();
        zadania.push(tokio::spawn(async move {
            vec![
                wyslij(
                    &app,
                    "POST",
                    "/api/v1/oferty",
                    Some(json!({ "klient_id": k })),
                )
                .await
                .0,
            ]
        }));
    }
    for i in 0..10 {
        let app = s.app.clone();
        zadania.push(tokio::spawn(async move {
            vec![
                wyslij(
                    &app,
                    "POST",
                    "/api/v1/klienci",
                    Some(json!({ "nazwa": format!("Klient {i}") })),
                )
                .await
                .0,
                wyslij(
                    &app,
                    "POST",
                    "/api/v1/import/slowniki",
                    Some(json!({ "slowniki": { "dekor": [format!("Dekor {i}")] } })),
                )
                .await
                .0,
            ]
        }));
    }
    let mut bledy = 0;
    for z in zadania {
        bledy += z.await.unwrap().iter().filter(|k| !k.is_success()).count();
    }
    assert_eq!(bledy, 0, "równoczesne zapisy zakończone błędem");
    let numery: Vec<String> = sqlx::query_scalar("SELECT numer FROM oferty")
        .fetch_all(&s.pula)
        .await
        .unwrap();
    let unikalne: std::collections::HashSet<_> = numery.iter().collect();
    assert_eq!(numery.len(), 24);
    assert_eq!(unikalne.len(), 24, "numery ofert muszą być unikalne");
    s.pula.close().await;
}

// ---------------------------------------------------------------------------
// 2. Przepełnienia i limity kwot
// ---------------------------------------------------------------------------

#[tokio::test]
async fn przepelnienia_walidacja_i_lista_odporna_na_zla_oferte() {
    let s = srodowisko().await;
    let (_, v, spec, sekcja, _, _) = oferta(&s.app, "Klient Testowy").await;
    let (kod, b) = wyslij(
        &s.app,
        "PUT",
        &format!("/api/v1/sekcje/{sekcja}"),
        Some(json!({ "cena_gr": 1_i64 << 62 })),
    )
    .await;
    assert_eq!(kod, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(
        b["blad"].as_str().unwrap().contains("1 000 000 000,00 zł"),
        "{b}"
    );
    let agd = ok(
        &s.app,
        "POST",
        &format!("/api/v1/specyfikacje/{spec}/sekcje"),
        Some(json!({ "nazwa": "AGD", "typ": "agd" })),
    )
    .await["id"]
        .as_i64()
        .unwrap();
    let c = ok(
        &s.app,
        "POST",
        &format!("/api/v1/sekcje/{agd}/czesci"),
        Some(json!({ "nazwa": "Sprzęt" })),
    )
    .await["id"]
        .as_i64()
        .unwrap();
    let (kod, b) = wyslij(
        &s.app,
        "POST",
        &format!("/api/v1/czesci/{c}/wiersze"),
        Some(json!({ "rodzaj": "Piekarnik", "cena_gr": 300_000, "ilosc": 1e30 })),
    )
    .await;
    assert_eq!(kod, StatusCode::UNPROCESSABLE_ENTITY, "{b}");
    let (kod, _) = wyslij(
        &s.app,
        "POST",
        &format!("/api/v1/czesci/{c}/wiersze"),
        Some(json!({ "rodzaj": "Zlew", "cena_gr": 1_000, "ilosc": 1e17 })),
    )
    .await;
    assert_eq!(kod, StatusCode::UNPROCESSABLE_ENTITY);
    let (kod, _) = wyslij(
        &s.app,
        "PUT",
        &format!("/api/v1/wersje-oferty/{v}"),
        Some(json!({ "rabat_termin_gr": 100_000_000_001_i64 })),
    )
    .await;
    assert_eq!(kod, StatusCode::UNPROCESSABLE_ENTITY);

    // Dane uszkodzone „z zewnątrz” (np. sprzed walidacji): dwie sekcje po 2^62 gr.
    let (_, _, _, s1, _, _) = oferta(&s.app, "Klient Uszkodzony").await;
    sqlx::query("UPDATE sekcje_specyfikacji SET cena_gr = ?1 WHERE id = ?2")
        .bind(1_i64 << 62)
        .bind(s1)
        .execute(&s.pula)
        .await
        .unwrap();
    let spec_s1: i64 =
        sqlx::query_scalar("SELECT specyfikacja_id FROM sekcje_specyfikacji WHERE id = ?1")
            .bind(s1)
            .fetch_one(&s.pula)
            .await
            .unwrap();
    sqlx::query("INSERT INTO sekcje_specyfikacji (specyfikacja_id, nazwa, cena_gr) VALUES (?1, 'Druga', ?2)").bind(spec_s1).bind(1_i64 << 62).execute(&s.pula).await.unwrap();
    let lista = ok(&s.app, "GET", "/api/v1/oferty", None).await;
    let lista = lista.as_array().unwrap();
    assert_eq!(lista.len(), 2, "lista nie może paść przez jedną ofertę");
    let zla = lista
        .iter()
        .find(|o| o["klient_nazwa"] == "Klient Uszkodzony")
        .unwrap();
    assert!(zla["suma_brutto_gr"].is_null());
    assert!(zla["blad_wyceny"]
        .as_str()
        .unwrap()
        .contains("poza zakresem"));
    let dobra = lista
        .iter()
        .find(|o| o["klient_nazwa"] == "Klient Testowy")
        .unwrap();
    assert_eq!(dobra["suma_brutto_gr"], 1_000_000);
    let wz = zla["wersja_id"].as_i64().unwrap();
    let pw = ok(&s.app, "GET", &format!("/api/v1/wersje-oferty/{wz}"), None).await;
    assert!(!pw["wycena"]["bledy"].as_array().unwrap().is_empty());
    let (kod, _) = wyslij(
        &s.app,
        "GET",
        &format!("/api/v1/wersje-oferty/{wz}/eksport/pdf"),
        None,
    )
    .await;
    assert_eq!(kod, StatusCode::UNPROCESSABLE_ENTITY);
}

// ---------------------------------------------------------------------------
// 4. Zmiany częściowe i konflikty dwóch stanowisk
// ---------------------------------------------------------------------------

#[tokio::test]
async fn dwa_stanowiska_zmiany_czesciowe_i_konflikt_pola() {
    let s = srodowisko().await;
    let (_, v, _, sekcja, czesc, w) = oferta(&s.app, "Klient Testowy").await;
    // Stanowisko A zmienia materiał, B – opis (różne pola): obie zmiany zostają.
    ok(
        &s.app,
        "PUT",
        &format!("/api/v1/wiersze/{w}"),
        Some(json!({ "material": "Płyta B", "poprzednie": { "material": "Płyta A" } })),
    )
    .await;
    let r = ok(
        &s.app,
        "PUT",
        &format!("/api/v1/wiersze/{w}"),
        Some(json!({ "opis": "Biały mat", "poprzednie": { "opis": null } })),
    )
    .await;
    let wiersz = &r["specyfikacja"]["sekcje"][0]["czesci"][0]["wiersze"][0];
    assert_eq!(
        wiersz["material"], "Płyta B",
        "cudza zmiana innego pola nie może zostać nadpisana"
    );
    assert_eq!(wiersz["opis"], "Biały mat");
    assert_eq!(wiersz["wersja"], 3);
    // B zmienia materiał, widząc jeszcze starą wartość → 409 z konfliktem
    let (kod, b) = wyslij(
        &s.app,
        "PUT",
        &format!("/api/v1/wiersze/{w}"),
        Some(json!({ "material": "Płyta C", "poprzednie": { "material": "Płyta A" } })),
    )
    .await;
    assert_eq!(kod, StatusCode::CONFLICT, "{b}");
    assert_eq!(b["kod"], "konflikt");
    assert_eq!(b["konflikty"][0]["pole"], "material");
    assert_eq!(b["konflikty"][0]["aktualna"], "Płyta B");
    assert_eq!(b["konflikty"][0]["twoja"], "Płyta C");
    assert!(b["blad"].as_str().unwrap().contains("Materiał"));
    // „moja” – ponowny zapis z aktualną wartością jako poprzednią
    let r = ok(
        &s.app,
        "PUT",
        &format!("/api/v1/wiersze/{w}"),
        Some(json!({ "material": "Płyta C", "poprzednie": { "material": "Płyta B" } })),
    )
    .await;
    assert_eq!(
        r["specyfikacja"]["sekcje"][0]["czesci"][0]["wiersze"][0]["material"],
        "Płyta C"
    );
    // ta sama wartość u obu – brak konfliktu
    ok(
        &s.app,
        "PUT",
        &format!("/api/v1/wiersze/{w}"),
        Some(json!({ "material": "Płyta C", "poprzednie": { "material": "Płyta A" } })),
    )
    .await;
    // nieznane pole
    let (kod, _) = wyslij(
        &s.app,
        "PUT",
        &format!("/api/v1/wiersze/{w}"),
        Some(json!({ "kolor": "x" })),
    )
    .await;
    assert_eq!(kod, StatusCode::UNPROCESSABLE_ENTITY);

    // sekcja, część, parametry wersji
    ok(
        &s.app,
        "PUT",
        &format!("/api/v1/sekcje/{sekcja}"),
        Some(json!({ "cena_gr": 2_000_000, "poprzednie": { "cena_gr": 1_000_000 } })),
    )
    .await;
    let (kod, _) = wyslij(
        &s.app,
        "PUT",
        &format!("/api/v1/sekcje/{sekcja}"),
        Some(json!({ "cena_gr": 3_000_000, "poprzednie": { "cena_gr": 1_000_000 } })),
    )
    .await;
    assert_eq!(kod, StatusCode::CONFLICT);
    ok(
        &s.app,
        "PUT",
        &format!("/api/v1/czesci/{czesc}"),
        Some(json!({ "nazwa": "Szafki dolne", "poprzednie": { "nazwa": "Szafki" } })),
    )
    .await;
    let (kod, _) = wyslij(
        &s.app,
        "PUT",
        &format!("/api/v1/czesci/{czesc}"),
        Some(json!({ "nazwa": "Inne", "poprzednie": { "nazwa": "Szafki" } })),
    )
    .await;
    assert_eq!(kod, StatusCode::CONFLICT);
    let p = ok(
        &s.app,
        "PUT",
        &format!("/api/v1/wersje-oferty/{v}"),
        Some(json!({ "rabat_termin_gr": 10_000, "poprzednie": { "rabat_termin_gr": 0 } })),
    )
    .await;
    assert_eq!(p["wersja"]["wersja"], 2);
    let (kod, b) = wyslij(
        &s.app,
        "PUT",
        &format!("/api/v1/wersje-oferty/{v}"),
        Some(json!({ "rabat_termin_gr": 20_000, "poprzednie": { "rabat_termin_gr": 0 } })),
    )
    .await;
    assert_eq!(kod, StatusCode::CONFLICT);
    assert_eq!(b["aktualny"]["rabat_termin_gr"], 10_000);
    // inny parametr tej samej wersji – bez konfliktu
    ok(
        &s.app,
        "PUT",
        &format!("/api/v1/wersje-oferty/{v}"),
        Some(json!({ "stawka_vat": 8, "poprzednie": { "stawka_vat": 23 } })),
    )
    .await;
}

// ---------------------------------------------------------------------------
// 6, 7. Rabaty większe niż suma, VAT sekcji AGD
// ---------------------------------------------------------------------------

#[tokio::test]
async fn rabaty_wieksze_niz_suma_blokuja_dokument() {
    let s = srodowisko().await;
    let (_, v, _, sekcja, _, _) = oferta(&s.app, "Klient Testowy").await;
    ok(
        &s.app,
        "PUT",
        &format!("/api/v1/sekcje/{sekcja}"),
        Some(json!({ "cena_gr": 10_000 })),
    )
    .await;
    let p = ok(
        &s.app,
        "PUT",
        &format!("/api/v1/wersje-oferty/{v}"),
        Some(json!({ "rabat_termin_gr": 50_000, "rabat_calosc_bp": 500 })),
    )
    .await;
    assert!(p["wycena"]["bledy"][0]
        .as_str()
        .unwrap()
        .contains("większe niż suma"));
    for f in ["pdf", "docx"] {
        let (kod, b) = wyslij(
            &s.app,
            "GET",
            &format!("/api/v1/wersje-oferty/{v}/eksport/{f}"),
            None,
        )
        .await;
        assert_eq!(kod, StatusCode::UNPROCESSABLE_ENTITY, "{f}");
        assert!(
            b["blad"]
                .as_str()
                .unwrap()
                .contains("nie można utworzyć dokumentu"),
            "{b}"
        );
    }
}

#[tokio::test]
async fn sekcja_agd_domyslnie_23_procent() {
    let s = srodowisko().await;
    let k = ok(
        &s.app,
        "POST",
        "/api/v1/klienci",
        Some(json!({ "nazwa": "Klient Testowy" })),
    )
    .await;
    let p = ok(
        &s.app,
        "POST",
        "/api/v1/oferty",
        Some(json!({ "klient_id": k["id"], "stawka_vat": 8 })),
    )
    .await;
    let spec = p["wersja"]["specyfikacja_id"].as_i64().unwrap();
    let v = p["wersja"]["id"].as_i64().unwrap();
    let r = ok(
        &s.app,
        "POST",
        &format!("/api/v1/specyfikacje/{spec}/sekcje"),
        Some(json!({ "nazwa": "AGD", "typ": "agd" })),
    )
    .await;
    assert_eq!(r["specyfikacja"]["sekcje"][0]["stawka_vat"], 23);
    let m = ok(
        &s.app,
        "POST",
        &format!("/api/v1/specyfikacje/{spec}/sekcje"),
        Some(json!({ "nazwa": "Zmywarka osobno" })),
    )
    .await["id"]
        .as_i64()
        .unwrap();
    let r = ok(
        &s.app,
        "PUT",
        &format!("/api/v1/sekcje/{m}"),
        Some(json!({ "typ": "agd" })),
    )
    .await;
    let sek = r["specyfikacja"]["sekcje"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["id"] == m)
        .unwrap()
        .clone();
    assert_eq!(sek["stawka_vat"], 23, "zmiana typu na AGD ustawia 23%");
    let c = ok(
        &s.app,
        "POST",
        &format!("/api/v1/sekcje/{m}/czesci"),
        Some(json!({ "nazwa": "Sprzęt" })),
    )
    .await["id"]
        .as_i64()
        .unwrap();
    ok(
        &s.app,
        "POST",
        &format!("/api/v1/czesci/{c}/wiersze"),
        Some(json!({ "rodzaj": "Zmywarka", "cena_gr": 123_000 })),
    )
    .await;
    let w = ok(&s.app, "GET", &format!("/api/v1/wersje-oferty/{v}"), None).await;
    assert_eq!(w["wycena"]["agd"][0]["stawka"], 23);
    assert_eq!(w["wycena"]["agd"][0]["netto_gr"], 100_000);
    // zmiana na 8% przez UI
    let r = ok(
        &s.app,
        "PUT",
        &format!("/api/v1/sekcje/{m}"),
        Some(json!({ "stawka_vat": 8 })),
    )
    .await;
    assert_eq!(
        r["specyfikacja"]["sekcje"]
            .as_array()
            .unwrap()
            .iter()
            .find(|x| x["id"] == m)
            .unwrap()["stawka_vat"],
        8
    );
}

// ---------------------------------------------------------------------------
// 8, 14. Zamrożenie wersji wysłanej, migawka stron, ważność nowej wersji
// ---------------------------------------------------------------------------

#[tokio::test]
async fn wyslana_wersja_zamrozona_z_migawka_stron() {
    let s = srodowisko().await;
    ok(&s.app, "PUT", "/api/v1/ustawienia/firma", Some(json!({ "firma_nazwa": "Stolarnia Przykładowa", "firma_adres": "ul. Testowa 1", "firma_nip": "", "firma_telefon": "", "firma_email": "", "oferta_waznosc_dni": 30, "oferta_termin_realizacji": "" }))).await;
    let (o, v, spec, sekcja, _, w) = oferta(&s.app, "Klient Pierwotny").await;
    ok(
        &s.app,
        "PUT",
        &format!("/api/v1/wersje-oferty/{v}"),
        Some(json!({ "data_waznosci_cen": "2020-01-31" })),
    )
    .await;
    ok(
        &s.app,
        "PUT",
        &format!("/api/v1/oferty/{o}/status"),
        Some(json!({ "status": "wyslana" })),
    )
    .await;
    let p = ok(&s.app, "GET", &format!("/api/v1/wersje-oferty/{v}"), None).await;
    assert_eq!(p["wersja"]["tylko_do_odczytu"], true);
    assert_eq!(
        p["wersja"]["migawka"]["inwestor"]["nazwa"],
        "Klient Pierwotny"
    );
    assert_eq!(p["specyfikacja"]["tylko_do_odczytu"], true);
    for (m, u, c) in [
        (
            "PUT",
            format!("/api/v1/wiersze/{w}"),
            json!({ "opis": "x" }),
        ),
        (
            "PUT",
            format!("/api/v1/sekcje/{sekcja}"),
            json!({ "cena_gr": 1 }),
        ),
        (
            "POST",
            format!("/api/v1/specyfikacje/{spec}/sekcje"),
            json!({ "nazwa": "Nowa" }),
        ),
        (
            "PUT",
            format!("/api/v1/wersje-oferty/{v}"),
            json!({ "rabat_termin_gr": 1 }),
        ),
        (
            "PUT",
            format!("/api/v1/oferty/{o}/status"),
            json!({ "status": "szkic" }),
        ),
    ] {
        let (kod, b) = wyslij(&s.app, m, &u, Some(c)).await;
        assert_eq!(kod, StatusCode::UNPROCESSABLE_ENTITY, "{m} {u}: {b}");
    }
    // zmiana danych klienta i firmy po wysłaniu nie zmienia dokumentu wysłanej wersji
    let k = p["oferta"]["klient_id"].as_i64().unwrap();
    ok(
        &s.app,
        "PUT",
        &format!("/api/v1/klienci/{k}"),
        Some(json!({ "nazwa": "Klient Zmieniony" })),
    )
    .await;
    ok(&s.app, "PUT", "/api/v1/ustawienia/firma", Some(json!({ "firma_nazwa": "Inna Firma", "firma_adres": "", "firma_nip": "", "firma_telefon": "", "firma_email": "", "oferta_waznosc_dni": 30, "oferta_termin_realizacji": "" }))).await;
    let (kod, _, docx) = surowe(
        &s.app,
        zadanie(
            "GET",
            &format!("/api/v1/wersje-oferty/{v}/eksport/docx"),
            None,
        ),
    )
    .await;
    assert_eq!(kod, StatusCode::OK);
    let t = tekst_docx(&docx);
    assert!(
        t.contains("Klient Pierwotny") && t.contains("Stolarnia Przykładowa"),
        "dokument wysłanej wersji ma dane z migawki"
    );
    assert!(!t.contains("Klient Zmieniony") && !t.contains("Inna Firma"));
    // nowa wersja: szkic, edytowalna, ważność od dziś (+30 dni), nie przeterminowana kopia
    let p2 = ok(
        &s.app,
        "POST",
        &format!("/api/v1/oferty/{o}/wersje"),
        Some(json!({})),
    )
    .await;
    assert_eq!(p2["oferta"]["status"], "szkic");
    assert_eq!(p2["wersja"]["tylko_do_odczytu"], false);
    let dzis = chrono::Local::now().date_naive();
    let oczek = (dzis + chrono::Days::new(30))
        .format("%Y-%m-%d")
        .to_string();
    assert_eq!(p2["wersja"]["data_waznosci_cen"], oczek);
    let w2 = p2["specyfikacja"]["sekcje"][0]["czesci"][0]["wiersze"][0]["id"]
        .as_i64()
        .unwrap();
    ok(
        &s.app,
        "PUT",
        &format!("/api/v1/wiersze/{w2}"),
        Some(json!({ "opis": "zmiana w v2" })),
    )
    .await;
    // szkic nowej wersji używa bieżących danych
    let v2 = p2["wersja"]["id"].as_i64().unwrap();
    let (_, _, docx) = surowe(
        &s.app,
        zadanie(
            "GET",
            &format!("/api/v1/wersje-oferty/{v2}/eksport/docx"),
            None,
        ),
    )
    .await;
    assert!(tekst_docx(&docx).contains("Klient Zmieniony"));
}

// ---------------------------------------------------------------------------
// 11, 12. Obrazy: limity, uszkodzone pliki, równoległe wysłania, sprzątanie
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn obrazy_limity_uszkodzone_rownolegle_i_sprzatanie() {
    let s = srodowisko_na_pliku().await;
    let (o, _, spec, _, _, _) = oferta(&s.app, "Klient Testowy").await;
    // nagłówek 15000×15000 przy pliku 1×1 – odrzucony bez dekodowania
    let mut bomba = png(1, 1, 0);
    bomba[16..20].copy_from_slice(&15_000u32.to_be_bytes());
    bomba[20..24].copy_from_slice(&15_000u32.to_be_bytes());
    let crc = {
        let mut c = 0xFFFF_FFFFu32;
        for b in &bomba[12..29] {
            c ^= u32::from(*b);
            for _ in 0..8 {
                c = if c & 1 != 0 {
                    0xEDB8_8320 ^ (c >> 1)
                } else {
                    c >> 1
                };
            }
        }
        !c
    };
    bomba[29..33].copy_from_slice(&crc.to_be_bytes());
    let (kod, b) = zdjecie(&s.app, spec, bomba).await;
    assert_eq!(kod, StatusCode::UNPROCESSABLE_ENTITY, "{b}");
    assert!(b["blad"].as_str().unwrap().contains("za duży"));
    // uszkodzony obraz z poprawnym nagłówkiem
    let mut zly = png(300, 200, 1);
    let n = zly.len();
    for x in zly.iter_mut().take(n - 12).skip(60) {
        *x = 0x33;
    }
    let (kod, b) = zdjecie(&s.app, spec, zly).await;
    assert_eq!(kod, StatusCode::UNPROCESSABLE_ENTITY, "{b}");
    // dwa równoczesne wysłania tego samego zdjęcia
    let obraz = png(2400, 1200, 9);
    let (a, b2) = tokio::join!(
        zdjecie(&s.app, spec, obraz.clone()),
        zdjecie(&s.app, spec, obraz.clone())
    );
    assert_eq!(a.0, StatusCode::CREATED, "{}", a.1);
    assert_eq!(b2.0, StatusCode::CREATED, "{}", b2.1);
    let pliki: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM pliki")
        .fetch_one(&s.pula)
        .await
        .unwrap();
    assert_eq!(pliki, 1, "ten sam plik zapisany raz");
    let plik: i64 = sqlx::query_scalar("SELECT id FROM pliki")
        .fetch_one(&s.pula)
        .await
        .unwrap();
    // podgląd JPEG ≤ 1600 px, nosniff
    let (kod, nag, b) = surowe(
        &s.app,
        zadanie("GET", &format!("/api/v1/pliki/{plik}?podglad=1"), None),
    )
    .await;
    assert_eq!(kod, StatusCode::OK);
    assert_eq!(nag["content-type"], "image/jpeg");
    assert_eq!(nag["x-content-type-options"], "nosniff");
    use image023::GenericImageView;
    assert_eq!(image023::load_from_memory(&b).unwrap().width(), 1600);
    let liczba_plikow = |f: &std::path::Path| walkdir(f);
    assert_eq!(
        liczba_plikow(&s.folder.join("pliki")),
        2,
        "oryginał + podgląd"
    );
    // usunięcie oferty → sprzątanie nieużywanych plików
    ok(&s.app, "DELETE", &format!("/api/v1/oferty/{o}"), None).await;
    for _ in 0..50 {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        if liczba_plikow(&s.folder.join("pliki")) == 0 {
            break;
        }
    }
    assert_eq!(
        liczba_plikow(&s.folder.join("pliki")),
        0,
        "pliki bez odwołań usunięte"
    );
    let pliki: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM pliki")
        .fetch_one(&s.pula)
        .await
        .unwrap();
    assert_eq!(pliki, 0);
    s.pula.close().await;
}

fn walkdir(p: &std::path::Path) -> usize {
    let Ok(r) = std::fs::read_dir(p) else {
        return 0;
    };
    r.map(|e| {
        let e = e.unwrap().path();
        if e.is_dir() {
            walkdir(&e)
        } else {
            1
        }
    })
    .sum()
}

// ---------------------------------------------------------------------------
// 13. Porównanie wersji bez szumu
// ---------------------------------------------------------------------------

#[tokio::test]
async fn porownanie_bez_szumu_po_przenumerowaniu() {
    let s = srodowisko().await;
    let (o, v1, spec, s1, c1, w1) = oferta(&s.app, "Klient Testowy").await;
    let s2 = ok(
        &s.app,
        "POST",
        &format!("/api/v1/specyfikacje/{spec}/sekcje"),
        Some(json!({ "nazwa": "Szafa", "cena_gr": 1 })),
    )
    .await["id"]
        .as_i64()
        .unwrap();
    let c2 = ok(
        &s.app,
        "POST",
        &format!("/api/v1/sekcje/{s2}/czesci"),
        Some(json!({ "nazwa": "Korpus szafy" })),
    )
    .await["id"]
        .as_i64()
        .unwrap();
    ok(
        &s.app,
        "POST",
        &format!("/api/v1/czesci/{c2}/wiersze"),
        Some(json!({ "rodzaj": "Fronty" })),
    )
    .await;
    let s3 = ok(
        &s.app,
        "POST",
        &format!("/api/v1/specyfikacje/{spec}/sekcje"),
        Some(json!({ "nazwa": "Łazienka", "cena_gr": 1 })),
    )
    .await["id"]
        .as_i64()
        .unwrap();
    let _ = (s1, c1, w1, s3);
    let p2 = ok(
        &s.app,
        "POST",
        &format!("/api/v1/oferty/{o}/wersje"),
        Some(json!({})),
    )
    .await;
    let v2 = p2["wersja"]["id"].as_i64().unwrap();
    let sek = p2["specyfikacja"]["sekcje"].as_array().unwrap().clone();
    // v2: usuń pierwszą sekcję (pozostałe zmieniają numery 02→01, 03→02)
    let pierwsza = sek[0]["id"].as_i64().unwrap();
    ok(
        &s.app,
        "DELETE",
        &format!("/api/v1/sekcje/{pierwsza}"),
        None,
    )
    .await;
    let por = ok(
        &s.app,
        "GET",
        &format!("/api/v1/oferty/{o}/porownanie?z={v1}&do={v2}"),
        None,
    )
    .await;
    let r = por["roznice"].as_array().unwrap();
    assert!(
        r.iter().all(|x| !x["pola"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["pole"] == "numer" || p["pole"] == "położenie")),
        "{r:?}"
    );
    assert_eq!(
        r.iter().filter(|x| x["poziom"] == "sekcja").count(),
        1,
        "tylko usunięcie sekcji: {r:?}"
    );
    // przeniesienie wiersza do innej części, tryb cen, zdjęcie
    let szafa = &p2["specyfikacja"]["sekcje"][1];
    let czesc_szafy = szafa["czesci"][0]["id"].as_i64().unwrap();
    let wiersz_szafy = szafa["czesci"][0]["wiersze"][0]["id"].as_i64().unwrap();
    let lazienka = p2["specyfikacja"]["sekcje"][2]["id"].as_i64().unwrap();
    let cl = ok(
        &s.app,
        "POST",
        &format!("/api/v1/sekcje/{lazienka}/czesci"),
        Some(json!({ "nazwa": "Szafka" })),
    )
    .await["id"]
        .as_i64()
        .unwrap();
    ok(
        &s.app,
        "POST",
        &format!("/api/v1/czesci/{cl}/wiersze/kolejnosc"),
        Some(json!({ "ids": [wiersz_szafy] })),
    )
    .await;
    ok(
        &s.app,
        "PUT",
        &format!("/api/v1/wersje-oferty/{v2}"),
        Some(json!({ "tryb_cen": "netto" })),
    )
    .await;
    let spec2 = p2["wersja"]["specyfikacja_id"].as_i64().unwrap();
    assert_eq!(
        zdjecie(&s.app, spec2, png(40, 30, 3)).await.0,
        StatusCode::CREATED
    );
    let por = ok(
        &s.app,
        "GET",
        &format!("/api/v1/oferty/{o}/porownanie?z={v1}&do={v2}"),
        None,
    )
    .await;
    let r = por["roznice"].as_array().unwrap();
    assert!(
        r.iter().any(|x| x["poziom"] == "wiersz"
            && x["zmiana"] == "przeniesiono"
            && x["pola"][0]["pole"] == "część"),
        "{r:?}"
    );
    assert!(r.iter().any(|x| x["poziom"] == "wycena"
        && x["pola"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["pole"] == "ceny wpisane")));
    assert!(r
        .iter()
        .any(|x| x["poziom"] == "zdjecie" && x["zmiana"] == "dodano"));
    let _ = czesc_szafy;
}

// ---------------------------------------------------------------------------
// 16, 19, 20. Błędy po polsku, klucz w adresie, kolejność, limity, klient usunięty
// ---------------------------------------------------------------------------

#[tokio::test]
async fn bledy_zawsze_w_json_po_polsku() {
    let s = srodowisko().await;
    let (_, _, spec, _, _, w) = oferta(&s.app, "Klient Testowy").await;
    // plik > 20 MB → 413
    let (kod, nag, b) = surowe(
        &s.app,
        Request::post(format!("/api/v1/specyfikacje/{spec}/zdjecia"))
            .header(header::AUTHORIZATION, format!("Bearer {KLUCZ}"))
            .body(Body::from(vec![0u8; 21 * 1024 * 1024]))
            .unwrap(),
    )
    .await;
    assert_eq!(kod, StatusCode::PAYLOAD_TOO_LARGE);
    assert!(nag["content-type"].starts_with("application/json"));
    let v: Value = serde_json::from_slice(&b).unwrap();
    assert!(v["blad"].as_str().unwrap().contains("za duży"), "{v}");
    // błędny typ w JSON
    let (kod, v) = wyslij(
        &s.app,
        "PUT",
        &format!("/api/v1/wiersze/{w}"),
        Some(json!({ "ilosc": "dużo" })),
    )
    .await;
    assert_eq!(kod, StatusCode::UNPROCESSABLE_ENTITY);
    let t = v["blad"].as_str().unwrap();
    assert!(
        !t.contains("invalid") && !t.contains("expected") && t.contains("oczekiwano"),
        "{t}"
    );
    let (kod, v) = wyslij(
        &s.app,
        "POST",
        "/api/v1/klienci",
        Some(json!({ "nazwa": 5 })),
    )
    .await;
    assert_eq!(kod, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(v["blad"].as_str().unwrap().contains("oczekiwano"), "{v}");
    // niepoprawny JSON, brak Content-Type, nieznana trasa, zła metoda, zły identyfikator
    let (kod, _, b) = surowe(
        &s.app,
        Request::post("/api/v1/klienci")
            .header(header::AUTHORIZATION, format!("Bearer {KLUCZ}"))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from("{zly"))
            .unwrap(),
    )
    .await;
    assert_eq!(kod, StatusCode::UNPROCESSABLE_ENTITY);
    let t = String::from_utf8_lossy(&b).to_string();
    assert!(
        t.contains("treść żądania") && !t.contains("expected"),
        "{t}"
    );
    let (kod, _, b) = surowe(
        &s.app,
        Request::post("/api/v1/klienci")
            .header(header::AUTHORIZATION, format!("Bearer {KLUCZ}"))
            .body(Body::from("{}"))
            .unwrap(),
    )
    .await;
    assert!(kod.is_client_error());
    assert!(serde_json::from_slice::<Value>(&b).unwrap()["blad"].is_string());
    for (m, u) in [
        ("GET", "/api/v1/nie-ma-takiej"),
        ("PATCH", "/api/v1/oferty"),
        ("GET", "/api/v1/oferty/abc"),
    ] {
        let (kod, nag, b) = surowe(&s.app, zadanie(m, u, None)).await;
        assert!(kod.is_client_error(), "{m} {u}");
        assert!(
            nag["content-type"].starts_with("application/json"),
            "{m} {u}"
        );
        let v: Value = serde_json::from_slice(&b).unwrap();
        assert!(v["blad"].is_string(), "{m} {u}: {v}");
    }
    // klucz w adresie: tylko WebSocket
    let (kod, _, _) = surowe(
        &s.app,
        Request::get(format!("/api/v1/oferty?klucz={KLUCZ}"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(kod, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn kolejnosc_limity_i_klient_usuniety() {
    let s = srodowisko().await;
    let (o, _, spec, s1, c1, w1) = oferta(&s.app, "Klient Testowy").await;
    let s2 = ok(
        &s.app,
        "POST",
        &format!("/api/v1/specyfikacje/{spec}/sekcje"),
        Some(json!({ "nazwa": "Szafa" })),
    )
    .await["id"]
        .as_i64()
        .unwrap();
    let w2 = ok(
        &s.app,
        "POST",
        &format!("/api/v1/czesci/{c1}/wiersze"),
        Some(json!({ "rodzaj": "Fronty" })),
    )
    .await["id"]
        .as_i64()
        .unwrap();
    for (u, ids) in [
        (
            format!("/api/v1/specyfikacje/{spec}/sekcje/kolejnosc"),
            json!([s1]),
        ),
        (
            format!("/api/v1/specyfikacje/{spec}/sekcje/kolejnosc"),
            json!([s1, s2, s1]),
        ),
        (
            format!("/api/v1/czesci/{c1}/wiersze/kolejnosc"),
            json!([w2]),
        ),
        (
            format!("/api/v1/czesci/{c1}/wiersze/kolejnosc"),
            json!([w1, w2, w2]),
        ),
    ] {
        let (kod, b) = wyslij(&s.app, "POST", &u, Some(json!({ "ids": ids }))).await;
        assert_eq!(kod, StatusCode::UNPROCESSABLE_ENTITY, "{u} {ids}: {b}");
    }
    let r = ok(
        &s.app,
        "POST",
        &format!("/api/v1/czesci/{c1}/wiersze/kolejnosc"),
        Some(json!({ "ids": [w2, w1] })),
    )
    .await;
    assert_eq!(
        r["specyfikacja"]["sekcje"][0]["czesci"][0]["wiersze"][0]["id"],
        w2
    );
    let (kod, _) = wyslij(
        &s.app,
        "PUT",
        &format!("/api/v1/wiersze/{w1}"),
        Some(json!({ "opis": "x".repeat(1001) })),
    )
    .await;
    assert_eq!(kod, StatusCode::UNPROCESSABLE_ENTITY);
    let (kod, _) = wyslij(
        &s.app,
        "PUT",
        &format!("/api/v1/sekcje/{s1}"),
        Some(json!({ "nazwa": "x".repeat(201) })),
    )
    .await;
    assert_eq!(kod, StatusCode::UNPROCESSABLE_ENTITY);
    // klient usunięty miękko
    let k = ok(&s.app, "GET", &format!("/api/v1/oferty/{o}"), None).await["oferta"]["klient_id"]
        .as_i64()
        .unwrap();
    ok(&s.app, "DELETE", &format!("/api/v1/klienci/{k}"), None).await;
    let l = ok(&s.app, "GET", "/api/v1/oferty", None).await;
    assert_eq!(l[0]["klient_usuniety"], true);
    let (kod, b) = wyslij(
        &s.app,
        "POST",
        &format!("/api/v1/oferty/{o}/wersje"),
        Some(json!({})),
    )
    .await;
    assert_eq!(kod, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(b["blad"].as_str().unwrap().contains("przywróć"));
    ok(
        &s.app,
        "POST",
        &format!("/api/v1/klienci/{k}/przywroc"),
        None,
    )
    .await;
    ok(
        &s.app,
        "POST",
        &format!("/api/v1/oferty/{o}/wersje"),
        Some(json!({})),
    )
    .await;
    // stronicowanie
    let l = ok(&s.app, "GET", "/api/v1/oferty?limit=1&od=0", None).await;
    assert_eq!(l.as_array().unwrap().len(), 1);
    let l = ok(&s.app, "GET", "/api/v1/oferty?limit=1&od=5", None).await;
    assert!(l.as_array().unwrap().is_empty());
}

// ---------------------------------------------------------------------------
// 9. Import szablonów – lista błędów
// ---------------------------------------------------------------------------

#[tokio::test]
async fn import_szablonow_zwraca_liste_bledow() {
    let s = srodowisko().await;
    let (kod, b) = wyslij(&s.app, "POST", "/api/v1/import/szablony-mebli", Some(json!({ "szablony": [{ "nazwa": "Zły", "wiersze": [{ "rodzaj": "Korpus", "podgrupa": "inna" }] }] }))).await;
    assert_eq!(kod, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(b["kod"], "bledy_pozycji");
    assert_eq!(b["bledy"][0]["pole"], "podgrupa");
    assert!(b["bledy"][0]["pozycja"]
        .as_str()
        .unwrap()
        .contains("wiersz 1"));
    assert!(b["blad"]
        .as_str()
        .unwrap()
        .contains("nic nie zaimportowano"));
}

// ---------------------------------------------------------------------------
// 15, 21. Eksport: kwoty w DOCX i PDF, brak kwot kosztu i marży, łamanie stron, rozmiar
// ---------------------------------------------------------------------------

#[tokio::test]
async fn eksport_kwoty_bez_danych_wewnetrznych_i_rozmiar_pdf() {
    let s = srodowisko().await;
    ok(&s.app, "PUT", "/api/v1/ustawienia/firma", Some(json!({ "firma_nazwa": "Stolarnia Przykładowa", "firma_adres": "", "firma_nip": "", "firma_telefon": "", "firma_email": "", "oferta_waznosc_dni": 30, "oferta_termin_realizacji": "" }))).await;
    let (_, v, spec, sekcja, c, _) = oferta(&s.app, "Klient Testowy").await;
    ok(
        &s.app,
        "PUT",
        &format!("/api/v1/sekcje/{sekcja}"),
        Some(json!({ "cena_gr": 4_321_000 })),
    )
    .await;
    // dużo wierszy z długim opisem – kilka stron
    for i in 0..60 {
        ok(&s.app, "POST", &format!("/api/v1/czesci/{c}/wiersze"), Some(json!({ "rodzaj": format!("Element {i}"), "material": "Płyta Testowa Dąb", "opis": "Opis testowy pozycji, który jest na tyle długi, że zajmuje w tabeli kilka linii tekstu w kolumnie opisu.", "ilosc": 2, "jednostka": "szt." }))).await;
    }
    ok(
        &s.app,
        "POST",
        &format!("/api/v1/czesci/{c}/wiersze"),
        Some(json!({ "rodzaj": "Uchwyty", "podgrupa": "wyposazenie", "material": "Aluminium" })),
    )
    .await;
    let agd = ok(
        &s.app,
        "POST",
        &format!("/api/v1/specyfikacje/{spec}/sekcje"),
        Some(json!({ "nazwa": "Sprzęt AGD", "typ": "agd" })),
    )
    .await["id"]
        .as_i64()
        .unwrap();
    let ca = ok(
        &s.app,
        "POST",
        &format!("/api/v1/sekcje/{agd}/czesci"),
        Some(json!({ "nazwa": "AGD" })),
    )
    .await["id"]
        .as_i64()
        .unwrap();
    ok(
        &s.app,
        "POST",
        &format!("/api/v1/czesci/{ca}/wiersze"),
        Some(json!({ "rodzaj": "Piekarnik", "cena_gr": 199_900, "ilosc": 2 })),
    )
    .await;
    ok(
        &s.app,
        "PUT",
        &format!("/api/v1/wersje-oferty/{v}"),
        Some(json!({ "rabat_termin_gr": 21_000, "koszt_wlasny_gr": 1_777_700 })),
    )
    .await;
    let p = ok(&s.app, "GET", &format!("/api/v1/wersje-oferty/{v}"), None).await;
    let marza = p["wycena"]["wewnetrzne"]["marza_gr"].as_i64().unwrap();
    let kwota = |gr: i64| {
        let zl = gr / 100;
        let t = zl.to_string();
        let mut z = String::new();
        for (i, ch) in t.chars().enumerate() {
            if i > 0 && (t.len() - i).is_multiple_of(3) {
                z.push('\u{a0}');
            }
            z.push(ch);
        }
        format!("{z},{:02}", gr % 100)
    };
    let oczekiwane = [
        kwota(4_321_000),
        kwota(21_000),
        kwota(399_800),
        kwota(4_321_000 - 21_000 + 399_800),
    ];
    let zakazane = [kwota(1_777_700), kwota(marza)];

    let (_, _, docx) = surowe(
        &s.app,
        zadanie(
            "GET",
            &format!("/api/v1/wersje-oferty/{v}/eksport/docx"),
            None,
        ),
    )
    .await;
    let t = tekst_docx(&docx);
    for k in &oczekiwane {
        assert!(t.contains(k.as_str()), "brak kwoty {k} w DOCX");
    }
    for k in &zakazane {
        assert!(!t.contains(k.as_str()), "kwota wewnętrzna {k} w DOCX");
    }
    assert!(!t.to_lowercase().contains("marża") && !t.to_lowercase().contains("koszt własny"));
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(&docx)).unwrap();
    let mut xml = String::new();
    z.by_name("word/document.xml")
        .unwrap()
        .read_to_string(&mut xml)
        .unwrap();
    let mut style = String::new();
    let _ = z
        .by_name("word/styles.xml")
        .map(|mut f| f.read_to_string(&mut style));
    assert!(
        !xml.contains("Inter") && !style.contains("\"Inter\""),
        "DOCX nie może wymagać nieosadzonej czcionki Inter"
    );

    let (kod, _, pdf) = surowe(
        &s.app,
        zadanie(
            "GET",
            &format!("/api/v1/wersje-oferty/{v}/eksport/pdf"),
            None,
        ),
    )
    .await;
    assert_eq!(kod, StatusCode::OK);
    if let Ok(cel) = std::env::var("ROBINWOOD_EKSPORT_DIR") {
        std::fs::create_dir_all(&cel).unwrap();
        std::fs::write(format!("{cel}/oferta-dluga.pdf"), &pdf).unwrap();
        std::fs::write(format!("{cel}/oferta-dluga.docx"), &docx).unwrap();
    }
    assert!(
        pdf.len() < 500_000,
        "PDF ma {} B – czcionki powinny być podzbiorem",
        pdf.len()
    );
    let t = tekst_pdf(&pdf);
    assert!(
        t.contains("OFERTA") && t.contains("Klient Testowy"),
        "tekst PDF: {}",
        &t[..t.len().min(400)]
    );
    for k in &oczekiwane {
        assert!(t.contains(k.as_str()), "brak kwoty {k} w PDF");
    }
    for k in &zakazane {
        assert!(!t.contains(k.as_str()), "kwota wewnętrzna {k} w PDF");
    }
    let dok = lopdf::Document::load_mem(&pdf).unwrap();
    let strony = dok.get_pages().len();
    assert!(strony >= 3, "{strony} stron");
    // nagłówek tabeli powtarza się na kolejnych stronach z tabelą
    assert!(
        t.matches("Rodzaj").count() >= strony - 1,
        "nagłówek tabeli powtarzany na stronach"
    );
    assert!(t.contains("(cd.)"));
}
