//! Testy HTTP silnika terminów na bazie w pamięci. Wyłącznie dane fikcyjne.

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use robinwood_serwer::{api, baza, StanApi};
use serde_json::{json, Value};
use tower::ServiceExt;

const KLUCZ: &str = "klucz-testowy";

struct Aplikacja {
    router: axum::Router,
    stan: StanApi,
}

impl Aplikacja {
    async fn nowa() -> Self {
        let pula = baza::otworz_w_pamieci().await.unwrap();
        let stan = StanApi::nowy(pula, KLUCZ.into(), "Test".into());
        Self {
            router: api::router(stan.clone()),
            stan,
        }
    }

    async fn wyslij(&self, metoda: &str, uri: &str, cialo: Option<Value>) -> (StatusCode, Value) {
        let b = Request::builder()
            .method(metoda)
            .uri(uri)
            .header(header::AUTHORIZATION, format!("Bearer {KLUCZ}"))
            .header(header::CONTENT_TYPE, "application/json");
        let req = match cialo {
            Some(v) => b.body(Body::from(v.to_string())).unwrap(),
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

    async fn ok(&self, metoda: &str, uri: &str, cialo: Option<Value>) -> Value {
        let (s, v) = self.wyslij(metoda, uri, cialo).await;
        assert!(s.is_success(), "{metoda} {uri} → {s}: {v}");
        v
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
    fronty: i64,
    plyta: i64,
    tapicerka: i64,
    dostawca_a: i64,
    klient: i64,
}

/// Ustawienia kalendarzowe, dostawcy, widełki, zlecenie z przykładu kontrolnego i trzy materiały.
async fn przygotuj(app: &Aplikacja) -> Scenariusz {
    let v = app
        .ok(
            "PUT",
            "/api/v1/kalendarz/ustawienia",
            Some(json!({ "liczenie_dni": "kalendarzowe" })),
        )
        .await;
    assert_eq!(v["liczenie_dni"], "kalendarzowe");
    assert_eq!(v["odstep_materialy_dni"], 7);

    let klient = app
        .ok(
            "POST",
            "/api/v1/klienci",
            Some(json!({ "nazwa": "Klient Testowy" })),
        )
        .await["id"]
        .as_i64()
        .unwrap();
    let dostawca_a = app
        .ok(
            "POST",
            "/api/v1/dostawcy",
            Some(json!({ "nazwa": "Dostawca Testowy A" })),
        )
        .await["id"]
        .as_i64()
        .unwrap();
    let dostawca_b = app
        .ok(
            "POST",
            "/api/v1/dostawcy",
            Some(json!({ "nazwa": "Dostawca Testowy B" })),
        )
        .await["id"]
        .as_i64()
        .unwrap();
    let k_fronty = kategoria(app, "Fronty MDF frezowane").await;
    let k_plyta = kategoria(app, "Płyta meblowa").await;
    let k_tapicerka = kategoria(app, "Tapicerka").await;

    // fronty: para kategoria + dostawca A; płyta: widełki kategorii i domyślny dostawca B
    let c = app
        .ok(
            "POST",
            "/api/v1/czasy-dostaw",
            Some(json!({ "kategoria_id": k_fronty, "dostawca_id": dostawca_a, "dni_min": 18, "dni_max": 24 })),
        )
        .await;
    assert_eq!(c["dostawca_nazwa"], "Dostawca Testowy A");
    app.ok(
        "PUT",
        &format!("/api/v1/kategorie-materialow/{k_plyta}/widelki"),
        Some(json!({ "dni_dostawy_min": 5, "dni_dostawy_max": 10, "domyslny_dostawca_id": dostawca_b })),
    )
    .await;

    let z = app
        .ok(
            "POST",
            "/api/v1/zlecenia",
            Some(
                json!({ "klient_id": klient, "nazwa": "Kuchnia testowa", "montaz_od": "2027-02-08",
                         "czas_warsztat_dni": 14, "czas_montazu_dni": 3 }),
            ),
        )
        .await;
    let zlecenie = z["id"].as_i64().unwrap();
    let mut ids = Vec::new();
    for (kat, dost) in [
        (k_fronty, Some(dostawca_a)),
        (k_plyta, None),
        (k_tapicerka, None),
    ] {
        let m = app
            .ok(
                "POST",
                &format!("/api/v1/zlecenia/{zlecenie}/materialy"),
                Some(
                    json!({ "kategoria_id": kat, "dostawca_id": dost, "opis": "pozycja testowa" }),
                ),
            )
            .await;
        ids.push(m["id"].as_i64().unwrap());
    }
    Scenariusz {
        zlecenie,
        fronty: ids[0],
        plyta: ids[1],
        tapicerka: ids[2],
        dostawca_a,
        klient,
    }
}

#[tokio::test]
async fn pelny_scenariusz_harmonogramu() {
    let app = Aplikacja::nowa().await;
    let mut zdarzenia = app.stan.zdarzenia.subskrybuj();
    let s = przygotuj(&app).await;
    let zl = s.zlecenie;

    // podgląd bez zapisu
    let h = app
        .ok("GET", &format!("/api/v1/zlecenia/{zl}/harmonogram"), None)
        .await;
    assert_eq!(h["warsztat_od"], "2027-01-25");
    let (_, lista) = app
        .wyslij(
            "GET",
            &format!("/api/v1/kalendarz/zadania?zlecenie_id={zl}"),
            None,
        )
        .await;
    assert_eq!(lista.as_array().unwrap().len(), 0);

    // generowanie i zapis
    let g = app
        .ok(
            "POST",
            &format!("/api/v1/zlecenia/{zl}/harmonogram?dzis=2026-12-01"),
            None,
        )
        .await;
    let h = &g["harmonogram"];
    assert_eq!(h["liczenie_dni"], "kalendarzowe");
    assert_eq!(h["warsztat_do"], "2027-02-05");
    assert_eq!(h["data_wymaganej_dostawy"], "2027-01-18");
    assert_eq!(h["rysunki_do"], "2026-12-21");
    assert_eq!(h["pomiar_do"], "2026-12-14");
    let fronty = &h["materialy"][0];
    assert_eq!(fronty["zrodlo_widelek"], "kategoria_dostawca");
    assert_eq!(fronty["data_sweet_spot"], "2026-12-23");
    assert_eq!(fronty["data_planowa"], "2026-12-28");
    assert_eq!(fronty["data_krytyczna"], "2026-12-31");
    assert_eq!(h["materialy"][1]["zrodlo_widelek"], "kategoria");
    assert_eq!(h["materialy"][1]["data_planowa"], "2027-01-11");
    assert_eq!(h["materialy"][2]["zrodlo_widelek"], "brak");
    assert!(h["ostrzezenia"]
        .as_array()
        .unwrap()
        .iter()
        .any(|o| o["kod"] == "brak_widelek" && o["material_id"] == s.tapicerka));
    assert_eq!(g["dodane"], 8); // montaż, warsztat, 2 zamówienia, rysunki, pomiar, 2 pomocnicze
    assert_eq!(g["zadania_zapisane"].as_array().unwrap().len(), 8);

    // migawka terminów w materiale
    let m = app
        .ok("GET", &format!("/api/v1/zlecenia/{zl}/materialy"), None)
        .await;
    assert_eq!(m[1]["data_planowa"], "2027-01-11");
    assert_eq!(m[1]["dostawca_nazwa"], "Dostawca Testowy B");
    assert_eq!(m[1]["dni_srednia"], 7);

    // zdarzenia WebSocket
    let mut zasoby = Vec::new();
    while let Ok(z) = zdarzenia.try_recv() {
        zasoby.push((z.zasob, z.id));
    }
    assert!(zasoby.contains(&("zadania_kalendarza", zl)), "{zasoby:?}");
    assert!(zasoby.contains(&("zlecenia", zl)));

    // odczyt w zakresie z filtrami i stanami (dziś = 29.12.2026)
    let z = app
        .ok(
            "GET",
            "/api/v1/kalendarz/zadania?od=2026-12-01&do=2026-12-31&typ=zamowienie&dzis=2026-12-29",
            None,
        )
        .await;
    assert_eq!(z.as_array().unwrap().len(), 1);
    assert_eq!(z[0]["stan"]["stan"], "pomaranczowy");
    assert_eq!(z[0]["termin"], "2026-12-28");
    assert_eq!(z[0]["zamowienie"]["dostawca_nazwa"], "Dostawca Testowy A");
    assert_eq!(z[0]["zamowienie"]["zablokowane"], false);

    let cz = app
        .ok(
            "GET",
            &format!("/api/v1/kalendarz/zadania?zlecenie_id={zl}&stan=czerwony&dzis=2027-01-02"),
            None,
        )
        .await;
    let fr = znajdz(&cz, "klucz", &format!("zamowienie:{}", s.fronty));
    // zamówione 2.01 + 21 = 23.01; wymagana 18.01 → 5 dni → montaż 13.02
    assert_eq!(fr["stan"]["przesuniecie_montazu_dni"], 5);
    assert_eq!(fr["stan"]["nowy_termin_montazu"], "2027-02-13");
    znajdz(&cz, "klucz", "rysunki");
    znajdz(&cz, "klucz", "pomiar");
    let (st, _) = app
        .wyslij("GET", "/api/v1/kalendarz/zadania?stan=fioletowy", None)
        .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);

    // tydzień 28.12.2026–3.01.2027: zamówienie frontów (okno), zaległe rysunki i pomiar
    let t = app
        .ok("GET", "/api/v1/kalendarz/tydzien?dzis=2026-12-29", None)
        .await;
    assert_eq!(t["od"], "2026-12-28");
    assert_eq!(t["do"], "2027-01-03");
    let tz = &t["zadania"];
    znajdz(tz, "klucz", &format!("zamowienie:{}", s.fronty));
    assert_eq!(znajdz(tz, "klucz", "rysunki")["stan"]["stan"], "czerwony");
    assert!(!tz
        .as_array()
        .unwrap()
        .iter()
        .any(|z| z["klucz"] == "montaz"));

    // do zamówienia, grupy wg dostawcy
    let dz = app
        .ok(
            "GET",
            "/api/v1/kalendarz/do-zamowienia?dzis=2026-12-29",
            None,
        )
        .await;
    let grupy = dz["grupy"].as_array().unwrap();
    assert_eq!(grupy.len(), 2);
    assert_eq!(grupy[0]["dostawca_nazwa"], "Dostawca Testowy A");
    assert_eq!(grupy[0]["stan"], "pomaranczowy");
    assert_eq!(grupy[1]["dostawca_nazwa"], "Dostawca Testowy B");
    assert_eq!(dz["bez_terminow"][0]["id"], s.tapicerka);

    // wykonanie: pomiar i zamówienie frontów (z datą zamówienia, potem dostawy)
    let pomiar_id = znajdz(&cz, "klucz", "pomiar")["id"].as_i64().unwrap();
    let w = app
        .ok(
            "PUT",
            &format!("/api/v1/kalendarz/zadania/{pomiar_id}/wykonanie"),
            Some(json!({ "wykonano": "2026-12-10" })),
        )
        .await;
    assert_eq!(w["stan"]["stan"], "wykonane");
    let fr_id = fr["id"].as_i64().unwrap();
    let w = app
        .ok(
            "PUT",
            &format!("/api/v1/kalendarz/zadania/{fr_id}/wykonanie"),
            Some(json!({ "wykonano": "2026-12-27" })),
        )
        .await;
    assert_eq!(w["zamowienie"]["status"], "zamowione");
    assert_eq!(w["zamowienie"]["data_zamowienia"], "2026-12-27");
    assert_eq!(w["zamowienie"]["oczekiwana_dostawa"], "2027-01-17");
    assert_eq!(w["zamowienie"]["dostawa_zagrozona"], false);
    let w = app
        .ok(
            "PUT",
            &format!("/api/v1/kalendarz/zadania/{fr_id}/wykonanie"),
            Some(json!({ "wykonano": "2026-12-27", "data_dostawy": "2027-01-15" })),
        )
        .await;
    assert_eq!(w["zamowienie"]["status"], "dostarczone");
    let (st, _) = app
        .wyslij(
            "PUT",
            &format!("/api/v1/kalendarz/zadania/{pomiar_id}/wykonanie"),
            Some(json!({ "wykonano": "2026-12-10", "data_dostawy": "2027-01-15" })),
        )
        .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);

    // statystyka rzeczywistych dostaw: 27.12 → 15.01 = 19 dni kalendarzowych
    let stat = app
        .ok("GET", "/api/v1/kalendarz/statystyka-dostaw", None)
        .await;
    assert_eq!(stat[0]["dostawca_id"], s.dostawca_a);
    assert_eq!(stat[0]["statystyka"]["min"], 19);
    assert_eq!(stat[0]["statystyka"]["srednia"], 19.0);
    assert_eq!(stat[0]["aktualne_widelki"]["max"], 24);
    assert_eq!(stat[0]["sugerowana_korekta"], true);
    let czasy = app.ok("GET", "/api/v1/czasy-dostaw", None).await;
    assert_eq!(czasy[0]["dni_srednia_rzeczywista"], 19.0);
    assert_eq!(
        czasy[0]["dni_max"], 24,
        "widełki nie są nadpisywane automatycznie"
    );

    // zmiana terminu montażu → przeliczenie tylko niewykonanych
    let z = app
        .ok(
            "PUT",
            &format!("/api/v1/zlecenia/{zl}"),
            Some(json!({ "klient_id": s.klient, "nazwa": "Kuchnia testowa", "montaz_od": "2027-02-15",
                         "czas_warsztat_dni": 14, "czas_montazu_dni": 3 })),
        )
        .await;
    assert_eq!(z["harmonogram_przeliczony"], true);
    let po = app
        .ok(
            "GET",
            &format!("/api/v1/kalendarz/zadania?zlecenie_id={zl}&dzis=2026-12-01"),
            None,
        )
        .await;
    assert_eq!(znajdz(&po, "klucz", "montaz")["data_od"], "2027-02-15");
    assert_eq!(
        znajdz(&po, "klucz", "pomiar")["data_od"],
        "2026-12-14",
        "wykonany pomiar zamrożony"
    );
    let fr_po = znajdz(&po, "klucz", &format!("zamowienie:{}", s.fronty));
    assert_eq!(
        fr_po["data_planowa"], "2026-12-28",
        "wykonane zamówienie zamrożone"
    );
    assert_eq!(fr_po["id"], fr_id);
    // płyta: wymagana 25.01 − 7 = 18.01; rysunki od najwcześniejszej planowej (fronty: 25.01 − 21 = 4.01) − 7
    assert_eq!(
        znajdz(&po, "klucz", &format!("zamowienie:{}", s.plyta))["data_planowa"],
        "2027-01-18"
    );
    assert_eq!(znajdz(&po, "klucz", "rysunki")["data_od"], "2026-12-28");

    // cofnięcie wykonania pomiaru → wraca do przeliczania (rysunki 28.12 − 7 = 21.12)
    app.ok(
        "PUT",
        &format!("/api/v1/kalendarz/zadania/{pomiar_id}/wykonanie"),
        Some(json!({ "wykonano": null })),
    )
    .await;
    let p = app
        .ok(
            "GET",
            &format!("/api/v1/kalendarz/zadania/{pomiar_id}?dzis=2026-12-01"),
            None,
        )
        .await;
    assert_eq!(p["data_od"], "2026-12-21");
    assert_eq!(p["wykonano"], Value::Null);

    // dzień wolny użytkownika → przeliczenie wszystkich (21.12 wolne → pomiar 18.12)
    let dw = app
        .ok(
            "PUT",
            "/api/v1/kalendarz/dni-wolne/2026-12-21",
            Some(json!({ "rodzaj": "urlop", "opis": "test" })),
        )
        .await;
    assert_eq!(dw["rodzaj"], "urlop");
    let p = app
        .ok(
            "GET",
            &format!("/api/v1/kalendarz/zadania/{pomiar_id}"),
            None,
        )
        .await;
    assert_eq!(p["data_od"], "2026-12-18");
    assert_eq!(
        app.ok("GET", "/api/v1/kalendarz/dni-wolne", None)
            .await
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let (st, _) = app
        .wyslij("DELETE", "/api/v1/kalendarz/dni-wolne/2026-12-21", None)
        .await;
    assert_eq!(st, StatusCode::NO_CONTENT);
    let (st, _) = app
        .wyslij(
            "PUT",
            "/api/v1/kalendarz/dni-wolne/2026-12-22",
            Some(json!({ "rodzaj": "wakacje" })),
        )
        .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);

    // usunięcie materiału usuwa jego niewykonane zadanie
    let (st, _) = app
        .wyslij(
            "DELETE",
            &format!("/api/v1/zamowienia-materialow/{}", s.plyta),
            None,
        )
        .await;
    assert_eq!(st, StatusCode::NO_CONTENT);
    let po = app
        .ok(
            "GET",
            &format!("/api/v1/kalendarz/zadania?zlecenie_id={zl}"),
            None,
        )
        .await;
    assert!(!po
        .as_array()
        .unwrap()
        .iter()
        .any(|z| z["klucz"] == format!("zamowienie:{}", s.plyta)));
}

#[tokio::test]
async fn kolizje_zadania_reczne_i_bledy() {
    let app = Aplikacja::nowa().await;
    let s = przygotuj(&app).await;
    app.ok(
        "POST",
        &format!("/api/v1/zlecenia/{}/harmonogram", s.zlecenie),
        None,
    )
    .await;

    // drugie zlecenie bez materiałów, montaż 10.02 (nachodzi na 8–10.02)
    let z2 = app
        .ok(
            "POST",
            "/api/v1/zlecenia",
            Some(
                json!({ "klient_id": s.klient, "nazwa": "Szafa testowa", "montaz_od": "2027-02-10",
                         "czas_warsztat_dni": 5, "czas_montazu_dni": 2 }),
            ),
        )
        .await["id"]
        .as_i64()
        .unwrap();
    let g = app
        .ok("POST", &format!("/api/v1/zlecenia/{z2}/harmonogram"), None)
        .await;
    assert!(g["harmonogram"]["ostrzezenia"]
        .as_array()
        .unwrap()
        .iter()
        .any(|o| o["kod"] == "brak_materialow"));

    let k = app
        .ok(
            "GET",
            "/api/v1/kalendarz/kolizje?od=2027-01-01&do=2027-03-31",
            None,
        )
        .await;
    let k = k.as_array().unwrap();
    // warsztat 1: 25.01–5.02, warsztat 2: 5.02–9.02 → 5.02; montaż 8–10.02 vs 10–11.02 → 10.02
    assert_eq!(k.len(), 2, "{k:?}");
    assert_eq!(k[0]["typ"], "warsztat");
    assert_eq!(k[0]["od"], "2027-02-05");
    assert_eq!(k[0]["do"], "2027-02-05");
    assert_eq!(k[1]["typ"], "montaz");
    assert_eq!(k[1]["od"], "2027-02-10");
    assert_eq!(k[1]["zlecenie_a"], s.zlecenie);
    assert_eq!(k[1]["zlecenie_b_nazwa"], "Szafa testowa");
    assert!(app
        .ok("GET", "/api/v1/kalendarz/kolizje?od=2027-03-01", None)
        .await
        .as_array()
        .unwrap()
        .is_empty());

    // zadanie ręczne: zostaje po przeliczeniu, można je usunąć; wyliczonego nie
    let r = app
        .ok(
            "POST",
            "/api/v1/kalendarz/zadania",
            Some(json!({ "zlecenie_id": z2, "typ": "inne", "tytul": "Odbiór próbki", "data_od": "2027-01-20" })),
        )
        .await;
    assert_eq!(r["wyliczone"], false);
    let rid = r["id"].as_i64().unwrap();
    let g = app
        .ok("POST", &format!("/api/v1/zlecenia/{z2}/harmonogram"), None)
        .await;
    assert_eq!(g["zamrozone"], 1);
    assert_eq!(g["dodane"], 0);
    let montaz2 = znajdz(&g["zadania_zapisane"], "klucz", "montaz")["id"]
        .as_i64()
        .unwrap();
    let (st, _) = app
        .wyslij(
            "DELETE",
            &format!("/api/v1/kalendarz/zadania/{montaz2}"),
            None,
        )
        .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
    let (st, _) = app
        .wyslij("DELETE", &format!("/api/v1/kalendarz/zadania/{rid}"), None)
        .await;
    assert_eq!(st, StatusCode::NO_CONTENT);

    // błędy walidacji
    let z3 = app
        .ok(
            "POST",
            "/api/v1/zlecenia",
            Some(json!({ "klient_id": s.klient, "nazwa": "Bez terminu" })),
        )
        .await["id"]
        .as_i64()
        .unwrap();
    let (st, v) = app
        .wyslij("POST", &format!("/api/v1/zlecenia/{z3}/harmonogram"), None)
        .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(v["blad"].as_str().unwrap().contains("montaz_od"), "{v}");
    let (st, _) = app
        .wyslij(
            "POST",
            "/api/v1/zlecenia",
            Some(json!({ "klient_id": 9999, "nazwa": "X" })),
        )
        .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
    let (st, _) = app.wyslij("GET", "/api/v1/zlecenia/9999", None).await;
    assert_eq!(st, StatusCode::NOT_FOUND);
    let k = kategoria(&app, "Okucia").await;
    let (st, _) = app
        .wyslij(
            "POST",
            "/api/v1/czasy-dostaw",
            Some(json!({ "kategoria_id": k, "dostawca_id": s.dostawca_a, "dni_min": 9, "dni_max": 3 })),
        )
        .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
    let (st, _) = app
        .wyslij(
            "POST",
            &format!("/api/v1/zlecenia/{z3}/materialy"),
            Some(json!({ "kategoria_id": k, "data_dostawy": "2027-01-01" })),
        )
        .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
    let (st, _) = app
        .wyslij(
            "PUT",
            "/api/v1/kalendarz/ustawienia",
            Some(json!({ "liczenie_dni": "tygodniowe" })),
        )
        .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn ustawienia_swieta_i_przeliczenie_trybu() {
    let app = Aplikacja::nowa().await;
    let u = app.ok("GET", "/api/v1/kalendarz/ustawienia", None).await;
    assert_eq!(u["liczenie_dni"], "robocze");
    assert_eq!(u["prog_pomaranczowy_dni"], 2);

    let sw = app
        .ok("GET", "/api/v1/kalendarz/swieta?rok=2027", None)
        .await;
    assert_eq!(sw.as_array().unwrap().len(), 14);
    assert_eq!(znajdz(&sw, "nazwa", "Boże Ciało")["data"], "2027-05-27");

    let s = przygotuj(&app).await; // przełącza na kalendarzowe
    app.ok(
        "POST",
        &format!("/api/v1/zlecenia/{}/harmonogram", s.zlecenie),
        None,
    )
    .await;
    let v = app
        .ok(
            "PUT",
            "/api/v1/kalendarz/ustawienia",
            Some(json!({ "liczenie_dni": "robocze" })),
        )
        .await;
    assert_eq!(v["przeliczone_zlecenia"], json!([s.zlecenie]));
    let h = app
        .ok(
            "GET",
            &format!("/api/v1/zlecenia/{}/harmonogram", s.zlecenie),
            None,
        )
        .await;
    assert_eq!(h["liczenie_dni"], "robocze");
    assert_eq!(h["warsztat_od"], "2027-01-19");
    let z = app
        .ok(
            "GET",
            &format!(
                "/api/v1/kalendarz/zadania?zlecenie_id={}&typ=warsztat",
                s.zlecenie
            ),
            None,
        )
        .await;
    assert_eq!(z[0]["data_od"], "2027-01-19");
}
