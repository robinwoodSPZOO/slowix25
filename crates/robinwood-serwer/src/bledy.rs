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
    #[error("Brak lub nieprawidłowy klucz dostępu")]
    BrakDostepu,
    #[error("Błąd serwera: {0}")]
    Wewnetrzny(String),
    #[error("Błąd bazy danych")]
    Baza(#[from] sqlx::Error),
}

impl IntoResponse for BladApi {
    fn into_response(self) -> Response {
        let kod = match &self {
            BladApi::NieZnaleziono(_) => StatusCode::NOT_FOUND,
            BladApi::NieprawidloweDane(_) => StatusCode::UNPROCESSABLE_ENTITY,
            BladApi::BrakDostepu => StatusCode::UNAUTHORIZED,
            BladApi::Wewnetrzny(e) => {
                tracing::error!("Błąd wewnętrzny: {e}");
                StatusCode::INTERNAL_SERVER_ERROR
            }
            BladApi::Baza(e) => {
                tracing::error!("Błąd bazy: {e}");
                StatusCode::INTERNAL_SERVER_ERROR
            }
        };
        (kod, Json(json!({ "blad": self.to_string() }))).into_response()
    }
}

pub type Wynik<T> = Result<T, BladApi>;
