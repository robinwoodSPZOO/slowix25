//! Testy HTTP Generatora Umów: oferta → umowa → formularz → dokument → podpisanie → zlecenie
//! i harmonogram → wpłata → aneks → protokół odbioru; numeracja (równoległa, przełom roku),
//! zamrożenie, szablon użytkownika, autoryzacja. Wyłącznie dane fikcyjne.
//!
//! `ROBINWOOD_EKSPORT_DIR=/tmp/x cargo test -p robinwood-serwer --test api_umowy` zapisuje
//! wygenerowane dokumenty do wskazanego folderu (do obejrzenia).

use std::io::{Cursor, Read, Write};

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use robinwood_serwer::{api, baza, umowy, StanApi};
use serde_json::{json, Value};
use tower::ServiceExt;

const KLUCZ: &str = "klucz-testowy";

fn sufiks() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos()
}

async fn aplikacja() -> (axum::Router, std::path::PathBuf) {
    let pula = baza::otworz_w_pamieci().await.unwrap();
    let folder = std::env::temp_dir().join(format!(
        "robinwood-umowy-{}-{}",
        std::process::id(),
        sufiks()
    ));
    let stan = StanApi::nowy(pula, KLUCZ.into(), "Test".into()).z_folderem_danych(folder.clone());
    (api::router(stan), folder)
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
    let b = odp.into_body().collect().await.unwrap().to_bytes();
    (kod, serde_json::from_slice(&b).unwrap_or(Value::Null))
}

async fn ok(app: &axum::Router, metoda: &str, uri: &str, cialo: Option<Value>) -> Value {
    let (kod, v) = wyslij(app, metoda, uri, cialo).await;
    assert!(kod.is_success(), "{metoda} {uri} → {kod}: {v}");
    v
}

async fn bajty(app: &axum::Router, uri: &str) -> (StatusCode, Vec<u8>) {
    let odp = app
        .clone()
        .oneshot(zadanie("GET", uri, None))
        .await
        .unwrap();
    let kod = odp.status();
    (
        kod,
        odp.into_body().collect().await.unwrap().to_bytes().to_vec(),
    )
}

fn czesc_docx(b: &[u8], nazwa: &str) -> String {
    let mut z = zip::ZipArchive::new(Cursor::new(b)).unwrap();
    let mut s = String::new();
    z.by_name(nazwa).unwrap().read_to_string(&mut s).unwrap();
    s
}

/// Sam tekst dokumentu (bez znaczników XML).
fn tekst(xml: &str) -> String {
    let mut s = String::new();
    let mut w_tagu = false;
    for c in xml.chars() {
        match c {
            '<' => w_tagu = true,
            '>' => w_tagu = false,
            c if !w_tagu => s.push(c),
            _ => {}
        }
    }
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&#91;", "[")
        .replace("&#93;", "]")
        .replace('\u{a0}', " ")
}

fn zapisz_do_obejrzenia(nazwa: &str, b: &[u8]) {
    if let Ok(k) = std::env::var("ROBINWOOD_EKSPORT_DIR") {
        std::fs::create_dir_all(&k).unwrap();
        std::fs::write(std::path::Path::new(&k).join(nazwa), b).unwrap();
    }
}

/// Oferta wysłana z jedną sekcją i trzema wierszami (dwie kategorie materiałów, jeden
/// wiersz „do potwierdzenia”), wycena na poziomie całości, koszt własny (dana wewnętrzna).
async fn oferta_wyslana(app: &axum::Router) -> (i64, i64, i64) {
    ok(app, "PUT", "/api/v1/ustawienia/firma", Some(json!({
        "firma_nazwa": "Stolarnia Przykładowa", "firma_adres": "ul. Testowa 1, 00-000 Przykładowo",
        "firma_nip": "000-000-00-00", "firma_telefon": "000 000 000", "firma_email": "biuro@example.com",
        "oferta_waznosc_dni": 30, "oferta_termin_realizacji": "do 10 tygodni",
        "firma_bank": "Bank Przykładowy", "firma_rachunek": "00 0000 0000 0000 0000 0000 0000", "firma_swift": "TESTPLPW"
    }))).await;
    for kat in [1, 3, 9] {
        ok(
            app,
            "PUT",
            &format!("/api/v1/kategorie-materialow/{kat}/widelki"),
            Some(json!({"dni_dostawy_min": 5, "dni_dostawy_max": 10})),
        )
        .await;
    }
    let k = ok(app, "POST", "/api/v1/klienci", Some(json!({
        "nazwa": "Klient Testowy", "adres_ulica": "ul. Fikcyjna 2", "adres_kod": "00-001", "adres_miasto": "Testowo",
        "email": "klient@example.com", "telefon": "111 111 111"
    }))).await;
    let kid = k["id"].as_i64().unwrap();
    let p = ok(app, "POST", "/api/v1/oferty", Some(json!({
        "klient_id": kid, "adres_inwestycji": "ul. Inwestycyjna 3, Testowo", "stawka_vat": 8, "poziom_wyceny": "calosc"
    }))).await;
    let oferta = p["oferta"]["id"].as_i64().unwrap();
    let wersja = p["wersja"]["id"].as_i64().unwrap();
    let spec = p["wersja"]["specyfikacja_id"].as_i64().unwrap();
    let s = ok(
        app,
        "POST",
        &format!("/api/v1/specyfikacje/{spec}/sekcje"),
        Some(json!({"nazwa": "Zabudowa kuchni"})),
    )
    .await;
    let s = s["id"].as_i64().unwrap();
    let c = ok(
        app,
        "POST",
        &format!("/api/v1/sekcje/{s}/czesci"),
        Some(json!({"nazwa": "Szafki dolne"})),
    )
    .await;
    let c = c["id"].as_i64().unwrap();
    for (rodzaj, material, kat, status) in [
        ("Korpus", "Płyta Testowa Biała", 1, "ustalone"),
        ("Fronty", "MDF [lakier] mat", 3, "do_potwierdzenia"),
        ("Blat", "Laminat Testowy", 7, "po_stronie_klienta"),
    ] {
        ok(app, "POST", &format!("/api/v1/czesci/{c}/wiersze"), Some(json!({
            "rodzaj": rodzaj, "material": material, "opis": "Opis testowy", "ilosc": 1, "jednostka": "kpl.",
            "kategoria_materialu_id": kat, "status": status
        }))).await;
    }
    ok(
        app,
        "PUT",
        &format!("/api/v1/wersje-oferty/{wersja}"),
        Some(json!({
            "cena_calosc_gr": 4_835_200, "koszt_wlasny_gr": 1_234_567
        })),
    )
    .await;
    ok(
        app,
        "PUT",
        &format!("/api/v1/oferty/{oferta}/status"),
        Some(json!({"status": "wyslana"})),
    )
    .await;
    (kid, oferta, wersja)
}

#[tokio::test]
async fn scenariusz_oferta_umowa_aneks_protokol() {
    let (app, folder) = aplikacja().await;
    let (kid, _oferta, wersja) = oferta_wyslana(&app).await;

    // umowy z oferty w szkicu nie da się utworzyć z wersji-szkicu; z wysłanej – tak
    let p = ok(
        &app,
        "POST",
        "/api/v1/umowy",
        Some(json!({"wersja_oferty_id": wersja, "data_zawarcia": "2026-11-20"})),
    )
    .await;
    let id = p["umowa"]["id"].as_i64().unwrap();
    assert_eq!(p["umowa"]["numer"], "01/2026");
    assert_eq!(p["umowa"]["status"], "szkic");
    assert_eq!(p["umowa"]["wartosc_brutto_gr"], 4_835_200);
    assert_eq!(p["umowa"]["stawka_vat"], 8);
    assert_eq!(p["umowa"]["klient_id"], kid);
    assert_eq!(p["umowa"]["montaz_ulica"], "ul. Inwestycyjna 3, Testowo");
    assert_eq!(
        p["umowa"]["zadatek_gr"], 1_450_600,
        "30% zaokrąglone do pełnych złotych"
    );
    assert_eq!(p["umowa"]["zalaczniki"], json!(["1", "4", "5"]));
    assert!(p["specyfikacja"]["zrodlo_id"].is_number());
    assert!(p["braki"]
        .as_array()
        .unwrap()
        .iter()
        .any(|b| b == "termin rozpoczęcia montażu"));
    // druga umowa z tej samej wersji – błąd
    let (kod, v) = wyslij(
        &app,
        "POST",
        "/api/v1/umowy",
        Some(json!({"wersja_oferty_id": wersja})),
    )
    .await;
    assert_eq!(kod, StatusCode::UNPROCESSABLE_ENTITY, "{v}");
    // bez różnic względem oferty
    let r = ok(&app, "GET", &format!("/api/v1/umowy/{id}/roznice"), None).await;
    assert_eq!(r["roznice"], json!([]));

    // formularz – zmiany częściowe; wariant B z kontrolą sumy
    let (kod, v) = wyslij(
        &app,
        "PUT",
        &format!("/api/v1/umowy/{id}"),
        Some(json!({
            "wariant_platnosci": "B", "kwota_przed_montazem_gr": 3_384_600
        })),
    )
    .await;
    assert_eq!(kod, StatusCode::OK);
    assert!(
        !v["plan"]["zgodny"].as_bool().unwrap(),
        "B bez reszty po odbiorze jest błędem"
    );
    let p = ok(&app, "PUT", &format!("/api/v1/umowy/{id}"), Some(json!({
        "miejscowosc": "Testowo", "montaz_do": "2027-02-08", "montaz_pietro": "3", "montaz_kod_domofonu": "12#",
        "montaz_parking": "przed budynkiem", "montaz_winda_wniesienie": "winda towarowa",
        "kwota_przed_montazem_gr": 2_000_000, "vat_rodzaj_nieruchomosci": "lokal_mieszkalny", "vat_powierzchnia_m2": 160.5,
        "vat_podstawa_powierzchni": "akt notarialny", "zalaczniki": ["2", "6"], "czas_warsztat_dni": 10, "czas_montazu_dni": 3,
        "dni_na_brakujace_informacje": 14
    }))).await;
    assert!(p["plan"]["zgodny"].as_bool().unwrap(), "{}", p["plan"]);
    let kwoty: Vec<i64> = p["plan"]["pozycje"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x["kwota_gr"].as_i64().unwrap())
        .collect();
    assert_eq!(kwoty, vec![1_450_600, 2_000_000, 1_384_600]);
    assert_eq!(kwoty.iter().sum::<i64>(), 4_835_200);
    assert_eq!(p["plan"]["pozycje"][0]["termin"], "2026-11-23");
    assert_eq!(p["plan"]["pozycje"][1]["termin"], "2027-02-03");
    assert!(p["ostrzezenia"]
        .as_array()
        .unwrap()
        .iter()
        .any(|o| o.as_str().unwrap().contains("150")));
    assert_eq!(p["umowa"]["zalaczniki"], json!(["1", "2", "4", "5", "6"]));
    assert_eq!(p["braki"], json!([]), "{}", p["braki"]);
    // konflikt: ktoś zmienił miejscowość w międzyczasie
    let (kod, v) = wyslij(
        &app,
        "PUT",
        &format!("/api/v1/umowy/{id}"),
        Some(json!({
            "miejscowosc": "Inne", "poprzednie": {"miejscowosc": "Stare"}
        })),
    )
    .await;
    assert_eq!(kod, StatusCode::CONFLICT, "{v}");
    assert_eq!(v["kod"], "konflikt");
    let (kod, _) = wyslij(
        &app,
        "PUT",
        &format!("/api/v1/umowy/{id}"),
        Some(json!({"nieznane": 1})),
    )
    .await;
    assert_eq!(kod, StatusCode::UNPROCESSABLE_ENTITY);
    // etapy (załącznik 2) – suma musi równać się wartości
    let p = ok(
        &app,
        "PUT",
        &format!("/api/v1/umowy/{id}/etapy"),
        Some(json!({"etapy": [
            {"nazwa": "Pomiar i projekt", "termin": "2026-12-01", "kwota_gr": 1_000_000},
            {"nazwa": "Montaż", "termin": "2027-02-08", "kwota_gr": 2_000_000}
        ]})),
    )
    .await;
    assert!(!p["kontrola_etapow"]["zgodna"].as_bool().unwrap());
    assert!(!p["braki"].as_array().unwrap().is_empty());
    let p = ok(
        &app,
        "PUT",
        &format!("/api/v1/umowy/{id}/etapy"),
        Some(json!({"etapy": [
            {"nazwa": "Pomiar i projekt", "termin": "2026-12-01", "kwota_gr": 1_835_200},
            {"nazwa": "Montaż", "termin": "2027-02-08", "kwota_gr": 3_000_000}
        ]})),
    )
    .await;
    assert!(p["kontrola_etapow"]["zgodna"].as_bool().unwrap());

    // zmiana specyfikacji umowy → różnice względem oferty
    let wiersz_frontow = p["specyfikacja"]["sekcje"][0]["czesci"][0]["wiersze"][1]["id"]
        .as_i64()
        .unwrap();
    ok(
        &app,
        "PUT",
        &format!("/api/v1/wiersze/{wiersz_frontow}"),
        Some(json!({"opis": "Opis po rozmowie z klientem"})),
    )
    .await;
    let r = ok(&app, "GET", &format!("/api/v1/umowy/{id}/roznice"), None).await;
    assert_eq!(r["roznice"].as_array().unwrap().len(), 1, "{r}");

    // podgląd pól i dokument z szablonu przykładowego
    let pola = ok(
        &app,
        "GET",
        &format!("/api/v1/umowy/{id}/dokument/pola"),
        None,
    )
    .await;
    assert_eq!(pola["zrodlo"], "przykladowy");
    assert_eq!(pola["raport"]["nieznane"], json!([]));
    let puste: Vec<&str> = pola["raport"]["puste"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap())
        .collect();
    assert!(puste.contains(&"data odbioru"), "{puste:?}");
    assert!(!puste.contains(&"numer umowy"));
    let (kod, docx) = bajty(&app, &format!("/api/v1/umowy/{id}/dokument/docx")).await;
    assert_eq!(kod, StatusCode::OK);
    zapisz_do_obejrzenia("umowa.docx", &docx);
    let xml = czesc_docx(&docx, "word/document.xml");
    let t = tekst(&xml);
    for oczekiwane in [
        "UMOWA NR 01/2026",
        "SZABLON PRZYKŁADOWY",
        "Klient Testowy",
        "48 352,00 zł brutto",
        "czterdzieści osiem tysięcy trzysta pięćdziesiąt dwa złote 00/100",
        "Stolarnia Przykładowa",
        "Wariant B –",
        "Opis po rozmowie z klientem",
        "MDF [lakier] mat",
        "Pomiar i projekt",
        "Zgoda na zdjęcia",
        "08.02.2027",
        "☒ lokal mieszkalny",
        "☐ budynek",
        "TESTPLPW",
    ] {
        assert!(
            t.contains(oczekiwane),
            "brak „{oczekiwane}” w dokumencie:\n{t}"
        );
    }
    assert!(!t.contains("Wariant A –"), "blok wariantu A usunięty");
    assert!(!t.contains("Cennik"), "załącznik 3 niezaznaczony");
    assert!(
        !t.contains("12 345,67") && !t.contains("12345,67"),
        "koszt własny nie może trafić do dokumentu"
    );
    assert!(!xml.contains("[[") && !t.contains("[numer umowy]"));
    let hdr = czesc_docx(&docx, "word/header1.xml");
    assert!(tekst(&hdr).contains("umowa nr 01/2026"), "{hdr}");
    // dokument otwiera się w LibreOffice bez naprawy – sprawdzane w scenariuszu ręcznym; tu: XML poprawny
    assert_eq!(
        xml.matches("<w:tbl>").count(),
        xml.matches("</w:tbl>").count()
    );

    // podpisanie → zamrożenie, zlecenie, materiały, harmonogram
    let w = ok(
        &app,
        "PUT",
        &format!("/api/v1/umowy/{id}/status"),
        Some(json!({"status": "podpisana", "data": "2026-11-20"})),
    )
    .await;
    assert_eq!(w["umowa"]["status"], "podpisana");
    let h = &w["harmonogram"];
    assert!(h["wygenerowano"].as_bool().unwrap(), "{h}");
    assert_eq!(
        h["materialy_dodane"], 2,
        "płyta i fronty (blat po stronie klienta – bez zamówienia)"
    );
    let zid = h["zlecenie_id"].as_i64().unwrap();
    assert_eq!(w["platnosci"].as_array().unwrap().len(), 3);
    assert!(w["umowa"]["migawka"]["zamawiajacy"]["nazwa"] == "Klient Testowy");
    let zlecenie = ok(&app, "GET", &format!("/api/v1/zlecenia/{zid}"), None).await;
    assert_eq!(zlecenie["montaz_od"], "2027-02-08");
    assert_eq!(zlecenie["czas_montazu_dni"], 3);
    assert_eq!(zlecenie["umowa_id"], id);
    let zadania = ok(
        &app,
        "GET",
        &format!("/api/v1/kalendarz/zadania?zlecenie_id={zid}&od=2026-01-01&do=2027-12-31"),
        None,
    )
    .await;
    let klucze: Vec<String> = zadania
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|z| z["klucz"].as_str().map(String::from))
        .collect();
    for k in [
        "montaz",
        "warsztat",
        "rysunki",
        "pomiar",
        "brakujace_informacje",
    ] {
        assert!(
            klucze.contains(&k.to_string()),
            "brak zadania {k}: {klucze:?}"
        );
    }
    assert_eq!(
        klucze.iter().filter(|k| k.starts_with("platnosc:")).count(),
        3
    );
    assert!(
        klucze.iter().any(|k| k.starts_with("potwierdzenie:")),
        "fronty do potwierdzenia: {klucze:?}"
    );
    // termin potwierdzenia pozycji = data podpisania + 14 dni
    let spec = ok(
        &app,
        "GET",
        &format!("/api/v1/specyfikacje/{}", w["umowa"]["specyfikacja_id"]),
        None,
    )
    .await;
    assert_eq!(
        spec["sekcje"][0]["czesci"][0]["wiersze"][1]["termin_potwierdzenia"],
        "2026-12-04"
    );
    assert!(spec["tylko_do_odczytu"].as_bool().unwrap());
    // zamrożenie
    let (kod, _) = wyslij(
        &app,
        "PUT",
        &format!("/api/v1/wiersze/{wiersz_frontow}"),
        Some(json!({"opis": "x"})),
    )
    .await;
    assert_eq!(kod, StatusCode::UNPROCESSABLE_ENTITY);
    let (kod, v) = wyslij(
        &app,
        "PUT",
        &format!("/api/v1/umowy/{id}"),
        Some(json!({"wartosc_brutto_gr": 1})),
    )
    .await;
    assert_eq!(kod, StatusCode::UNPROCESSABLE_ENTITY, "{v}");
    let (kod, _) = wyslij(
        &app,
        "PUT",
        &format!("/api/v1/umowy/{id}/status"),
        Some(json!({"status": "szkic"})),
    )
    .await;
    assert_eq!(kod, StatusCode::UNPROCESSABLE_ENTITY);
    let (kod, _) = wyslij(&app, "DELETE", &format!("/api/v1/umowy/{id}"), None).await;
    assert_eq!(kod, StatusCode::UNPROCESSABLE_ENTITY);
    // pola wewnętrzne można zmieniać – zlecenie się przelicza
    ok(
        &app,
        "PUT",
        &format!("/api/v1/umowy/{id}"),
        Some(json!({"czas_warsztat_dni": 12})),
    )
    .await;
    let zlecenie = ok(&app, "GET", &format!("/api/v1/zlecenia/{zid}"), None).await;
    assert_eq!(zlecenie["czas_warsztat_dni"], 12);

    // wpłata zadatku → saldo, zadanie płatności wykonane
    let p = ok(
        &app,
        "POST",
        &format!("/api/v1/umowy/{id}/wplaty"),
        Some(json!({"data": "2026-11-22", "kwota_gr": 1_450_600, "sposob": "przelew"})),
    )
    .await;
    assert_eq!(p["saldo"]["wplacono_gr"], 1_450_600);
    assert_eq!(p["saldo"]["pozostalo_gr"], 3_384_600);
    assert_eq!(p["saldo"]["nastepna"]["opis"], "Kwota przed montażem");
    let zadania = ok(
        &app,
        "GET",
        &format!(
            "/api/v1/kalendarz/zadania?zlecenie_id={zid}&typ=platnosc&od=2026-01-01&do=2027-12-31"
        ),
        None,
    )
    .await;
    let wykonane = zadania
        .as_array()
        .unwrap()
        .iter()
        .filter(|z| !z["wykonano"].is_null())
        .count();
    assert_eq!(wykonane, 1);
    let lista = ok(&app, "GET", "/api/v1/umowy?rok=2026&szukaj=testowy", None).await;
    assert_eq!(lista[0]["wplacono_gr"], 1_450_600);
    assert_eq!(lista[0]["saldo_gr"], 3_384_600);

    // aneks: zmiana specyfikacji, kwoty i terminu
    let (kod, _) = wyslij(&app, "POST", &format!("/api/v1/umowy/{id}/aneksy"), None).await;
    assert_eq!(kod, StatusCode::CREATED);
    let p = ok(&app, "GET", &format!("/api/v1/umowy/{id}"), None).await;
    let aid = p["aneksy"][0]["id"].as_i64().unwrap();
    let a = ok(&app, "GET", &format!("/api/v1/aneksy/{aid}"), None).await;
    assert!(!a["specyfikacja"]["tylko_do_odczytu"].as_bool().unwrap());
    let (kod, _) = wyslij(&app, "POST", &format!("/api/v1/umowy/{id}/aneksy"), None).await;
    assert_eq!(kod, StatusCode::UNPROCESSABLE_ENTITY, "drugi otwarty aneks");
    let czesc = a["specyfikacja"]["sekcje"][0]["czesci"][0]["id"]
        .as_i64()
        .unwrap();
    ok(&app, "POST", &format!("/api/v1/czesci/{czesc}/wiersze"), Some(json!({
        "rodzaj": "Cokół", "material": "Aluminium testowe", "ilosc": 2, "jednostka": "mb", "kategoria_materialu_id": 9
    }))).await;
    let (kod, _) = wyslij(
        &app,
        "PUT",
        &format!("/api/v1/aneksy/{aid}/status"),
        Some(json!({"status": "podpisany"})),
    )
    .await;
    assert_eq!(
        kod,
        StatusCode::OK,
        "zmiana specyfikacji wystarcza jako treść aneksu"
    );
    // ...ale do tego aneksu już nic nie dopiszemy – kolejny aneks z kwotą i terminem
    let (kod, a2) = wyslij(&app, "POST", &format!("/api/v1/umowy/{id}/aneksy"), None).await;
    assert_eq!(kod, StatusCode::CREATED);
    let aid2 = a2["aneks"]["id"].as_i64().unwrap();
    assert_eq!(a2["aneks"]["nr"], 2);
    let a2 = ok(&app, "PUT", &format!("/api/v1/aneksy/{aid2}"), Some(json!({
        "opis_zmian": "Dodatkowa szafka wisząca", "zmiana_ceny_gr": 120_000, "nowy_termin_montazu": "2027-02-15"
    }))).await;
    assert_eq!(a2["nowa_wartosc_gr"], 4_955_200);
    assert!(a2["lista_zmian"]
        .as_array()
        .unwrap()
        .iter()
        .any(|z| z.as_str().unwrap().contains("15.02.2027")));
    let (kod, docx) = bajty(&app, &format!("/api/v1/aneksy/{aid2}/dokument/docx")).await;
    assert_eq!(kod, StatusCode::OK);
    zapisz_do_obejrzenia("aneks.docx", &docx);
    let t = tekst(&czesc_docx(&docx, "word/document.xml"));
    assert!(
        t.contains("ANEKS NR 2 DO UMOWY NR 01/2026")
            && t.contains("49 552,00")
            && t.contains("Dodatkowa szafka wisząca"),
        "{t}"
    );
    let w = ok(
        &app,
        "PUT",
        &format!("/api/v1/aneksy/{aid2}/status"),
        Some(json!({"status": "podpisany", "data": "2026-12-02"})),
    )
    .await;
    assert!(
        w["harmonogram"]["wygenerowano"].as_bool().unwrap(),
        "{}",
        w["harmonogram"]
    );
    let p = ok(&app, "GET", &format!("/api/v1/umowy/{id}"), None).await;
    assert_eq!(p["umowa"]["wartosc_brutto_gr"], 4_955_200);
    assert_eq!(p["umowa"]["montaz_do"], "2027-02-15");
    assert_eq!(
        p["umowa"]["specyfikacja_id"],
        a2["aneks"]["specyfikacja_id"]
    );
    assert_eq!(
        p["platnosci"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x["kwota_gr"].as_i64().unwrap())
            .sum::<i64>(),
        4_955_200
    );
    let zlecenie = ok(&app, "GET", &format!("/api/v1/zlecenia/{zid}"), None).await;
    assert_eq!(
        zlecenie["montaz_od"], "2027-02-15",
        "termin z aneksu przelicza harmonogram"
    );
    let materialy = ok(
        &app,
        "GET",
        &format!("/api/v1/zlecenia/{zid}/materialy"),
        None,
    )
    .await;
    assert_eq!(
        materialy.as_array().unwrap().len(),
        3,
        "okucia z aneksu nr 1: {materialy}"
    );

    // protokół odbioru → status odebrana, termin pozostałości od odbioru
    let p = ok(
        &app,
        "POST",
        &format!("/api/v1/umowy/{id}/protokol"),
        Some(json!({"data": "2027-02-19", "bez_zastrzezen": true})),
    )
    .await;
    assert_eq!(p["umowa"]["status"], "odebrana");
    let koncowa = p["platnosci"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["rodzaj"] == "koncowa")
        .unwrap()
        .clone();
    assert_eq!(koncowa["termin"], "2027-02-24", "pt. 19.02 + 3 dni robocze");
    let (kod, docx) = bajty(&app, &format!("/api/v1/umowy/{id}/protokol/docx")).await;
    assert_eq!(kod, StatusCode::OK);
    zapisz_do_obejrzenia("protokol.docx", &docx);
    let t = tekst(&czesc_docx(&docx, "word/document.xml"));
    assert!(
        t.contains("Protokół odbioru")
            && t.contains("19.02.2027")
            && t.contains("wpłacono: 14 506,00")
            && t.contains("pozostało do zapłaty: 35 046,00"),
        "{t}"
    );
    assert!(!t.contains("UMOWA NR"), "protokół bez treści umowy");
    // PDF – przez Word/LibreOffice; bez nich jasny komunikat 422
    let (kod, pdf) = bajty(&app, &format!("/api/v1/umowy/{id}/protokol/pdf")).await;
    if kod == StatusCode::OK {
        assert!(pdf.starts_with(b"%PDF"));
        zapisz_do_obejrzenia("protokol.pdf", &pdf);
    } else {
        assert_eq!(kod, StatusCode::UNPROCESSABLE_ENTITY);
        assert!(String::from_utf8_lossy(&pdf).contains("Word"));
    }
    ok(
        &app,
        "PUT",
        &format!("/api/v1/umowy/{id}/status"),
        Some(json!({"status": "zamknieta"})),
    )
    .await;
    let zlecenie = ok(&app, "GET", &format!("/api/v1/zlecenia/{zid}"), None).await;
    assert_eq!(zlecenie["etap"], "zakonczone");
    let _ = std::fs::remove_dir_all(folder);
}

#[tokio::test]
async fn umowa_od_zera_numeracja_i_anulowanie() {
    let (app, folder) = aplikacja().await;
    let k = ok(
        &app,
        "POST",
        "/api/v1/klienci",
        Some(json!({"nazwa": "Klient Drugi"})),
    )
    .await;
    let kid = k["id"].as_i64().unwrap();
    let a = ok(
        &app,
        "POST",
        "/api/v1/umowy",
        Some(json!({"klient_id": kid, "data_zawarcia": "2026-12-30"})),
    )
    .await;
    let b = ok(
        &app,
        "POST",
        "/api/v1/umowy",
        Some(json!({"klient_id": kid, "data_zawarcia": "2026-12-31"})),
    )
    .await;
    let c = ok(
        &app,
        "POST",
        "/api/v1/umowy",
        Some(json!({"klient_id": kid, "data_zawarcia": "2027-01-02"})),
    )
    .await;
    assert_eq!(a["umowa"]["numer"], "01/2026");
    assert_eq!(b["umowa"]["numer"], "02/2026");
    assert_eq!(c["umowa"]["numer"], "01/2027", "nowy rok – numeracja od 1");
    assert!(c["specyfikacja"]["sekcje"].as_array().unwrap().is_empty());
    let cid = c["umowa"]["id"].as_i64().unwrap();
    // ręczna zmiana numeru: zajęty → 422; wolny w formacie → licznik roku idzie dalej
    let (kod, v) = wyslij(
        &app,
        "PUT",
        &format!("/api/v1/umowy/{cid}"),
        Some(json!({"numer": "02/2026"})),
    )
    .await;
    assert_eq!(kod, StatusCode::UNPROCESSABLE_ENTITY, "{v}");
    assert!(v["blad"].as_str().unwrap().contains("zajęty"));
    ok(
        &app,
        "PUT",
        &format!("/api/v1/umowy/{cid}"),
        Some(json!({"numer": "15/2027"})),
    )
    .await;
    let d = ok(
        &app,
        "POST",
        "/api/v1/umowy",
        Some(json!({"klient_id": kid, "data_zawarcia": "2027-03-01"})),
    )
    .await;
    assert_eq!(d["umowa"]["numer"], "16/2027");
    // format z ustawień
    let mut u = ok(&app, "GET", "/api/v1/ustawienia/umowy", None).await;
    u["format_numeru"] = json!("UM/{RRRR}/{NNN}");
    ok(&app, "PUT", "/api/v1/ustawienia/umowy", Some(u.clone())).await;
    let e = ok(
        &app,
        "POST",
        "/api/v1/umowy",
        Some(json!({"klient_id": kid, "data_zawarcia": "2027-03-02"})),
    )
    .await;
    assert_eq!(e["umowa"]["numer"], "UM/2027/017");
    u["format_numeru"] = json!("bez numeru");
    let (kod, _) = wyslij(&app, "PUT", "/api/v1/ustawienia/umowy", Some(u)).await;
    assert_eq!(kod, StatusCode::UNPROCESSABLE_ENTITY);
    // wyjście ze szkicu wymaga kompletu danych
    let (kod, v) = wyslij(
        &app,
        "PUT",
        &format!("/api/v1/umowy/{cid}/status"),
        Some(json!({"status": "wyslana"})),
    )
    .await;
    assert_eq!(kod, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(v["blad"].as_str().unwrap().contains("wartość umowy"), "{v}");
    // szkic można usunąć; anulowanie z blokadą
    let aid = a["umowa"]["id"].as_i64().unwrap();
    let (kod, _) = wyslij(&app, "DELETE", &format!("/api/v1/umowy/{aid}"), None).await;
    assert_eq!(kod, StatusCode::NO_CONTENT);
    let bid = b["umowa"]["id"].as_i64().unwrap();
    let w = ok(
        &app,
        "PUT",
        &format!("/api/v1/umowy/{bid}/status"),
        Some(json!({"status": "anulowana", "powod": "rezygnacja"})),
    )
    .await;
    assert_eq!(w["umowa"]["status"], "anulowana");
    assert!(w["specyfikacja"]["tylko_do_odczytu"].as_bool().unwrap());
    let (kod, _) = wyslij(
        &app,
        "PUT",
        &format!("/api/v1/umowy/{bid}"),
        Some(json!({"miejscowosc": "X"})),
    )
    .await;
    assert_eq!(kod, StatusCode::UNPROCESSABLE_ENTITY);
    let lista = ok(&app, "GET", "/api/v1/umowy?status=anulowana", None).await;
    assert_eq!(lista.as_array().unwrap().len(), 1);
    let lista = ok(&app, "GET", "/api/v1/umowy?rok=2027", None).await;
    assert_eq!(lista.as_array().unwrap().len(), 3);
    // osoby kontaktowe
    let o = ok(
        &app,
        "PUT",
        &format!("/api/v1/klienci/{kid}/osoby"),
        Some(json!({"osoby": [{"imie_nazwisko": "Osoba Testowa", "telefon": "222"}]})),
    )
    .await;
    assert_eq!(o[0]["imie_nazwisko"], "Osoba Testowa");
    let _ = std::fs::remove_dir_all(folder);
}

/// Równoległe tworzenie umów na prawdziwym pliku bazy (WAL, kilka połączeń) – bez kolizji numerów.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn numeracja_rownolegla_bez_kolizji() {
    let folder = std::env::temp_dir().join(format!("robinwood-umowy-rown-{}", sufiks()));
    let pula = baza::otworz(&folder.join("robinwood.db")).await.unwrap();
    let kid: i64 = sqlx::query("INSERT INTO klienci (nazwa) VALUES ('Klient Testowy')")
        .execute(&pula)
        .await
        .unwrap()
        .last_insert_rowid();
    let mut zadania = Vec::new();
    for _ in 0..12 {
        let pula = pula.clone();
        zadania.push(tokio::spawn(async move {
            umowy::dodaj(
                &pula,
                umowy::DaneNowejUmowy {
                    klient_id: Some(kid),
                    data_zawarcia: Some("2026-11-20".into()),
                    ..Default::default()
                },
            )
            .await
            .map(|p| p.umowa.numer().to_string())
        }));
    }
    let mut numery = Vec::new();
    for z in zadania {
        numery.push(
            z.await
                .unwrap()
                .expect("bez „database is locked” i kolizji"),
        );
    }
    numery.sort();
    let oczekiwane: Vec<String> = (1..=12).map(|n| format!("{n:02}/2026")).collect();
    assert_eq!(numery, oczekiwane);
    pula.close().await;
    let _ = std::fs::remove_dir_all(folder);
}

/// Szablon „jak z Worda”: pole rozbite na przebiegi, kontrolka treści, blok warunkowy, nieznane pole.
fn szablon_uzytkownika() -> Vec<u8> {
    let body = "<w:p><w:r><w:t xml:space=\"preserve\">Umowa nr </w:t></w:r><w:r><w:rPr><w:b/></w:rPr><w:t>[nu</w:t></w:r><w:r><w:t>mer umowy]</w:t></w:r></w:p>\
        <w:p><w:r><w:t>Klient: </w:t></w:r><w:sdt><w:sdtPr><w:alias w:val=\"klient\"/><w:tag w:val=\"client\"/><w:dataBinding w:xpath=\"/x\"/></w:sdtPr><w:sdtContent><w:r><w:t>[klient]</w:t></w:r></w:sdtContent></w:sdt></w:p>\
        <w:p><w:r><w:t>[Wartość] zł, słownie: [kwota słownie]. [pole własne firmy] [uzupełnij]</w:t></w:r></w:p>\
        <w:p><w:r><w:t>[[załącznik 3]]</w:t></w:r></w:p><w:p><w:r><w:t>CENNIK AGD</w:t></w:r></w:p><w:p><w:r><w:t>[[/załącznik 3]]</w:t></w:r></w:p>\
        <w:p><w:r><w:t>[specyfikacja]</w:t></w:r></w:p>";
    let doc = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:body>{body}<w:sectPr/></w:body></w:document>"
    );
    let mut z = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let o = zip::write::SimpleFileOptions::default();
    z.start_file("[Content_Types].xml", o).unwrap();
    z.write_all(b"<?xml version=\"1.0\"?><Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"xml\" ContentType=\"application/xml\"/><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Override PartName=\"/word/document.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml\"/></Types>").unwrap();
    z.start_file("_rels/.rels", o).unwrap();
    z.write_all(b"<?xml version=\"1.0\"?><Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"word/document.xml\"/></Relationships>").unwrap();
    z.start_file("word/document.xml", o).unwrap();
    z.write_all(doc.as_bytes()).unwrap();
    z.finish().unwrap().into_inner()
}

#[tokio::test]
async fn szablon_uzytkownika_i_autoryzacja() {
    let (app, folder) = aplikacja().await;
    // bez klucza – 401 na nowych trasach
    for uri in [
        "/api/v1/umowy",
        "/api/v1/umowy/1",
        "/api/v1/umowy/1/dokument/docx",
        "/api/v1/szablony-dokumentow",
        "/api/v1/ustawienia/umowy",
        "/api/v1/aneksy/1",
        "/api/v1/klienci/1/osoby",
    ] {
        let odp = app
            .clone()
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(odp.status(), StatusCode::UNAUTHORIZED, "{uri}");
    }
    let st = ok(&app, "GET", "/api/v1/szablony-dokumentow", None).await;
    assert_eq!(st[0]["zrodlo"], "przykladowy");
    // wgranie szablonu (bajty pliku)
    let odp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/v1/szablony-dokumentow/umowa?nazwa=Moja%20umowa.docx")
                .header(header::AUTHORIZATION, format!("Bearer {KLUCZ}"))
                .body(Body::from(szablon_uzytkownika()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(odp.status(), StatusCode::OK);
    let b = odp.into_body().collect().await.unwrap().to_bytes();
    let s: Value = serde_json::from_slice(&b).unwrap();
    assert_eq!(s["zrodlo"], "wlasny");
    assert_eq!(s["meta"]["nazwa_pliku"], "Moja umowa.docx");
    assert_eq!(s["raport"]["nieznane"], json!(["pole własne firmy"]));
    assert_eq!(s["raport"]["do_uzupelnienia"], 1);
    // zły plik → 422
    let odp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/v1/szablony-dokumentow/umowa")
                .header(header::AUTHORIZATION, format!("Bearer {KLUCZ}"))
                .body(Body::from("to nie jest docx"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(odp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    // generowanie z szablonu użytkownika
    let k = ok(
        &app,
        "POST",
        "/api/v1/klienci",
        Some(json!({"nazwa": "Klient & Syn <Test>"})),
    )
    .await;
    let p = ok(
        &app,
        "POST",
        "/api/v1/umowy",
        Some(json!({"klient_id": k["id"], "data_zawarcia": "2026-11-20"})),
    )
    .await;
    let id = p["umowa"]["id"].as_i64().unwrap();
    ok(
        &app,
        "PUT",
        &format!("/api/v1/umowy/{id}"),
        Some(json!({"wartosc_brutto_gr": 2_200})),
    )
    .await;
    let pola = ok(
        &app,
        "GET",
        &format!("/api/v1/umowy/{id}/dokument/pola"),
        None,
    )
    .await;
    assert_eq!(pola["zrodlo"], "wlasny");
    assert_eq!(pola["raport"]["nieznane"], json!(["pole własne firmy"]));
    let (kod, docx) = bajty(&app, &format!("/api/v1/umowy/{id}/dokument/docx")).await;
    assert_eq!(kod, StatusCode::OK);
    zapisz_do_obejrzenia("umowa-szablon-uzytkownika.docx", &docx);
    let xml = czesc_docx(&docx, "word/document.xml");
    let t = tekst(&xml);
    assert!(t.contains("Umowa nr 01/2026"), "{t}");
    assert!(t.contains("Klient: Klient & Syn <Test>"), "{t}");
    assert!(xml.contains("Klient &amp; Syn &lt;Test&gt;"));
    assert!(
        t.contains("22,00 zł, słownie: dwadzieścia dwa złote 00/100"),
        "{t}"
    );
    assert!(t.contains("[pole własne firmy] [uzupełnij]"));
    assert!(!t.contains("CENNIK AGD"));
    assert!(!xml.contains("dataBinding"));
    // protokół z szablonu bez bloku [[załącznik 5]] → czytelny błąd
    let (kod, v) = wyslij(
        &app,
        "GET",
        &format!("/api/v1/umowy/{id}/protokol/docx"),
        None,
    )
    .await;
    assert_eq!(kod, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(
        v["blad"].as_str().unwrap().contains("[[załącznik 5]]"),
        "{v}"
    );
    // usunięcie → znów przykładowy
    let s = ok(&app, "DELETE", "/api/v1/szablony-dokumentow/umowa", None).await;
    assert_eq!(s["zrodlo"], "przykladowy");
    let (kod, b) = bajty(&app, "/api/v1/szablony-dokumentow/umowa/plik").await;
    assert_eq!(kod, StatusCode::OK);
    assert!(tekst(&czesc_docx(&b, "word/document.xml")).contains("SZABLON PRZYKŁADOWY"));
    let opisy = ok(&app, "GET", "/api/v1/szablony-dokumentow/pola", None).await;
    assert!(opisy.as_array().unwrap().len() > 50);
    let _ = std::fs::remove_dir_all(folder);
}
