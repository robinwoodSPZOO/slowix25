//! Testy integracji modułów (oferty + specyfikacja + silnik terminów) i spójności schematu.
//! Wyłącznie dane fikcyjne.

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use robinwood_serwer::{api, baza, StanApi};
use serde_json::{json, Value};
use sqlx::SqlitePool;
use tower::ServiceExt;

const KLUCZ: &str = "klucz-testowy";

async fn wyslij(
    app: &axum::Router,
    metoda: &str,
    uri: &str,
    cialo: Option<Value>,
) -> (StatusCode, Value) {
    let b = Request::builder()
        .method(metoda)
        .uri(uri)
        .header(header::AUTHORIZATION, format!("Bearer {KLUCZ}"))
        .header(header::CONTENT_TYPE, "application/json");
    let r = match cialo {
        Some(v) => b.body(Body::from(v.to_string())).unwrap(),
        None => b.body(Body::empty()).unwrap(),
    };
    let odp = app.clone().oneshot(r).await.unwrap();
    let kod = odp.status();
    let bajty = odp.into_body().collect().await.unwrap().to_bytes();
    (kod, serde_json::from_slice(&bajty).unwrap_or(Value::Null))
}

async fn ok(app: &axum::Router, metoda: &str, uri: &str, cialo: Option<Value>) -> Value {
    let (kod, v) = wyslij(app, metoda, uri, cialo).await;
    assert!(kod.is_success(), "{metoda} {uri} → {kod}: {v}");
    v
}

fn folder(n: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "robinwood-int-{n}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

type KluczObcy = (
    i64,
    i64,
    String,
    String,
    Option<String>,
    String,
    String,
    String,
);

/// Wszystkie migracje (0001–000N) przechodzą na pustej bazie, numeracja jest rosnąca
/// i jednoznaczna, a schemat jest spójny (klucze obce, integralność).
#[tokio::test]
async fn migracje_na_pustej_bazie_i_spojny_schemat() {
    let f = folder("migracje");
    let pula = baza::otworz(&f.join("robinwood.db")).await.unwrap();
    let wersje: Vec<i64> =
        sqlx::query_scalar("SELECT version FROM _sqlx_migrations ORDER BY version")
            .fetch_all(&pula)
            .await
            .unwrap();
    let w_kodzie: Vec<i64> = baza::MIGRACJE.iter().map(|m| m.version).collect();
    assert_eq!(wersje, w_kodzie, "zastosowano wszystkie migracje");
    assert_eq!(
        wersje,
        (1..=wersje.len() as i64).collect::<Vec<_>>(),
        "numeracja 0001…000N bez luk i powtórzeń"
    );
    assert!(wersje.len() >= 5);
    let fk: Vec<(String, i64, String, i64)> = sqlx::query_as("PRAGMA foreign_key_check")
        .fetch_all(&pula)
        .await
        .unwrap();
    assert!(fk.is_empty(), "naruszenia kluczy obcych: {fk:?}");
    let integralnosc: String = sqlx::query_scalar("PRAGMA integrity_check")
        .fetch_one(&pula)
        .await
        .unwrap();
    assert_eq!(integralnosc, "ok");
    // Każdy klucz obcy wskazuje istniejącą tabelę i kolumnę.
    let tabele: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
    )
    .fetch_all(&pula)
    .await
    .unwrap();
    for t in &tabele {
        let fks: Vec<KluczObcy> = sqlx::query_as(&format!("PRAGMA foreign_key_list('{t}')"))
            .fetch_all(&pula)
            .await
            .unwrap();
        for (_, _, cel, od, do_, ..) in fks {
            assert!(
                tabele.contains(&cel),
                "{t}.{od} → nieistniejąca tabela {cel}"
            );
            let kolumny: Vec<(i64, String, String, i64, Option<String>, i64)> =
                sqlx::query_as(&format!("PRAGMA table_info('{cel}')"))
                    .fetch_all(&pula)
                    .await
                    .unwrap();
            let kol = do_.unwrap_or_else(|| "id".into());
            assert!(
                kolumny.iter().any(|k| k.1 == kol),
                "{t}.{od} → {cel}.{kol} nie istnieje"
            );
        }
    }
    // Kolumny dodane przez obie gałęzie istnieją.
    for (t, k) in [
        ("wiersze_specyfikacji", "wersja"),
        ("wersje_oferty", "migawka"),
        ("zadania_kalendarza", "klucz"),
        ("zlecenia", "harmonogram_nieaktualny_powod"),
    ] {
        let kolumny: Vec<(i64, String, String, i64, Option<String>, i64)> =
            sqlx::query_as(&format!("PRAGMA table_info('{t}')"))
                .fetch_all(&pula)
                .await
                .unwrap();
        assert!(kolumny.iter().any(|x| x.1 == k), "brak {t}.{k}");
    }
    pula.close().await;
    let _ = std::fs::remove_dir_all(f);
}

struct Zlecenie {
    zlecenie: i64,
    material: i64,
    wiersz: i64,
}

/// Klient, kategoria z widełkami, zlecenie z materiałem i harmonogramem, specyfikacja umowy
/// zlecenia z wierszem tej kategorii (umowa – wpis SQL, Generator Umów jeszcze nie istnieje).
async fn zlecenie_z_umowa(app: &axum::Router, pula: &SqlitePool) -> Zlecenie {
    let k = ok(
        app,
        "POST",
        "/api/v1/klienci",
        Some(json!({ "nazwa": "Klient Testowy" })),
    )
    .await["id"]
        .as_i64()
        .unwrap();
    let kat = ok(app, "GET", "/api/v1/kategorie-materialow", None).await[0]["id"]
        .as_i64()
        .unwrap();
    ok(
        app,
        "PUT",
        &format!("/api/v1/kategorie-materialow/{kat}/widelki"),
        Some(json!({ "dni_dostawy_min": 5, "dni_dostawy_max": 10 })),
    )
    .await;
    let z = ok(app, "POST", "/api/v1/zlecenia", Some(json!({ "klient_id": k, "nazwa": "Kuchnia testowa", "montaz_od": "2027-03-08", "czas_warsztat_dni": 10, "czas_montazu_dni": 2 }))).await["id"]
        .as_i64()
        .unwrap();
    let m = ok(
        app,
        "POST",
        &format!("/api/v1/zlecenia/{z}/materialy"),
        Some(json!({ "kategoria_id": kat })),
    )
    .await["id"]
        .as_i64()
        .unwrap();
    ok(
        app,
        "POST",
        &format!("/api/v1/zlecenia/{z}/harmonogram?dzis=2027-01-04"),
        None,
    )
    .await;
    let spec = sqlx::query("INSERT INTO specyfikacje DEFAULT VALUES")
        .execute(pula)
        .await
        .unwrap()
        .last_insert_rowid();
    let umowa = sqlx::query("INSERT INTO umowy (rok, nr_kolejny, numer, klient_id, specyfikacja_id) VALUES (2027, 1, '01/2027', ?1, ?2)")
        .bind(k)
        .bind(spec)
        .execute(pula)
        .await
        .unwrap()
        .last_insert_rowid();
    sqlx::query("UPDATE zlecenia SET umowa_id = ?1 WHERE id = ?2")
        .bind(umowa)
        .bind(z)
        .execute(pula)
        .await
        .unwrap();
    let s = ok(
        app,
        "POST",
        &format!("/api/v1/specyfikacje/{spec}/sekcje"),
        Some(json!({ "nazwa": "Kuchnia" })),
    )
    .await["id"]
        .as_i64()
        .unwrap();
    let c = ok(
        app,
        "POST",
        &format!("/api/v1/sekcje/{s}/czesci"),
        Some(json!({ "nazwa": "Słupki" })),
    )
    .await["id"]
        .as_i64()
        .unwrap();
    let w = ok(
        app,
        "POST",
        &format!("/api/v1/czesci/{c}/wiersze"),
        Some(json!({ "rodzaj": "Fronty", "kategoria_materialu_id": kat })),
    )
    .await;
    assert_eq!(w["specyfikacja"]["wlasciciel"]["rodzaj"], "umowa");
    Zlecenie {
        zlecenie: z,
        material: m,
        wiersz: w["id"].as_i64().unwrap(),
    }
}

async fn klucze_zadan(app: &axum::Router, zlecenie: i64) -> Vec<String> {
    ok(
        app,
        "GET",
        &format!("/api/v1/kalendarz/zadania?zlecenie_id={zlecenie}"),
        None,
    )
    .await
    .as_array()
    .unwrap()
    .iter()
    .filter_map(|z| z["klucz"].as_str().map(String::from))
    .collect()
}

/// Zmiana statusu wiersza specyfikacji z/na „do potwierdzenia” przelicza harmonogram zlecenia
/// (blokada zamówienia i zadanie „potwierdzenie” pojawiają się i znikają bez ręcznego przeliczania).
#[tokio::test]
async fn status_wiersza_specyfikacji_przelicza_harmonogram() {
    let pula = baza::otworz_w_pamieci().await.unwrap();
    let f = folder("status");
    let app = api::router(
        StanApi::nowy(pula.clone(), KLUCZ.into(), "Test".into()).z_folderem_danych(f.clone()),
    );
    let z = zlecenie_z_umowa(&app, &pula).await;
    let klucz = format!("potwierdzenie:{}", z.material);
    assert!(!klucze_zadan(&app, z.zlecenie).await.contains(&klucz));

    ok(
        &app,
        "PUT",
        &format!("/api/v1/wiersze/{}", z.wiersz),
        Some(json!({ "status": "do_potwierdzenia" })),
    )
    .await;
    assert!(
        klucze_zadan(&app, z.zlecenie).await.contains(&klucz),
        "zmiana na „do potwierdzenia” przelicza zlecenie"
    );

    // inne pole wiersza – bez zmian w harmonogramie
    ok(
        &app,
        "PUT",
        &format!("/api/v1/wiersze/{}", z.wiersz),
        Some(json!({ "opis": "Biały mat" })),
    )
    .await;
    assert!(klucze_zadan(&app, z.zlecenie).await.contains(&klucz));

    ok(
        &app,
        "PUT",
        &format!("/api/v1/wiersze/{}", z.wiersz),
        Some(json!({ "status": "ustalone" })),
    )
    .await;
    assert!(
        !klucze_zadan(&app, z.zlecenie).await.contains(&klucz),
        "zmiana z „do potwierdzenia” zdejmuje blokadę"
    );

    // dodanie i usunięcie wiersza „do potwierdzenia”
    let c: i64 = sqlx::query_scalar("SELECT czesc_id FROM wiersze_specyfikacji WHERE id = ?1")
        .bind(z.wiersz)
        .fetch_one(&pula)
        .await
        .unwrap();
    let kat: i64 =
        sqlx::query_scalar("SELECT kategoria_materialu_id FROM wiersze_specyfikacji WHERE id = ?1")
            .bind(z.wiersz)
            .fetch_one(&pula)
            .await
            .unwrap();
    let w2 = ok(&app, "POST", &format!("/api/v1/czesci/{c}/wiersze"), Some(json!({ "rodzaj": "Blat", "status": "do_potwierdzenia", "kategoria_materialu_id": kat }))).await["id"]
        .as_i64()
        .unwrap();
    assert!(klucze_zadan(&app, z.zlecenie).await.contains(&klucz));
    ok(&app, "DELETE", &format!("/api/v1/wiersze/{w2}"), None).await;
    assert!(!klucze_zadan(&app, z.zlecenie).await.contains(&klucz));
    let _ = std::fs::remove_dir_all(f);
}

/// Równoczesne zapisy ofert, specyfikacji i kalendarza na prawdziwym pliku bazy.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn rownolegle_zapisy_oferty_i_kalendarz() {
    let f = folder("rownolegle");
    let pula = baza::otworz(&f.join("robinwood.db")).await.unwrap();
    let app = api::router(
        StanApi::nowy(pula.clone(), KLUCZ.into(), "Test".into()).z_folderem_danych(f.clone()),
    );
    let z = zlecenie_z_umowa(&app, &pula).await;
    let k = ok(
        &app,
        "POST",
        "/api/v1/klienci",
        Some(json!({ "nazwa": "Klient Równoległy" })),
    )
    .await["id"]
        .clone();
    let kat = ok(&app, "GET", "/api/v1/kategorie-materialow", None).await[0]["id"]
        .as_i64()
        .unwrap();
    let mut zadania = vec![];
    for i in 0..10 {
        let app = app.clone();
        let k = k.clone();
        let zl = z.zlecenie;
        let w = z.wiersz;
        zadania.push(tokio::spawn(async move {
            vec![
                wyslij(&app, "POST", "/api/v1/oferty", Some(json!({ "klient_id": k }))).await.0,
                wyslij(&app, "POST", &format!("/api/v1/zlecenia/{zl}/materialy"), Some(json!({ "kategoria_id": kat }))).await.0,
                wyslij(&app, "POST", &format!("/api/v1/zlecenia/{zl}/przelicz"), None).await.0,
                wyslij(&app, "PUT", &format!("/api/v1/wiersze/{w}"), Some(json!({ "status": if i % 2 == 0 { "do_potwierdzenia" } else { "ustalone" } }))).await.0,
                wyslij(&app, "PUT", &format!("/api/v1/kalendarz/dni-wolne/2027-02-{:02}", i + 1), Some(json!({ "opis": "Dzień wolny testowy" }))).await.0,
                wyslij(&app, "POST", "/api/v1/kalendarz/przelicz", None).await.0,
            ]
        }));
    }
    let mut kody = vec![];
    for zad in zadania {
        kody.extend(zad.await.unwrap());
    }
    let bledy: Vec<_> = kody.iter().filter(|k| !k.is_success()).collect();
    assert!(bledy.is_empty(), "błędy równoczesnych zapisów: {bledy:?}");
    pula.close().await;
    let _ = std::fs::remove_dir_all(f);
}
