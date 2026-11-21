//! Testy HTTP tras pomocniczych interfejsu kalendarza (Faza 4b): import kategorii z pliku CSV,
//! zakresy dni wolnych, opis wierszy „do potwierdzenia”. Wyłącznie dane fikcyjne.

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use robinwood_serwer::{api, baza, StanApi};
use serde_json::{json, Value};
use tower::ServiceExt;

const KLUCZ: &str = "klucz-testowy";

async fn aplikacja() -> (axum::Router, StanApi) {
    let pula = baza::otworz_w_pamieci().await.unwrap();
    let stan = StanApi::nowy(pula, KLUCZ.into(), "Test".into());
    (api::router(stan.clone()), stan)
}

async fn wyslij(
    r: &axum::Router,
    metoda: &str,
    uri: &str,
    cialo: Option<Value>,
) -> (StatusCode, Value) {
    let b = Request::builder()
        .method(metoda)
        .uri(uri)
        .header(header::AUTHORIZATION, format!("Bearer {KLUCZ}"))
        .header(header::CONTENT_TYPE, "application/json");
    let req = match cialo {
        Some(v) => b.body(Body::from(v.to_string())).unwrap(),
        None => b.body(Body::empty()).unwrap(),
    };
    let odp = r.clone().oneshot(req).await.unwrap();
    let s = odp.status();
    let bajty = odp.into_body().collect().await.unwrap().to_bytes();
    (s, serde_json::from_slice(&bajty).unwrap_or(Value::Null))
}

#[tokio::test]
async fn import_kategorii_tworzy_dostawcow_i_czasy_oraz_raportuje_pominiete() {
    let (r, _) = aplikacja().await;
    let (s, v) = wyslij(
        &r,
        "POST",
        "/api/v1/import/kategorie-materialow",
        Some(json!({"wiersze": [
            {"wiersz": 2, "kategoria": "Płyta meblowa", "przykladowe_materialy": "płyta testowa 18 mm", "dostawca": "Hurtownia Testowa", "czas_dostawy_min_dni": 5, "czas_dostawy_max_dni": 10},
            {"wiersz": 3, "kategoria": "Kategoria nowa testowa", "czas_dostawy_min_dni": 3, "czas_dostawy_max_dni": 4},
            {"wiersz": 4, "kategoria": "Bez czasu testowa", "dostawca": "hurtownia testowa"},
            {"wiersz": 5, "kategoria": "Zła testowa", "czas_dostawy_min_dni": 9, "czas_dostawy_max_dni": 2},
            {"wiersz": 6, "kategoria": "", "czas_dostawy_min_dni": 1, "czas_dostawy_max_dni": 2},
            {"wiersz": 7, "kategoria": "Połowa testowa", "czas_dostawy_min_dni": 1}
        ]})),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["kategorie_dodane"], 2);
    assert_eq!(v["kategorie_zaktualizowane"], 1);
    assert_eq!(
        v["dostawcy_dodani"], 1,
        "ta sama nazwa bez względu na wielkość liter"
    );
    assert_eq!(v["czasy_dostaw_ustawione"], 2);
    assert_eq!(v["bez_czasu_dostawy"], json!([4]));
    let pominiete: Vec<i64> = v["pominiete"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["wiersz"].as_i64().unwrap())
        .collect();
    assert_eq!(pominiete, vec![5, 6, 7]);
    assert!(v["przeliczone_zlecenia"].is_array());

    let (_, czasy) = wyslij(&r, "GET", "/api/v1/czasy-dostaw", None).await;
    let c = &czasy.as_array().unwrap()[0];
    assert_eq!(
        (
            c["kategoria_nazwa"].as_str(),
            c["dni_min"].as_i64(),
            c["dni_max"].as_i64()
        ),
        (Some("Płyta meblowa"), Some(5), Some(10))
    );
    let (_, kat) = wyslij(&r, "GET", "/api/v1/kategorie-materialow", None).await;
    let nowa = kat
        .as_array()
        .unwrap()
        .iter()
        .find(|k| k["nazwa"] == "Kategoria nowa testowa")
        .unwrap();
    assert_eq!(
        (
            nowa["dni_dostawy_min"].as_i64(),
            nowa["dni_dostawy_max"].as_i64()
        ),
        (Some(3), Some(4))
    );
    let plyta = kat
        .as_array()
        .unwrap()
        .iter()
        .find(|k| k["nazwa"] == "Płyta meblowa")
        .unwrap();
    assert!(
        plyta["domyslny_dostawca_id"].is_i64(),
        "dostawca z pliku staje się domyślnym"
    );

    // Ponowny import tego samego pliku nie tworzy duplikatów.
    let (s, v) = wyslij(
        &r,
        "POST",
        "/api/v1/import/kategorie-materialow",
        Some(json!({"wiersze": [{"kategoria": "płyta MEBLOWA", "dostawca": "Hurtownia Testowa", "czas_dostawy_min_dni": 6, "czas_dostawy_max_dni": 9}]})),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(
        (
            v["kategorie_dodane"].as_i64(),
            v["dostawcy_dodani"].as_i64()
        ),
        (Some(0), Some(0))
    );
    let (_, czasy) = wyslij(&r, "GET", "/api/v1/czasy-dostaw", None).await;
    assert_eq!(czasy.as_array().unwrap().len(), 1);
    assert_eq!(czasy[0]["dni_min"], 6);

    let (s, v) = wyslij(
        &r,
        "POST",
        "/api/v1/import/kategorie-materialow",
        Some(json!({"wiersze": [{"kategorja": "x"}]})),
    )
    .await;
    assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(v["blad"].is_string());
}

#[tokio::test]
async fn zakres_dni_wolnych_jednym_zapisem() {
    let (r, _) = aplikacja().await;
    let (s, v) = wyslij(
        &r,
        "POST",
        "/api/v1/kalendarz/dni-wolne/zakres",
        Some(json!({"od": "2027-07-05", "do": "2027-07-16", "rodzaj": "urlop", "opis": "Urlop firmowy"})),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["dni"], 12);
    let (_, lista) = wyslij(
        &r,
        "GET",
        "/api/v1/kalendarz/dni-wolne?od=2027-07-01&do=2027-07-31",
        None,
    )
    .await;
    assert_eq!(lista.as_array().unwrap().len(), 12);
    assert_eq!(lista[0]["opis"], "Urlop firmowy");

    let (s, _) = wyslij(
        &r,
        "POST",
        "/api/v1/kalendarz/dni-wolne/zakres",
        Some(json!({"od": "2027-07-10", "do": "2027-07-01"})),
    )
    .await;
    assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY);
    let (s, _) = wyslij(
        &r,
        "POST",
        "/api/v1/kalendarz/dni-wolne/zakres",
        Some(json!({"od": "2027-01-01", "do": "2028-12-31"})),
    )
    .await;
    assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY);

    let (s, v) = wyslij(
        &r,
        "POST",
        "/api/v1/kalendarz/dni-wolne/usun-zakres",
        Some(json!({"od": "2027-07-10", "do": "2027-07-31"})),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(v["dni"], 7);
}

#[tokio::test]
async fn opis_wierszy_specyfikacji() {
    let (r, stan) = aplikacja().await;
    let spec = sqlx::query("INSERT INTO specyfikacje DEFAULT VALUES")
        .execute(&stan.pula)
        .await
        .unwrap()
        .last_insert_rowid();
    let sekcja = sqlx::query("INSERT INTO sekcje_specyfikacji (specyfikacja_id, numer, nazwa) VALUES (?1, '01', 'Kuchnia testowa')")
        .bind(spec)
        .execute(&stan.pula)
        .await
        .unwrap()
        .last_insert_rowid();
    let czesc =
        sqlx::query("INSERT INTO czesci_specyfikacji (sekcja_id, nazwa) VALUES (?1, 'Słupki')")
            .bind(sekcja)
            .execute(&stan.pula)
            .await
            .unwrap()
            .last_insert_rowid();
    let w = sqlx::query("INSERT INTO wiersze_specyfikacji (czesc_id, rodzaj, material, status) VALUES (?1, 'Fronty', 'MDF testowy', 'do_potwierdzenia')")
        .bind(czesc)
        .execute(&stan.pula)
        .await
        .unwrap()
        .last_insert_rowid();
    let (s, v) = wyslij(
        &r,
        "GET",
        &format!("/api/v1/kalendarz/wiersze-specyfikacji?ids={w},999"),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v.as_array().unwrap().len(), 1);
    assert_eq!(v[0]["sekcja"], "01 Kuchnia testowa");
    assert_eq!(v[0]["czesc"], "Słupki");
    assert_eq!(v[0]["material"], "MDF testowy");
    let (s, _) = wyslij(
        &r,
        "GET",
        "/api/v1/kalendarz/wiersze-specyfikacji?ids=abc",
        None,
    )
    .await;
    assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY);
}
