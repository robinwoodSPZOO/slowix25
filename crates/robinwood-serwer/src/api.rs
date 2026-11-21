//! API HTTP/WebSocket (axum). Wszystkie ścieżki pod `/api/v1`.
//!
//! * `GET  /api/v1/status` – bez klucza; nazwa serwera i wersja (do wykrywania i testu połączenia)
//! * `GET  /api/v1/klienci?szukaj=` / `POST /api/v1/klienci`
//! * `GET|PUT|DELETE /api/v1/klienci/{id}`
//! * `GET  /api/v1/slowniki`, `GET /api/v1/slowniki/{kod}`
//! * `GET  /api/v1/kategorie-materialow`
//! * `GET  /api/v1/zdarzenia?klucz=...` – WebSocket ze zdarzeniami o zmianach
//! * kalendarz / silnik terminów – trasy w [`crate::api_kalendarz`]
//!
//! * `POST /api/v1/klienci/{id}/przywroc` – cofnięcie usunięcia (miękkiego)
//!
//! Klucz dostępu: nagłówek `Authorization: Bearer <klucz>`. Parametr `?klucz=` jest przyjmowany
//! WYŁĄCZNIE dla WebSocket `/zdarzenia` (przeglądarka nie wysyła tam nagłówków) – klucz w adresie
//! trafiałby inaczej do historii, logów i pamięci podręcznej (np. w `<img src>`).
//!
//! Błędy: zawsze JSON `{"blad": …}` po polsku (`crate::bledy`, ekstraktory `crate::wejscie`).

use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, Request, State};
use axum::http::{header, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tower_http::cors::CorsLayer;

use crate::bledy::{BladApi, Wynik};
use crate::klienci::{self, DaneKlienta, Klient};
use crate::slowniki;
use crate::wejscie::{Cialo, Parametry, Sciezka};
use crate::zdarzenia::{Operacja, Rozglosnia, Zdarzenie};

#[derive(Clone)]
pub struct StanApi {
    pub pula: SqlitePool,
    pub zdarzenia: Rozglosnia,
    /// Folder danych serwera (pliki, zdjęcia). Domyślnie katalog tymczasowy – `uruchom`
    /// ustawia folder obok pliku bazy.
    pub folder_danych: Arc<PathBuf>,
    klucz_api: Arc<str>,
    nazwa: Arc<str>,
}

impl StanApi {
    pub fn nowy(pula: SqlitePool, klucz_api: String, nazwa: String) -> Self {
        Self {
            pula,
            zdarzenia: Rozglosnia::default(),
            folder_danych: Arc::new(std::env::temp_dir().join("robinwood-dane")),
            klucz_api: klucz_api.into(),
            nazwa: nazwa.into(),
        }
    }

    /// Ustawia folder danych serwera (pliki przesłane przez stanowiska).
    pub fn z_folderem_danych(mut self, folder: PathBuf) -> Self {
        self.folder_danych = Arc::new(folder);
        self
    }
}

pub fn router(stan: StanApi) -> Router {
    let chronione = Router::new()
        .route("/klienci", get(lista_klientow).post(dodaj_klienta))
        .route(
            "/klienci/{id}",
            get(pobierz_klienta).put(zmien_klienta).delete(usun_klienta),
        )
        .route(
            "/klienci/{id}/przywroc",
            axum::routing::post(przywroc_klienta),
        )
        .route("/slowniki", get(lista_slownikow))
        .route("/slowniki/{kod}", get(pozycje_slownika))
        .route("/kategorie-materialow", get(kategorie_materialow))
        .route("/zdarzenia", get(zdarzenia_ws))
        // Faza 2: specyfikacje, oferty, szablony, zdjęcia, eksport (crate::api_oferty)
        .merge(crate::api_oferty::trasy())
        .merge(crate::api_kalendarz::trasy())
        // Faza 3: umowy, wpłaty, aneksy, dokumenty z szablonów (crate::api_umowy)
        .merge(crate::api_umowy::trasy())
        .route_layer(middleware::from_fn_with_state(stan.clone(), sprawdz_klucz));

    let api = Router::new().route("/status", get(status)).merge(chronione);

    Router::new()
        .nest("/api/v1", api)
        // Okno Tauri ma inne „origin” niż serwer (tauri://localhost, http://tauri.localhost),
        // a dostęp i tak chroni klucz API, więc CORS jest otwarty.
        .layer(middleware::from_fn(crate::bledy::ujednolic_bledy))
        .layer(CorsLayer::permissive())
        .with_state(stan)
}

// --- Klucz dostępu ---------------------------------------------------------

#[derive(Deserialize)]
struct ParametrKlucza {
    klucz: Option<String>,
}

async fn sprawdz_klucz(
    State(stan): State<StanApi>,
    Query(parametr): Query<ParametrKlucza>,
    zadanie: Request,
    dalej: Next,
) -> Response {
    let z_naglowka = zadanie
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(str::to_owned);
    // Klucz w adresie tylko dla WebSocket (zob. opis modułu).
    let z_adresu = if zadanie.uri().path().ends_with("/zdarzenia") {
        parametr.klucz
    } else {
        None
    };
    let podany = z_naglowka.or(z_adresu);
    match podany {
        Some(k) if porownaj_stalo_czasowo(k.as_bytes(), stan.klucz_api.as_bytes()) => {
            dalej.run(zadanie).await
        }
        _ => BladApi::BrakDostepu.into_response(),
    }
}

fn porownaj_stalo_czasowo(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

// --- Status ----------------------------------------------------------------

#[derive(Serialize)]
pub struct Status {
    pub aplikacja: &'static str,
    pub nazwa: String,
    pub wersja: &'static str,
    pub wersja_api: u32,
}

async fn status(State(stan): State<StanApi>) -> Json<Status> {
    Json(Status {
        aplikacja: "robinwood",
        nazwa: stan.nazwa.to_string(),
        wersja: env!("CARGO_PKG_VERSION"),
        wersja_api: crate::WERSJA_API,
    })
}

// --- Klienci ---------------------------------------------------------------

#[derive(Deserialize)]
struct ParametryListy {
    szukaj: Option<String>,
}

async fn lista_klientow(
    State(stan): State<StanApi>,
    Parametry(p): Parametry<ParametryListy>,
) -> Wynik<Json<Vec<Klient>>> {
    Ok(Json(klienci::lista(&stan.pula, p.szukaj.as_deref()).await?))
}

async fn pobierz_klienta(
    State(stan): State<StanApi>,
    Sciezka(id): Sciezka<i64>,
) -> Wynik<Json<Klient>> {
    Ok(Json(klienci::pobierz(&stan.pula, id).await?))
}

async fn dodaj_klienta(
    State(stan): State<StanApi>,
    Cialo(dane): Cialo<DaneKlienta>,
) -> Wynik<(StatusCode, Json<Klient>)> {
    let k = klienci::dodaj(&stan.pula, dane).await?;
    stan.zdarzenia.wyslij(Zdarzenie {
        zasob: "klienci",
        id: k.id,
        operacja: Operacja::Dodanie,
    });
    Ok((StatusCode::CREATED, Json(k)))
}

async fn zmien_klienta(
    State(stan): State<StanApi>,
    Sciezka(id): Sciezka<i64>,
    Cialo(dane): Cialo<DaneKlienta>,
) -> Wynik<Json<Klient>> {
    let k = klienci::zmien(&stan.pula, id, dane).await?;
    stan.zdarzenia.wyslij(Zdarzenie {
        zasob: "klienci",
        id,
        operacja: Operacja::Zmiana,
    });
    Ok(Json(k))
}

async fn usun_klienta(State(stan): State<StanApi>, Sciezka(id): Sciezka<i64>) -> Wynik<StatusCode> {
    klienci::usun(&stan.pula, id).await?;
    stan.zdarzenia.wyslij(Zdarzenie {
        zasob: "klienci",
        id,
        operacja: Operacja::Usuniecie,
    });
    Ok(StatusCode::NO_CONTENT)
}

async fn przywroc_klienta(
    State(stan): State<StanApi>,
    Sciezka(id): Sciezka<i64>,
) -> Wynik<Json<Klient>> {
    let k = klienci::przywroc(&stan.pula, id).await?;
    stan.zdarzenia.wyslij(Zdarzenie {
        zasob: "klienci",
        id,
        operacja: Operacja::Zmiana,
    });
    // Oferty klienta znów można rozwijać – odśwież listy.
    stan.zdarzenia.wyslij(Zdarzenie {
        zasob: "oferty",
        id: 0,
        operacja: Operacja::Zmiana,
    });
    Ok(Json(k))
}

// --- Słowniki --------------------------------------------------------------

async fn lista_slownikow(State(stan): State<StanApi>) -> Wynik<Json<Vec<slowniki::Slownik>>> {
    Ok(Json(slowniki::lista_slownikow(&stan.pula).await?))
}

async fn pozycje_slownika(
    State(stan): State<StanApi>,
    Sciezka(kod): Sciezka<String>,
) -> Wynik<Json<Vec<slowniki::PozycjaSlownika>>> {
    Ok(Json(slowniki::pozycje(&stan.pula, &kod).await?))
}

async fn kategorie_materialow(
    State(stan): State<StanApi>,
) -> Wynik<Json<Vec<slowniki::KategoriaMaterialu>>> {
    Ok(Json(slowniki::kategorie_materialow(&stan.pula).await?))
}

// --- WebSocket ze zdarzeniami ----------------------------------------------

async fn zdarzenia_ws(State(stan): State<StanApi>, ws: WebSocketUpgrade) -> Response {
    ws.on_upgrade(move |gniazdo| obsluz_gniazdo(gniazdo, stan.zdarzenia.clone()))
}

async fn obsluz_gniazdo(mut gniazdo: WebSocket, rozglosnia: Rozglosnia) {
    let mut odbiornik = rozglosnia.subskrybuj();
    loop {
        tokio::select! {
            z = odbiornik.recv() => match z {
                Ok(z) => {
                    let tekst = serde_json::to_string(&z).unwrap_or_default();
                    if gniazdo.send(Message::Text(tekst.into())).await.is_err() {
                        break;
                    }
                }
                // Klient nie nadążał – niech odświeży wszystko.
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    let _ = gniazdo
                        .send(Message::Text(r#"{"zasob":"*","id":0,"operacja":"zmiana"}"#.into()))
                        .await;
                }
                Err(_) => break,
            },
            wiadomosc = gniazdo.recv() => match wiadomosc {
                Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                _ => {}
            },
        }
    }
}
