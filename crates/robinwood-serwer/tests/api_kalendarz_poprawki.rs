//! Testy regresyjne poprawek silnika terminów po recenzji PR nr 2 (punkty 1–14).
//! Każdy test odtwarza jeden zgłoszony błąd. Wyłącznie dane fikcyjne.

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use robinwood_serwer::{api, baza, StanApi};
use serde_json::{json, Value};
use sqlx::SqlitePool;
use tower::ServiceExt;

const KLUCZ: &str = "klucz-testowy";

struct Aplikacja {
    router: axum::Router,
    pula: SqlitePool,
}

impl Aplikacja {
    async fn nowa() -> Self {
        let pula = baza::otworz_w_pamieci().await.unwrap();
        let stan = StanApi::nowy(pula.clone(), KLUCZ.into(), "Test".into());
        Self {
            router: api::router(stan),
            pula,
        }
    }

    async fn surowe(
        &self,
        metoda: &str,
        uri: &str,
        cialo: Option<String>,
        klucz: bool,
    ) -> (StatusCode, Value) {
        let mut b = Request::builder()
            .method(metoda)
            .uri(uri)
            .header(header::CONTENT_TYPE, "application/json");
        if klucz {
            b = b.header(header::AUTHORIZATION, format!("Bearer {KLUCZ}"));
        }
        let req = match cialo {
            Some(v) => b.body(Body::from(v)).unwrap(),
            None => b.body(Body::empty()).unwrap(),
        };
        let odp = self.router.clone().oneshot(req).await.unwrap();
        let status = odp.status();
        let bajty = odp.into_body().collect().await.unwrap().to_bytes();
        (
            status,
            serde_json::from_slice(&bajty).unwrap_or(Value::Null),
        )
    }

    async fn wyslij(&self, metoda: &str, uri: &str, cialo: Option<Value>) -> (StatusCode, Value) {
        self.surowe(metoda, uri, cialo.map(|v| v.to_string()), true)
            .await
    }

    async fn ok(&self, metoda: &str, uri: &str, cialo: Option<Value>) -> Value {
        let (s, v) = self.wyslij(metoda, uri, cialo).await;
        assert!(s.is_success(), "{metoda} {uri} → {s}: {v}");
        v
    }

    async fn ile(&self, sql: &str, id: i64) -> i64 {
        sqlx::query_scalar(sql)
            .bind(id)
            .fetch_one(&self.pula)
            .await
            .unwrap()
    }
}

async fn kategoria(app: &Aplikacja, nazwa: &str) -> i64 {
    let v = app.ok("GET", "/api/v1/kategorie-materialow", None).await;
    v.as_array()
        .unwrap()
        .iter()
        .find(|k| k["nazwa"] == nazwa)
        .unwrap()["id"]
        .as_i64()
        .unwrap()
}

fn znajdz<'a>(lista: &'a Value, pole: &str, wartosc: &str) -> &'a Value {
    lista
        .as_array()
        .unwrap()
        .iter()
        .find(|z| z[pole] == wartosc)
        .unwrap_or_else(|| panic!("brak {pole}={wartosc} w {lista}"))
}

struct Scenariusz {
    zlecenie: i64,
    klient: i64,
    fronty: i64,
    plyta: i64,
    k_fronty: i64,
    k_plyta: i64,
}

/// Zlecenie z przykładu kontrolnego (kalendarzowe, montaż 8.02.2027, warsztat 14 dni),
/// fronty MDF 18–24 (para z dostawcą A) i płyta 5–10 (widełki kategorii), harmonogram zapisany.
async fn przygotuj(app: &Aplikacja) -> Scenariusz {
    app.ok(
        "PUT",
        "/api/v1/kalendarz/ustawienia",
        Some(json!({ "liczenie_dni": "kalendarzowe" })),
    )
    .await;
    let klient = app
        .ok(
            "POST",
            "/api/v1/klienci",
            Some(json!({ "nazwa": "Klient Testowy" })),
        )
        .await["id"]
        .as_i64()
        .unwrap();
    let dostawca = app
        .ok(
            "POST",
            "/api/v1/dostawcy",
            Some(json!({ "nazwa": "Dostawca Testowy A" })),
        )
        .await["id"]
        .as_i64()
        .unwrap();
    let k_fronty = kategoria(app, "Fronty MDF frezowane").await;
    let k_plyta = kategoria(app, "Płyta meblowa").await;
    app.ok(
        "POST",
        "/api/v1/czasy-dostaw",
        Some(json!({ "kategoria_id": k_fronty, "dostawca_id": dostawca, "dni_min": 18, "dni_max": 24 })),
    )
    .await;
    app.ok(
        "PUT",
        &format!("/api/v1/kategorie-materialow/{k_plyta}/widelki"),
        Some(json!({ "dni_dostawy_min": 5, "dni_dostawy_max": 10 })),
    )
    .await;
    let zlecenie = app
        .ok(
            "POST",
            "/api/v1/zlecenia",
            Some(
                json!({ "klient_id": klient, "nazwa": "Kuchnia testowa", "montaz_od": "2027-02-08",
                         "czas_warsztat_dni": 14, "czas_montazu_dni": 3 }),
            ),
        )
        .await["id"]
        .as_i64()
        .unwrap();
    let fronty = app
        .ok(
            "POST",
            &format!("/api/v1/zlecenia/{zlecenie}/materialy"),
            Some(json!({ "kategoria_id": k_fronty, "dostawca_id": dostawca, "opis": "fronty testowe" })),
        )
        .await["id"]
        .as_i64()
        .unwrap();
    let plyta = app
        .ok(
            "POST",
            &format!("/api/v1/zlecenia/{zlecenie}/materialy"),
            Some(json!({ "kategoria_id": k_plyta })),
        )
        .await["id"]
        .as_i64()
        .unwrap();
    app.ok(
        "POST",
        &format!("/api/v1/zlecenia/{zlecenie}/harmonogram?dzis=2026-12-01"),
        None,
    )
    .await;
    Scenariusz {
        zlecenie,
        klient,
        fronty,
        plyta,
        k_fronty,
        k_plyta,
    }
}

async fn id_zadania(app: &Aplikacja, zl: i64, klucz: &str) -> i64 {
    let lista = app
        .ok(
            "GET",
            &format!("/api/v1/kalendarz/zadania?zlecenie_id={zl}&dzis=2026-12-01"),
            None,
        )
        .await;
    znajdz(&lista, "klucz", klucz)["id"].as_i64().unwrap()
}

/// Pkt 1: materiału zamówionego nie można usunąć (jego wykonane zadanie znikało przez
/// ON DELETE CASCADE); niezamówiony można, a usunięcie i zadania trafiają do dziennika.
#[tokio::test]
async fn p01_usuniecie_zamowionego_materialu_odrzucone() {
    let app = Aplikacja::nowa().await;
    let s = przygotuj(&app).await;
    let zad = id_zadania(&app, s.zlecenie, &format!("zamowienie:{}", s.fronty)).await;
    app.ok(
        "PUT",
        &format!("/api/v1/kalendarz/zadania/{zad}/wykonanie?dzis=2027-01-05"),
        Some(json!({ "wykonano": "2026-12-27" })),
    )
    .await;

    let (st, v) = app
        .wyslij(
            "DELETE",
            &format!("/api/v1/zamowienia-materialow/{}", s.fronty),
            None,
        )
        .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "{v}");
    assert!(v["blad"].as_str().unwrap().contains("cofnij"), "{v}");
    let z = app
        .ok("GET", &format!("/api/v1/kalendarz/zadania/{zad}"), None)
        .await;
    assert_eq!(z["wykonano"], "2026-12-27", "wykonane zadanie zostaje");

    // cofnięcie zamówienia jest osobną, jawną operacją; potem usunięcie jest możliwe
    let m = app
        .ok(
            "POST",
            &format!(
                "/api/v1/zamowienia-materialow/{}/cofnij-zamowienie",
                s.fronty
            ),
            None,
        )
        .await;
    assert_eq!(m["status"], "do_zamowienia");
    assert_eq!(m["data_zamowienia"], Value::Null);
    let (st, _) = app
        .wyslij(
            "DELETE",
            &format!("/api/v1/zamowienia-materialow/{}", s.fronty),
            None,
        )
        .await;
    assert_eq!(st, StatusCode::NO_CONTENT);
    assert_eq!(
        app.ile(
            "SELECT COUNT(*) FROM dziennik_zmian WHERE tabela = 'zamowienia_materialow' AND rekord_id = ?1 AND operacja = 'usuniecie'",
            s.fronty
        )
        .await,
        1
    );
    assert_eq!(
        app.ile(
            "SELECT COUNT(*) FROM dziennik_zmian WHERE tabela = 'zadania_kalendarza' AND rekord_id = ?1 AND operacja = 'usuniecie'",
            zad
        )
        .await,
        1,
        "usunięte zadanie materiału zapisane w dzienniku"
    );
}

/// Pkt 2: ponowne `wykonanie` bez pola `data_dostawy` nie zeruje dostawy; `null` czyści.
#[tokio::test]
async fn p02_wykonanie_bez_daty_dostawy_jej_nie_zeruje() {
    let app = Aplikacja::nowa().await;
    let s = przygotuj(&app).await;
    let zad = id_zadania(&app, s.zlecenie, &format!("zamowienie:{}", s.fronty)).await;
    let uri = format!("/api/v1/kalendarz/zadania/{zad}/wykonanie?dzis=2027-01-20");
    let w = app
        .ok(
            "PUT",
            &uri,
            Some(json!({ "wykonano": "2026-12-27", "data_dostawy": "2027-01-15" })),
        )
        .await;
    assert_eq!(w["zamowienie"]["status"], "dostarczone");
    let w = app
        .ok(
            "PUT",
            &uri,
            Some(json!({ "wykonano": "2026-12-27", "uwagi": "dopisana uwaga" })),
        )
        .await;
    assert_eq!(w["zamowienie"]["status"], "dostarczone", "{w}");
    assert_eq!(w["zamowienie"]["data_dostawy"], "2027-01-15");
    assert_eq!(w["uwagi"], "dopisana uwaga");
    let w = app
        .ok(
            "PUT",
            &uri,
            Some(json!({ "wykonano": "2026-12-27", "data_dostawy": null })),
        )
        .await;
    assert_eq!(w["zamowienie"]["status"], "zamowione");
    assert_eq!(w["zamowienie"]["data_dostawy"], Value::Null);
    assert_eq!(w["uwagi"], "dopisana uwaga", "pominięte uwagi bez zmian");
    // bez pola `wykonano` – błąd, a nie ciche cofnięcie
    let (st, v) = app.wyslij("PUT", &uri, Some(json!({}))).await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "{v}");
}

/// Pkt 3: `PUT /zamowienia-materialow/{id}` bez dat nie „odzamawia” materiału.
#[tokio::test]
async fn p03_zmiana_opisu_materialu_nie_cofa_zamowienia() {
    let app = Aplikacja::nowa().await;
    let s = przygotuj(&app).await;
    let zad = id_zadania(&app, s.zlecenie, &format!("zamowienie:{}", s.plyta)).await;
    app.ok(
        "PUT",
        &format!("/api/v1/kalendarz/zadania/{zad}/wykonanie?dzis=2027-01-10"),
        Some(json!({ "wykonano": "2027-01-08" })),
    )
    .await;
    let m = app
        .ok(
            "PUT",
            &format!("/api/v1/zamowienia-materialow/{}", s.plyta),
            Some(json!({ "kategoria_id": s.k_plyta, "opis": "nowy opis" })),
        )
        .await;
    assert_eq!(m["opis"], "nowy opis");
    assert_eq!(m["status"], "zamowione", "{m}");
    assert_eq!(m["data_zamowienia"], "2027-01-08");
    let z = app
        .ok("GET", &format!("/api/v1/kalendarz/zadania/{zad}"), None)
        .await;
    assert_eq!(z["wykonano"], "2027-01-08", "stan wykonania bez zmian");
    // jawne wyczyszczenie daty zamówienia w PUT – odrzucone (służy do tego cofnij-zamowienie)
    let (st, v) = app
        .wyslij(
            "PUT",
            &format!("/api/v1/zamowienia-materialow/{}", s.plyta),
            Some(json!({ "data_zamowienia": null })),
        )
        .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "{v}");
    assert!(v["blad"].as_str().unwrap().contains("cofnij-zamowienie"));
    // zmiana kategorii bez dat – zamówienie zostaje
    let m = app
        .ok(
            "PUT",
            &format!("/api/v1/zamowienia-materialow/{}", s.plyta),
            Some(json!({ "kategoria_id": s.k_fronty })),
        )
        .await;
    assert_eq!(m["data_zamowienia"], "2027-01-08");
}

/// Pkt 4: po przesunięciu montażu wcześniej data wymaganej dostawy zamówionego materiału
/// wynika z aktualnego harmonogramu, a `dostawa_zagrozona` się zapala.
#[tokio::test]
async fn p04_wymagana_dostawa_zamowionego_po_przesunieciu_montazu() {
    let app = Aplikacja::nowa().await;
    let s = przygotuj(&app).await;
    let zad = id_zadania(&app, s.zlecenie, &format!("zamowienie:{}", s.fronty)).await;
    let w = app
        .ok(
            "PUT",
            &format!("/api/v1/kalendarz/zadania/{zad}/wykonanie?dzis=2026-12-28"),
            Some(json!({ "wykonano": "2026-12-27" })),
        )
        .await;
    assert_eq!(w["zamowienie"]["data_wymaganej_dostawy"], "2027-01-18");
    assert_eq!(w["zamowienie"]["oczekiwana_dostawa"], "2027-01-17");
    assert_eq!(w["zamowienie"]["dostawa_zagrozona"], false);
    app.ok(
        "PUT",
        &format!("/api/v1/zlecenia/{}", s.zlecenie),
        Some(
            json!({ "klient_id": s.klient, "nazwa": "Kuchnia testowa", "montaz_od": "2027-02-01",
                     "czas_warsztat_dni": 14, "czas_montazu_dni": 3 }),
        ),
    )
    .await;
    let z = app
        .ok(
            "GET",
            &format!("/api/v1/kalendarz/zadania/{zad}?dzis=2026-12-28"),
            None,
        )
        .await;
    // warsztat 1.02 − 14 = 18.01, wymagana 18.01 − 7 = 11.01; dostawa ok. 17.01 → zagrożona
    assert_eq!(
        z["zamowienie"]["data_wymaganej_dostawy"], "2027-01-11",
        "{z}"
    );
    assert_eq!(z["zamowienie"]["dostawa_zagrozona"], true);
    assert_eq!(z["data_planowa"], "2026-12-28", "daty zamówienia zamrożone");
    let m = app
        .ok(
            "GET",
            &format!("/api/v1/zlecenia/{}/materialy", s.zlecenie),
            None,
        )
        .await;
    assert_eq!(m[0]["data_wymaganej_dostawy"], "2027-01-11");
    assert_eq!(m[0]["data_planowa"], "2026-12-28");
}

/// Pkt 5: nowy termin montażu przy czerwonym zamówieniu (tryb kalendarzowy) nie wypada
/// w dzień wolny – 13.02.2027 (sobota) → 15.02.2027.
#[tokio::test]
async fn p05_nowy_termin_montazu_na_dzien_roboczy() {
    let app = Aplikacja::nowa().await;
    let s = przygotuj(&app).await;
    let cz = app
        .ok(
            "GET",
            &format!(
                "/api/v1/kalendarz/zadania?zlecenie_id={}&stan=czerwony&dzis=2027-01-02",
                s.zlecenie
            ),
            None,
        )
        .await;
    let fr = znajdz(&cz, "klucz", &format!("zamowienie:{}", s.fronty));
    assert_eq!(fr["stan"]["przesuniecie_montazu_dni"], 5);
    assert_eq!(fr["stan"]["nowy_termin_montazu"], "2027-02-15", "{fr}");
}

/// Pkt 6: nieudane przeliczenie oznacza zlecenie jako `harmonogram_nieaktualny` z powodem,
/// widoczne w zleceniu, zadaniach, widokach i odpowiedzi na zmianę ustawień.
#[tokio::test]
async fn p06_nieaktualny_harmonogram_jawnie_oznaczony() {
    let app = Aplikacja::nowa().await;
    let s = przygotuj(&app).await;
    let zl = s.zlecenie;
    let z = app
        .ok(
            "PUT",
            &format!("/api/v1/zlecenia/{zl}"),
            Some(json!({ "klient_id": s.klient, "nazwa": "Kuchnia testowa",
                         "czas_warsztat_dni": 14, "czas_montazu_dni": 3 })),
        )
        .await;
    assert_eq!(z["harmonogram_przeliczony"], false);
    assert_eq!(z["harmonogram_nieaktualny"], true, "{z}");
    assert!(z["harmonogram_nieaktualny_powod"]
        .as_str()
        .unwrap()
        .contains("termin rozpoczęcia montażu"));
    let z = app.ok("GET", &format!("/api/v1/zlecenia/{zl}"), None).await;
    assert_eq!(z["harmonogram_nieaktualny"], true);

    let t = app
        .ok("GET", "/api/v1/kalendarz/tydzien?dzis=2026-12-29", None)
        .await;
    let fr = znajdz(&t["zadania"], "klucz", &format!("zamowienie:{}", s.fronty));
    assert_eq!(fr["harmonogram_nieaktualny"], true);
    assert_eq!(t["nieaktualne_harmonogramy"][0]["zlecenie_id"], zl, "{t}");
    let dz = app
        .ok(
            "GET",
            "/api/v1/kalendarz/do-zamowienia?dzis=2026-12-29",
            None,
        )
        .await;
    assert_eq!(dz["nieaktualne_harmonogramy"][0]["zlecenie_id"], zl);
    assert_eq!(
        dz["grupy"][0]["pozycje"][0]["harmonogram_nieaktualny"],
        true
    );

    let u = app
        .ok(
            "PUT",
            "/api/v1/kalendarz/ustawienia",
            Some(json!({ "prog_zbliza_sie_dni": 4 })),
        )
        .await;
    assert_eq!(u["przeliczone_zlecenia"], json!([]));
    assert_eq!(u["nieprzeliczone_zlecenia"][0]["zlecenie_id"], zl, "{u}");
    assert!(u["nieprzeliczone_zlecenia"][0]["powod"]
        .as_str()
        .unwrap()
        .contains("termin rozpoczęcia montażu"));

    // po uzupełnieniu terminu znacznik znika
    let z = app
        .ok(
            "PUT",
            &format!("/api/v1/zlecenia/{zl}"),
            Some(json!({ "klient_id": s.klient, "nazwa": "Kuchnia testowa", "montaz_od": "2027-02-08",
                         "czas_warsztat_dni": 14, "czas_montazu_dni": 3 })),
        )
        .await;
    assert_eq!(z["harmonogram_przeliczony"], true);
    assert_eq!(z["harmonogram_nieaktualny"], false);
    assert_eq!(z["harmonogram_nieaktualny_powod"], Value::Null);
}

/// Pkt 7: przeliczenie wszystkich pomija zlecenia po montażu; przeliczenie bez różnic
/// nie zapisuje „zmian”.
#[tokio::test]
async fn p07_przeliczenie_pomija_po_montazu_i_liczy_tylko_zmiany() {
    let app = Aplikacja::nowa().await;
    let s = przygotuj(&app).await;
    let zl = s.zlecenie;
    const ZMIANY: &str = "SELECT COUNT(*) FROM dziennik_zmian WHERE tabela = 'zadania_kalendarza' AND operacja = 'zmiana' AND rekord_id > ?1";
    let zmiany_przed = app.ile(ZMIANY, 0).await;
    for _ in 0..3 {
        let g = app
            .ok("POST", &format!("/api/v1/zlecenia/{zl}/harmonogram"), None)
            .await;
        assert_eq!(g["zmienione"], 0, "przeliczenie bez różnic");
        assert_eq!(g["dodane"], 0);
    }
    let zmiany_po = app.ile(ZMIANY, 0).await;
    assert_eq!(zmiany_po, zmiany_przed, "brak wpisów „zmiana” bez różnicy");

    // etap po montażu: zmiana trybu liczenia nie przepisuje dat
    for etap in ["odbior", "gwarancja"] {
        app.ok(
            "PUT",
            &format!("/api/v1/zlecenia/{zl}"),
            Some(json!({ "klient_id": s.klient, "nazwa": "Kuchnia testowa", "etap": etap, "montaz_od": "2027-02-08",
                         "czas_warsztat_dni": 14, "czas_montazu_dni": 3 })),
        )
        .await;
        let u = app
            .ok(
                "PUT",
                "/api/v1/kalendarz/ustawienia",
                Some(json!({ "liczenie_dni": "robocze" })),
            )
            .await;
        assert_eq!(u["przeliczone_zlecenia"], json!([]), "{etap}: {u}");
        let w = app
            .ok(
                "GET",
                &format!("/api/v1/kalendarz/zadania?zlecenie_id={zl}&typ=warsztat"),
                None,
            )
            .await;
        assert_eq!(w[0]["data_od"], "2027-01-25", "{etap}: daty bez zmian");
        app.ok(
            "PUT",
            "/api/v1/kalendarz/ustawienia",
            Some(json!({ "liczenie_dni": "kalendarzowe" })),
        )
        .await;
    }
}

/// Pkt 8: błąd przeliczenia po zapisie nie zostawia częściowo zapisanych danych (jedna
/// transakcja), a cofnięcie wykonania przy nieudanym przeliczeniu daje jawny znacznik.
#[tokio::test]
async fn p08_zapis_i_przeliczenie_w_jednej_transakcji() {
    let app = Aplikacja::nowa().await;
    let s = przygotuj(&app).await;
    let zl = s.zlecenie;
    // symulacja awarii bazy przy zapisie zadań
    sqlx::query(
        "CREATE TRIGGER awaria BEFORE UPDATE ON zadania_kalendarza BEGIN SELECT RAISE(ABORT, 'awaria testowa'); END",
    )
    .execute(&app.pula)
    .await
    .unwrap();
    let (st, v) = app
        .wyslij(
            "PUT",
            &format!("/api/v1/zlecenia/{zl}"),
            Some(json!({ "klient_id": s.klient, "nazwa": "Kuchnia zmieniona", "montaz_od": "2027-02-15",
                         "czas_warsztat_dni": 14, "czas_montazu_dni": 3 })),
        )
        .await;
    assert_eq!(st, StatusCode::INTERNAL_SERVER_ERROR, "{v}");
    assert!(v["blad"].is_string());
    let z = app.ok("GET", &format!("/api/v1/zlecenia/{zl}"), None).await;
    assert_eq!(
        z["montaz_od"], "2027-02-08",
        "zapis wycofany razem z przeliczeniem"
    );
    assert_eq!(z["nazwa"], "Kuchnia testowa");
    sqlx::query("DROP TRIGGER awaria")
        .execute(&app.pula)
        .await
        .unwrap();

    // cofnięcie wykonania, gdy przeliczenie się nie uda: 200 + jawne oznaczenie
    let pomiar = id_zadania(&app, zl, "pomiar").await;
    app.ok(
        "PUT",
        &format!("/api/v1/kalendarz/zadania/{pomiar}/wykonanie?dzis=2026-12-20"),
        Some(json!({ "wykonano": "2026-12-10" })),
    )
    .await;
    sqlx::query("UPDATE zlecenia SET czas_montazu_dni = NULL WHERE id = ?1")
        .bind(zl)
        .execute(&app.pula)
        .await
        .unwrap();
    let w = app
        .ok(
            "PUT",
            &format!("/api/v1/kalendarz/zadania/{pomiar}/wykonanie"),
            Some(json!({ "wykonano": null })),
        )
        .await;
    assert_eq!(w["wykonano"], Value::Null);
    assert_eq!(w["harmonogram_nieaktualny"], true, "{w}");
    assert!(w["harmonogram_nieaktualny_powod"]
        .as_str()
        .unwrap()
        .contains("czas montażu"));
}

async fn specyfikacja_do_potwierdzenia(app: &Aplikacja, s: &Scenariusz) -> i64 {
    let p = &app.pula;
    let spec = sqlx::query("INSERT INTO specyfikacje DEFAULT VALUES")
        .execute(p)
        .await
        .unwrap()
        .last_insert_rowid();
    let sekcja = sqlx::query(
        "INSERT INTO sekcje_specyfikacji (specyfikacja_id, nazwa) VALUES (?1, 'Kuchnia')",
    )
    .bind(spec)
    .execute(p)
    .await
    .unwrap()
    .last_insert_rowid();
    let czesc =
        sqlx::query("INSERT INTO czesci_specyfikacji (sekcja_id, nazwa) VALUES (?1, 'Słupki')")
            .bind(sekcja)
            .execute(p)
            .await
            .unwrap()
            .last_insert_rowid();
    let wiersz = sqlx::query(
        "INSERT INTO wiersze_specyfikacji (czesc_id, rodzaj, status, kategoria_materialu_id) VALUES (?1, 'Fronty', 'do_potwierdzenia', ?2)",
    )
    .bind(czesc)
    .bind(s.k_fronty)
    .execute(p)
    .await
    .unwrap()
    .last_insert_rowid();
    let umowa = sqlx::query(
        "INSERT INTO umowy (rok, nr_kolejny, numer, klient_id, specyfikacja_id) VALUES (2026, 1, '01/2026', ?1, ?2)",
    )
    .bind(s.klient)
    .bind(spec)
    .execute(p)
    .await
    .unwrap()
    .last_insert_rowid();
    sqlx::query("UPDATE zlecenia SET umowa_id = ?1 WHERE id = ?2")
        .bind(umowa)
        .bind(s.zlecenie)
        .execute(p)
        .await
        .unwrap();
    wiersz
}

/// Pkt 9: zamówienie materiału zablokowanego wierszem „do potwierdzenia” wymaga jawnego
/// `mimo_blokady: true`; przyjęcie mimo blokady trafia do dziennika.
#[tokio::test]
async fn p09_miekka_blokada_do_potwierdzenia() {
    let app = Aplikacja::nowa().await;
    let s = przygotuj(&app).await;
    let wiersz = specyfikacja_do_potwierdzenia(&app, &s).await;
    // po zmianie statusu wiersza moduł specyfikacji woła przeliczenie zlecenia
    let p = app
        .ok(
            "POST",
            &format!("/api/v1/zlecenia/{}/przelicz", s.zlecenie),
            None,
        )
        .await;
    assert_eq!(p["harmonogram_przeliczony"], true, "{p}");
    znajdz(
        &app.ok(
            "GET",
            &format!("/api/v1/kalendarz/zadania?zlecenie_id={}", s.zlecenie),
            None,
        )
        .await,
        "klucz",
        &format!("potwierdzenie:{}", s.fronty),
    );

    let zad = id_zadania(&app, s.zlecenie, &format!("zamowienie:{}", s.fronty)).await;
    let uri = format!("/api/v1/kalendarz/zadania/{zad}/wykonanie?dzis=2026-12-28");
    let (st, v) = app
        .wyslij("PUT", &uri, Some(json!({ "wykonano": "2026-12-27" })))
        .await;
    assert_eq!(st, StatusCode::CONFLICT, "{v}");
    assert_eq!(v["wiersze_do_potwierdzenia"], json!([wiersz]));
    assert!(v["blad"].as_str().unwrap().contains("mimo_blokady"));
    let z = app
        .ok("GET", &format!("/api/v1/kalendarz/zadania/{zad}"), None)
        .await;
    assert_eq!(z["wykonano"], Value::Null, "odmowa niczego nie zapisała");

    let w = app
        .ok(
            "PUT",
            &uri,
            Some(json!({ "wykonano": "2026-12-27", "mimo_blokady": true })),
        )
        .await;
    assert_eq!(w["zamowienie"]["status"], "zamowione");
    let opis: Option<String> = sqlx::query_scalar(
        "SELECT opis FROM dziennik_zmian WHERE tabela = 'zamowienia_materialow' AND rekord_id = ?1 AND opis IS NOT NULL",
    )
    .bind(s.fronty)
    .fetch_optional(&app.pula)
    .await
    .unwrap()
    .flatten();
    assert!(
        opis.as_deref().unwrap_or("").contains("mimo blokady"),
        "{opis:?}"
    );
}

/// Pkt 10: stare zadania `wyliczone = 1` bez klucza nie dublują się przy przeliczeniu,
/// a te bez zlecenia da się zmienić i usunąć.
#[tokio::test]
async fn p10_zadania_wyliczone_bez_klucza() {
    let app = Aplikacja::nowa().await;
    let s = przygotuj(&app).await;
    let zl = s.zlecenie;
    // usunięcie klucza symuluje wiersz sprzed migracji 0003 / wstawiony przez inny moduł
    sqlx::query(
        "UPDATE zadania_kalendarza SET klucz = NULL, data_od = '2027-01-01' WHERE zlecenie_id = ?1 AND klucz IN ('montaz', 'rysunki')",
    )
    .bind(zl)
    .execute(&app.pula)
    .await
    .unwrap();
    let g = app
        .ok("POST", &format!("/api/v1/zlecenia/{zl}/harmonogram"), None)
        .await;
    assert_eq!(g["dodane"], 0, "{g}");
    for typ in ["montaz", "rysunki"] {
        assert_eq!(
            app.ile(
                &format!("SELECT COUNT(*) FROM zadania_kalendarza WHERE zlecenie_id = ?1 AND typ = '{typ}'"),
                zl
            )
            .await,
            1,
            "{typ} bez duplikatu"
        );
    }
    let m = app
        .ok(
            "GET",
            &format!("/api/v1/kalendarz/zadania?zlecenie_id={zl}&typ=montaz"),
            None,
        )
        .await;
    assert_eq!(m[0]["klucz"], "montaz");
    assert_eq!(m[0]["data_od"], "2027-02-08", "dopasowane i przeliczone");

    let sierota = sqlx::query(
        "INSERT INTO zadania_kalendarza (typ, tytul, data_od, wyliczone) VALUES ('inne', 'Stare zadanie', '2026-12-01', 1)",
    )
    .execute(&app.pula)
    .await
    .unwrap()
    .last_insert_rowid();
    app.ok(
        "PUT",
        &format!("/api/v1/kalendarz/zadania/{sierota}"),
        Some(json!({ "typ": "inne", "tytul": "Stare zadanie (poprawione)", "data_od": "2026-12-02" })),
    )
    .await;
    let (st, v) = app
        .wyslij(
            "DELETE",
            &format!("/api/v1/kalendarz/zadania/{sierota}"),
            None,
        )
        .await;
    assert_eq!(st, StatusCode::NO_CONTENT, "{v}");
}

/// Pkt 11: zakres roku w `swieta` (bez paniki), 422 dla bezsensownych lat.
#[tokio::test]
async fn p11_swieta_zakres_roku() {
    let app = Aplikacja::nowa().await;
    for rok in ["300000", "-5", "1989", "2101"] {
        let (st, v) = app
            .wyslij("GET", &format!("/api/v1/kalendarz/swieta?rok={rok}"), None)
            .await;
        assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "rok {rok}: {v}");
        assert!(v["blad"].is_string());
    }
    let sw = app
        .ok("GET", "/api/v1/kalendarz/swieta?rok=2030", None)
        .await;
    assert_eq!(znajdz(&sw, "nazwa", "Wielkanoc")["data"], "2030-04-21");
    // daty spoza zakresu w danych wejściowych też są odrzucane, bez paniki
    let (st, _) = app
        .wyslij(
            "GET",
            "/api/v1/kalendarz/tydzien?dzis=%2B262142-12-31",
            None,
        )
        .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
}

/// Pkt 12: nieznane pola odrzucane, błędy deserializacji jako JSON `{"blad": …}`,
/// data wykonania z przyszłości odrzucana.
#[tokio::test]
async fn p12_walidacja_wejscia() {
    let app = Aplikacja::nowa().await;
    let s = przygotuj(&app).await;
    let (st, v) = app
        .wyslij(
            "POST",
            "/api/v1/zlecenia",
            Some(
                json!({ "klient_id": s.klient, "nazwa": "Literówka", "montaz_od": "2027-03-01",
                         "czas_warsztatu_dni": 10, "czas_montazu_dni": 2 }),
            ),
        )
        .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "{v}");
    assert!(
        v["blad"].as_str().unwrap().contains("czas_warsztatu_dni"),
        "{v}"
    );
    let (st, v) = app
        .surowe("POST", "/api/v1/zlecenia", Some("{nie json".into()), true)
        .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(v["blad"].is_string(), "{v}");
    let (st, v) = app.wyslij("GET", "/api/v1/zlecenia/abc", None).await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(v["blad"].is_string(), "{v}");
    let (st, v) = app
        .wyslij("GET", "/api/v1/kalendarz/zadania?dzis=wczoraj", None)
        .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(v["blad"].is_string(), "{v}");
    let (st, v) = app
        .wyslij("GET", "/api/v1/kalendarz/tydzien?dzien=2027-01-01", None)
        .await;
    assert_eq!(
        st,
        StatusCode::UNPROCESSABLE_ENTITY,
        "nieznany parametr: {v}"
    );
    let (st, v) = app
        .wyslij(
            "PUT",
            &format!("/api/v1/zamowienia-materialow/{}", s.plyta),
            Some(json!({ "opsi": "literówka" })),
        )
        .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "{v}");

    let zad = id_zadania(&app, s.zlecenie, "pomiar").await;
    let (st, v) = app
        .wyslij(
            "PUT",
            &format!("/api/v1/kalendarz/zadania/{zad}/wykonanie?dzis=2026-12-20"),
            Some(json!({ "wykonano": "2026-12-21" })),
        )
        .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "{v}");
    assert!(v["blad"].as_str().unwrap().contains("przyszłości"), "{v}");
    // trasy Klientów działają jak dotąd
    let k = app
        .ok("GET", &format!("/api/v1/klienci/{}", s.klient), None)
        .await;
    assert_eq!(k["nazwa"], "Klient Testowy");
}

/// Pkt 13: odczyt jednego zadania bez zlecenia i widok tygodnia działają na zawężonych
/// zapytaniach (zadanie po id, niewykonane do końca tygodnia).
#[tokio::test]
async fn p13_zadanie_bez_zlecenia_i_tydzien() {
    let app = Aplikacja::nowa().await;
    let r = app
        .ok(
            "POST",
            "/api/v1/kalendarz/zadania",
            Some(json!({ "typ": "inne", "tytul": "Przegląd maszyn", "data_od": "2027-01-04" })),
        )
        .await;
    let id = r["id"].as_i64().unwrap();
    for i in 0..30 {
        sqlx::query(
            "INSERT INTO zadania_kalendarza (typ, tytul, data_od, wyliczone, wykonano) VALUES ('inne', 'Historia', date('2020-01-01', ?1), 0, '2020-01-01')",
        )
        .bind(format!("+{i} days"))
        .execute(&app.pula)
        .await
        .unwrap();
    }
    let z = app
        .ok(
            "GET",
            &format!("/api/v1/kalendarz/zadania/{id}?dzis=2027-01-04"),
            None,
        )
        .await;
    assert_eq!(z["tytul"], "Przegląd maszyn");
    let t = app
        .ok("GET", "/api/v1/kalendarz/tydzien?dzis=2027-01-05", None)
        .await;
    assert_eq!(t["zadania"].as_array().unwrap().len(), 1);
    // indeksy dla zakresów dat
    let ix: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master WHERE type = 'index' AND tbl_name = 'zadania_kalendarza'",
    )
    .fetch_all(&app.pula)
    .await
    .unwrap();
    assert!(ix.iter().any(|n| n == "ix_zadania_niewykonane"), "{ix:?}");
    assert!(ix.iter().any(|n| n == "ix_zadania_koniec"), "{ix:?}");
}

/// Pkt 14: nowe trasy wymagają klucza (401).
#[tokio::test]
async fn p14_nowe_trasy_wymagaja_klucza() {
    let app = Aplikacja::nowa().await;
    for (metoda, uri) in [
        ("GET", "/api/v1/zlecenia"),
        ("POST", "/api/v1/zlecenia"),
        ("GET", "/api/v1/zlecenia/1"),
        ("PUT", "/api/v1/zlecenia/1"),
        ("GET", "/api/v1/zlecenia/1/materialy"),
        ("POST", "/api/v1/zlecenia/1/materialy"),
        ("GET", "/api/v1/zlecenia/1/harmonogram"),
        ("POST", "/api/v1/zlecenia/1/harmonogram"),
        ("POST", "/api/v1/zlecenia/1/przelicz"),
        ("PUT", "/api/v1/zamowienia-materialow/1"),
        ("DELETE", "/api/v1/zamowienia-materialow/1"),
        ("POST", "/api/v1/zamowienia-materialow/1/cofnij-zamowienie"),
        ("GET", "/api/v1/dostawcy"),
        ("POST", "/api/v1/dostawcy"),
        ("PUT", "/api/v1/dostawcy/1"),
        ("PUT", "/api/v1/kategorie-materialow/1/widelki"),
        ("GET", "/api/v1/czasy-dostaw"),
        ("POST", "/api/v1/czasy-dostaw"),
        ("PUT", "/api/v1/czasy-dostaw/1"),
        ("DELETE", "/api/v1/czasy-dostaw/1"),
        ("GET", "/api/v1/kalendarz/zadania"),
        ("POST", "/api/v1/kalendarz/zadania"),
        ("GET", "/api/v1/kalendarz/zadania/1"),
        ("PUT", "/api/v1/kalendarz/zadania/1"),
        ("DELETE", "/api/v1/kalendarz/zadania/1"),
        ("PUT", "/api/v1/kalendarz/zadania/1/wykonanie"),
        ("GET", "/api/v1/kalendarz/tydzien"),
        ("GET", "/api/v1/kalendarz/do-zamowienia"),
        ("GET", "/api/v1/kalendarz/kolizje"),
        ("GET", "/api/v1/kalendarz/statystyka-dostaw"),
        ("GET", "/api/v1/kalendarz/ustawienia"),
        ("PUT", "/api/v1/kalendarz/ustawienia"),
        ("GET", "/api/v1/kalendarz/dni-wolne"),
        ("PUT", "/api/v1/kalendarz/dni-wolne/2027-01-04"),
        ("DELETE", "/api/v1/kalendarz/dni-wolne/2027-01-04"),
        ("GET", "/api/v1/kalendarz/swieta"),
        ("POST", "/api/v1/kalendarz/przelicz"),
    ] {
        let (st, v) = app.surowe(metoda, uri, Some("{}".into()), false).await;
        assert_eq!(st, StatusCode::UNAUTHORIZED, "{metoda} {uri}");
        assert!(v["blad"].is_string(), "{metoda} {uri}");
    }
}

/// Pkt 13/14: migracje 0003+ na bazie po samej 0001 z danymi (w tym ustawieniem, które
/// 0003 też wstawia – `INSERT OR IGNORE` nie może się wywrócić ani nadpisać wartości).
#[tokio::test]
async fn p14_migracja_na_bazie_po_0001() {
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::str::FromStr;

    let katalog = std::env::temp_dir().join(format!("rw-migracje-{}", std::process::id()));
    std::fs::create_dir_all(&katalog).unwrap();
    let zrodlo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    std::fs::copy(
        zrodlo.join("0001_schemat_poczatkowy.sql"),
        katalog.join("0001_schemat_poczatkowy.sql"),
    )
    .unwrap();
    let tylko_0001 = sqlx::migrate::Migrator::new(katalog.as_path())
        .await
        .unwrap();

    let opcje = SqliteConnectOptions::from_str("sqlite::memory:")
        .unwrap()
        .foreign_keys(true);
    let pula = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opcje)
        .await
        .unwrap();
    tylko_0001.run(&pula).await.unwrap();
    std::fs::remove_dir_all(&katalog).ok();

    sqlx::query("INSERT INTO ustawienia (klucz, wartosc) VALUES ('prog_pomaranczowy_dni', '5')")
        .execute(&pula)
        .await
        .unwrap();
    sqlx::query("INSERT INTO klienci (nazwa) VALUES ('Klient Testowy')")
        .execute(&pula)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO zlecenia (klient_id, nazwa, etap, montaz_od, czas_warsztat_dni, czas_montazu_dni) \
         VALUES (1, 'Zlecenie testowe', 'umowa', '2027-03-01', 5, 1)",
    )
    .execute(&pula)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO zadania_kalendarza (zlecenie_id, typ, tytul, data_od) VALUES (1, 'montaz', 'Montaż (stary)', '2027-03-01')",
    )
    .execute(&pula)
    .await
    .unwrap();

    baza::MIGRACJE.run(&pula).await.unwrap();

    let prog: String =
        sqlx::query_scalar("SELECT wartosc FROM ustawienia WHERE klucz = 'prog_pomaranczowy_dni'")
            .fetch_one(&pula)
            .await
            .unwrap();
    assert_eq!(prog, "5", "istniejąca wartość zostaje");
    let zbliza: String =
        sqlx::query_scalar("SELECT wartosc FROM ustawienia WHERE klucz = 'prog_zbliza_sie_dni'")
            .fetch_one(&pula)
            .await
            .unwrap();
    assert_eq!(zbliza, "3");
    let (klucz, nieakt): (Option<String>, Option<String>) = sqlx::query_as(
        "SELECT t.klucz, z.harmonogram_nieaktualny_powod FROM zadania_kalendarza t JOIN zlecenia z ON z.id = t.zlecenie_id",
    )
    .fetch_one(&pula)
    .await
    .unwrap();
    assert_eq!(klucz, None);
    assert_eq!(nieakt, None);
    let opis: Option<String> = sqlx::query_scalar("SELECT opis FROM dziennik_zmian LIMIT 1")
        .fetch_optional(&pula)
        .await
        .unwrap()
        .flatten();
    assert_eq!(opis, None);
}
