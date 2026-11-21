//! Trasy API silnika terminów (zlecenia, materiały, zadania kalendarza, czasy dostaw,
//! ustawienia odstępów, dni wolne). Kontrakt: `docs/ARCHITEKTURA.md`, rozdz. „Silnik terminów”.
//!
//! Każda zmiana danych rozsyła zdarzenie WebSocket. Dla zasobu `zadania_kalendarza` pole `id`
//! to **id zlecenia**, którego zadania się zmieniły (`0` = wiele zleceń / zadanie bez zlecenia).

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, put};
use axum::{Json, Router};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::api::StanApi;
use crate::bledy::Wynik;
use crate::kalendarz::{self, Harmonogram, Swieto, UstawieniaTerminow};
use crate::terminarz::{self as t, ZadanieKalendarza};
use crate::zdarzenia::{Operacja, Zdarzenie};

pub fn trasy() -> Router<StanApi> {
    Router::new()
        .route("/zlecenia", get(lista_zlecen).post(dodaj_zlecenie))
        .route("/zlecenia/{id}", get(pobierz_zlecenie).put(zmien_zlecenie))
        .route(
            "/zlecenia/{id}/materialy",
            get(lista_materialow).post(dodaj_material),
        )
        .route(
            "/zlecenia/{id}/harmonogram",
            get(podglad_harmonogramu).post(generuj_harmonogram),
        )
        .route(
            "/zamowienia-materialow/{id}",
            put(zmien_material).delete(usun_material),
        )
        .route("/dostawcy", get(lista_dostawcow).post(dodaj_dostawce))
        .route("/dostawcy/{id}", put(zmien_dostawce))
        .route("/kategorie-materialow/{id}/widelki", put(zmien_widelki))
        .route("/czasy-dostaw", get(lista_czasow).post(dodaj_czas))
        .route("/czasy-dostaw/{id}", put(zmien_czas).delete(usun_czas))
        .route("/kalendarz/zadania", get(lista_zadan).post(dodaj_zadanie))
        .route(
            "/kalendarz/zadania/{id}",
            get(pobierz_zadanie).put(zmien_zadanie).delete(usun_zadanie),
        )
        .route("/kalendarz/zadania/{id}/wykonanie", put(wykonaj_zadanie))
        .route("/kalendarz/tydzien", get(tydzien))
        .route("/kalendarz/do-zamowienia", get(do_zamowienia))
        .route("/kalendarz/kolizje", get(kolizje))
        .route("/kalendarz/statystyka-dostaw", get(statystyka))
        .route(
            "/kalendarz/ustawienia",
            get(pobierz_ustawienia).put(zmien_ustawienia),
        )
        .route("/kalendarz/dni-wolne", get(lista_dni_wolnych))
        .route(
            "/kalendarz/dni-wolne/{data}",
            put(ustaw_dzien_wolny).delete(usun_dzien_wolny),
        )
        .route("/kalendarz/swieta", get(swieta))
        .route(
            "/kalendarz/przelicz",
            axum::routing::post(przelicz_wszystko),
        )
}

fn dzis_serwera() -> NaiveDate {
    chrono::Local::now().date_naive()
}

fn zdarzenie(stan: &StanApi, zasob: &'static str, id: i64, operacja: Operacja) {
    stan.zdarzenia.wyslij(Zdarzenie {
        zasob,
        id,
        operacja,
    });
}

fn zmienione_zadania(stan: &StanApi, zlecenie_id: Option<i64>) {
    zdarzenie(
        stan,
        "zadania_kalendarza",
        zlecenie_id.unwrap_or(0),
        Operacja::Zmiana,
    );
}

async fn przelicz_wszystko_i_powiadom(stan: &StanApi) -> Wynik<Vec<i64>> {
    let ids = t::przelicz_wszystkie(&stan.pula).await?;
    for id in &ids {
        zmienione_zadania(stan, Some(*id));
    }
    Ok(ids)
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ParametrDzis {
    dzis: Option<NaiveDate>,
    horyzont: Option<NaiveDate>,
    od: Option<NaiveDate>,
    #[serde(rename = "do")]
    do_: Option<NaiveDate>,
    rok: Option<i32>,
}

// --- Zlecenia --------------------------------------------------------------

async fn lista_zlecen(State(s): State<StanApi>) -> Wynik<Json<Vec<t::Zlecenie>>> {
    Ok(Json(t::zlecenia(&s.pula).await?))
}

async fn pobierz_zlecenie(
    State(s): State<StanApi>,
    Path(id): Path<i64>,
) -> Wynik<Json<t::Zlecenie>> {
    Ok(Json(t::zlecenie(&mut *s.pula.acquire().await?, id).await?))
}

async fn dodaj_zlecenie(
    State(s): State<StanApi>,
    Json(d): Json<t::DaneZlecenia>,
) -> Wynik<(StatusCode, Json<t::Zlecenie>)> {
    let z = t::zapisz_zlecenie(&s.pula, None, d).await?;
    zdarzenie(&s, "zlecenia", z.id, Operacja::Dodanie);
    Ok((StatusCode::CREATED, Json(z)))
}

/// Zlecenie po zmianie; `harmonogram_przeliczony` = czy istniejący harmonogram przeliczono.
#[derive(Serialize)]
struct ZlecenieIPrzeliczenie {
    #[serde(flatten)]
    zlecenie: t::Zlecenie,
    harmonogram_przeliczony: bool,
}

async fn zmien_zlecenie(
    State(s): State<StanApi>,
    Path(id): Path<i64>,
    Json(d): Json<t::DaneZlecenia>,
) -> Wynik<Json<ZlecenieIPrzeliczenie>> {
    let z = t::zapisz_zlecenie(&s.pula, Some(id), d).await?;
    zdarzenie(&s, "zlecenia", id, Operacja::Zmiana);
    let p = t::przelicz_jesli_jest(&s.pula, id).await?;
    if p {
        zmienione_zadania(&s, Some(id));
    }
    Ok(Json(ZlecenieIPrzeliczenie {
        zlecenie: z,
        harmonogram_przeliczony: p,
    }))
}

// --- Materiały -------------------------------------------------------------

async fn lista_materialow(
    State(s): State<StanApi>,
    Path(id): Path<i64>,
) -> Wynik<Json<Vec<t::Material>>> {
    let mut c = s.pula.acquire().await?;
    t::zlecenie(&mut c, id).await?;
    Ok(Json(t::materialy(&mut c, id).await?))
}

async fn po_zmianie_materialu(
    s: &StanApi,
    material_id: i64,
    zlecenie_id: i64,
    op: Operacja,
) -> Wynik<()> {
    zdarzenie(s, "zamowienia_materialow", material_id, op);
    if t::przelicz_jesli_jest(&s.pula, zlecenie_id).await? {
        zmienione_zadania(s, Some(zlecenie_id));
    }
    Ok(())
}

async fn dodaj_material(
    State(s): State<StanApi>,
    Path(id): Path<i64>,
    Json(d): Json<t::DaneMaterialu>,
) -> Wynik<(StatusCode, Json<t::Material>)> {
    let m = t::zapisz_material(&s.pula, Some(id), None, d).await?;
    po_zmianie_materialu(&s, m.id, id, Operacja::Dodanie).await?;
    let m = t::material(&mut *s.pula.acquire().await?, m.id).await?;
    Ok((StatusCode::CREATED, Json(m)))
}

async fn zmien_material(
    State(s): State<StanApi>,
    Path(id): Path<i64>,
    Json(d): Json<t::DaneMaterialu>,
) -> Wynik<Json<t::Material>> {
    let m = t::zapisz_material(&s.pula, None, Some(id), d).await?;
    po_zmianie_materialu(&s, id, m.zlecenie_id, Operacja::Zmiana).await?;
    Ok(Json(t::material(&mut *s.pula.acquire().await?, id).await?))
}

async fn usun_material(State(s): State<StanApi>, Path(id): Path<i64>) -> Wynik<StatusCode> {
    let zl = t::usun_material(&s.pula, id).await?;
    po_zmianie_materialu(&s, id, zl, Operacja::Usuniecie).await?;
    Ok(StatusCode::NO_CONTENT)
}

// --- Harmonogram -----------------------------------------------------------

async fn podglad_harmonogramu(
    State(s): State<StanApi>,
    Path(id): Path<i64>,
) -> Wynik<Json<Harmonogram>> {
    Ok(Json(t::podglad(&s.pula, id).await?))
}

#[derive(Serialize)]
struct OdpowiedzGenerowania {
    #[serde(flatten)]
    wynik: t::WynikGenerowania,
    /// Zadania zlecenia po zapisie, ze stanami na dziś.
    zadania_zapisane: Vec<ZadanieKalendarza>,
}

async fn generuj_harmonogram(
    State(s): State<StanApi>,
    Path(id): Path<i64>,
    Query(p): Query<ParametrDzis>,
) -> Wynik<Json<OdpowiedzGenerowania>> {
    let wynik = t::generuj(&s.pula, id).await?;
    zmienione_zadania(&s, Some(id));
    zdarzenie(&s, "zamowienia_materialow", 0, Operacja::Zmiana);
    let f = t::FiltrZadan {
        zlecenie_id: Some(id),
        ..Default::default()
    };
    let zadania_zapisane = t::zadania(&s.pula, &f, p.dzis.unwrap_or_else(dzis_serwera)).await?;
    Ok(Json(OdpowiedzGenerowania {
        wynik,
        zadania_zapisane,
    }))
}

async fn przelicz_wszystko(State(s): State<StanApi>) -> Wynik<Json<serde_json::Value>> {
    let ids = przelicz_wszystko_i_powiadom(&s).await?;
    Ok(Json(serde_json::json!({ "przeliczone_zlecenia": ids })))
}

// --- Dostawcy, widełki, czasy dostaw ---------------------------------------

async fn lista_dostawcow(State(s): State<StanApi>) -> Wynik<Json<Vec<t::Dostawca>>> {
    Ok(Json(t::dostawcy(&s.pula).await?))
}

async fn dodaj_dostawce(
    State(s): State<StanApi>,
    Json(d): Json<t::DaneDostawcy>,
) -> Wynik<(StatusCode, Json<t::Dostawca>)> {
    let x = t::zapisz_dostawce(&s.pula, None, d).await?;
    zdarzenie(&s, "dostawcy", x.id, Operacja::Dodanie);
    Ok((StatusCode::CREATED, Json(x)))
}

async fn zmien_dostawce(
    State(s): State<StanApi>,
    Path(id): Path<i64>,
    Json(d): Json<t::DaneDostawcy>,
) -> Wynik<Json<t::Dostawca>> {
    let x = t::zapisz_dostawce(&s.pula, Some(id), d).await?;
    zdarzenie(&s, "dostawcy", id, Operacja::Zmiana);
    Ok(Json(x))
}

async fn zmien_widelki(
    State(s): State<StanApi>,
    Path(id): Path<i64>,
    Json(w): Json<t::WidelkiKategorii>,
) -> Wynik<StatusCode> {
    t::zmien_widelki_kategorii(&s.pula, id, w).await?;
    zdarzenie(&s, "kategorie_materialow", id, Operacja::Zmiana);
    przelicz_wszystko_i_powiadom(&s).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn lista_czasow(State(s): State<StanApi>) -> Wynik<Json<Vec<t::CzasDostawy>>> {
    Ok(Json(t::czasy_dostaw(&s.pula).await?))
}

async fn dodaj_czas(
    State(s): State<StanApi>,
    Json(d): Json<t::DaneCzasuDostawy>,
) -> Wynik<(StatusCode, Json<t::CzasDostawy>)> {
    let c = t::zapisz_czas_dostawy(&s.pula, None, d).await?;
    zdarzenie(&s, "czasy_dostaw", c.id, Operacja::Dodanie);
    przelicz_wszystko_i_powiadom(&s).await?;
    Ok((StatusCode::CREATED, Json(c)))
}

async fn zmien_czas(
    State(s): State<StanApi>,
    Path(id): Path<i64>,
    Json(d): Json<t::DaneCzasuDostawy>,
) -> Wynik<Json<t::CzasDostawy>> {
    let c = t::zapisz_czas_dostawy(&s.pula, Some(id), d).await?;
    zdarzenie(&s, "czasy_dostaw", id, Operacja::Zmiana);
    przelicz_wszystko_i_powiadom(&s).await?;
    Ok(Json(c))
}

async fn usun_czas(State(s): State<StanApi>, Path(id): Path<i64>) -> Wynik<StatusCode> {
    t::usun_czas_dostawy(&s.pula, id).await?;
    zdarzenie(&s, "czasy_dostaw", id, Operacja::Usuniecie);
    przelicz_wszystko_i_powiadom(&s).await?;
    Ok(StatusCode::NO_CONTENT)
}

// --- Zadania ---------------------------------------------------------------

async fn lista_zadan(
    State(s): State<StanApi>,
    Query(f): Query<t::FiltrZadan>,
) -> Wynik<Json<Vec<ZadanieKalendarza>>> {
    let dzis = f.dzis.unwrap_or_else(dzis_serwera);
    Ok(Json(t::zadania(&s.pula, &f, dzis).await?))
}

async fn pobierz_zadanie(
    State(s): State<StanApi>,
    Path(id): Path<i64>,
    Query(p): Query<ParametrDzis>,
) -> Wynik<Json<ZadanieKalendarza>> {
    Ok(Json(
        t::zadanie(&s.pula, id, p.dzis.unwrap_or_else(dzis_serwera)).await?,
    ))
}

async fn dodaj_zadanie(
    State(s): State<StanApi>,
    Json(d): Json<t::DaneZadaniaRecznego>,
) -> Wynik<(StatusCode, Json<ZadanieKalendarza>)> {
    let zl = d.zlecenie_id;
    let id = t::zapisz_zadanie_reczne(&s.pula, None, d).await?;
    zmienione_zadania(&s, zl);
    Ok((
        StatusCode::CREATED,
        Json(t::zadanie(&s.pula, id, dzis_serwera()).await?),
    ))
}

async fn zmien_zadanie(
    State(s): State<StanApi>,
    Path(id): Path<i64>,
    Json(d): Json<t::DaneZadaniaRecznego>,
) -> Wynik<Json<ZadanieKalendarza>> {
    let zl = d.zlecenie_id;
    t::zapisz_zadanie_reczne(&s.pula, Some(id), d).await?;
    zmienione_zadania(&s, zl);
    Ok(Json(t::zadanie(&s.pula, id, dzis_serwera()).await?))
}

async fn usun_zadanie(State(s): State<StanApi>, Path(id): Path<i64>) -> Wynik<StatusCode> {
    let zl = t::usun_zadanie_reczne(&s.pula, id).await?;
    zmienione_zadania(&s, zl);
    Ok(StatusCode::NO_CONTENT)
}

async fn wykonaj_zadanie(
    State(s): State<StanApi>,
    Path(id): Path<i64>,
    Query(p): Query<ParametrDzis>,
    Json(w): Json<t::Wykonanie>,
) -> Wynik<Json<ZadanieKalendarza>> {
    let zl = t::wykonaj(&s.pula, id, w).await?;
    zmienione_zadania(&s, zl);
    zdarzenie(&s, "zamowienia_materialow", 0, Operacja::Zmiana);
    Ok(Json(
        t::zadanie(&s.pula, id, p.dzis.unwrap_or_else(dzis_serwera)).await?,
    ))
}

async fn tydzien(
    State(s): State<StanApi>,
    Query(p): Query<ParametrDzis>,
) -> Wynik<Json<t::Tydzien>> {
    Ok(Json(
        t::tydzien(&s.pula, p.dzis.unwrap_or_else(dzis_serwera)).await?,
    ))
}

async fn do_zamowienia(
    State(s): State<StanApi>,
    Query(p): Query<ParametrDzis>,
) -> Wynik<Json<t::DoZamowienia>> {
    Ok(Json(
        t::do_zamowienia(&s.pula, p.dzis.unwrap_or_else(dzis_serwera), p.horyzont).await?,
    ))
}

async fn kolizje(
    State(s): State<StanApi>,
    Query(p): Query<ParametrDzis>,
) -> Wynik<Json<Vec<t::KolizjaZlecen>>> {
    Ok(Json(t::kolizje(&s.pula, p.od, p.do_).await?))
}

async fn statystyka(State(s): State<StanApi>) -> Wynik<Json<Vec<t::StatystykaPary>>> {
    Ok(Json(t::statystyka_dostaw(&s.pula).await?))
}

// --- Ustawienia i dni wolne --------------------------------------------------

async fn pobierz_ustawienia(State(s): State<StanApi>) -> Wynik<Json<UstawieniaTerminow>> {
    Ok(Json(t::ustawienia(&mut *s.pula.acquire().await?).await?))
}

#[derive(Serialize)]
struct UstawieniaIPrzeliczenie {
    #[serde(flatten)]
    ustawienia: UstawieniaTerminow,
    przeliczone_zlecenia: Vec<i64>,
}

async fn zmien_ustawienia(
    State(s): State<StanApi>,
    Json(z): Json<t::ZmianaUstawien>,
) -> Wynik<Json<UstawieniaIPrzeliczenie>> {
    let u = t::zmien_ustawienia(&s.pula, z).await?;
    zdarzenie(&s, "ustawienia", 0, Operacja::Zmiana);
    let ids = przelicz_wszystko_i_powiadom(&s).await?;
    Ok(Json(UstawieniaIPrzeliczenie {
        ustawienia: u,
        przeliczone_zlecenia: ids,
    }))
}

async fn lista_dni_wolnych(
    State(s): State<StanApi>,
    Query(p): Query<ParametrDzis>,
) -> Wynik<Json<Vec<t::DzienWolny>>> {
    Ok(Json(
        t::dni_wolne(&mut *s.pula.acquire().await?, p.od, p.do_).await?,
    ))
}

async fn ustaw_dzien_wolny(
    State(s): State<StanApi>,
    Path(data): Path<String>,
    Json(d): Json<t::DaneDniaWolnego>,
) -> Wynik<Json<t::DzienWolny>> {
    let w = t::ustaw_dzien_wolny(&s.pula, &data, d).await?;
    zdarzenie(&s, "dni_wolne", 0, Operacja::Zmiana);
    przelicz_wszystko_i_powiadom(&s).await?;
    Ok(Json(w))
}

async fn usun_dzien_wolny(State(s): State<StanApi>, Path(data): Path<String>) -> Wynik<StatusCode> {
    t::usun_dzien_wolny(&s.pula, &data).await?;
    zdarzenie(&s, "dni_wolne", 0, Operacja::Usuniecie);
    przelicz_wszystko_i_powiadom(&s).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn swieta(Query(p): Query<ParametrDzis>) -> Json<Vec<Swieto>> {
    let rok = p
        .rok
        .unwrap_or_else(|| chrono::Datelike::year(&dzis_serwera()));
    Json(kalendarz::swieta(rok))
}
