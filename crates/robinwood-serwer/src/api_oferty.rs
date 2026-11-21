//! Trasy API Fazy 2 (Generator Ofert): specyfikacje, oferty i wersje, szablony mebli, import
//! słowników, zdjęcia, eksport DOCX/PDF, dane firmy. Wszystkie pod `/api/v1`, chronione kluczem.
//!
//! Specyfikacja (wspólna z umowami):
//! * `GET  /specyfikacje/{id}` – drzewo sekcje → części → wiersze + zdjęcia
//! * `POST /specyfikacje/{id}/sekcje`, `POST /specyfikacje/{id}/sekcje/kolejnosc` `{ids}`
//! * `POST /specyfikacje/{id}/z-szablonu` `{szablon_id, nazwa?, czesci?}` – nowa sekcja z szablonu
//! * `PUT|DELETE /sekcje/{id}`, `POST /sekcje/{id}/kopiuj` `{cel_id?}`
//! * `POST /sekcje/{id}/czesci`, `POST /sekcje/{id}/czesci/kolejnosc`, `POST /sekcje/{id}/z-szablonu`
//! * `PUT|DELETE /czesci/{id}`, `POST /czesci/{id}/kopiuj` `{cel_id?}`
//! * `POST /czesci/{id}/wiersze`, `POST /czesci/{id}/wiersze/kolejnosc`
//! * `PUT|DELETE /wiersze/{id}`, `POST /wiersze/{id}/kopiuj-material`
//! * `POST /specyfikacje/{id}/zdjecia?sekcja_id=&czesc_id=&nazwa=&podpis=` (treść = bajty JPG/PNG)
//! * `PUT|DELETE /zdjecia/{id}`, `GET /pliki/{id}` (także `?klucz=` dla `<img src>`)
//!
//! Każda zmiana specyfikacji zwraca `{id, specyfikacja}` (całe aktualne drzewo).
//!
//! Oferty:
//! * `GET /oferty?status=&klient_id=&szukaj=`, `POST /oferty`
//! * `GET|PUT|DELETE /oferty/{id}` (PUT: zmiana inwestora), `PUT /oferty/{id}/status`
//! * `POST /oferty/{id}/wersje` `{z_wersji_id?}`, `GET /oferty/{id}/porownanie?z=&do=`
//! * `GET|PUT /wersje-oferty/{id}` – nagłówek, parametry wyceny, wycena, specyfikacja
//! * `GET /wersje-oferty/{id}/eksport/{pdf|docx}?zdjecia=1`
//!
//! Pozostałe: `GET /szablony-mebli`, `POST /import/slowniki`, `POST /import/szablony-mebli`,
//! `GET|PUT /ustawienia/firma`.

use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::Value;

use crate::api::StanApi;
use crate::bledy::{BladApi, Wynik};
use crate::firma::{self, DaneFirmy};
use crate::oferty::{self, PelnaWersja};
use crate::pliki;
use crate::specyfikacje::{self as spec, WynikZmiany};
use crate::szablony;
use crate::zdarzenia::{Operacja, Zdarzenie};
use crate::{eksport, slowniki};

pub fn trasy() -> Router<StanApi> {
    Router::new()
        // specyfikacja
        .route("/specyfikacje/{id}", get(pobierz_specyfikacje))
        .route("/specyfikacje/{id}/sekcje", post(dodaj_sekcje))
        .route(
            "/specyfikacje/{id}/sekcje/kolejnosc",
            post(kolejnosc_sekcji),
        )
        .route("/specyfikacje/{id}/z-szablonu", post(sekcja_z_szablonu))
        .route(
            "/specyfikacje/{id}/zdjecia",
            post(dodaj_zdjecie).layer(DefaultBodyLimit::max(pliki::MAKS_ROZMIAR + 1024)),
        )
        .route("/sekcje/{id}", put(zmien_sekcje).delete(usun_sekcje))
        .route("/sekcje/{id}/kopiuj", post(kopiuj_sekcje))
        .route("/sekcje/{id}/czesci", post(dodaj_czesc))
        .route("/sekcje/{id}/czesci/kolejnosc", post(kolejnosc_czesci))
        .route("/sekcje/{id}/z-szablonu", post(czesc_z_szablonu))
        .route("/czesci/{id}", put(zmien_czesc).delete(usun_czesc))
        .route("/czesci/{id}/kopiuj", post(kopiuj_czesc))
        .route("/czesci/{id}/wiersze", post(dodaj_wiersz))
        .route("/czesci/{id}/wiersze/kolejnosc", post(kolejnosc_wierszy))
        .route("/wiersze/{id}", put(zmien_wiersz).delete(usun_wiersz))
        .route("/wiersze/{id}/kopiuj-material", post(kopiuj_material))
        .route("/zdjecia/{id}", put(zmien_zdjecie).delete(usun_zdjecie))
        .route("/pliki/{id}", get(pobierz_plik))
        // oferty
        .route("/oferty", get(lista_ofert).post(dodaj_oferte))
        .route(
            "/oferty/{id}",
            get(szczegoly_oferty).put(zmien_oferte).delete(usun_oferte),
        )
        .route("/oferty/{id}/status", put(zmien_status))
        .route("/oferty/{id}/wersje", post(nowa_wersja))
        .route("/oferty/{id}/porownanie", get(porownaj))
        .route("/wersje-oferty/{id}", get(pobierz_wersje).put(zmien_wersje))
        .route("/wersje-oferty/{id}/eksport/{format}", get(eksportuj))
        // szablony, import, ustawienia
        .route("/szablony-mebli", get(lista_szablonow))
        .route(
            "/import/slowniki",
            post(import_slownikow).layer(DefaultBodyLimit::max(16 * 1024 * 1024)),
        )
        .route(
            "/import/szablony-mebli",
            post(import_szablonow).layer(DefaultBodyLimit::max(16 * 1024 * 1024)),
        )
        .route("/ustawienia/firma", get(pobierz_firme).put(zapisz_firme))
        .route("/slowniki-podpowiedzi", get(podpowiedzi))
}

fn zmiana_spec(stan: &StanApi, w: &WynikZmiany) {
    stan.zdarzenia.wyslij(Zdarzenie {
        zasob: "specyfikacje",
        id: w.specyfikacja.id,
        operacja: Operacja::Zmiana,
    });
    if let Some(wl) = &w.specyfikacja.wlasciciel {
        stan.zdarzenia.wyslij(Zdarzenie {
            zasob: "oferty",
            id: wl.nadrzedny_id,
            operacja: Operacja::Zmiana,
        });
    }
}

fn zmiana_oferty(stan: &StanApi, id: i64, operacja: Operacja) {
    stan.zdarzenia.wyslij(Zdarzenie {
        zasob: "oferty",
        id,
        operacja,
    });
}

type OdpSpec = Wynik<Json<WynikZmiany>>;

fn ok_spec(stan: &StanApi, w: WynikZmiany) -> OdpSpec {
    zmiana_spec(stan, &w);
    Ok(Json(w))
}

// --- Specyfikacja ------------------------------------------------------------

async fn pobierz_specyfikacje(
    State(stan): State<StanApi>,
    Path(id): Path<i64>,
) -> Wynik<Json<spec::Specyfikacja>> {
    Ok(Json(spec::pobierz(&stan.pula, id).await?))
}

async fn dodaj_sekcje(
    State(stan): State<StanApi>,
    Path(id): Path<i64>,
    Json(d): Json<spec::DaneSekcji>,
) -> OdpSpec {
    ok_spec(&stan, spec::dodaj_sekcje(&stan.pula, id, d).await?)
}

async fn zmien_sekcje(
    State(stan): State<StanApi>,
    Path(id): Path<i64>,
    Json(d): Json<spec::DaneSekcji>,
) -> OdpSpec {
    ok_spec(&stan, spec::zmien_sekcje(&stan.pula, id, d).await?)
}

async fn usun_sekcje(State(stan): State<StanApi>, Path(id): Path<i64>) -> OdpSpec {
    ok_spec(&stan, spec::usun_sekcje(&stan.pula, id).await?)
}

async fn kolejnosc_sekcji(
    State(stan): State<StanApi>,
    Path(id): Path<i64>,
    Json(k): Json<spec::Kolejnosc>,
) -> OdpSpec {
    ok_spec(&stan, spec::kolejnosc_sekcji(&stan.pula, id, k).await?)
}

async fn kopiuj_sekcje(
    State(stan): State<StanApi>,
    Path(id): Path<i64>,
    Json(d): Json<spec::DaneKopii>,
) -> OdpSpec {
    ok_spec(&stan, spec::kopiuj_sekcje(&stan.pula, id, d).await?)
}

async fn sekcja_z_szablonu(
    State(stan): State<StanApi>,
    Path(id): Path<i64>,
    Json(d): Json<szablony::WstawSekcje>,
) -> OdpSpec {
    ok_spec(&stan, szablony::wstaw_sekcje(&stan.pula, id, d).await?)
}

async fn dodaj_czesc(
    State(stan): State<StanApi>,
    Path(id): Path<i64>,
    Json(d): Json<spec::DaneCzesci>,
) -> OdpSpec {
    ok_spec(&stan, spec::dodaj_czesc(&stan.pula, id, d).await?)
}

async fn zmien_czesc(
    State(stan): State<StanApi>,
    Path(id): Path<i64>,
    Json(d): Json<spec::DaneCzesci>,
) -> OdpSpec {
    ok_spec(&stan, spec::zmien_czesc(&stan.pula, id, d).await?)
}

async fn usun_czesc(State(stan): State<StanApi>, Path(id): Path<i64>) -> OdpSpec {
    ok_spec(&stan, spec::usun_czesc(&stan.pula, id).await?)
}

async fn kolejnosc_czesci(
    State(stan): State<StanApi>,
    Path(id): Path<i64>,
    Json(k): Json<spec::Kolejnosc>,
) -> OdpSpec {
    ok_spec(&stan, spec::kolejnosc_czesci(&stan.pula, id, k).await?)
}

async fn kopiuj_czesc(
    State(stan): State<StanApi>,
    Path(id): Path<i64>,
    Json(d): Json<spec::DaneKopii>,
) -> OdpSpec {
    ok_spec(&stan, spec::kopiuj_czesc(&stan.pula, id, d).await?)
}

async fn czesc_z_szablonu(
    State(stan): State<StanApi>,
    Path(id): Path<i64>,
    Json(d): Json<szablony::WstawCzesc>,
) -> OdpSpec {
    ok_spec(&stan, szablony::wstaw_czesc(&stan.pula, id, d).await?)
}

async fn dodaj_wiersz(
    State(stan): State<StanApi>,
    Path(id): Path<i64>,
    Json(d): Json<spec::DaneWiersza>,
) -> OdpSpec {
    ok_spec(&stan, spec::dodaj_wiersz(&stan.pula, id, d).await?)
}

async fn zmien_wiersz(
    State(stan): State<StanApi>,
    Path(id): Path<i64>,
    Json(d): Json<spec::DaneWiersza>,
) -> OdpSpec {
    ok_spec(&stan, spec::zmien_wiersz(&stan.pula, id, d).await?)
}

async fn usun_wiersz(State(stan): State<StanApi>, Path(id): Path<i64>) -> OdpSpec {
    ok_spec(&stan, spec::usun_wiersz(&stan.pula, id).await?)
}

async fn kolejnosc_wierszy(
    State(stan): State<StanApi>,
    Path(id): Path<i64>,
    Json(k): Json<spec::Kolejnosc>,
) -> OdpSpec {
    ok_spec(&stan, spec::kolejnosc_wierszy(&stan.pula, id, k).await?)
}

async fn kopiuj_material(State(stan): State<StanApi>, Path(id): Path<i64>) -> OdpSpec {
    ok_spec(&stan, spec::kopiuj_material_w_dol(&stan.pula, id).await?)
}

// --- Zdjęcia i pliki -----------------------------------------------------------

async fn dodaj_zdjecie(
    State(stan): State<StanApi>,
    Path(id): Path<i64>,
    Query(p): Query<pliki::ParametryZdjecia>,
    tresc: Bytes,
) -> Wynik<(StatusCode, Json<WynikZmiany>)> {
    let w = pliki::dodaj_zdjecie(&stan.pula, &stan.folder_danych, id, p, &tresc).await?;
    zmiana_spec(&stan, &w);
    Ok((StatusCode::CREATED, Json(w)))
}

async fn zmien_zdjecie(
    State(stan): State<StanApi>,
    Path(id): Path<i64>,
    Json(d): Json<pliki::DaneZdjecia>,
) -> OdpSpec {
    ok_spec(&stan, pliki::zmien_zdjecie(&stan.pula, id, d).await?)
}

async fn usun_zdjecie(State(stan): State<StanApi>, Path(id): Path<i64>) -> OdpSpec {
    ok_spec(&stan, pliki::usun_zdjecie(&stan.pula, id).await?)
}

async fn pobierz_plik(State(stan): State<StanApi>, Path(id): Path<i64>) -> Wynik<Response> {
    let (mime, _nazwa, bajty) = pliki::odczytaj(&stan.pula, &stan.folder_danych, id).await?;
    Ok((
        [
            (header::CONTENT_TYPE, mime),
            (header::CACHE_CONTROL, "private, max-age=86400".into()),
        ],
        bajty,
    )
        .into_response())
}

// --- Oferty ------------------------------------------------------------------

async fn lista_ofert(
    State(stan): State<StanApi>,
    Query(f): Query<oferty::FiltrOfert>,
) -> Wynik<Json<Vec<oferty::OfertaNaLiscie>>> {
    Ok(Json(oferty::lista(&stan.pula, f).await?))
}

async fn dodaj_oferte(
    State(stan): State<StanApi>,
    Json(d): Json<oferty::DaneNowejOferty>,
) -> Wynik<(StatusCode, Json<PelnaWersja>)> {
    let p = oferty::dodaj(&stan.pula, d).await?;
    zmiana_oferty(&stan, p.oferta.id, Operacja::Dodanie);
    Ok((StatusCode::CREATED, Json(p)))
}

async fn szczegoly_oferty(
    State(stan): State<StanApi>,
    Path(id): Path<i64>,
) -> Wynik<Json<oferty::SzczegolyOferty>> {
    Ok(Json(oferty::szczegoly(&stan.pula, id).await?))
}

async fn zmien_oferte(
    State(stan): State<StanApi>,
    Path(id): Path<i64>,
    Json(d): Json<oferty::DaneOferty>,
) -> Wynik<Json<oferty::SzczegolyOferty>> {
    let s = oferty::zmien_klienta(&stan.pula, id, d).await?;
    zmiana_oferty(&stan, id, Operacja::Zmiana);
    Ok(Json(s))
}

async fn zmien_status(
    State(stan): State<StanApi>,
    Path(id): Path<i64>,
    Json(z): Json<oferty::ZmianaStatusu>,
) -> Wynik<Json<oferty::SzczegolyOferty>> {
    let s = oferty::zmien_status(&stan.pula, id, z).await?;
    zmiana_oferty(&stan, id, Operacja::Zmiana);
    Ok(Json(s))
}

async fn usun_oferte(State(stan): State<StanApi>, Path(id): Path<i64>) -> Wynik<StatusCode> {
    oferty::usun(&stan.pula, id).await?;
    zmiana_oferty(&stan, id, Operacja::Usuniecie);
    Ok(StatusCode::NO_CONTENT)
}

async fn nowa_wersja(
    State(stan): State<StanApi>,
    Path(id): Path<i64>,
    Json(d): Json<oferty::DaneNowejWersji>,
) -> Wynik<(StatusCode, Json<PelnaWersja>)> {
    let p = oferty::nowa_wersja(&stan.pula, id, d).await?;
    zmiana_oferty(&stan, id, Operacja::Zmiana);
    Ok((StatusCode::CREATED, Json(p)))
}

#[derive(Deserialize)]
struct ParametryPorownania {
    z: i64,
    #[serde(rename = "do")]
    do_: i64,
}

async fn porownaj(
    State(stan): State<StanApi>,
    Path(id): Path<i64>,
    Query(p): Query<ParametryPorownania>,
) -> Wynik<Json<oferty::Porownanie>> {
    Ok(Json(oferty::porownaj(&stan.pula, id, p.z, p.do_).await?))
}

async fn pobierz_wersje(
    State(stan): State<StanApi>,
    Path(id): Path<i64>,
) -> Wynik<Json<PelnaWersja>> {
    Ok(Json(oferty::pelna_wersja(&stan.pula, id).await?))
}

async fn zmien_wersje(
    State(stan): State<StanApi>,
    Path(id): Path<i64>,
    Json(d): Json<oferty::DaneWersji>,
) -> Wynik<Json<PelnaWersja>> {
    let p = oferty::zmien_wersje(&stan.pula, id, d).await?;
    zmiana_oferty(&stan, p.oferta.id, Operacja::Zmiana);
    Ok(Json(p))
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ParametryEksportu {
    zdjecia: Option<u8>,
}

async fn eksportuj(
    State(stan): State<StanApi>,
    Path((id, format)): Path<(i64, String)>,
    Query(p): Query<ParametryEksportu>,
) -> Wynik<Response> {
    let (mime, rozszerzenie) = match format.as_str() {
        "pdf" => ("application/pdf", "pdf"),
        "docx" => (
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            "docx",
        ),
        _ => {
            return Err(BladApi::NieprawidloweDane(
                "format eksportu: pdf albo docx".into(),
            ))
        }
    };
    let wersja = oferty::pelna_wersja(&stan.pula, id).await?;
    let dok = eksport::zbuduj(
        &stan.pula,
        &stan.folder_danych,
        id,
        p.zdjecia.unwrap_or(1) != 0,
    )
    .await?;
    let bajty = tokio::task::spawn_blocking(move || {
        if rozszerzenie == "pdf" {
            eksport::renderuj_pdf(&dok)
        } else {
            eksport::renderuj_docx(&dok)
        }
    })
    .await
    .map_err(eksport::blad_renderowania)??;
    let nazwa = eksport::nazwa_pliku(&wersja.oferta.numer, wersja.wersja.nr_wersji, rozszerzenie);
    let dyspozycja = HeaderValue::from_str(&format!("attachment; filename=\"{nazwa}\""))
        .map_err(eksport::blad_renderowania)?;
    Ok((
        [
            (header::CONTENT_TYPE, HeaderValue::from_static(mime)),
            (header::CONTENT_DISPOSITION, dyspozycja),
        ],
        bajty,
    )
        .into_response())
}

// --- Szablony, import, ustawienia ------------------------------------------------

async fn lista_szablonow(State(stan): State<StanApi>) -> Wynik<Json<Vec<szablony::Szablon>>> {
    Ok(Json(szablony::lista(&stan.pula).await?))
}

async fn import_slownikow(
    State(stan): State<StanApi>,
    Json(j): Json<Value>,
) -> Wynik<Json<szablony::RaportImportu>> {
    let r = szablony::importuj_slowniki(&stan.pula, &j).await?;
    stan.zdarzenia.wyslij(Zdarzenie {
        zasob: "slowniki",
        id: 0,
        operacja: Operacja::Zmiana,
    });
    Ok(Json(r))
}

async fn import_szablonow(
    State(stan): State<StanApi>,
    Json(j): Json<Value>,
) -> Wynik<Json<szablony::RaportImportu>> {
    let r = szablony::importuj_szablony(&stan.pula, &j).await?;
    stan.zdarzenia.wyslij(Zdarzenie {
        zasob: "szablony_mebli",
        id: 0,
        operacja: Operacja::Zmiana,
    });
    Ok(Json(r))
}

async fn pobierz_firme(State(stan): State<StanApi>) -> Wynik<Json<DaneFirmy>> {
    Ok(Json(firma::pobierz(&stan.pula).await?))
}

async fn zapisz_firme(
    State(stan): State<StanApi>,
    Json(d): Json<DaneFirmy>,
) -> Wynik<Json<DaneFirmy>> {
    let f = firma::zapisz(&stan.pula, d).await?;
    stan.zdarzenia.wyslij(Zdarzenie {
        zasob: "ustawienia",
        id: 0,
        operacja: Operacja::Zmiana,
    });
    Ok(Json(f))
}

/// Wszystkie podpowiedzi edytora specyfikacji jednym zapytaniem:
/// `{ "rodzaj_elementu": [...], "material": [...], ... }`.
async fn podpowiedzi(
    State(stan): State<StanApi>,
) -> Wynik<Json<std::collections::BTreeMap<String, Vec<slowniki::PozycjaSlownika>>>> {
    let mut wynik = std::collections::BTreeMap::new();
    for s in slowniki::lista_slownikow(&stan.pula).await? {
        let p = slowniki::pozycje(&stan.pula, &s.kod).await?;
        wynik.insert(s.kod, p);
    }
    Ok(Json(wynik))
}
