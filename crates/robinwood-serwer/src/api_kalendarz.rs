//! Trasy API silnika terminów (zlecenia, materiały, zadania kalendarza, czasy dostaw,
//! ustawienia odstępów, dni wolne). Kontrakt: `docs/ARCHITEKTURA.md`, rozdz. „Silnik terminów”.
//!
//! Każda zmiana danych rozsyła zdarzenie WebSocket (po zatwierdzeniu transakcji). Dla zasobu
//! `zadania_kalendarza` pole `id` to **id zlecenia**, którego zadania się zmieniły (`0` = wiele
//! zleceń / zadanie bez zlecenia).
//!
//! Zapis danych i wywołane nim przeliczenie harmonogramu idą w **jednej transakcji**: błąd bazy
//! wycofuje wszystko (500, nic nie zapisano); nieudane przeliczenie z powodu danych (np. brak
//! terminu montażu) nie blokuje zapisu – zlecenie dostaje znacznik `harmonogram_nieaktualny`.
//!
//! Wejście: nieznane pola są odrzucane, a błędy deserializacji treści, ścieżki i parametrów
//! zwracają 422 z JSON `{"blad": …}` po polsku – wspólne ekstraktory serwera `crate::wejscie`.

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post, put};
use axum::{Json, Router};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::api::StanApi;
use crate::bledy::{BladApi, Wynik};
use crate::kalendarz::{self, Harmonogram, Swieto, UstawieniaTerminow};
use crate::terminarz::{self as t, wej, Przeliczenie, ZadanieKalendarza};
use crate::wejscie::{Cialo, Parametry, Sciezka};
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
        .route("/zlecenia/{id}/przelicz", post(przelicz_zlecenie))
        .route(
            "/zamowienia-materialow/{id}",
            put(zmien_material).delete(usun_material),
        )
        .route(
            "/zamowienia-materialow/{id}/cofnij-zamowienie",
            post(cofnij_zamowienie),
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
        .route("/kalendarz/przelicz", post(przelicz_wszystko))
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

/// Zdarzenia po przeliczeniu wszystkich zleceń: zadania – tylko gdy faktycznie się zmieniły,
/// zlecenia – gdy dostały znacznik nieaktualności.
fn powiadom_o_przeliczeniu(stan: &StanApi, p: &t::WynikPrzeliczeniaWszystkich) {
    for id in &p.zmienione_zlecenia {
        zmienione_zadania(stan, Some(*id));
    }
    for n in &p.nieprzeliczone_zlecenia {
        zdarzenie(stan, "zlecenia", n.zlecenie_id, Operacja::Zmiana);
    }
}

#[derive(Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
struct ParametrDzis {
    #[serde(deserialize_with = "wej::data_opt")]
    dzis: Option<NaiveDate>,
    #[serde(deserialize_with = "wej::data_opt")]
    horyzont: Option<NaiveDate>,
    #[serde(deserialize_with = "wej::data_opt")]
    od: Option<NaiveDate>,
    #[serde(rename = "do", deserialize_with = "wej::data_opt")]
    do_: Option<NaiveDate>,
    rok: Option<i32>,
    /// Klucz API w parametrze (sprawdza go warstwa dostępu).
    #[serde(rename = "klucz")]
    _klucz: Option<String>,
}

// --- Zlecenia --------------------------------------------------------------

async fn lista_zlecen(State(s): State<StanApi>) -> Wynik<Json<Vec<t::Zlecenie>>> {
    Ok(Json(t::zlecenia(&s.pula).await?))
}

async fn pobierz_zlecenie(
    State(s): State<StanApi>,
    Sciezka(id): Sciezka<i64>,
) -> Wynik<Json<t::Zlecenie>> {
    Ok(Json(t::zlecenie(&mut *s.pula.acquire().await?, id).await?))
}

async fn dodaj_zlecenie(
    State(s): State<StanApi>,
    Cialo(d): Cialo<t::DaneZlecenia>,
) -> Wynik<(StatusCode, Json<t::Zlecenie>)> {
    let (z, _) = t::zapisz_zlecenie(&s.pula, None, d).await?;
    zdarzenie(&s, "zlecenia", z.id, Operacja::Dodanie);
    Ok((StatusCode::CREATED, Json(z)))
}

/// Zlecenie po zmianie; `harmonogram_przeliczony` = czy istniejący harmonogram przeliczono.
/// Gdy przeliczenie się nie udało: `harmonogram_przeliczony: false`, `harmonogram_nieaktualny: true`.
#[derive(Serialize)]
struct ZlecenieIPrzeliczenie {
    #[serde(flatten)]
    zlecenie: t::Zlecenie,
    harmonogram_przeliczony: bool,
}

async fn zmien_zlecenie(
    State(s): State<StanApi>,
    Sciezka(id): Sciezka<i64>,
    Cialo(d): Cialo<t::DaneZlecenia>,
) -> Wynik<Json<ZlecenieIPrzeliczenie>> {
    let (z, p) = t::zapisz_zlecenie(&s.pula, Some(id), d).await?;
    zdarzenie(&s, "zlecenia", id, Operacja::Zmiana);
    if p {
        zmienione_zadania(&s, Some(id));
    }
    Ok(Json(ZlecenieIPrzeliczenie {
        zlecenie: z,
        harmonogram_przeliczony: p,
    }))
}

async fn przelicz_zlecenie(
    State(s): State<StanApi>,
    Sciezka(id): Sciezka<i64>,
) -> Wynik<Json<ZlecenieIPrzeliczenie>> {
    let (z, p) = t::przelicz_zlecenie(&s.pula, id).await?;
    let przeliczono = matches!(p, Przeliczenie::Przeliczone(_));
    match p {
        Przeliczenie::Przeliczone(w) if w.dodane + w.zmienione + w.usuniete > 0 => {
            zmienione_zadania(&s, Some(id))
        }
        Przeliczenie::Nieaktualne(_) => zdarzenie(&s, "zlecenia", id, Operacja::Zmiana),
        _ => {}
    }
    Ok(Json(ZlecenieIPrzeliczenie {
        zlecenie: z,
        harmonogram_przeliczony: przeliczono,
    }))
}

// --- Materiały -------------------------------------------------------------

async fn lista_materialow(
    State(s): State<StanApi>,
    Sciezka(id): Sciezka<i64>,
) -> Wynik<Json<Vec<t::Material>>> {
    let mut c = s.pula.acquire().await?;
    t::zlecenie(&mut c, id).await?;
    Ok(Json(t::materialy(&mut c, id).await?))
}

fn po_zmianie_materialu(s: &StanApi, material_id: i64, zlecenie_id: i64, op: Operacja) {
    zdarzenie(s, "zamowienia_materialow", material_id, op);
    zdarzenie(s, "zlecenia", zlecenie_id, Operacja::Zmiana);
    zmienione_zadania(s, Some(zlecenie_id));
}

async fn dodaj_material(
    State(s): State<StanApi>,
    Sciezka(id): Sciezka<i64>,
    Cialo(d): Cialo<t::DaneMaterialu>,
) -> Wynik<(StatusCode, Json<t::Material>)> {
    let m = t::dodaj_material(&s.pula, id, d).await?;
    po_zmianie_materialu(&s, m.id, id, Operacja::Dodanie);
    Ok((StatusCode::CREATED, Json(m)))
}

async fn zmien_material(
    State(s): State<StanApi>,
    Sciezka(id): Sciezka<i64>,
    Cialo(z): Cialo<t::ZmianaMaterialu>,
) -> Wynik<Json<t::Material>> {
    let m = t::zmien_material(&s.pula, id, z).await?;
    po_zmianie_materialu(&s, id, m.zlecenie_id, Operacja::Zmiana);
    Ok(Json(m))
}

async fn cofnij_zamowienie(
    State(s): State<StanApi>,
    Sciezka(id): Sciezka<i64>,
) -> Wynik<Json<t::Material>> {
    let m = t::cofnij_zamowienie(&s.pula, id).await?;
    po_zmianie_materialu(&s, id, m.zlecenie_id, Operacja::Zmiana);
    Ok(Json(m))
}

async fn usun_material(State(s): State<StanApi>, Sciezka(id): Sciezka<i64>) -> Wynik<StatusCode> {
    let zl = t::usun_material(&s.pula, id).await?;
    po_zmianie_materialu(&s, id, zl, Operacja::Usuniecie);
    Ok(StatusCode::NO_CONTENT)
}

// --- Harmonogram -----------------------------------------------------------

async fn podglad_harmonogramu(
    State(s): State<StanApi>,
    Sciezka(id): Sciezka<i64>,
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
    Sciezka(id): Sciezka<i64>,
    Parametry(p): Parametry<ParametrDzis>,
) -> Wynik<Json<OdpowiedzGenerowania>> {
    let wynik = t::generuj(&s.pula, id).await;
    zdarzenie(&s, "zlecenia", id, Operacja::Zmiana);
    let wynik = wynik?;
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

async fn przelicz_wszystko(
    State(s): State<StanApi>,
) -> Wynik<Json<t::WynikPrzeliczeniaWszystkich>> {
    let p = t::przelicz_wszystkie(&s.pula).await?;
    powiadom_o_przeliczeniu(&s, &p);
    Ok(Json(p))
}

// --- Dostawcy, widełki, czasy dostaw ---------------------------------------

async fn lista_dostawcow(State(s): State<StanApi>) -> Wynik<Json<Vec<t::Dostawca>>> {
    Ok(Json(t::dostawcy(&s.pula).await?))
}

async fn dodaj_dostawce(
    State(s): State<StanApi>,
    Cialo(d): Cialo<t::DaneDostawcy>,
) -> Wynik<(StatusCode, Json<t::Dostawca>)> {
    let x = t::zapisz_dostawce(&s.pula, None, d).await?;
    zdarzenie(&s, "dostawcy", x.id, Operacja::Dodanie);
    Ok((StatusCode::CREATED, Json(x)))
}

async fn zmien_dostawce(
    State(s): State<StanApi>,
    Sciezka(id): Sciezka<i64>,
    Cialo(d): Cialo<t::DaneDostawcy>,
) -> Wynik<Json<t::Dostawca>> {
    let x = t::zapisz_dostawce(&s.pula, Some(id), d).await?;
    zdarzenie(&s, "dostawcy", id, Operacja::Zmiana);
    Ok(Json(x))
}

async fn zmien_widelki(
    State(s): State<StanApi>,
    Sciezka(id): Sciezka<i64>,
    Cialo(w): Cialo<t::WidelkiKategorii>,
) -> Wynik<StatusCode> {
    let p = t::zmien_widelki_kategorii(&s.pula, id, w).await?;
    zdarzenie(&s, "kategorie_materialow", id, Operacja::Zmiana);
    powiadom_o_przeliczeniu(&s, &p);
    Ok(StatusCode::NO_CONTENT)
}

async fn lista_czasow(State(s): State<StanApi>) -> Wynik<Json<Vec<t::CzasDostawy>>> {
    Ok(Json(t::czasy_dostaw(&s.pula).await?))
}

/// Odpowiedź z wynikiem przeliczenia wszystkich zleceń dołączonym do zapisanego rekordu.
#[derive(Serialize)]
struct ZPrzeliczeniem<T: Serialize> {
    #[serde(flatten)]
    rekord: T,
    #[serde(flatten)]
    przeliczenie: t::WynikPrzeliczeniaWszystkich,
}

async fn dodaj_czas(
    State(s): State<StanApi>,
    Cialo(d): Cialo<t::DaneCzasuDostawy>,
) -> Wynik<(StatusCode, Json<ZPrzeliczeniem<t::CzasDostawy>>)> {
    let (c, p) = t::zapisz_czas_dostawy(&s.pula, None, d).await?;
    zdarzenie(&s, "czasy_dostaw", c.id, Operacja::Dodanie);
    powiadom_o_przeliczeniu(&s, &p);
    Ok((
        StatusCode::CREATED,
        Json(ZPrzeliczeniem {
            rekord: c,
            przeliczenie: p,
        }),
    ))
}

async fn zmien_czas(
    State(s): State<StanApi>,
    Sciezka(id): Sciezka<i64>,
    Cialo(d): Cialo<t::DaneCzasuDostawy>,
) -> Wynik<Json<ZPrzeliczeniem<t::CzasDostawy>>> {
    let (c, p) = t::zapisz_czas_dostawy(&s.pula, Some(id), d).await?;
    zdarzenie(&s, "czasy_dostaw", id, Operacja::Zmiana);
    powiadom_o_przeliczeniu(&s, &p);
    Ok(Json(ZPrzeliczeniem {
        rekord: c,
        przeliczenie: p,
    }))
}

async fn usun_czas(State(s): State<StanApi>, Sciezka(id): Sciezka<i64>) -> Wynik<StatusCode> {
    let p = t::usun_czas_dostawy(&s.pula, id).await?;
    zdarzenie(&s, "czasy_dostaw", id, Operacja::Usuniecie);
    powiadom_o_przeliczeniu(&s, &p);
    Ok(StatusCode::NO_CONTENT)
}

// --- Zadania ---------------------------------------------------------------

async fn lista_zadan(
    State(s): State<StanApi>,
    Parametry(f): Parametry<t::FiltrZadan>,
) -> Wynik<Json<Vec<ZadanieKalendarza>>> {
    let dzis = f.dzis.unwrap_or_else(dzis_serwera);
    Ok(Json(t::zadania(&s.pula, &f, dzis).await?))
}

async fn pobierz_zadanie(
    State(s): State<StanApi>,
    Sciezka(id): Sciezka<i64>,
    Parametry(p): Parametry<ParametrDzis>,
) -> Wynik<Json<ZadanieKalendarza>> {
    Ok(Json(
        t::zadanie(&s.pula, id, p.dzis.unwrap_or_else(dzis_serwera)).await?,
    ))
}

async fn dodaj_zadanie(
    State(s): State<StanApi>,
    Cialo(d): Cialo<t::DaneZadaniaRecznego>,
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
    Sciezka(id): Sciezka<i64>,
    Cialo(d): Cialo<t::DaneZadaniaRecznego>,
) -> Wynik<Json<ZadanieKalendarza>> {
    let zl = d.zlecenie_id;
    t::zapisz_zadanie_reczne(&s.pula, Some(id), d).await?;
    zmienione_zadania(&s, zl);
    Ok(Json(t::zadanie(&s.pula, id, dzis_serwera()).await?))
}

async fn usun_zadanie(State(s): State<StanApi>, Sciezka(id): Sciezka<i64>) -> Wynik<StatusCode> {
    let zl = t::usun_zadanie_reczne(&s.pula, id).await?;
    zmienione_zadania(&s, zl);
    Ok(StatusCode::NO_CONTENT)
}

async fn wykonaj_zadanie(
    State(s): State<StanApi>,
    Sciezka(id): Sciezka<i64>,
    Parametry(p): Parametry<ParametrDzis>,
    Cialo(w): Cialo<t::Wykonanie>,
) -> Wynik<Json<ZadanieKalendarza>> {
    let dzis = p.dzis.unwrap_or_else(dzis_serwera);
    let zl = t::wykonaj(&s.pula, id, w, dzis).await?;
    zmienione_zadania(&s, zl);
    if let Some(zl) = zl {
        zdarzenie(&s, "zlecenia", zl, Operacja::Zmiana);
    }
    zdarzenie(&s, "zamowienia_materialow", 0, Operacja::Zmiana);
    Ok(Json(t::zadanie(&s.pula, id, dzis).await?))
}

async fn tydzien(
    State(s): State<StanApi>,
    Parametry(p): Parametry<ParametrDzis>,
) -> Wynik<Json<t::Tydzien>> {
    Ok(Json(
        t::tydzien(&s.pula, p.dzis.unwrap_or_else(dzis_serwera)).await?,
    ))
}

async fn do_zamowienia(
    State(s): State<StanApi>,
    Parametry(p): Parametry<ParametrDzis>,
) -> Wynik<Json<t::DoZamowienia>> {
    Ok(Json(
        t::do_zamowienia(&s.pula, p.dzis.unwrap_or_else(dzis_serwera), p.horyzont).await?,
    ))
}

async fn kolizje(
    State(s): State<StanApi>,
    Parametry(p): Parametry<ParametrDzis>,
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

async fn zmien_ustawienia(
    State(s): State<StanApi>,
    Cialo(z): Cialo<t::ZmianaUstawien>,
) -> Wynik<Json<ZPrzeliczeniem<UstawieniaTerminow>>> {
    let (u, p) = t::zmien_ustawienia(&s.pula, z).await?;
    zdarzenie(&s, "ustawienia", 0, Operacja::Zmiana);
    powiadom_o_przeliczeniu(&s, &p);
    Ok(Json(ZPrzeliczeniem {
        rekord: u,
        przeliczenie: p,
    }))
}

async fn lista_dni_wolnych(
    State(s): State<StanApi>,
    Parametry(p): Parametry<ParametrDzis>,
) -> Wynik<Json<Vec<t::DzienWolny>>> {
    Ok(Json(
        t::dni_wolne(&mut *s.pula.acquire().await?, p.od, p.do_).await?,
    ))
}

async fn ustaw_dzien_wolny(
    State(s): State<StanApi>,
    Sciezka(data): Sciezka<String>,
    Cialo(d): Cialo<t::DaneDniaWolnego>,
) -> Wynik<Json<ZPrzeliczeniem<t::DzienWolny>>> {
    let (w, p) = t::ustaw_dzien_wolny(&s.pula, &data, d).await?;
    zdarzenie(&s, "dni_wolne", 0, Operacja::Zmiana);
    powiadom_o_przeliczeniu(&s, &p);
    Ok(Json(ZPrzeliczeniem {
        rekord: w,
        przeliczenie: p,
    }))
}

async fn usun_dzien_wolny(
    State(s): State<StanApi>,
    Sciezka(data): Sciezka<String>,
) -> Wynik<StatusCode> {
    let p = t::usun_dzien_wolny(&s.pula, &data).await?;
    zdarzenie(&s, "dni_wolne", 0, Operacja::Usuniecie);
    powiadom_o_przeliczeniu(&s, &p);
    Ok(StatusCode::NO_CONTENT)
}

async fn swieta(Parametry(p): Parametry<ParametrDzis>) -> Wynik<Json<Vec<Swieto>>> {
    let rok = p
        .rok
        .unwrap_or_else(|| chrono::Datelike::year(&dzis_serwera()));
    if !kalendarz::obslugiwany_rok(rok) {
        return Err(BladApi::NieprawidloweDane(format!(
            "rok {rok} spoza obsługiwanego zakresu {}–{}",
            kalendarz::ROK_MIN,
            kalendarz::ROK_MAX
        )));
    }
    Ok(Json(kalendarz::swieta(rok)))
}
