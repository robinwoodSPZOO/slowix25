//! Trasy pomocnicze dla interfejsu kalendarza (Faza 4b), których brakowało w API silnika terminów:
//!
//! * `POST /import/kategorie-materialow` – import wierszy pliku `kategorie-materialow.csv`
//!   (kategorie, dostawcy, czasy dostaw) w **jednej transakcji** z jednym przeliczeniem wszystkich
//!   zleceń; wiersze błędne są pomijane z powodem (raport), poprawne – zapisywane,
//! * `POST /kalendarz/dni-wolne/zakres` i `POST /kalendarz/dni-wolne/usun-zakres` – zakres dni wolnych
//!   (np. urlop 2 tygodnie) jednym zapisem i jednym przeliczeniem zamiast N osobnych,
//! * `GET /kalendarz/wiersze-specyfikacji?ids=1,2` – opis wierszy „do potwierdzenia” (sekcja, część,
//!   rodzaj, materiał, opis) do okna „zamówić mimo to?”.
//!
//! Plik CSV parsuje interfejs (`src/moduly/kalendarz/importCsv.ts`); serwer i tak sprawdza każdy wiersz.

use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use sqlx::SqliteConnection;

use crate::api::StanApi;
use crate::bledy::{BladApi, Wynik};
use std::collections::HashMap;

use crate::kalendarz;
use crate::nazwy;
use crate::terminarz::{self as t, WynikPrzeliczeniaWszystkich};
use crate::wejscie::{Cialo, Parametry};
use crate::zdarzenia::{Operacja, Zdarzenie};

/// Największa liczba wierszy jednego importu i dni jednego zakresu.
pub const MAKS_WIERSZY_IMPORTU: usize = 5000;
pub const MAKS_DNI_ZAKRESU: i64 = 366;
/// Największy czas dostawy przyjmowany z pliku (dni).
pub const MAKS_DNI_DOSTAWY: i64 = 365;

pub fn trasy() -> Router<StanApi> {
    Router::new()
        .route("/import/kategorie-materialow", post(importuj))
        .route("/kalendarz/dni-wolne/zakres", post(dodaj_zakres))
        .route("/kalendarz/dni-wolne/usun-zakres", post(usun_zakres))
        .route("/kalendarz/dni-wolne/kolizje", post(podglad_kolizji))
        .route("/kalendarz/wiersze-specyfikacji", get(wiersze_specyfikacji))
}

fn wyslij(s: &StanApi, zasob: &'static str, id: i64) {
    s.zdarzenia.wyslij(Zdarzenie {
        zasob,
        id,
        operacja: Operacja::Zmiana,
    });
}

fn powiadom(s: &StanApi, p: &WynikPrzeliczeniaWszystkich) {
    for id in &p.zmienione_zlecenia {
        wyslij(s, "zadania_kalendarza", *id);
    }
    for n in &p.nieprzeliczone_zlecenia {
        wyslij(s, "zlecenia", n.zlecenie_id);
    }
}

async fn do_dziennika(
    c: &mut SqliteConnection,
    tabela: &str,
    id: i64,
    operacja: &str,
    opis: &str,
) -> Wynik<()> {
    sqlx::query(
        "INSERT INTO dziennik_zmian (tabela, rekord_id, operacja, opis) VALUES (?1, ?2, ?3, ?4)",
    )
    .bind(tabela)
    .bind(id)
    .bind(operacja)
    .bind(opis)
    .execute(&mut *c)
    .await?;
    Ok(())
}

// --- Import kategorii, dostawców i czasów dostaw ------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WierszImportu {
    /// Numer wiersza w pliku (do raportu); brak = kolejny numer w liście.
    #[serde(default)]
    pub wiersz: Option<usize>,
    pub kategoria: String,
    #[serde(default)]
    pub przykladowe_materialy: Option<String>,
    #[serde(default)]
    pub dostawca: Option<String>,
    #[serde(default)]
    pub czas_dostawy_min_dni: Option<i64>,
    #[serde(default)]
    pub czas_dostawy_max_dni: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DaneImportu {
    pub wiersze: Vec<WierszImportu>,
    /// `false` (domyślnie) – import tylko UZUPEŁNIA: dodaje brakujące kategorie, dostawców i czasy
    /// dostaw, wpisuje widełki tylko tam, gdzie ich nie ustawiono (0–0); ustawionych nie zmienia.
    /// `true` – nadpisuje także ustawione widełki kategorii i par kategoria + dostawca.
    #[serde(default)]
    pub nadpisz: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PominietyWiersz {
    pub wiersz: usize,
    pub powod: String,
}

/// Raport importu. Wszystkie liczniki liczą tylko **faktyczne** zmiany w bazie.
#[derive(Debug, Clone, Serialize, Default)]
pub struct RaportImportuKategorii {
    pub kategorie_dodane: usize,
    /// Istniejące kategorie, w których coś się zmieniło (opis, domyślny dostawca, widełki).
    pub kategorie_zaktualizowane: usize,
    pub dostawcy_dodani: usize,
    /// Wpisane albo zmienione widełki (kategorii lub pary kategoria + dostawca).
    pub czasy_dostaw_ustawione: usize,
    /// Wiersze bez żadnej zmiany (wszystko już tak było).
    pub bez_zmian: usize,
    /// Wiersze, w których plik podaje inne widełki niż ustawione – pozostawiono ustawione
    /// (tryb uzupełniania); `powod` mówi, co jest w bazie, a co w pliku.
    pub pozostawione: Vec<PominietyWiersz>,
    /// Wiersze bez czasu dostawy (kategoria/dostawca zapisane, widełki „nie ustawiono”).
    pub bez_czasu_dostawy: Vec<usize>,
    pub pominiete: Vec<PominietyWiersz>,
    #[serde(flatten)]
    pub przeliczenie: WynikPrzeliczeniaWszystkich,
}

fn bez_pustych(s: Option<String>) -> Option<String> {
    s.map(|x| nazwy::oczysc(&x)).filter(|x| !x.is_empty())
}

/// Sprawdza widełki wiersza: obie wartości albo żadna, 0 ≤ min ≤ max ≤ 365.
pub fn sprawdz_wiersz(w: &WierszImportu) -> Result<Option<(i64, i64)>, String> {
    if w.kategoria.trim().is_empty() {
        return Err("brak nazwy kategorii".into());
    }
    if nazwy::oczysc(&w.kategoria).chars().count() > nazwy::MAKS_NAZWY {
        return Err(format!(
            "nazwa kategorii dłuższa niż {} znaków",
            nazwy::MAKS_NAZWY
        ));
    }
    if w.dostawca
        .as_deref()
        .is_some_and(|d| nazwy::oczysc(d).chars().count() > nazwy::MAKS_NAZWY)
    {
        return Err(format!(
            "nazwa dostawcy dłuższa niż {} znaków",
            nazwy::MAKS_NAZWY
        ));
    }
    match (w.czas_dostawy_min_dni, w.czas_dostawy_max_dni) {
        (None, None) => Ok(None),
        (Some(_), None) | (None, Some(_)) => Err(
            "podano tylko jeden z czasów dostawy (najkrótszy albo najdłuższy) – uzupełnij oba"
                .into(),
        ),
        (Some(a), Some(b)) => {
            if a < 0 || b < 0 {
                Err("czas dostawy nie może być ujemny".into())
            } else if a > MAKS_DNI_DOSTAWY || b > MAKS_DNI_DOSTAWY {
                Err(format!("czas dostawy ponad {MAKS_DNI_DOSTAWY} dni"))
            } else if a > b {
                Err(format!(
                    "najkrótszy czas dostawy ({a}) dłuższy niż najdłuższy ({b})"
                ))
            } else {
                Ok(Some((a, b)))
            }
        }
    }
}

/// Dostawcy wg klucza nazwy (wczytani raz; nowi dopisywani w trakcie importu).
async fn znajdz_lub_dodaj_dostawce(
    c: &mut SqliteConnection,
    znani: &mut HashMap<String, i64>,
    nazwa: &str,
    raport: &mut RaportImportuKategorii,
) -> Wynik<i64> {
    if let Some(id) = znani.get(&nazwy::klucz(nazwa)) {
        return Ok(*id);
    }
    let id = sqlx::query("INSERT INTO dostawcy (nazwa) VALUES (?1)")
        .bind(nazwa)
        .execute(&mut *c)
        .await?
        .last_insert_rowid();
    do_dziennika(
        c,
        "dostawcy",
        id,
        "dodanie",
        "import kategorie-materialow.csv",
    )
    .await?;
    znani.insert(nazwy::klucz(nazwa), id);
    raport.dostawcy_dodani += 1;
    Ok(id)
}

/// (id, nazwa, opis, domyślny dostawca, min, max)
type WierszKategorii = (i64, String, Option<String>, Option<i64>, i64, i64);

struct Kategoria {
    id: i64,
    opis: Option<String>,
    domyslny: Option<i64>,
    min: i64,
    max: i64,
}

pub async fn importuj_kategorie(
    pula: &sqlx::SqlitePool,
    d: DaneImportu,
) -> Wynik<RaportImportuKategorii> {
    if d.wiersze.len() > MAKS_WIERSZY_IMPORTU {
        return Err(BladApi::NieprawidloweDane(format!(
            "plik ma {} wierszy – najwyżej {MAKS_WIERSZY_IMPORTU}",
            d.wiersze.len()
        )));
    }
    let nadpisz = d.nadpisz;
    let mut raport = RaportImportuKategorii::default();
    let mut tx = crate::baza::transakcja_zapisu(pula).await?;

    let mut kategorie: HashMap<String, Kategoria> = HashMap::new();
    let rows: Vec<WierszKategorii> = sqlx::query_as(
        "SELECT id, nazwa, opis, domyslny_dostawca_id, dni_dostawy_min, dni_dostawy_max FROM kategorie_materialow ORDER BY id",
    )
    .fetch_all(&mut *tx)
    .await?;
    for (id, n, opis, domyslny, min, max) in rows {
        kategorie.entry(nazwy::klucz(&n)).or_insert(Kategoria {
            id,
            opis,
            domyslny,
            min,
            max,
        });
    }
    let mut dostawcy: HashMap<String, i64> = HashMap::new();
    let rows: Vec<(i64, String)> = sqlx::query_as("SELECT id, nazwa FROM dostawcy ORDER BY id")
        .fetch_all(&mut *tx)
        .await?;
    for (id, n) in rows {
        dostawcy.entry(nazwy::klucz(&n)).or_insert(id);
    }
    // Duplikaty w pliku: (kategoria, dostawca) po kluczu → numer pierwszego wiersza.
    let mut widziane: HashMap<(String, String), usize> = HashMap::new();
    let mut cokolwiek = false;
    let mut zaktualizowane: std::collections::HashSet<i64> = Default::default();

    for (i, w) in d.wiersze.into_iter().enumerate() {
        let nr = w.wiersz.unwrap_or(i + 2);
        let widelki = match sprawdz_wiersz(&w) {
            Ok(x) => x,
            Err(powod) => {
                raport.pominiete.push(PominietyWiersz { wiersz: nr, powod });
                continue;
            }
        };
        let nazwa = nazwy::oczysc(&w.kategoria);
        let przyklady = bez_pustych(w.przykladowe_materialy);
        let dostawca = bez_pustych(w.dostawca);
        let klucz_k = nazwy::klucz(&nazwa);
        let klucz_wiersza = (
            klucz_k.clone(),
            dostawca.as_deref().map(nazwy::klucz).unwrap_or_default(),
        );
        if let Some(pierwszy) = widziane.get(&klucz_wiersza) {
            raport.pominiete.push(PominietyWiersz {
                wiersz: nr,
                powod: format!(
                    "powtórzona kategoria{} – obowiązuje wiersz {pierwszy}",
                    if dostawca.is_some() {
                        " i dostawca"
                    } else {
                        ""
                    }
                ),
            });
            continue;
        }
        widziane.insert(klucz_wiersza, nr);

        let mut zmiany_kategorii: Vec<String> = Vec::new();
        let dostawcy_przed = raport.dostawcy_dodani;
        let dodane_przed = raport.kategorie_dodane;
        let kat_id = match kategorie.get_mut(&klucz_k) {
            Some(k) => {
                if k.opis.is_none() && przyklady.is_some() {
                    sqlx::query("UPDATE kategorie_materialow SET opis = ?1 WHERE id = ?2")
                        .bind(&przyklady)
                        .bind(k.id)
                        .execute(&mut *tx)
                        .await?;
                    k.opis = przyklady.clone();
                    zmiany_kategorii.push("opis".into());
                }
                k.id
            }
            None => {
                let id =
                    sqlx::query("INSERT INTO kategorie_materialow (nazwa, opis) VALUES (?1, ?2)")
                        .bind(&nazwa)
                        .bind(&przyklady)
                        .execute(&mut *tx)
                        .await?
                        .last_insert_rowid();
                do_dziennika(
                    &mut tx,
                    "kategorie_materialow",
                    id,
                    "dodanie",
                    &format!("import kategorie-materialow.csv, wiersz {nr}"),
                )
                .await?;
                raport.kategorie_dodane += 1;
                cokolwiek = true;
                kategorie.insert(
                    klucz_k.clone(),
                    Kategoria {
                        id,
                        opis: przyklady.clone(),
                        domyslny: None,
                        min: 0,
                        max: 0,
                    },
                );
                id
            }
        };

        let dostawca_id = match &dostawca {
            Some(n) => {
                Some(znajdz_lub_dodaj_dostawce(&mut tx, &mut dostawcy, n, &mut raport).await?)
            }
            None => None,
        };
        let k = kategorie
            .get_mut(&klucz_k)
            .expect("kategoria wczytana lub dodana");
        if let (Some(dost), None) = (dostawca_id, k.domyslny) {
            sqlx::query("UPDATE kategorie_materialow SET domyslny_dostawca_id = ?1 WHERE id = ?2")
                .bind(dost)
                .bind(kat_id)
                .execute(&mut *tx)
                .await?;
            k.domyslny = Some(dost);
            zmiany_kategorii.push("domyślny dostawca".into());
        }
        let mut zmieniono_czas = false;
        match (widelki, dostawca_id) {
            (None, dost) => {
                // „do uzupełnienia” tylko wtedy, gdy kategoria (i para) nadal nie ma widełek
                let para: Option<i64> = match dost {
                    Some(dost) => sqlx::query_scalar(
                        "SELECT id FROM czasy_dostaw WHERE kategoria_id = ?1 AND dostawca_id = ?2",
                    )
                    .bind(kat_id)
                    .bind(dost)
                    .fetch_optional(&mut *tx)
                    .await?,
                    None => None,
                };
                if k.min == 0 && k.max == 0 && para.is_none() {
                    raport.bez_czasu_dostawy.push(nr);
                }
            }
            (Some((min, max)), Some(dost)) => {
                let obecna: Option<(i64, i64, i64)> = sqlx::query_as(
                    "SELECT id, dni_min, dni_max FROM czasy_dostaw WHERE kategoria_id = ?1 AND dostawca_id = ?2",
                )
                .bind(kat_id)
                .bind(dost)
                .fetch_optional(&mut *tx)
                .await?;
                match obecna {
                    Some((_, a, b)) if (a, b) == (min, max) => {}
                    Some((_, a, b)) if !nadpisz => {
                        raport.pozostawione.push(PominietyWiersz {
                            wiersz: nr,
                            powod: format!(
                                "{nazwa} – {}: ustawione {a}–{b} dni, w pliku {min}–{max} – pozostawiono ustawione",
                                dostawca.as_deref().unwrap_or_default()
                            ),
                        });
                    }
                    Some((id, _, _)) => {
                        sqlx::query(
                            "UPDATE czasy_dostaw SET dni_min = ?1, dni_max = ?2 WHERE id = ?3",
                        )
                        .bind(min)
                        .bind(max)
                        .bind(id)
                        .execute(&mut *tx)
                        .await?;
                        do_dziennika(
                            &mut tx,
                            "czasy_dostaw",
                            id,
                            "zmiana",
                            &format!("import kategorie-materialow.csv (nadpisanie), wiersz {nr}"),
                        )
                        .await?;
                        zmieniono_czas = true;
                    }
                    None => {
                        let id = sqlx::query(
                            "INSERT INTO czasy_dostaw (kategoria_id, dostawca_id, dni_min, dni_max) VALUES (?1, ?2, ?3, ?4)",
                        )
                        .bind(kat_id)
                        .bind(dost)
                        .bind(min)
                        .bind(max)
                        .execute(&mut *tx)
                        .await?
                        .last_insert_rowid();
                        do_dziennika(
                            &mut tx,
                            "czasy_dostaw",
                            id,
                            "dodanie",
                            &format!("import kategorie-materialow.csv, wiersz {nr}"),
                        )
                        .await?;
                        zmieniono_czas = true;
                    }
                }
            }
            (Some((min, max)), None) => {
                let nieustawione = k.min == 0 && k.max == 0;
                if (k.min, k.max) == (min, max) {
                    // bez zmian
                } else if nieustawione || nadpisz {
                    sqlx::query(
                        "UPDATE kategorie_materialow SET dni_dostawy_min = ?1, dni_dostawy_max = ?2 WHERE id = ?3",
                    )
                    .bind(min)
                    .bind(max)
                    .bind(kat_id)
                    .execute(&mut *tx)
                    .await?;
                    k.min = min;
                    k.max = max;
                    zmiany_kategorii.push(format!("widełki {min}–{max} dni"));
                    zmieniono_czas = true;
                } else {
                    raport.pozostawione.push(PominietyWiersz {
                        wiersz: nr,
                        powod: format!(
                            "{nazwa}: ustawione {}–{} dni, w pliku {min}–{max} – pozostawiono ustawione",
                            k.min, k.max
                        ),
                    });
                }
            }
        }
        if !zmiany_kategorii.is_empty() {
            do_dziennika(
                &mut tx,
                "kategorie_materialow",
                kat_id,
                "zmiana",
                &format!(
                    "import kategorie-materialow.csv, wiersz {nr}: {}",
                    zmiany_kategorii.join(", ")
                ),
            )
            .await?;
            cokolwiek = true;
        }
        if zmieniono_czas {
            raport.czasy_dostaw_ustawione += 1;
            cokolwiek = true;
        }
        let nowa = raport.kategorie_dodane > dodane_przed;
        if !nowa && !zmiany_kategorii.is_empty() {
            zaktualizowane.insert(kat_id);
        }
        if !nowa
            && !zmieniono_czas
            && zmiany_kategorii.is_empty()
            && raport.dostawcy_dodani == dostawcy_przed
            && !raport.pozostawione.iter().any(|p| p.wiersz == nr)
        {
            raport.bez_zmian += 1;
        }
    }
    if cokolwiek || raport.dostawcy_dodani > 0 {
        raport.przeliczenie = t::przelicz_wszystkie_w(&mut tx).await?;
    }
    raport.kategorie_zaktualizowane = zaktualizowane.len();
    tx.commit().await?;
    Ok(raport)
}

async fn importuj(
    State(s): State<StanApi>,
    Cialo(d): Cialo<DaneImportu>,
) -> Wynik<Json<RaportImportuKategorii>> {
    let r = importuj_kategorie(&s.pula, d).await?;
    wyslij(&s, "kategorie_materialow", 0);
    wyslij(&s, "dostawcy", 0);
    wyslij(&s, "czasy_dostaw", 0);
    powiadom(&s, &r.przeliczenie);
    Ok(Json(r))
}

// --- Zakres dni wolnych -----------------------------------------------------------

/// Co zrobić z dniami w zakresie, które są już wpisane z innym rodzajem albo opisem.
#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Kolizje {
    /// Domyślnie: zostawić je bez zmian (zakres obejmie tylko wolne dni).
    #[default]
    Pomin,
    /// Za jawną zgodą użytkownika: zastąpić je rodzajem i opisem zakresu.
    Nadpisz,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ZakresDniWolnych {
    #[serde(deserialize_with = "t::wej::data")]
    pub od: NaiveDate,
    #[serde(rename = "do", deserialize_with = "t::wej::data")]
    pub do_: NaiveDate,
    #[serde(default)]
    pub rodzaj: Option<String>,
    #[serde(default)]
    pub opis: Option<String>,
    #[serde(default)]
    pub kolizje: Kolizje,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow, PartialEq)]
pub struct DzienWZakresie {
    pub data: String,
    pub rodzaj: String,
    pub opis: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WynikZakresu {
    /// Dni zapisane (dodane albo zmienione) / usunięte.
    pub dni: usize,
    /// Dni wpisane wcześniej z innym rodzajem lub opisem, pozostawione bez zmian.
    pub pozostawione: Vec<DzienWZakresie>,
    /// Dni wpisane wcześniej z innym rodzajem lub opisem, zastąpione (tylko `kolizje: "nadpisz"`).
    pub nadpisane: Vec<DzienWZakresie>,
    #[serde(flatten)]
    pub przeliczenie: WynikPrzeliczeniaWszystkich,
}

fn sprawdz_zakres(od: NaiveDate, do_: NaiveDate) -> Wynik<Vec<String>> {
    for d in [od, do_] {
        if !kalendarz::obslugiwany_rok(chrono::Datelike::year(&d)) {
            return Err(BladApi::NieprawidloweDane(format!(
                "data {} spoza obsługiwanego zakresu lat {}–{}",
                kalendarz::data_pl(d),
                kalendarz::ROK_MIN,
                kalendarz::ROK_MAX
            )));
        }
    }
    if do_ < od {
        return Err(BladApi::NieprawidloweDane(
            "koniec zakresu jest wcześniej niż początek".into(),
        ));
    }
    let n = (do_ - od).num_days() + 1;
    if n > MAKS_DNI_ZAKRESU {
        return Err(BladApi::NieprawidloweDane(format!(
            "zakres ma {n} dni – najwyżej {MAKS_DNI_ZAKRESU}"
        )));
    }
    Ok(od
        .iter_days()
        .take(n as usize)
        .map(|d| d.format("%Y-%m-%d").to_string())
        .collect())
}

const RODZAJE: [&str; 4] = ["swieto", "urlop", "przerwa_dostawcow", "inne"];

fn sprawdz_rodzaj(r: Option<String>) -> Wynik<String> {
    let rodzaj = r.unwrap_or_else(|| "urlop".into());
    if !RODZAJE.contains(&rodzaj.as_str()) {
        return Err(BladApi::NieprawidloweDane(format!(
            "nieznany rodzaj dnia wolnego „{rodzaj}” (dozwolone: {})",
            RODZAJE.join(", ")
        )));
    }
    Ok(rodzaj)
}

async fn dni_w_zakresie(
    c: &mut SqliteConnection,
    od: NaiveDate,
    do_: NaiveDate,
) -> Wynik<Vec<DzienWZakresie>> {
    Ok(sqlx::query_as(
        "SELECT data, rodzaj, opis FROM dni_wolne WHERE data >= ?1 AND data <= ?2 ORDER BY data",
    )
    .bind(od.format("%Y-%m-%d").to_string())
    .bind(do_.format("%Y-%m-%d").to_string())
    .fetch_all(&mut *c)
    .await?)
}

/// Dni w zakresie wpisane już z innym rodzajem lub opisem niż podane (kolizje).
pub async fn kolizje_zakresu(
    c: &mut SqliteConnection,
    od: NaiveDate,
    do_: NaiveDate,
    rodzaj: &str,
    opis: &Option<String>,
) -> Wynik<Vec<DzienWZakresie>> {
    Ok(dni_w_zakresie(c, od, do_)
        .await?
        .into_iter()
        .filter(|d| d.rodzaj != rodzaj || &d.opis != opis)
        .collect())
}

/// `POST /kalendarz/dni-wolne/kolizje` – podgląd przed zapisem zakresu (bez zmian w bazie).
async fn podglad_kolizji(
    State(s): State<StanApi>,
    Cialo(z): Cialo<ZakresDniWolnych>,
) -> Wynik<Json<Vec<DzienWZakresie>>> {
    sprawdz_zakres(z.od, z.do_)?;
    let rodzaj = sprawdz_rodzaj(z.rodzaj)?;
    let opis = bez_pustych(z.opis);
    let mut c = s.pula.acquire().await?;
    Ok(Json(
        kolizje_zakresu(&mut c, z.od, z.do_, &rodzaj, &opis).await?,
    ))
}

pub async fn zapisz_zakres(pula: &sqlx::SqlitePool, z: ZakresDniWolnych) -> Wynik<WynikZakresu> {
    let dni = sprawdz_zakres(z.od, z.do_)?;
    let rodzaj = sprawdz_rodzaj(z.rodzaj)?;
    let opis = bez_pustych(z.opis);
    let mut tx = crate::baza::transakcja_zapisu(pula).await?;
    let istniejace = dni_w_zakresie(&mut tx, z.od, z.do_).await?;
    let mut pozostawione = Vec::new();
    let mut nadpisane = Vec::new();
    let mut zapisane = 0;
    for d in &dni {
        match istniejace.iter().find(|x| &x.data == d) {
            Some(x) if x.rodzaj == rodzaj && x.opis == opis => continue,
            Some(x) if z.kolizje == Kolizje::Pomin => {
                pozostawione.push(x.clone());
                continue;
            }
            Some(x) => nadpisane.push(x.clone()),
            None => {}
        }
        sqlx::query(
            "INSERT INTO dni_wolne (data, rodzaj, opis) VALUES (?1, ?2, ?3) \
             ON CONFLICT(data) DO UPDATE SET rodzaj = excluded.rodzaj, opis = excluded.opis",
        )
        .bind(d)
        .bind(&rodzaj)
        .bind(&opis)
        .execute(&mut *tx)
        .await?;
        zapisane += 1;
    }
    let p = if zapisane > 0 {
        do_dziennika(
            &mut tx,
            "dni_wolne",
            0,
            "zmiana",
            &format!(
                "zakres {} – {}: zapisano {zapisane} dni{}",
                kalendarz::data_pl(z.od),
                kalendarz::data_pl(z.do_),
                if nadpisane.is_empty() {
                    String::new()
                } else {
                    format!(", zastąpiono {} wpisanych osobno", nadpisane.len())
                }
            ),
        )
        .await?;
        t::przelicz_wszystkie_w(&mut tx).await?
    } else {
        WynikPrzeliczeniaWszystkich::default()
    };
    tx.commit().await?;
    Ok(WynikZakresu {
        dni: zapisane,
        pozostawione,
        nadpisane,
        przeliczenie: p,
    })
}

async fn dodaj_zakres(
    State(s): State<StanApi>,
    Cialo(z): Cialo<ZakresDniWolnych>,
) -> Wynik<Json<WynikZakresu>> {
    let w = zapisz_zakres(&s.pula, z).await?;
    if w.dni > 0 {
        wyslij(&s, "dni_wolne", 0);
        powiadom(&s, &w.przeliczenie);
    }
    Ok(Json(w))
}

/// Usunięcie zakresu. Domyślnie usuwane są tylko dni o podanym rodzaju i opisie (tak jak je
/// pogrupowano w tabeli ustawień); dni wpisane osobno z innym rodzajem/opisem zostają
/// (`pozostawione`). `wszystkie: true` – jawna zgoda na usunięcie każdego dnia w zakresie.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ZakresDoUsuniecia {
    #[serde(deserialize_with = "t::wej::data")]
    pub od: NaiveDate,
    #[serde(rename = "do", deserialize_with = "t::wej::data")]
    pub do_: NaiveDate,
    #[serde(default)]
    pub rodzaj: Option<String>,
    #[serde(default)]
    pub opis: Option<String>,
    #[serde(default)]
    pub wszystkie: bool,
}

pub async fn usun_zakres_dni(pula: &sqlx::SqlitePool, z: ZakresDoUsuniecia) -> Wynik<WynikZakresu> {
    sprawdz_zakres(z.od, z.do_)?;
    if z.rodzaj.is_none() && !z.wszystkie {
        return Err(BladApi::NieprawidloweDane(
            "podaj rodzaj (i opis) usuwanych dni albo „wszystkie”: true".into(),
        ));
    }
    let rodzaj = match z.rodzaj {
        Some(r) => Some(sprawdz_rodzaj(Some(r))?),
        None => None,
    };
    let opis = bez_pustych(z.opis);
    let mut tx = crate::baza::transakcja_zapisu(pula).await?;
    let istniejace = dni_w_zakresie(&mut tx, z.od, z.do_).await?;
    let mut pozostawione = Vec::new();
    let mut usuniete = 0;
    for d in istniejace {
        let pasuje = z.wszystkie || (Some(&d.rodzaj) == rodzaj.as_ref() && d.opis == opis);
        if !pasuje {
            pozostawione.push(d);
            continue;
        }
        sqlx::query("DELETE FROM dni_wolne WHERE data = ?1")
            .bind(&d.data)
            .execute(&mut *tx)
            .await?;
        usuniete += 1;
    }
    let p = if usuniete > 0 {
        do_dziennika(
            &mut tx,
            "dni_wolne",
            0,
            "usuniecie",
            &format!(
                "zakres {} – {}: usunięto {usuniete} dni",
                kalendarz::data_pl(z.od),
                kalendarz::data_pl(z.do_)
            ),
        )
        .await?;
        t::przelicz_wszystkie_w(&mut tx).await?
    } else {
        WynikPrzeliczeniaWszystkich::default()
    };
    tx.commit().await?;
    Ok(WynikZakresu {
        dni: usuniete,
        pozostawione,
        nadpisane: Vec::new(),
        przeliczenie: p,
    })
}

async fn usun_zakres(
    State(s): State<StanApi>,
    Cialo(z): Cialo<ZakresDoUsuniecia>,
) -> Wynik<Json<WynikZakresu>> {
    let w = usun_zakres_dni(&s.pula, z).await?;
    if w.dni > 0 {
        wyslij(&s, "dni_wolne", 0);
        powiadom(&s, &w.przeliczenie);
    }
    Ok(Json(w))
}

// --- Wiersze specyfikacji do okna „zamówić mimo to?” --------------------------------

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
struct ParametryWierszy {
    ids: String,
    #[serde(rename = "klucz")]
    _klucz: Option<String>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct OpisWiersza {
    pub id: i64,
    pub sekcja: String,
    pub czesc: String,
    pub rodzaj: Option<String>,
    pub material: Option<String>,
    pub opis: Option<String>,
    pub status: String,
}

async fn wiersze_specyfikacji(
    State(s): State<StanApi>,
    Parametry(p): Parametry<ParametryWierszy>,
) -> Wynik<Json<Vec<OpisWiersza>>> {
    let mut ids = Vec::new();
    for x in p.ids.split(',').map(str::trim).filter(|x| !x.is_empty()) {
        ids.push(
            x.parse::<i64>().map_err(|_| {
                BladApi::NieprawidloweDane(format!("„{x}” nie jest numerem wiersza"))
            })?,
        );
    }
    if ids.len() > 200 {
        return Err(BladApi::NieprawidloweDane(
            "najwyżej 200 wierszy naraz".into(),
        ));
    }
    let mut wynik = Vec::with_capacity(ids.len());
    let mut c = s.pula.acquire().await?;
    for id in ids {
        let w: Option<OpisWiersza> = sqlx::query_as(
            "SELECT w.id, TRIM(COALESCE(s.numer || ' ', '') || s.nazwa) AS sekcja, c.nazwa AS czesc, \
                    w.rodzaj, w.material, w.opis, w.status \
               FROM wiersze_specyfikacji w \
               JOIN czesci_specyfikacji c ON c.id = w.czesc_id \
               JOIN sekcje_specyfikacji s ON s.id = c.sekcja_id \
              WHERE w.id = ?1",
        )
        .bind(id)
        .fetch_optional(&mut *c)
        .await?;
        wynik.extend(w);
    }
    Ok(Json(wynik))
}

#[cfg(test)]
mod testy {
    use super::*;

    fn w(min: Option<i64>, max: Option<i64>) -> WierszImportu {
        WierszImportu {
            wiersz: None,
            kategoria: "Płyta".into(),
            przykladowe_materialy: None,
            dostawca: None,
            czas_dostawy_min_dni: min,
            czas_dostawy_max_dni: max,
        }
    }

    #[test]
    fn walidacja_widelek_wiersza() {
        assert_eq!(sprawdz_wiersz(&w(None, None)), Ok(None));
        assert_eq!(sprawdz_wiersz(&w(Some(5), Some(10))), Ok(Some((5, 10))));
        assert!(sprawdz_wiersz(&w(Some(5), None)).is_err());
        assert!(sprawdz_wiersz(&w(Some(10), Some(5))).is_err());
        assert!(sprawdz_wiersz(&w(Some(-1), Some(5))).is_err());
        assert!(sprawdz_wiersz(&w(Some(1), Some(400))).is_err());
        let mut pusty = w(None, None);
        pusty.kategoria = "  ".into();
        assert!(sprawdz_wiersz(&pusty).is_err());
    }
}
