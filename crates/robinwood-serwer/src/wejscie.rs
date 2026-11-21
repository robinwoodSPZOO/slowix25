//! Ekstraktory wejścia dla WSZYSTKICH tras serwera: błędy treści JSON, ścieżki i parametrów
//! zapytania wracają jako `{"blad": "…"}` po polsku (422 / 413 / 415) zamiast tekstu axum.
//!
//! * [`Cialo<T>`] – treść JSON,
//! * [`Sciezka<T>`] – parametry ścieżki (`/oferty/{id}`),
//! * [`Parametry<T>`] – parametry zapytania (`?status=…`),
//! * [`Tresc`] – surowe bajty (np. zdjęcie) z limitem rozmiaru → 413 po polsku.

use axum::body::Bytes;
use axum::extract::rejection::{BytesRejection, JsonRejection, PathRejection, QueryRejection};
use axum::extract::{FromRequest, FromRequestParts, Request};
use axum::http::request::Parts;
use axum::http::StatusCode;
use axum::Json;
use serde::de::DeserializeOwned;

use crate::bledy::{tlumacz_serde, BladApi};

const FRAZY_SERDE: &[&str] = &[
    "invalid type",
    "invalid value",
    "missing field",
    "unknown field",
    "unknown variant",
    "duplicate field",
    "invalid length",
    "EOF while parsing",
    "expected ",
    "trailing ",
    "key must be",
    "number out of range",
    "invalid number",
];

/// Komunikat serde z opcjonalną ścieżką pola (`a.b[0]: invalid type …`) → polski.
pub fn tlumacz_z_polem(tekst: &str) -> String {
    let mut najblizszy: Option<usize> = None;
    for f in FRAZY_SERDE {
        if let Some(i) = tekst.find(f) {
            najblizszy = Some(najblizszy.map_or(i, |n: usize| n.min(i)));
        }
    }
    match najblizszy {
        Some(i) if i > 0 => {
            let pole = tekst[..i].trim().trim_end_matches(':').trim();
            let reszta = tlumacz_serde(&tekst[i..]);
            if pole.is_empty() || pole == "." {
                reszta
            } else {
                format!("pole „{pole}”: {reszta}")
            }
        }
        _ => tlumacz_serde(tekst),
    }
}

fn bez_prefiksu<'a>(t: &'a str, prefiksy: &[&str]) -> &'a str {
    for p in prefiksy {
        if let Some(r) = t.strip_prefix(p) {
            return r;
        }
    }
    t
}

/// Treść JSON; błąd → 422 / 413 / 415 z `{"blad": …}`.
pub struct Cialo<T>(pub T);

impl<S: Send + Sync, T: DeserializeOwned> FromRequest<S> for Cialo<T> {
    type Rejection = BladApi;

    async fn from_request(req: Request, stan: &S) -> Result<Self, Self::Rejection> {
        match Json::<T>::from_request(req, stan).await {
            Ok(Json(v)) => Ok(Cialo(v)),
            Err(e) => Err(blad_json(e)),
        }
    }
}

fn blad_json(e: JsonRejection) -> BladApi {
    match e {
        JsonRejection::MissingJsonContentType(_) => BladApi::NieprawidloweDane(
            "treść żądania musi być JSON-em (nagłówek Content-Type: application/json)".into(),
        ),
        JsonRejection::BytesRejection(b) => blad_bajtow(b),
        inne if inne.status() == StatusCode::PAYLOAD_TOO_LARGE => {
            BladApi::ZaDuzo("za duża treść żądania".into())
        }
        inne => {
            let t = inne.body_text();
            let t = bez_prefiksu(
                &t,
                &[
                    "Failed to deserialize the JSON body into the target type: ",
                    "Failed to parse the request body as JSON: ",
                ],
            );
            BladApi::NieprawidloweDane(format!("treść żądania: {}", tlumacz_z_polem(t)))
        }
    }
}

fn blad_bajtow(b: BytesRejection) -> BladApi {
    if b.status() == StatusCode::PAYLOAD_TOO_LARGE {
        BladApi::ZaDuzo("za duży plik lub treść żądania (przekroczony limit rozmiaru)".into())
    } else {
        BladApi::NieprawidloweDane("nie udało się odczytać treści żądania".into())
    }
}

/// Surowe bajty treści (np. zdjęcie); przekroczony limit → 413 `{"blad": …}`.
pub struct Tresc(pub Bytes);

impl<S: Send + Sync> FromRequest<S> for Tresc {
    type Rejection = BladApi;

    async fn from_request(req: Request, stan: &S) -> Result<Self, Self::Rejection> {
        Bytes::from_request(req, stan)
            .await
            .map(Tresc)
            .map_err(blad_bajtow)
    }
}

/// Parametry ścieżki; błąd → 422 JSON.
pub struct Sciezka<T>(pub T);

impl<S: Send + Sync, T: DeserializeOwned + Send> FromRequestParts<S> for Sciezka<T> {
    type Rejection = BladApi;

    async fn from_request_parts(parts: &mut Parts, stan: &S) -> Result<Self, Self::Rejection> {
        axum::extract::Path::<T>::from_request_parts(parts, stan)
            .await
            .map(|axum::extract::Path(v)| Sciezka(v))
            .map_err(|e: PathRejection| {
                let t = e.body_text();
                let opis = if t.contains("Cannot parse") {
                    let wartosc = t
                        .split('`')
                        .nth(1)
                        .map(|w| format!(" „{w}”"))
                        .unwrap_or_default();
                    format!("nieprawidłowa wartość{wartosc} w adresie – oczekiwano liczby")
                } else {
                    "nieprawidłowy adres".into()
                };
                BladApi::NieprawidloweDane(format!("ścieżka: {opis}"))
            })
    }
}

/// Parametry zapytania; błąd (także nieznany parametr przy `deny_unknown_fields`) → 422 JSON.
pub struct Parametry<T>(pub T);

impl<S: Send + Sync, T: DeserializeOwned> FromRequestParts<S> for Parametry<T> {
    type Rejection = BladApi;

    async fn from_request_parts(parts: &mut Parts, stan: &S) -> Result<Self, Self::Rejection> {
        axum::extract::Query::<T>::from_request_parts(parts, stan)
            .await
            .map(|axum::extract::Query(v)| Parametry(v))
            .map_err(|e: QueryRejection| {
                let t = e.body_text();
                let t = bez_prefiksu(&t, &["Failed to deserialize query string: "]);
                let opis = if t.contains("invalid digit")
                    || t.contains("cannot parse")
                    || t.contains("invalid float")
                {
                    "nieprawidłowa liczba".to_string()
                } else {
                    tlumacz_z_polem(t)
                };
                BladApi::NieprawidloweDane(format!("parametry adresu: {opis}"))
            })
    }
}

#[cfg(test)]
mod testy {
    use super::*;

    #[test]
    fn sciezka_pola_w_komunikacie() {
        assert_eq!(
            tlumacz_z_polem("ilosc: invalid type: string \"x\", expected f64 at line 1 column 12"),
            "pole „ilosc”: nieprawidłowy typ: tekst \"x\" – oczekiwano: liczba (wiersz 1, kolumna 12)"
        );
        assert!(
            tlumacz_z_polem("invalid type: null, expected i64").starts_with("nieprawidłowy typ")
        );
    }
}
