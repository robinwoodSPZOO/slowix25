//! Trasy API Fazy 3 (Generator Umów). Wszystkie pod `/api/v1`, chronione kluczem.
//!
//! * `GET /umowy?status=&klient_id=&rok=&szukaj=`, `POST /umowy` `{wersja_oferty_id? | klient_id?, data_zawarcia?}`
//! * `GET|PUT|DELETE /umowy/{id}` – PUT = zmiana częściowa pól formularza (+ `poprzednie`, 409)
//! * `PUT /umowy/{id}/status` `{status, data?, powod?}`, `PUT /umowy/{id}/etapy` `{etapy}`
//! * `GET /umowy/{id}/roznice` – różnice specyfikacji względem oferty źródłowej
//! * `POST /umowy/{id}/wplaty`, `DELETE /wplaty/{id}`, `POST /umowy/{id}/protokol`
//! * `GET /umowy/{id}/dokument/pola`, `GET /umowy/{id}/dokument/{docx|pdf}`,
//!   `GET /umowy/{id}/protokol/{docx|pdf}`
//! * `POST /umowy/{id}/aneksy`, `GET|PUT|DELETE /aneksy/{id}`, `PUT /aneksy/{id}/status`,
//!   `GET /aneksy/{id}/dokument/pola`, `GET /aneksy/{id}/dokument/{docx|pdf}`
//! * `GET|PUT /ustawienia/umowy`
//! * `GET /szablony-dokumentow`, `GET /szablony-dokumentow/pola`,
//!   `GET|PUT|DELETE /szablony-dokumentow/{rodzaj}` (PUT: bajty pliku, `?nazwa=`),
//!   `GET /szablony-dokumentow/{rodzaj}/plik`
//! * `GET|PUT /klienci/{id}/osoby` – osoby kontaktowe klienta

use axum::extract::{DefaultBodyLimit, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::Value;

use crate::aneksy::{self, PelnyAneks, WynikStatusuAneksu, ZmianaStatusuAneksu};
use crate::api::StanApi;
use crate::bledy::{BladApi, Wynik};
use crate::dokument_umowy::{self as dok, OpisPola, PodgladPol, StanSzablonu};
use crate::umowy::{
    self, DaneEtapow, DaneNowejUmowy, DaneOsob, FiltrUmow, OsobaKontaktowa, PelnaUmowa, RozniceUmowy,
    UmowaNaLiscie, UstawieniaUmow, WynikStatusu, ZmianaStatusuUmowy,
};
use crate::wejscie::{Cialo, Parametry, Sciezka, Tresc};
use crate::wplaty::{self, DaneProtokolu, DaneWplaty};
use crate::zdarzenia::{Operacja, Zdarzenie};

pub fn trasy() -> Router<StanApi> {
    Router::new()
        .route("/umowy", get(lista).post(dodaj))
        .route("/umowy/{id}", get(pobierz).put(zmien).delete(usun))
        .route("/umowy/{id}/status", put(zmien_status))
        .route("/umowy/{id}/etapy", put(zapisz_etapy))
        .route("/umowy/{id}/roznice", get(roznice))
        .route("/umowy/{id}/wplaty", post(dodaj_wplate))
        .route("/wplaty/{id}", axum::routing::delete(usun_wplate))
        .route("/umowy/{id}/protokol", post(zapisz_protokol))
        .route("/umowy/{id}/dokument/pola", get(pola_umowy))
        .route("/umowy/{id}/dokument/{format}", get(dokument_umowy))
        .route("/umowy/{id}/protokol/{format}", get(dokument_protokolu))
        .route("/umowy/{id}/aneksy", post(dodaj_aneks))
        .route("/aneksy/{id}", get(pobierz_aneks).put(zmien_aneks).delete(usun_aneks))
        .route("/aneksy/{id}/status", put(status_aneksu))
        .route("/aneksy/{id}/dokument/pola", get(pola_aneksu))
        .route("/aneksy/{id}/dokument/{format}", get(dokument_aneksu))
        .route("/ustawienia/umowy", get(ustawienia).put(zapisz_ustawienia))
        .route("/szablony-dokumentow", get(szablony))
        .route("/szablony-dokumentow/pola", get(opisy_pol))
        .route(
            "/szablony-dokumentow/{rodzaj}",
            get(stan_szablonu)
                .put(wgraj_szablon)
                .delete(usun_szablon)
                .layer(DefaultBodyLimit::max(51 * 1024 * 1024)),
        )
        .route("/szablony-dokumentow/{rodzaj}/plik", get(plik_szablonu))
        .route("/klienci/{id}/osoby", get(osoby).put(zapisz_osoby))
}

fn zdarzenie(stan: &StanApi, zasob: &'static str, id: i64, operacja: Operacja) {
    stan.zdarzenia.wyslij(Zdarzenie { zasob, id, operacja });
}

/// Zmiana umowy dotyka też kalendarza (zlecenie, płatności, zadania).
fn zdarzenia_umowy(stan: &StanApi, p: &PelnaUmowa) {
    zdarzenie(stan, "umowy", p.umowa.id, Operacja::Zmiana);
    zdarzenie(stan, "specyfikacje", p.umowa.specyfikacja_id, Operacja::Zmiana);
    if let Some(z) = &p.zlecenie {
        zdarzenie(stan, "zlecenia", z.id, Operacja::Zmiana);
        zdarzenie(stan, "zadania_kalendarza", z.id, Operacja::Zmiana);
        zdarzenie(stan, "zamowienia_materialow", 0, Operacja::Zmiana);
    }
}

async fn lista(State(stan): State<StanApi>, Parametry(f): Parametry<FiltrUmow>) -> Wynik<Json<Vec<UmowaNaLiscie>>> {
    Ok(Json(umowy::lista(&stan.pula, f).await?))
}

async fn dodaj(State(stan): State<StanApi>, Cialo(d): Cialo<DaneNowejUmowy>) -> Wynik<(StatusCode, Json<PelnaUmowa>)> {
    let p = umowy::dodaj(&stan.pula, d).await?;
    zdarzenie(&stan, "umowy", p.umowa.id, Operacja::Dodanie);
    zdarzenie(&stan, "oferty", 0, Operacja::Zmiana);
    Ok((StatusCode::CREATED, Json(p)))
}

async fn pobierz(State(stan): State<StanApi>, Sciezka(id): Sciezka<i64>) -> Wynik<Json<PelnaUmowa>> {
    Ok(Json(umowy::pelna(&stan.pula, id).await?))
}

async fn zmien(State(stan): State<StanApi>, Sciezka(id): Sciezka<i64>, Cialo(v): Cialo<Value>) -> Wynik<Json<PelnaUmowa>> {
    let p = umowy::zmien(&stan.pula, id, v).await?;
    zdarzenia_umowy(&stan, &p);
    Ok(Json(p))
}

async fn usun(State(stan): State<StanApi>, Sciezka(id): Sciezka<i64>) -> Wynik<StatusCode> {
    umowy::usun(&stan.pula, id).await?;
    zdarzenie(&stan, "umowy", id, Operacja::Usuniecie);
    Ok(StatusCode::NO_CONTENT)
}

async fn zmien_status(
    State(stan): State<StanApi>,
    Sciezka(id): Sciezka<i64>,
    Cialo(z): Cialo<ZmianaStatusuUmowy>,
) -> Wynik<Json<WynikStatusu>> {
    let w = umowy::zmien_status(&stan.pula, id, z).await?;
    zdarzenia_umowy(&stan, &w.umowa);
    Ok(Json(w))
}

async fn zapisz_etapy(State(stan): State<StanApi>, Sciezka(id): Sciezka<i64>, Cialo(d): Cialo<DaneEtapow>) -> Wynik<Json<PelnaUmowa>> {
    let p = umowy::zapisz_etapy(&stan.pula, id, d).await?;
    zdarzenie(&stan, "umowy", id, Operacja::Zmiana);
    Ok(Json(p))
}

async fn roznice(State(stan): State<StanApi>, Sciezka(id): Sciezka<i64>) -> Wynik<Json<RozniceUmowy>> {
    Ok(Json(umowy::roznice(&stan.pula, id).await?))
}

async fn dodaj_wplate(
    State(stan): State<StanApi>,
    Sciezka(id): Sciezka<i64>,
    Cialo(d): Cialo<DaneWplaty>,
) -> Wynik<(StatusCode, Json<PelnaUmowa>)> {
    let p = wplaty::dodaj(&stan.pula, id, d).await?;
    zdarzenia_umowy(&stan, &p);
    Ok((StatusCode::CREATED, Json(p)))
}

async fn usun_wplate(State(stan): State<StanApi>, Sciezka(id): Sciezka<i64>) -> Wynik<Json<PelnaUmowa>> {
    let (_, p) = wplaty::usun(&stan.pula, id).await?;
    zdarzenia_umowy(&stan, &p);
    Ok(Json(p))
}

async fn zapisz_protokol(
    State(stan): State<StanApi>,
    Sciezka(id): Sciezka<i64>,
    Cialo(d): Cialo<DaneProtokolu>,
) -> Wynik<Json<PelnaUmowa>> {
    let p = wplaty::zapisz_protokol(&stan.pula, id, d).await?;
    zdarzenia_umowy(&stan, &p);
    Ok(Json(p))
}

// --- Dokumenty ---------------------------------------------------------------

fn plik(bajty: Vec<u8>, mime: &'static str, nazwa: &str) -> Wynik<Response> {
    let dyspozycja = HeaderValue::from_str(&format!("attachment; filename=\"{nazwa}\""))
        .map_err(|e| BladApi::Wewnetrzny(e.to_string()))?;
    Ok(([(header::CONTENT_TYPE, HeaderValue::from_static(mime)), (header::CONTENT_DISPOSITION, dyspozycja)], bajty).into_response())
}

const MIME_DOCX: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.document";

async fn wyslij_dokument(d: dok::Dokument, format: &str, prefiks: &str, numer: &str) -> Wynik<Response> {
    match format {
        "docx" => plik(d.bajty, MIME_DOCX, &dok::nazwa_pliku(prefiks, numer, "docx")),
        "pdf" => {
            let pdf = dok::pdf_z_docx(d.bajty).await?;
            plik(pdf, "application/pdf", &dok::nazwa_pliku(prefiks, numer, "pdf"))
        }
        _ => Err(BladApi::NieprawidloweDane("format dokumentu: docx albo pdf".into())),
    }
}

async fn pola_umowy(State(stan): State<StanApi>, Sciezka(id): Sciezka<i64>) -> Wynik<Json<PodgladPol>> {
    let p = umowy::pelna(&stan.pula, id).await?;
    let d = dok::dane_umowy(&stan.pula, &p).await?;
    let g = dok::generuj(&stan.pula, &stan.folder_danych, "umowa", d.clone(), None).await?;
    Ok(Json(dok::podglad(&d, &g, dok::meta(&stan.folder_danych, "umowa"))))
}

async fn dokument_umowy(State(stan): State<StanApi>, Sciezka((id, format)): Sciezka<(i64, String)>) -> Wynik<Response> {
    let p = umowy::pelna(&stan.pula, id).await?;
    let d = dok::dane_umowy(&stan.pula, &p).await?;
    let g = dok::generuj(&stan.pula, &stan.folder_danych, "umowa", d, None).await?;
    wyslij_dokument(g, &format, "Umowa", p.umowa.numer()).await
}

async fn dokument_protokolu(State(stan): State<StanApi>, Sciezka((id, format)): Sciezka<(i64, String)>) -> Wynik<Response> {
    let p = umowy::pelna(&stan.pula, id).await?;
    let d = dok::dane_umowy(&stan.pula, &p).await?;
    let g = dok::generuj(&stan.pula, &stan.folder_danych, "umowa", d, Some("załącznik 5".into()))
        .await
        .map_err(|e| match e {
            BladApi::NieprawidloweDane(m) => BladApi::NieprawidloweDane(format!(
                "{m} – oznacz w szablonie umowy protokół odbioru znacznikami [[załącznik 5]] i [[/załącznik 5]]"
            )),
            inny => inny,
        })?;
    wyslij_dokument(g, &format, "Protokol_odbioru", p.umowa.numer()).await
}

// --- Aneksy ------------------------------------------------------------------

fn zdarzenia_aneksu(stan: &StanApi, a: &PelnyAneks) {
    zdarzenie(stan, "umowy", a.aneks.umowa_id, Operacja::Zmiana);
    if let Some(s) = a.aneks.specyfikacja_id {
        zdarzenie(stan, "specyfikacje", s, Operacja::Zmiana);
    }
}

async fn dodaj_aneks(State(stan): State<StanApi>, Sciezka(id): Sciezka<i64>) -> Wynik<(StatusCode, Json<PelnyAneks>)> {
    let a = aneksy::dodaj(&stan.pula, id).await?;
    zdarzenia_aneksu(&stan, &a);
    Ok((StatusCode::CREATED, Json(a)))
}

async fn pobierz_aneks(State(stan): State<StanApi>, Sciezka(id): Sciezka<i64>) -> Wynik<Json<PelnyAneks>> {
    Ok(Json(aneksy::pelny(&stan.pula, id).await?))
}

async fn zmien_aneks(State(stan): State<StanApi>, Sciezka(id): Sciezka<i64>, Cialo(v): Cialo<Value>) -> Wynik<Json<PelnyAneks>> {
    let a = aneksy::zmien(&stan.pula, id, v).await?;
    zdarzenia_aneksu(&stan, &a);
    Ok(Json(a))
}

async fn usun_aneks(State(stan): State<StanApi>, Sciezka(id): Sciezka<i64>) -> Wynik<StatusCode> {
    let umowa = aneksy::usun(&stan.pula, id).await?;
    zdarzenie(&stan, "umowy", umowa, Operacja::Zmiana);
    Ok(StatusCode::NO_CONTENT)
}

async fn status_aneksu(
    State(stan): State<StanApi>,
    Sciezka(id): Sciezka<i64>,
    Cialo(z): Cialo<ZmianaStatusuAneksu>,
) -> Wynik<Json<WynikStatusuAneksu>> {
    let w = aneksy::zmien_status(&stan.pula, id, z).await?;
    zdarzenia_aneksu(&stan, &w.aneks);
    if let Some(h) = &w.harmonogram {
        zdarzenie(&stan, "zlecenia", h.zlecenie_id, Operacja::Zmiana);
        zdarzenie(&stan, "zadania_kalendarza", h.zlecenie_id, Operacja::Zmiana);
        zdarzenie(&stan, "zamowienia_materialow", 0, Operacja::Zmiana);
    }
    Ok(Json(w))
}

async fn pola_aneksu(State(stan): State<StanApi>, Sciezka(id): Sciezka<i64>) -> Wynik<Json<PodgladPol>> {
    let a = aneksy::pelny(&stan.pula, id).await?;
    let d = dok::dane_aneksu(&stan.pula, &a).await?;
    let g = dok::generuj(&stan.pula, &stan.folder_danych, "aneks", d.clone(), None).await?;
    Ok(Json(dok::podglad(&d, &g, dok::meta(&stan.folder_danych, "aneks"))))
}

async fn dokument_aneksu(State(stan): State<StanApi>, Sciezka((id, format)): Sciezka<(i64, String)>) -> Wynik<Response> {
    let a = aneksy::pelny(&stan.pula, id).await?;
    let d = dok::dane_aneksu(&stan.pula, &a).await?;
    let g = dok::generuj(&stan.pula, &stan.folder_danych, "aneks", d, None).await?;
    let numer = format!("{}_aneks_{}", a.umowa_numer, a.aneks.nr);
    wyslij_dokument(g, &format, "Umowa", &numer).await
}

// --- Ustawienia i szablony ---------------------------------------------------

async fn ustawienia(State(stan): State<StanApi>) -> Wynik<Json<UstawieniaUmow>> {
    Ok(Json(umowy::ustawienia(&stan.pula).await?))
}

async fn zapisz_ustawienia(State(stan): State<StanApi>, Cialo(u): Cialo<UstawieniaUmow>) -> Wynik<Json<UstawieniaUmow>> {
    let w = umowy::zapisz_ustawienia(&stan.pula, u).await?;
    zdarzenie(&stan, "ustawienia", 0, Operacja::Zmiana);
    Ok(Json(w))
}

async fn szablony(State(stan): State<StanApi>) -> Wynik<Json<Vec<StanSzablonu>>> {
    let mut w = Vec::new();
    for r in dok::RODZAJE_SZABLONOW {
        w.push(dok::stan(&stan.folder_danych, r)?);
    }
    Ok(Json(w))
}

async fn opisy_pol() -> Json<&'static [OpisPola]> {
    Json(dok::POLA)
}

async fn stan_szablonu(State(stan): State<StanApi>, Sciezka(rodzaj): Sciezka<String>) -> Wynik<Json<StanSzablonu>> {
    Ok(Json(dok::stan(&stan.folder_danych, &rodzaj)?))
}

#[derive(Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
struct ParametryWgrania {
    nazwa: Option<String>,
}

async fn wgraj_szablon(
    State(stan): State<StanApi>,
    Sciezka(rodzaj): Sciezka<String>,
    Parametry(p): Parametry<ParametryWgrania>,
    Tresc(b): Tresc,
) -> Wynik<Json<StanSzablonu>> {
    let folder = stan.folder_danych.clone();
    let nazwa = p.nazwa.unwrap_or_else(|| format!("{rodzaj}.docx"));
    let s = tokio::task::spawn_blocking(move || dok::zapisz(&folder, &rodzaj, &nazwa, &b))
        .await
        .map_err(|e| BladApi::Wewnetrzny(e.to_string()))??;
    zdarzenie(&stan, "szablony_dokumentow", 0, Operacja::Zmiana);
    Ok(Json(s))
}

async fn usun_szablon(State(stan): State<StanApi>, Sciezka(rodzaj): Sciezka<String>) -> Wynik<Json<StanSzablonu>> {
    let s = dok::usun(&stan.folder_danych, &rodzaj)?;
    zdarzenie(&stan, "szablony_dokumentow", 0, Operacja::Zmiana);
    Ok(Json(s))
}

async fn plik_szablonu(State(stan): State<StanApi>, Sciezka(rodzaj): Sciezka<String>) -> Wynik<Response> {
    let (b, przykladowy) = dok::szablon(&stan.folder_danych, &rodzaj)?;
    let nazwa = if przykladowy {
        format!("Szablon_przykladowy_{rodzaj}.docx")
    } else {
        format!("Szablon_{rodzaj}.docx")
    };
    plik(b, MIME_DOCX, &nazwa)
}

async fn osoby(State(stan): State<StanApi>, Sciezka(id): Sciezka<i64>) -> Wynik<Json<Vec<OsobaKontaktowa>>> {
    Ok(Json(umowy::osoby_kontaktowe(&stan.pula, id).await?))
}

async fn zapisz_osoby(
    State(stan): State<StanApi>,
    Sciezka(id): Sciezka<i64>,
    Cialo(d): Cialo<DaneOsob>,
) -> Wynik<Json<Vec<OsobaKontaktowa>>> {
    let w = umowy::zapisz_osoby_kontaktowe(&stan.pula, id, d).await?;
    zdarzenie(&stan, "klienci", id, Operacja::Zmiana);
    Ok(Json(w))
}
