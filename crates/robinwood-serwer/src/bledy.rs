//! Wspólny typ błędu API; KAŻDA odpowiedź błędu serwera to JSON `{"blad": "komunikat po polsku"}`
//! (ewentualnie z dodatkowymi polami, np. `kod`, `bledy`, `konflikty`).
//!
//! * błędy z kodu – [`BladApi`];
//! * błędy treści / ścieżki / parametrów zapytania – ekstraktory z `crate::wejscie`;
//! * wszystko inne (np. 404 nieznanej trasy, 405, 413 z limitu treści) – warstwa
//!   [`ujednolic_bledy`] w routerze zamienia odpowiedź na JSON po polsku.

use axum::body::Body;
use axum::extract::Request;
use axum::http::{header, HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;
use serde_json::{json, Value};

/// Błąd jednej pozycji (np. wiersza pliku importu): gdzie, które pole i dlaczego.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct BladPozycji {
    pub pozycja: String,
    pub pole: String,
    pub powod: String,
}

/// Konflikt równoczesnej edycji tego samego pola na dwóch stanowiskach.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct KonfliktPola {
    pub pole: String,
    /// Wartość, którą stanowisko widziało przed edycją.
    pub poprzednia: Value,
    /// Wartość, którą stanowisko chciało zapisać.
    pub twoja: Value,
    /// Wartość zapisana w międzyczasie przez kogoś innego.
    pub aktualna: Value,
}

#[derive(Debug, thiserror::Error)]
pub enum BladApi {
    #[error("Nie znaleziono: {0}")]
    NieZnaleziono(String),
    #[error("Nieprawidłowe dane: {0}")]
    NieprawidloweDane(String),
    /// 422 z listą błędów poszczególnych pozycji (np. import pliku).
    #[error("{komunikat}")]
    BledyPozycji {
        komunikat: String,
        bledy: Vec<BladPozycji>,
    },
    /// 409: ktoś inny zmienił w międzyczasie te same pola. `aktualny` – rekord z serwera.
    #[error("{komunikat}")]
    Konflikt {
        komunikat: String,
        konflikty: Vec<KonfliktPola>,
        aktualny: Value,
    },
    /// 413: za duża treść żądania.
    #[error("{0}")]
    ZaDuzo(String),
    #[error("Brak lub nieprawidłowy klucz dostępu")]
    BrakDostepu,
    #[error("Błąd serwera: {0}")]
    Wewnetrzny(String),
    #[error("Błąd bazy danych")]
    Baza(#[from] sqlx::Error),
}

/// SQLITE_BUSY / SQLITE_LOCKED (także rozszerzone kody) – baza chwilowo zajęta.
fn baza_zajeta(e: &sqlx::Error) -> bool {
    match e {
        sqlx::Error::Database(d) => d
            .code()
            .and_then(|k| k.parse::<i32>().ok())
            .is_some_and(|k| k & 0xff == 5 || k & 0xff == 6),
        sqlx::Error::PoolTimedOut => true,
        _ => false,
    }
}

impl IntoResponse for BladApi {
    fn into_response(self) -> Response {
        let kod = match &self {
            BladApi::NieZnaleziono(_) => StatusCode::NOT_FOUND,
            BladApi::NieprawidloweDane(_) | BladApi::BledyPozycji { .. } => {
                StatusCode::UNPROCESSABLE_ENTITY
            }
            BladApi::Konflikt { .. } => StatusCode::CONFLICT,
            BladApi::ZaDuzo(_) => StatusCode::PAYLOAD_TOO_LARGE,
            BladApi::BrakDostepu => StatusCode::UNAUTHORIZED,
            BladApi::Wewnetrzny(e) => {
                tracing::error!("Błąd wewnętrzny: {e}");
                StatusCode::INTERNAL_SERVER_ERROR
            }
            BladApi::Baza(e) if baza_zajeta(e) => {
                tracing::warn!("Baza zajęta: {e}");
                StatusCode::SERVICE_UNAVAILABLE
            }
            BladApi::Baza(e) => {
                tracing::error!("Błąd bazy: {e}");
                StatusCode::INTERNAL_SERVER_ERROR
            }
        };
        let cialo = match &self {
            BladApi::BledyPozycji { komunikat, bledy } => {
                json!({ "blad": komunikat, "kod": "bledy_pozycji", "bledy": bledy })
            }
            BladApi::Konflikt {
                komunikat,
                konflikty,
                aktualny,
            } => json!({
                "blad": komunikat,
                "kod": "konflikt",
                "konflikty": konflikty,
                "aktualny": aktualny,
            }),
            BladApi::Baza(e) if baza_zajeta(e) => json!({
                "blad": "Serwer jest chwilowo zajęty zapisem z innego stanowiska – spróbuj ponownie.",
                "kod": "zajety",
            }),
            _ => json!({ "blad": self.to_string() }),
        };
        (kod, Json(cialo)).into_response()
    }
}

pub type Wynik<T> = Result<T, BladApi>;

// ---------------------------------------------------------------------------
// Tłumaczenie komunikatów serde_json na polski
// ---------------------------------------------------------------------------

fn typ_po_polsku(t: &str) -> String {
    let t = t.trim();
    let zamiany: &[(&str, &str)] = &[
        ("i64", "liczba całkowita"),
        ("i32", "liczba całkowita"),
        ("u64", "liczba całkowita nieujemna"),
        ("u32", "liczba całkowita nieujemna"),
        ("u16", "liczba całkowita nieujemna"),
        ("u8", "liczba całkowita nieujemna"),
        ("f64", "liczba"),
        ("f32", "liczba"),
        ("a string", "tekst"),
        ("a boolean", "prawda/fałsz"),
        ("a sequence", "lista"),
        ("a map", "obiekt"),
        ("struct", "obiekt"),
        ("a borrowed string", "tekst"),
        ("unit variant", "wartość"),
    ];
    for (a, b) in zamiany {
        if t == *a || t.starts_with(&format!("{a} ")) || t.starts_with(&format!("struct {a}")) {
            return (*b).to_string();
        }
    }
    if t.starts_with("struct ") {
        return "obiekt".into();
    }
    if let Some(r) = t.strip_prefix("string ") {
        return format!("tekst {r}");
    }
    if let Some(r) = t.strip_prefix("integer ") {
        return format!("liczba {r}");
    }
    if let Some(r) = t.strip_prefix("floating point ") {
        return format!("liczba {r}");
    }
    match t {
        "null" => "brak wartości (null)".into(),
        "map" => "obiekt".into(),
        "sequence" => "lista".into(),
        "boolean `true`" | "boolean `false`" => "prawda/fałsz".into(),
        _ => t.to_string(),
    }
}

/// Zamienia komunikat serde (np. `invalid type: string "abc", expected i64 at line 1 column 9`)
/// na polski (`nieprawidłowy typ: tekst "abc" – oczekiwano: liczba całkowita (wiersz 1, kolumna 9)`).
pub fn tlumacz_serde(komunikat: &str) -> String {
    let (tresc, polozenie) = match komunikat.rfind(" at line ") {
        Some(i) => {
            let reszta = &komunikat[i + " at line ".len()..];
            let pol = reszta
                .split_once(" column ")
                .map(|(w, k)| format!(" (wiersz {w}, kolumna {k})"))
                .unwrap_or_default();
            (&komunikat[..i], pol)
        }
        None => (komunikat, String::new()),
    };
    let t = tresc.trim();
    let wynik = if let Some(r) = t.strip_prefix("invalid type: ") {
        match r.split_once(", expected ") {
            Some((jest, oczek)) => format!(
                "nieprawidłowy typ: {} – oczekiwano: {}",
                typ_po_polsku(jest),
                typ_po_polsku(oczek)
            ),
            None => format!("nieprawidłowy typ: {r}"),
        }
    } else if let Some(r) = t.strip_prefix("invalid value: ") {
        match r.split_once(", expected ") {
            Some((jest, oczek)) => format!(
                "nieprawidłowa wartość: {} – oczekiwano: {}",
                typ_po_polsku(jest),
                typ_po_polsku(oczek)
            ),
            None => format!("nieprawidłowa wartość: {r}"),
        }
    } else if let Some(r) = t.strip_prefix("missing field ") {
        format!("brak wymaganego pola {}", r.replace('`', ""))
    } else if let Some(r) = t.strip_prefix("unknown field ") {
        let (pole, reszta) = r.split_once(", expected").unwrap_or((r, ""));
        let dozwolone = reszta
            .trim_start_matches(" one of ")
            .trim_start_matches(' ')
            .replace('`', "");
        if dozwolone.is_empty() {
            format!("nieznane pole {}", pole.replace('`', ""))
        } else {
            format!(
                "nieznane pole {} (dozwolone: {dozwolone})",
                pole.replace('`', "")
            )
        }
    } else if let Some(r) = t.strip_prefix("unknown variant ") {
        let (wart, reszta) = r.split_once(", expected").unwrap_or((r, ""));
        format!(
            "nieznana wartość {} (dozwolone: {})",
            wart.replace('`', ""),
            reszta
                .trim_start_matches(" one of ")
                .trim_start_matches(' ')
                .replace('`', "")
        )
    } else if let Some(r) = t.strip_prefix("duplicate field ") {
        format!("pole {} podano dwa razy", r.replace('`', ""))
    } else if t.starts_with("EOF while parsing") {
        "niekompletny JSON (treść urwana)".into()
    } else if t.starts_with("expected value")
        || t.starts_with("expected `")
        || t.starts_with("expected ident")
    {
        "treść nie jest poprawnym JSON-em".into()
    } else if t.starts_with("trailing characters") || t.starts_with("trailing comma") {
        "nadmiarowe znaki po końcu JSON".into()
    } else if t.starts_with("key must be a string") {
        "klucz obiektu musi być tekstem".into()
    } else if t.contains("number out of range") {
        "liczba poza dozwolonym zakresem".into()
    } else if t.starts_with("invalid number") {
        "nieprawidłowa liczba".into()
    } else if t.starts_with("control character") || t.starts_with("invalid escape") {
        "nieprawidłowy znak w tekście".into()
    } else if t.starts_with("recursion limit exceeded") {
        "zbyt głęboko zagnieżdżony JSON".into()
    } else {
        // Komunikaty naszego kodu (np. z deserialize_with) są już po polsku.
        t.to_string()
    };
    format!("{wynik}{polozenie}")
}

// ---------------------------------------------------------------------------
// Warstwa: każda odpowiedź błędu w JSON po polsku
// ---------------------------------------------------------------------------

/// Komunikat dla odpowiedzi błędu bez treści JSON (wygenerowanych poza naszym kodem).
pub fn komunikat_dla_kodu(kod: StatusCode) -> &'static str {
    match kod {
        StatusCode::NOT_FOUND => "Nie znaleziono: nieznany adres API",
        StatusCode::METHOD_NOT_ALLOWED => "Ta metoda nie jest obsługiwana dla tego adresu",
        StatusCode::PAYLOAD_TOO_LARGE => "Za duży plik lub treść żądania",
        StatusCode::UNSUPPORTED_MEDIA_TYPE => {
            "Nieobsługiwany typ treści – oczekiwano JSON (Content-Type: application/json)"
        }
        StatusCode::BAD_REQUEST => "Nieprawidłowe żądanie",
        StatusCode::UNPROCESSABLE_ENTITY => "Nieprawidłowe dane",
        StatusCode::UNAUTHORIZED => "Brak lub nieprawidłowy klucz dostępu",
        StatusCode::REQUEST_TIMEOUT => "Przekroczono czas żądania",
        StatusCode::SERVICE_UNAVAILABLE => "Serwer jest chwilowo niedostępny",
        _ => "Błąd serwera",
    }
}

/// Warstwa (middleware) routera: odpowiedź błędu (4xx/5xx) bez treści JSON zamienia na
/// `{"blad": "…"}` po polsku. Dodaje też `X-Content-Type-Options: nosniff` do każdej odpowiedzi.
pub async fn ujednolic_bledy(zadanie: Request, dalej: Next) -> Response {
    let mut odp = dalej.run(zadanie).await;
    odp.headers_mut().insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    let kod = odp.status();
    if !(kod.is_client_error() || kod.is_server_error()) {
        return odp;
    }
    let json = odp
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("application/json"));
    if json {
        return odp;
    }
    let cialo = json!({ "blad": komunikat_dla_kodu(kod) }).to_string();
    let (mut czesci, _) = odp.into_parts();
    czesci.headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    czesci.headers.remove(header::CONTENT_LENGTH);
    Response::from_parts(czesci, Body::from(cialo))
}

#[cfg(test)]
mod testy {
    use super::*;

    #[test]
    fn tlumaczenie_bledow_serde() {
        assert_eq!(
            tlumacz_serde("invalid type: string \"abc\", expected i64 at line 1 column 9"),
            "nieprawidłowy typ: tekst \"abc\" – oczekiwano: liczba całkowita (wiersz 1, kolumna 9)"
        );
        assert!(tlumacz_serde("missing field `nazwa` at line 1 column 2")
            .starts_with("brak wymaganego pola"));
        assert!(
            tlumacz_serde("EOF while parsing an object at line 1 column 5")
                .starts_with("niekompletny JSON")
        );
        assert!(
            tlumacz_serde("unknown field `xyz`, expected one of `a`, `b`")
                .contains("nieznane pole xyz")
        );
        assert!(tlumacz_serde("expected value at line 1 column 1").starts_with("treść nie jest"));
        for k in [
            "invalid type: integer `5`, expected a string",
            "invalid type: null, expected f64",
            "invalid type: boolean `true`, expected i64",
        ] {
            assert!(!tlumacz_serde(k).contains("expected"), "{k}");
        }
    }
}
