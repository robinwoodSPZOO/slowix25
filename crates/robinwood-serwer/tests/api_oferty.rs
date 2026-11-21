//! Testy HTTP Generatora Ofert: klient → oferta → sekcje → części → wiersze → wycena →
//! nowa wersja → porównanie → zdjęcie → eksport DOCX/PDF. Wyłącznie dane fikcyjne.
//!
//! `ROBINWOOD_EKSPORT_DIR=/tmp/x cargo test -p robinwood-serwer --test api_oferty` zapisuje
//! wygenerowane dokumenty do wskazanego folderu (do obejrzenia).

use std::io::Read;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use robinwood_serwer::{api, baza, StanApi};
use serde_json::{json, Value};
use tower::ServiceExt;

const KLUCZ: &str = "klucz-testowy";

async fn aplikacja() -> (axum::Router, std::path::PathBuf) {
    let pula = baza::otworz_w_pamieci().await.unwrap();
    let folder = std::env::temp_dir().join(format!(
        "robinwood-test-{}-{}",
        std::process::id(),
        rand_suffix()
    ));
    let stan = StanApi::nowy(pula, KLUCZ.into(), "Test".into()).z_folderem_danych(folder.clone());
    (api::router(stan), folder)
}

fn rand_suffix() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos()
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

async fn bajty(app: &axum::Router, uri: &str) -> (StatusCode, String, Vec<u8>) {
    let odp = app
        .clone()
        .oneshot(zadanie("GET", uri, None))
        .await
        .unwrap();
    let kod = odp.status();
    let typ = odp
        .headers()
        .get(header::CONTENT_TYPE)
        .map(|v| v.to_str().unwrap().to_string())
        .unwrap_or_default();
    let b = odp.into_body().collect().await.unwrap().to_bytes().to_vec();
    (kod, typ, b)
}

fn maly_png() -> Vec<u8> {
    let img = image023::RgbImage::from_fn(40, 30, |x, y| {
        image023::Rgb([(x * 6) as u8, (y * 8) as u8, 120])
    });
    let mut png = Vec::new();
    image023::png::PngEncoder::new(&mut png)
        .encode(img.as_raw(), 40, 30, image023::ColorType::Rgb8)
        .unwrap();
    png
}

fn tekst_docx(b: &[u8]) -> String {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(b)).unwrap();
    let mut s = String::new();
    z.by_name("word/document.xml")
        .unwrap()
        .read_to_string(&mut s)
        .unwrap();
    s
}

#[tokio::test]
async fn pelny_scenariusz_oferty() {
    let (app, folder) = aplikacja().await;

    ok(&app, "PUT", "/api/v1/ustawienia/firma", Some(json!({
        "firma_nazwa": "Stolarnia Przykładowa", "firma_adres": "ul. Testowa 1\n00-000 Przykładowo",
        "firma_nip": "", "firma_telefon": "000 000 000", "firma_email": "biuro@example.com",
        "oferta_waznosc_dni": 30, "oferta_termin_realizacji": "do 10 tygodni"
    }))).await;

    let klient = ok(&app, "POST", "/api/v1/klienci", Some(json!({
        "nazwa": "Klient Testowy", "adres_ulica": "ul. Fikcyjna 2", "adres_kod": "00-001", "adres_miasto": "Testowo"
    }))).await;

    let (kod, _) = wyslij(
        &app,
        "POST",
        "/api/v1/oferty",
        Some(json!({ "klient_id": 9999 })),
    )
    .await;
    assert_eq!(kod, StatusCode::UNPROCESSABLE_ENTITY);

    let p = ok(&app, "POST", "/api/v1/oferty", Some(json!({
        "klient_id": klient["id"], "adres_inwestycji": "ul. Inwestycyjna 3, Testowo", "stawka_vat": 8
    }))).await;
    let oferta = p["oferta"]["id"].as_i64().unwrap();
    let v1 = p["wersja"]["id"].as_i64().unwrap();
    let spec = p["wersja"]["specyfikacja_id"].as_i64().unwrap();
    assert_eq!(p["wersja"]["termin_realizacji"], "do 10 tygodni");
    assert_eq!(p["wersja"]["nr_wersji"], 1);

    // sekcja z szablonu + sekcja ręczna + AGD
    let szablony = ok(&app, "GET", "/api/v1/szablony-mebli", None).await;
    let kuchnia = szablony
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["kod"] == "kuchnia")
        .unwrap();
    let w = ok(
        &app,
        "POST",
        &format!("/api/v1/specyfikacje/{spec}/z-szablonu"),
        Some(json!({
            "szablon_id": kuchnia["id"], "czesci": ["Słupki", "Szafki pod blatem"]
        })),
    )
    .await;
    let s1 = w["id"].as_i64().unwrap();
    let sekcja = &w["specyfikacja"]["sekcje"][0];
    assert_eq!(sekcja["numer"], "01");
    assert_eq!(sekcja["nazwa"], "Zabudowa Kuchni");
    let slupki = sekcja["czesci"][0]["id"].as_i64().unwrap();
    let korpus = sekcja["czesci"][0]["wiersze"][0]["id"].as_i64().unwrap();
    assert_eq!(sekcja["czesci"][0]["wiersze"][0]["rodzaj"], "Korpus");

    // zmiana materiału korpusu spływa na półki
    ok(&app, "PUT", &format!("/api/v1/wiersze/{korpus}"), Some(json!({
        "rodzaj": "Korpus", "material": "Płyta Testowa Dąb", "opis": "Kolor: do potwierdzenia", "status": "do_potwierdzenia"
    }))).await;
    let w = ok(&app, "POST", &format!("/api/v1/czesci/{slupki}/wiersze"), Some(json!({
        "rodzaj": "Fronty", "material": "MDF lakierowany", "opis": "Biały mat", "ilosc": 4, "jednostka": "szt."
    }))).await;
    let wiersze = &w["specyfikacja"]["sekcje"][0]["czesci"][0]["wiersze"];
    let polki = wiersze
        .as_array()
        .unwrap()
        .iter()
        .find(|w| w["rodzaj"] == "Półki")
        .unwrap();
    assert_eq!(polki["material"], "Płyta Testowa Dąb");

    let s2 = ok(
        &app,
        "POST",
        &format!("/api/v1/specyfikacje/{spec}/sekcje"),
        Some(json!({ "nazwa": "Szafa w przedpokoju" })),
    )
    .await["id"]
        .as_i64()
        .unwrap();
    let c2 = ok(
        &app,
        "POST",
        &format!("/api/v1/sekcje/{s2}/czesci"),
        Some(json!({ "nazwa": "Szafa" })),
    )
    .await["id"]
        .as_i64()
        .unwrap();
    ok(&app, "POST", &format!("/api/v1/czesci/{c2}/wiersze"), Some(json!({
        "rodzaj": "Drążek / wieszak", "podgrupa": "wyposazenie", "material": "Aluminium", "ilosc": 2, "jednostka": "szt."
    }))).await;
    let agd = ok(
        &app,
        "POST",
        &format!("/api/v1/specyfikacje/{spec}/sekcje"),
        Some(json!({ "nazwa": "Sprzęt AGD", "typ": "agd", "stawka_vat": 23 })),
    )
    .await;
    let agd_id = agd["id"].as_i64().unwrap();
    assert!(agd["specyfikacja"]["sekcje"][2]["numer"].is_null());
    let agd_c = ok(
        &app,
        "POST",
        &format!("/api/v1/sekcje/{agd_id}/czesci"),
        Some(json!({ "nazwa": "AGD" })),
    )
    .await["id"]
        .as_i64()
        .unwrap();
    ok(&app, "POST", &format!("/api/v1/czesci/{agd_c}/wiersze"), Some(json!({ "rodzaj": "Piekarnik", "material": "Marka X model 1", "ilosc": 1, "cena_gr": 300_000 }))).await;
    ok(
        &app,
        "POST",
        &format!("/api/v1/czesci/{agd_c}/wiersze"),
        Some(json!({ "rodzaj": "Lodówka", "status": "po_stronie_klienta", "cena_gr": 500_000 })),
    )
    .await;

    // ceny sekcji i wycena
    ok(
        &app,
        "PUT",
        &format!("/api/v1/sekcje/{s1}"),
        Some(json!({ "nazwa": "Zabudowa Kuchni", "cena_gr": 3_240_000 })),
    )
    .await;
    ok(
        &app,
        "PUT",
        &format!("/api/v1/sekcje/{s2}"),
        Some(json!({ "nazwa": "Szafa w przedpokoju", "cena_gr": 1_080_000 })),
    )
    .await;
    let mut wersja =
        ok(&app, "GET", &format!("/api/v1/wersje-oferty/{v1}"), None).await["wersja"].clone();
    wersja["rabat_termin_gr"] = json!(20_000);
    wersja["rabat_calosc_bp"] = json!(500);
    wersja["koszt_wlasny_gr"] = json!(2_500_000);
    let p = ok(
        &app,
        "PUT",
        &format!("/api/v1/wersje-oferty/{v1}"),
        Some(wersja.clone()),
    )
    .await;
    let wyc = &p["wycena"];
    assert_eq!(wyc["suma_meble_gr"], 4_320_000);
    assert_eq!(wyc["rabat_calosc_gr"], 216_000);
    assert_eq!(wyc["meble_po_rabatach_gr"], 4_320_000 - 20_000 - 216_000);
    assert_eq!(wyc["suma_agd_gr"], 300_000);
    assert_eq!(wyc["suma_brutto_gr"], 4_084_000 + 300_000);
    assert_eq!(wyc["meble"]["stawka"], 8);
    assert_eq!(wyc["agd"][0]["stawka"], 23);
    assert!(wyc["wewnetrzne"]["marza_gr"].as_i64().unwrap() > 0);

    // kopiowanie części i kolejność
    let w = ok(
        &app,
        "POST",
        &format!("/api/v1/czesci/{slupki}/kopiuj"),
        Some(json!({})),
    )
    .await;
    assert_eq!(
        w["specyfikacja"]["sekcje"][0]["czesci"][1]["nazwa"],
        "Słupki (kopia)"
    );
    let kopia = w["id"].as_i64().unwrap();
    ok(&app, "DELETE", &format!("/api/v1/czesci/{kopia}"), None).await;

    // zdjęcie przy części + odczyt pliku z kluczem w adresie
    let odp = app.clone().oneshot(
        Request::post(format!("/api/v1/specyfikacje/{spec}/zdjecia?czesc_id={slupki}&nazwa=wizualizacja.png&podpis=Wizualizacja%20testowa"))
            .header(header::AUTHORIZATION, format!("Bearer {KLUCZ}"))
            .header(header::CONTENT_TYPE, "image/png")
            .body(Body::from(maly_png()))
            .unwrap(),
    ).await.unwrap();
    assert_eq!(odp.status(), StatusCode::CREATED);
    let v: Value =
        serde_json::from_slice(&odp.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let zdj = &v["specyfikacja"]["zdjecia"][0];
    assert_eq!(zdj["czesc_id"], slupki);
    assert_eq!(zdj["sekcja_id"], s1);
    let plik = zdj["plik_id"].as_i64().unwrap();
    let odp = app
        .clone()
        .oneshot(
            Request::get(format!("/api/v1/pliki/{plik}?klucz={KLUCZ}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(odp.status(), StatusCode::OK);
    assert_eq!(odp.headers()[header::CONTENT_TYPE], "image/png");
    let odp = app
        .clone()
        .oneshot(
            Request::post(format!("/api/v1/specyfikacje/{spec}/zdjecia"))
                .header(header::AUTHORIZATION, format!("Bearer {KLUCZ}"))
                .body(Body::from("to nie jest obraz"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(odp.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // nowa wersja: kopia; stara tylko do odczytu
    let p2 = ok(
        &app,
        "POST",
        &format!("/api/v1/oferty/{oferta}/wersje"),
        Some(json!({})),
    )
    .await;
    let v2 = p2["wersja"]["id"].as_i64().unwrap();
    assert_eq!(p2["wersja"]["nr_wersji"], 2);
    assert_eq!(p2["wycena"]["suma_brutto_gr"], 4_384_000);
    assert_eq!(p2["specyfikacja"]["zdjecia"].as_array().unwrap().len(), 1);
    let (kod, b) = wyslij(
        &app,
        "PUT",
        &format!("/api/v1/wiersze/{korpus}"),
        Some(json!({ "rodzaj": "Korpus" })),
    )
    .await;
    assert_eq!(kod, StatusCode::UNPROCESSABLE_ENTITY, "{b}");
    let (kod, _) = wyslij(
        &app,
        "PUT",
        &format!("/api/v1/wersje-oferty/{v1}"),
        Some(wersja),
    )
    .await;
    assert_eq!(kod, StatusCode::UNPROCESSABLE_ENTITY);
    let s1_v2 = p2["specyfikacja"]["sekcje"][0]["id"].as_i64().unwrap();
    ok(
        &app,
        "PUT",
        &format!("/api/v1/sekcje/{s1_v2}"),
        Some(json!({ "nazwa": "Zabudowa Kuchni", "cena_gr": 3_500_000 })),
    )
    .await;

    let por = ok(
        &app,
        "GET",
        &format!("/api/v1/oferty/{oferta}/porownanie?z={v1}&do={v2}"),
        None,
    )
    .await;
    let r = por["roznice"].as_array().unwrap();
    assert!(r
        .iter()
        .any(|x| x["poziom"] == "sekcja" && x["zmiana"] == "zmieniono"));
    assert!(r.iter().any(|x| x["poziom"] == "wycena"));

    // statusy i lista
    ok(
        &app,
        "PUT",
        &format!("/api/v1/oferty/{oferta}/status"),
        Some(json!({ "status": "wyslana" })),
    )
    .await;
    let lista = ok(&app, "GET", "/api/v1/oferty?status=wyslana", None).await;
    assert_eq!(lista.as_array().unwrap().len(), 1);
    assert_eq!(lista[0]["nr_wersji"], 2);
    assert_eq!(
        lista[0]["suma_brutto_gr"],
        3_500_000 - 20_000 - 175_000 + 1_080_000 - 54_000 + 300_000
    );
    assert!(ok(&app, "GET", "/api/v1/oferty?status=szkic", None)
        .await
        .as_array()
        .unwrap()
        .is_empty());
    let (kod, _) = wyslij(&app, "GET", "/api/v1/oferty?status=nieznany", None).await;
    assert_eq!(kod, StatusCode::UNPROCESSABLE_ENTITY);

    // eksport
    let (kod, typ, pdf) = bajty(&app, &format!("/api/v1/wersje-oferty/{v2}/eksport/pdf")).await;
    assert_eq!(kod, StatusCode::OK);
    assert_eq!(typ, "application/pdf");
    assert!(pdf.starts_with(b"%PDF"));
    let (kod, _, docx) = bajty(&app, &format!("/api/v1/wersje-oferty/{v2}/eksport/docx")).await;
    assert_eq!(kod, StatusCode::OK);
    let xml = tekst_docx(&docx);
    for oczekiwany in [
        "OFERTA",
        "Klient Testowy",
        "Stolarnia Przykładowa",
        "01 Zabudowa Kuchni",
        "Słupki",
        "Suma za meble",
        "SUMA brutto",
        "Stawka VAT: meble 8%",
        "Wyposażenie",
        "Płyta Testowa Dąb",
        "Sprzęt AGD",
        "(po stronie klienta)",
        "ważne do",
    ] {
        assert!(xml.contains(oczekiwany), "brak w DOCX: {oczekiwany}");
    }
    assert!(
        !xml.contains("marża") && !xml.contains("Koszt"),
        "dane wewnętrzne w dokumencie"
    );
    let (kod, _, _) = bajty(&app, &format!("/api/v1/wersje-oferty/{v2}/eksport/odt")).await;
    assert_eq!(kod, StatusCode::UNPROCESSABLE_ENTITY);
    if let Ok(cel) = std::env::var("ROBINWOOD_EKSPORT_DIR") {
        std::fs::create_dir_all(&cel).unwrap();
        std::fs::write(format!("{cel}/oferta-test.pdf"), &pdf).unwrap();
        std::fs::write(format!("{cel}/oferta-test.docx"), &docx).unwrap();
    }

    // usunięcie oferty
    ok(&app, "DELETE", &format!("/api/v1/oferty/{oferta}"), None).await;
    let (kod, _) = wyslij(&app, "GET", &format!("/api/v1/oferty/{oferta}"), None).await;
    assert_eq!(kod, StatusCode::NOT_FOUND);
    let _ = std::fs::remove_dir_all(folder);
}

#[tokio::test]
async fn import_slownikow_i_szablonow_przez_http() {
    let (app, _) = aplikacja().await;
    let sl: Value = serde_json::from_str(include_str!("dane/slowniki-przyklad.json")).unwrap();
    let r = ok(&app, "POST", "/api/v1/import/slowniki", Some(sl)).await;
    assert_eq!(r["dodane"]["dekor"], 2);
    let sz: Value =
        serde_json::from_str(include_str!("dane/szablony-mebli-przyklad.json")).unwrap();
    let r = ok(&app, "POST", "/api/v1/import/szablony-mebli", Some(sz)).await;
    assert_eq!(r["dodane"]["szablony"], 1);
    let p = ok(&app, "GET", "/api/v1/slowniki-podpowiedzi", None).await;
    assert!(p["dekor"]
        .as_array()
        .unwrap()
        .iter()
        .any(|x| x["wartosc"] == "T001 Dąb Testowy"));
    assert!(p["rodzaj_elementu"]
        .as_array()
        .unwrap()
        .iter()
        .any(|x| x["podgrupa"] == "wyposazenie"));
    let (kod, _) = wyslij(
        &app,
        "POST",
        "/api/v1/import/szablony-mebli",
        Some(json!({ "x": 1 })),
    )
    .await;
    assert_eq!(kod, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn trasy_chronione_kluczem() {
    let (app, _) = aplikacja().await;
    for uri in [
        "/api/v1/oferty",
        "/api/v1/specyfikacje/1",
        "/api/v1/pliki/1",
        "/api/v1/ustawienia/firma",
    ] {
        let odp = app
            .clone()
            .oneshot(Request::get(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(odp.status(), StatusCode::UNAUTHORIZED, "{uri}");
    }
}
