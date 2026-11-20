//! Testy API na bazie w pamięci (bez sieci).

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use robinwood_serwer::{api, baza, StanApi};
use serde_json::{json, Value};
use tower::ServiceExt;

const KLUCZ: &str = "klucz-testowy";

async fn aplikacja() -> axum::Router {
    let pula = baza::otworz_w_pamieci().await.unwrap();
    api::router(StanApi::nowy(pula, KLUCZ.into(), "Test".into()))
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

async fn json_z(odp: axum::response::Response) -> Value {
    let bajty = odp.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bajty).unwrap_or(Value::Null)
}

#[tokio::test]
async fn status_bez_klucza() {
    let app = aplikacja().await;
    let odp = app
        .oneshot(Request::get("/api/v1/status").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(odp.status(), StatusCode::OK);
    let v = json_z(odp).await;
    assert_eq!(v["aplikacja"], "robinwood");
    assert_eq!(v["wersja_api"], 1);
}

#[tokio::test]
async fn brak_klucza_odrzucony() {
    let app = aplikacja().await;
    let odp = app
        .clone()
        .oneshot(Request::get("/api/v1/klienci").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(odp.status(), StatusCode::UNAUTHORIZED);

    let odp = app
        .oneshot(
            Request::get("/api/v1/klienci")
                .header(header::AUTHORIZATION, "Bearer zly")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(odp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn klienci_crud_przez_http() {
    let app = aplikacja().await;

    let odp = app
        .clone()
        .oneshot(zadanie(
            "POST",
            "/api/v1/klienci",
            Some(json!({ "nazwa": "Przykładowy Klient", "typ": "firma", "nip": "0000000000" })),
        ))
        .await
        .unwrap();
    assert_eq!(odp.status(), StatusCode::CREATED);
    let k = json_z(odp).await;
    let id = k["id"].as_i64().unwrap();
    assert_eq!(k["typ"], "firma");

    let odp = app
        .clone()
        .oneshot(zadanie(
            "PUT",
            &format!("/api/v1/klienci/{id}"),
            Some(json!({ "nazwa": "Przykładowy Klient sp. z o.o.", "typ": "firma" })),
        ))
        .await
        .unwrap();
    assert_eq!(odp.status(), StatusCode::OK);
    assert_eq!(json_z(odp).await["nazwa"], "Przykładowy Klient sp. z o.o.");

    let odp = app
        .clone()
        .oneshot(zadanie("GET", "/api/v1/klienci?szukaj=sp.%20z", None))
        .await
        .unwrap();
    assert_eq!(json_z(odp).await.as_array().unwrap().len(), 1);

    let odp = app
        .clone()
        .oneshot(zadanie(
            "POST",
            "/api/v1/klienci",
            Some(json!({ "nazwa": "" })),
        ))
        .await
        .unwrap();
    assert_eq!(odp.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let odp = app
        .clone()
        .oneshot(zadanie("DELETE", &format!("/api/v1/klienci/{id}"), None))
        .await
        .unwrap();
    assert_eq!(odp.status(), StatusCode::NO_CONTENT);

    let odp = app
        .oneshot(zadanie("GET", &format!("/api/v1/klienci/{id}"), None))
        .await
        .unwrap();
    assert_eq!(odp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn slowniki_i_kategorie() {
    let app = aplikacja().await;
    let odp = app
        .clone()
        .oneshot(zadanie("GET", "/api/v1/slowniki/rodzaj_elementu", None))
        .await
        .unwrap();
    let v = json_z(odp).await;
    assert_eq!(v[0]["wartosc"], "Korpus");

    let odp = app
        .oneshot(zadanie("GET", "/api/v1/kategorie-materialow", None))
        .await
        .unwrap();
    assert!(json_z(odp).await.as_array().unwrap().len() >= 10);
}
