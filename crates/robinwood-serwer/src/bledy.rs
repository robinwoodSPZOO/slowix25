//! Wspólny typ błędu API; odpowiedzi błędów są w JSON z komunikatem po polsku.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum BladApi {
    #[error("Nie znaleziono: {0}")]
    NieZnaleziono(String),
    #[error("Nieprawidłowe dane: {0}")]
    NieprawidloweDane(String),
    /// Operacja wstrzymana do jawnego potwierdzenia (409), np. zamówienie materiału, na który
    /// czekają wiersze specyfikacji „do potwierdzenia”. Odpowiedź zawiera `kod` i listę wierszy.
    #[error("{komunikat}")]
    Zablokowane {
        komunikat: String,
        wiersze_do_potwierdzenia: Vec<i64>,
    },
    #[error("Brak lub nieprawidłowy klucz dostępu")]
    BrakDostepu,
    #[error("Błąd bazy danych")]
    Baza(#[from] sqlx::Error),
}

impl IntoResponse for BladApi {
    fn into_response(self) -> Response {
        let kod = match &self {
            BladApi::NieZnaleziono(_) => StatusCode::NOT_FOUND,
            BladApi::NieprawidloweDane(_) => StatusCode::UNPROCESSABLE_ENTITY,
            BladApi::Zablokowane { .. } => StatusCode::CONFLICT,
            BladApi::BrakDostepu => StatusCode::UNAUTHORIZED,
            BladApi::Baza(e) => {
                tracing::error!("Błąd bazy: {e}");
                StatusCode::INTERNAL_SERVER_ERROR
            }
        };
        let cialo = match &self {
            BladApi::Zablokowane {
                wiersze_do_potwierdzenia,
                ..
            } => json!({
                "blad": self.to_string(),
                "kod": "do_potwierdzenia",
                "wiersze_do_potwierdzenia": wiersze_do_potwierdzenia,
            }),
            _ => json!({ "blad": self.to_string() }),
        };
        (kod, Json(cialo)).into_response()
    }
}

pub type Wynik<T> = Result<T, BladApi>;
