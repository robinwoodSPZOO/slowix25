//! Oferty i ich wersje. Każda wersja ma własną specyfikację (kopię poprzedniej) i parametry
//! wyceny; wycenę liczy [`crate::wycena::wylicz`].

use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqliteConnection, SqlitePool};

use crate::bledy::{BladApi, Wynik};
use crate::formaty;
use crate::limity::{self, MAKS_DLUGI, MAKS_KLAUZUL, MAKS_NAZWA};
use crate::roznice::{self, pole, Roznica};
use crate::specyfikacje::{self, Specyfikacja};
use crate::wycena::{self, PoziomWyceny, TrybCen, Wycena};
use crate::zmiany::ZmianaCzesciowa;

pub const STATUSY_OFERTY: &[&str] = &["szkic", "wyslana", "zaakceptowana", "odrzucona", "wygasla"];

#[derive(Debug, Clone, Serialize, FromRow, PartialEq)]
pub struct Oferta {
    pub id: i64,
    pub numer: String,
    pub klient_id: i64,
    pub klient_nazwa: String,
    pub inwestycja_id: Option<i64>,
    pub status: String,
    pub data_akceptacji: Option<String>,
    pub utworzono: String,
    pub zmieniono: String,
    /// Klient usunięty (miękko) – oferta tylko do podglądu, bez nowych wersji.
    pub klient_usuniety: bool,
}

/// Dane inwestora w migawce wersji (stan w chwili wysłania).
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default)]
pub struct DaneInwestora {
    pub nazwa: String,
    pub adres_ulica: Option<String>,
    pub adres_kod: Option<String>,
    pub adres_miasto: Option<String>,
    pub nip: Option<String>,
    pub email: Option<String>,
    pub telefon: Option<String>,
}

/// Dane wykonawcy w migawce wersji.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default)]
pub struct DaneWykonawcy {
    pub nazwa: String,
    pub adres: String,
    pub nip: String,
    pub telefon: String,
    pub email: String,
}

/// Migawka stron zapisywana przy wyjściu wersji ze szkicu – eksport tej wersji jej używa.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default)]
pub struct MigawkaStron {
    pub inwestor: DaneInwestora,
    pub wykonawca: DaneWykonawcy,
}

#[derive(Debug, Clone, Serialize, FromRow, PartialEq)]
pub struct OfertaNaLiscie {
    pub id: i64,
    pub numer: String,
    pub klient_id: i64,
    pub klient_nazwa: String,
    pub status: String,
    pub zmieniono: String,
    pub wersja_id: i64,
    pub nr_wersji: i64,
    pub data_oferty: Option<String>,
    pub data_waznosci_cen: Option<String>,
    pub adres_inwestycji: Option<String>,
    pub klient_usuniety: bool,
    pub specyfikacja_id: i64,
    /// `None`, gdy wyceny nie da się policzyć (błąd w `blad_wyceny`).
    #[sqlx(skip)]
    pub suma_brutto_gr: Option<i64>,
    #[sqlx(skip)]
    pub blad_wyceny: Option<String>,
    /// Oferta wysłana, a termin ważności cen minął.
    #[sqlx(skip)]
    pub po_terminie_waznosci: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct WersjaOferty {
    pub id: i64,
    pub oferta_id: i64,
    pub nr_wersji: i64,
    pub specyfikacja_id: i64,
    pub data_oferty: Option<String>,
    pub termin_realizacji: Option<String>,
    pub data_waznosci_cen: Option<String>,
    pub adres_inwestycji: Option<String>,
    pub poziom_wyceny: String,
    pub cena_calosc_gr: Option<i64>,
    pub rabat_termin_gr: i64,
    pub rabat_calosc_bp: i64,
    pub stawka_vat: i64,
    pub tryb_cen: String,
    pub koszt_wlasny_gr: Option<i64>,
    pub klauzule: Vec<String>,
    pub uwagi: Option<String>,
    pub utworzono: String,
    pub zmieniono: Option<String>,
    /// Czy to najnowsza wersja (tylko ją można zmieniać, i tylko w szkicu).
    pub najnowsza: bool,
    /// Licznik zmian parametrów wersji.
    pub wersja: i64,
    /// Kiedy wersja wyszła ze szkicu (wysłana / zaakceptowana …).
    pub wyslano: Option<String>,
    /// Dane stron zamrożone przy wysłaniu (używane przez eksport tej wersji).
    pub migawka: Option<MigawkaStron>,
    /// Starsza wersja albo oferta poza szkicem – zmiany tylko przez nową wersję.
    pub tylko_do_odczytu: bool,
    pub powod_blokady: Option<String>,
}

#[derive(FromRow)]
struct WersjaBaza {
    id: i64,
    oferta_id: i64,
    nr_wersji: i64,
    specyfikacja_id: i64,
    data_oferty: Option<String>,
    termin_realizacji: Option<String>,
    data_waznosci_cen: Option<String>,
    adres_inwestycji: Option<String>,
    poziom_wyceny: String,
    cena_calosc_gr: Option<i64>,
    rabat_termin_gr: i64,
    rabat_calosc_bp: i64,
    stawka_vat: i64,
    tryb_cen: String,
    koszt_wlasny_gr: Option<i64>,
    klauzule: Option<String>,
    uwagi: Option<String>,
    utworzono: String,
    zmieniono: Option<String>,
    max_nr: i64,
    wersja: i64,
    wyslano: Option<String>,
    migawka: Option<String>,
    status_oferty: String,
}

impl From<WersjaBaza> for WersjaOferty {
    fn from(w: WersjaBaza) -> Self {
        let powod = if w.nr_wersji != w.max_nr {
            Some("starsza wersja oferty jest tylko do odczytu – utwórz nową wersję".to_string())
        } else if w.status_oferty != "szkic" {
            Some(
                "oferta nie jest już szkicem – wersja jest zamrożona; utwórz nową wersję, aby wprowadzić zmiany"
                    .to_string(),
            )
        } else {
            None
        };
        let klauzule = w
            .klauzule
            .as_deref()
            .and_then(|k| serde_json::from_str::<Vec<String>>(k).ok())
            .unwrap_or_default();
        WersjaOferty {
            id: w.id,
            oferta_id: w.oferta_id,
            nr_wersji: w.nr_wersji,
            specyfikacja_id: w.specyfikacja_id,
            data_oferty: w.data_oferty,
            termin_realizacji: w.termin_realizacji,
            data_waznosci_cen: w.data_waznosci_cen,
            adres_inwestycji: w.adres_inwestycji,
            poziom_wyceny: w.poziom_wyceny,
            cena_calosc_gr: w.cena_calosc_gr,
            rabat_termin_gr: w.rabat_termin_gr,
            rabat_calosc_bp: w.rabat_calosc_bp,
            stawka_vat: w.stawka_vat,
            tryb_cen: w.tryb_cen,
            koszt_wlasny_gr: w.koszt_wlasny_gr,
            klauzule,
            uwagi: w.uwagi,
            utworzono: w.utworzono,
            zmieniono: w.zmieniono,
            najnowsza: w.nr_wersji == w.max_nr,
            wersja: w.wersja,
            wyslano: w.wyslano,
            migawka: w
                .migawka
                .as_deref()
                .and_then(|m| serde_json::from_str(m).ok()),
            tylko_do_odczytu: powod.is_some(),
            powod_blokady: powod,
        }
    }
}

#[derive(Debug, Clone, Serialize, FromRow, PartialEq)]
pub struct WersjaKrotko {
    pub id: i64,
    pub nr_wersji: i64,
    pub data_oferty: Option<String>,
    pub utworzono: String,
    pub wyslano: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SzczegolyOferty {
    pub oferta: Oferta,
    pub wersje: Vec<WersjaKrotko>,
}

/// Wszystko, czego potrzebuje ekran oferty: nagłówek, wersja, wycena i specyfikacja.
#[derive(Debug, Clone, Serialize)]
pub struct PelnaWersja {
    pub oferta: Oferta,
    pub wersje: Vec<WersjaKrotko>,
    pub wersja: WersjaOferty,
    pub wycena: Wycena,
    pub specyfikacja: Specyfikacja,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct DaneNowejOferty {
    pub klient_id: i64,
    pub data_oferty: Option<String>,
    pub termin_realizacji: Option<String>,
    pub data_waznosci_cen: Option<String>,
    pub adres_inwestycji: Option<String>,
    pub poziom_wyceny: Option<String>,
    pub stawka_vat: Option<i64>,
}

/// Pełna zmiana nagłówka i parametrów wyceny wersji (PUT przyjmuje też zmianę częściową –
/// patrz [`zmien_wersje_czesciowo`]).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct DaneWersji {
    pub data_oferty: Option<String>,
    pub termin_realizacji: Option<String>,
    pub data_waznosci_cen: Option<String>,
    pub adres_inwestycji: Option<String>,
    pub poziom_wyceny: String,
    pub cena_calosc_gr: Option<i64>,
    pub rabat_termin_gr: i64,
    pub rabat_calosc_bp: i64,
    pub stawka_vat: i64,
    pub tryb_cen: String,
    pub koszt_wlasny_gr: Option<i64>,
    pub klauzule: Vec<String>,
    pub uwagi: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct FiltrOfert {
    pub status: Option<String>,
    pub klient_id: Option<i64>,
    pub szukaj: Option<String>,
    /// Stronicowanie: najwyżej `limit` ofert (domyślnie 200, maks. 1000) od pozycji `od`.
    pub limit: Option<i64>,
    pub od: Option<i64>,
}

/// Pola wersji zmieniane przez `PUT /wersje-oferty/{id}` (zmiana częściowa).
pub const POLA_WERSJI: &[&str] = &[
    "data_oferty",
    "termin_realizacji",
    "data_waznosci_cen",
    "adres_inwestycji",
    "poziom_wyceny",
    "cena_calosc_gr",
    "rabat_termin_gr",
    "rabat_calosc_bp",
    "stawka_vat",
    "tryb_cen",
    "koszt_wlasny_gr",
    "klauzule",
    "uwagi",
];

fn nazwa_pola_wersji(p: &str) -> String {
    match p {
        "data_oferty" => "data oferty",
        "termin_realizacji" => "termin realizacji",
        "data_waznosci_cen" => "ważność cen",
        "adres_inwestycji" => "adres inwestycji",
        "poziom_wyceny" => "poziom wyceny",
        "cena_calosc_gr" => "cena za całość",
        "rabat_termin_gr" => "rabat za termin",
        "rabat_calosc_bp" => "rabat za całość",
        "stawka_vat" => "stawka VAT",
        "tryb_cen" => "tryb cen",
        "koszt_wlasny_gr" => "koszt własny",
        "klauzule" => "klauzule",
        "uwagi" => "uwagi",
        inne => inne,
    }
    .to_string()
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct DaneNowejWersji {
    /// Wersja, którą kopiujemy (domyślnie najnowsza).
    pub z_wersji_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Porownanie {
    pub z_wersji: i64,
    pub do_wersji: i64,
    pub roznice: Vec<Roznica>,
}

fn sprawdz_date(d: &Option<String>, nazwa: &str) -> Wynik<()> {
    if let Some(d) = d {
        if chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").is_err() {
            return Err(BladApi::NieprawidloweDane(format!(
                "{nazwa}: oczekiwano daty RRRR-MM-DD"
            )));
        }
    }
    Ok(())
}

fn oczysc(t: Option<String>) -> Option<String> {
    t.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

impl DaneWersji {
    fn oczysc(mut self) -> Wynik<Self> {
        self.data_oferty = oczysc(self.data_oferty);
        self.data_waznosci_cen = oczysc(self.data_waznosci_cen);
        self.termin_realizacji = oczysc(self.termin_realizacji);
        self.adres_inwestycji = oczysc(self.adres_inwestycji);
        self.uwagi = oczysc(self.uwagi);
        sprawdz_date(&self.data_oferty, "data oferty")?;
        sprawdz_date(&self.data_waznosci_cen, "data ważności cen")?;
        if PoziomWyceny::z_tekstu(&self.poziom_wyceny).is_none() {
            return Err(BladApi::NieprawidloweDane(
                "poziom wyceny: czesc, sekcja albo calosc".into(),
            ));
        }
        if TrybCen::z_tekstu(&self.tryb_cen).is_none() {
            return Err(BladApi::NieprawidloweDane(
                "tryb cen: brutto albo netto".into(),
            ));
        }
        if self.stawka_vat != 8 && self.stawka_vat != 23 {
            return Err(BladApi::NieprawidloweDane("stawka VAT: 8 albo 23".into()));
        }
        if !(0..=10_000).contains(&self.rabat_calosc_bp) {
            return Err(BladApi::NieprawidloweDane(
                "rabat za całość: od 0 do 100%".into(),
            ));
        }
        limity::kwota("rabat za termin", Some(self.rabat_termin_gr))?;
        limity::kwota("cena za całość", self.cena_calosc_gr)?;
        limity::kwota("koszt własny", self.koszt_wlasny_gr)?;
        limity::tekst(
            "termin realizacji",
            self.termin_realizacji.as_deref(),
            MAKS_NAZWA,
        )?;
        limity::tekst(
            "adres inwestycji",
            self.adres_inwestycji.as_deref(),
            MAKS_DLUGI,
        )?;
        limity::tekst("uwagi", self.uwagi.as_deref(), MAKS_DLUGI)?;
        self.klauzule = self
            .klauzule
            .into_iter()
            .map(|k| k.trim().to_string())
            .filter(|k| !k.is_empty())
            .collect();
        if self.klauzule.len() > MAKS_KLAUZUL {
            return Err(BladApi::NieprawidloweDane(format!(
                "najwyżej {MAKS_KLAUZUL} klauzul"
            )));
        }
        for k in &self.klauzule {
            limity::tekst("klauzula", Some(k), MAKS_DLUGI)?;
        }
        Ok(self)
    }
}

// ---------------------------------------------------------------------------
// Odczyt
// ---------------------------------------------------------------------------

const KOLUMNY_OFERTY: &str =
    "o.id, o.numer, o.klient_id, k.nazwa AS klient_nazwa, o.inwestycja_id, o.status, \
    o.data_akceptacji, o.utworzono, o.zmieniono, (k.usunieto IS NOT NULL) AS klient_usuniety";
const KOLUMNY_WERSJI: &str = "w.id, w.oferta_id, w.nr_wersji, w.specyfikacja_id, w.data_oferty, w.termin_realizacji, \
    w.data_waznosci_cen, w.adres_inwestycji, w.poziom_wyceny, w.cena_calosc_gr, w.rabat_termin_gr, w.rabat_calosc_bp, \
    w.stawka_vat, w.tryb_cen, w.koszt_wlasny_gr, w.klauzule, w.uwagi, w.utworzono, w.zmieniono, \
    (SELECT MAX(nr_wersji) FROM wersje_oferty x WHERE x.oferta_id = w.oferta_id) AS max_nr, \
    w.wersja, w.wyslano, w.migawka, (SELECT status FROM oferty o WHERE o.id = w.oferta_id) AS status_oferty";

fn dzis() -> chrono::NaiveDate {
    chrono::Local::now().date_naive()
}

pub async fn pobierz_oferte_w(db: &mut SqliteConnection, id: i64) -> Wynik<Oferta> {
    sqlx::query_as(&format!(
        "SELECT {KOLUMNY_OFERTY} FROM oferty o JOIN klienci k ON k.id = o.klient_id WHERE o.id = ?1"
    ))
    .bind(id)
    .fetch_optional(&mut *db)
    .await?
    .ok_or_else(|| BladApi::NieZnaleziono(format!("oferta {id}")))
}

pub async fn pobierz_wersje_w(db: &mut SqliteConnection, id: i64) -> Wynik<WersjaOferty> {
    let w: Option<WersjaBaza> = sqlx::query_as(&format!(
        "SELECT {KOLUMNY_WERSJI} FROM wersje_oferty w WHERE w.id = ?1"
    ))
    .bind(id)
    .fetch_optional(&mut *db)
    .await?;
    w.map(Into::into)
        .ok_or_else(|| BladApi::NieZnaleziono(format!("wersja oferty {id}")))
}

async fn wersje_oferty(db: &mut SqliteConnection, oferta_id: i64) -> Wynik<Vec<WersjaKrotko>> {
    Ok(sqlx::query_as(
        "SELECT id, nr_wersji, data_oferty, utworzono, wyslano FROM wersje_oferty WHERE oferta_id = ?1 ORDER BY nr_wersji",
    )
    .bind(oferta_id)
    .fetch_all(&mut *db)
    .await?)
}

/// Dane specyfikacji przeliczone na wejście funkcji wyceny.
pub fn wycen(w: &WersjaOferty, s: &Specyfikacja) -> Wycena {
    let p = wycena::ParametryWyceny {
        poziom: PoziomWyceny::z_tekstu(&w.poziom_wyceny).unwrap_or(PoziomWyceny::Sekcja),
        cena_calosc_gr: w.cena_calosc_gr,
        rabat_termin_gr: w.rabat_termin_gr,
        rabat_calosc_bp: w.rabat_calosc_bp,
        stawka_vat: w.stawka_vat,
        tryb_cen: TrybCen::z_tekstu(&w.tryb_cen).unwrap_or(TrybCen::Brutto),
        koszt_wlasny_gr: w.koszt_wlasny_gr,
    };
    let sekcje: Vec<wycena::SekcjaDoWyceny> = s
        .sekcje
        .iter()
        .map(|sek| wycena::SekcjaDoWyceny {
            id: sek.id,
            numer: sek.numer.clone(),
            nazwa: sek.nazwa.clone(),
            agd: sek.typ == "agd",
            cena_gr: sek.cena_gr,
            stawka_vat: sek.stawka_vat,
            czesci: sek
                .czesci
                .iter()
                .map(|c| wycena::CzescDoWyceny {
                    id: c.id,
                    nazwa: c.nazwa.clone(),
                    cena_gr: c.cena_gr,
                })
                .collect(),
            pozycje_agd: sek
                .czesci
                .iter()
                .flat_map(|c| c.wiersze.iter())
                .map(|w| wycena::PozycjaAgd {
                    cena_gr: w.cena_gr,
                    ilosc: w.ilosc,
                    status: w.status.clone(),
                })
                .collect(),
        })
        .collect();
    wycena::wylicz(&p, &sekcje)
}

pub async fn pelna_wersja_w(db: &mut SqliteConnection, wersja_id: i64) -> Wynik<PelnaWersja> {
    let wersja = pobierz_wersje_w(db, wersja_id).await?;
    let oferta = pobierz_oferte_w(db, wersja.oferta_id).await?;
    let wersje = wersje_oferty(db, wersja.oferta_id).await?;
    let specyfikacja = specyfikacje::pobierz_w(db, wersja.specyfikacja_id).await?;
    let wycena = wycen(&wersja, &specyfikacja);
    Ok(PelnaWersja {
        oferta,
        wersje,
        wersja,
        wycena,
        specyfikacja,
    })
}

pub async fn pelna_wersja(pula: &SqlitePool, wersja_id: i64) -> Wynik<PelnaWersja> {
    let mut db = pula.acquire().await?;
    pelna_wersja_w(&mut db, wersja_id).await
}

/// Lista ofert (najnowsza wersja każdej + suma brutto). Stała liczba zapytań niezależnie od
/// liczby ofert; jedna uszkodzona oferta (np. kwoty poza zakresem) nie psuje listy – ma
/// `suma_brutto_gr: null` i `blad_wyceny`.
pub async fn lista(pula: &SqlitePool, f: FiltrOfert) -> Wynik<Vec<OfertaNaLiscie>> {
    if let Some(s) = &f.status {
        if !STATUSY_OFERTY.contains(&s.as_str()) {
            return Err(BladApi::NieprawidloweDane(format!("nieznany status „{s}”")));
        }
    }
    let limit = f.limit.unwrap_or(200).clamp(1, 1000);
    let od = f.od.unwrap_or(0).max(0);
    let szukaj = f.szukaj.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let mut db = pula.acquire().await?;
    let mut lista: Vec<OfertaNaLiscie> = sqlx::query_as(
        "SELECT o.id, o.numer, o.klient_id, k.nazwa AS klient_nazwa, o.status, o.zmieniono, \
           w.id AS wersja_id, w.nr_wersji, w.data_oferty, w.data_waznosci_cen, w.adres_inwestycji, \
           (k.usunieto IS NOT NULL) AS klient_usuniety, w.specyfikacja_id \
         FROM oferty o JOIN klienci k ON k.id = o.klient_id \
         JOIN wersje_oferty w ON w.oferta_id = o.id \
           AND w.nr_wersji = (SELECT MAX(nr_wersji) FROM wersje_oferty x WHERE x.oferta_id = o.id) \
         WHERE (?1 IS NULL OR o.status = ?1) AND (?2 IS NULL OR o.klient_id = ?2) \
           AND (?3 IS NULL OR o.numer LIKE '%' || ?3 || '%' OR k.nazwa LIKE '%' || ?3 || '%' \
                OR w.adres_inwestycji LIKE '%' || ?3 || '%') \
         ORDER BY o.zmieniono DESC, o.id DESC LIMIT ?4 OFFSET ?5",
    )
    .bind(&f.status)
    .bind(f.klient_id)
    .bind(szukaj)
    .bind(limit)
    .bind(od)
    .fetch_all(&mut *db)
    .await?;
    let wersje: Vec<WersjaBaza> = sqlx::query_as(&format!(
        "SELECT {KOLUMNY_WERSJI} FROM wersje_oferty w WHERE w.id IN (SELECT value FROM json_each(?1))"
    ))
    .bind(serde_json::to_string(&lista.iter().map(|o| o.wersja_id).collect::<Vec<_>>()).unwrap_or_default())
    .fetch_all(&mut *db)
    .await?;
    let wersje: std::collections::HashMap<i64, WersjaOferty> = wersje
        .into_iter()
        .map(|w| (w.id, WersjaOferty::from(w)))
        .collect();
    let drzewa = specyfikacje::drzewa_w(
        &mut db,
        &lista.iter().map(|o| o.specyfikacja_id).collect::<Vec<_>>(),
    )
    .await?;
    let dzis = dzis().format("%Y-%m-%d").to_string();
    for o in &mut lista {
        if let (Some(w), Some(s)) = (wersje.get(&o.wersja_id), drzewa.get(&o.specyfikacja_id)) {
            let wy = wycen(w, s);
            if wy.bledy.iter().any(|b| b == wycena::BLAD_PRZEPELNIENIA) {
                o.blad_wyceny = Some(wycena::BLAD_PRZEPELNIENIA.into());
            } else {
                o.suma_brutto_gr = Some(wy.suma_brutto_gr);
                o.blad_wyceny = wy.bledy.first().cloned();
            }
        }
        o.po_terminie_waznosci = o.status == "wyslana"
            && o.data_waznosci_cen
                .as_deref()
                .is_some_and(|d| d < dzis.as_str());
    }
    Ok(lista)
}

pub async fn szczegoly(pula: &SqlitePool, id: i64) -> Wynik<SzczegolyOferty> {
    let mut db = pula.acquire().await?;
    Ok(SzczegolyOferty {
        oferta: pobierz_oferte_w(&mut db, id).await?,
        wersje: wersje_oferty(&mut db, id).await?,
    })
}

// ---------------------------------------------------------------------------
// Zmiany
// ---------------------------------------------------------------------------

async fn ustawienie(db: &mut SqliteConnection, klucz: &str) -> Wynik<Option<String>> {
    Ok(
        sqlx::query_scalar("SELECT wartosc FROM ustawienia WHERE klucz = ?1")
            .bind(klucz)
            .fetch_optional(&mut *db)
            .await?,
    )
}

async fn nowy_numer(db: &mut SqliteConnection, rok: i32) -> Wynik<String> {
    let prefiks = format!("OF/{rok}/");
    let numery: Vec<String> =
        sqlx::query_scalar("SELECT numer FROM oferty WHERE numer LIKE ?1 || '%'")
            .bind(&prefiks)
            .fetch_all(&mut *db)
            .await?;
    let max = numery
        .iter()
        .filter_map(|n| n.strip_prefix(&prefiks)?.parse::<u32>().ok())
        .max()
        .unwrap_or(0);
    Ok(format!("{prefiks}{:03}", max + 1))
}

async fn dotknij_oferte(db: &mut SqliteConnection, id: i64) -> Wynik<()> {
    sqlx::query(
        "UPDATE oferty SET zmieniono = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = ?1",
    )
    .bind(id)
    .execute(&mut *db)
    .await?;
    Ok(())
}

/// Nowa oferta z wersją v1 i pustą specyfikacją. Wartości domyślne z ustawień.
pub async fn dodaj(pula: &SqlitePool, d: DaneNowejOferty) -> Wynik<PelnaWersja> {
    let mut tx = crate::baza::transakcja_zapisu(pula).await?;
    let klient: Option<i64> =
        sqlx::query_scalar("SELECT id FROM klienci WHERE id = ?1 AND usunieto IS NULL")
            .bind(d.klient_id)
            .fetch_optional(&mut *tx)
            .await?;
    if klient.is_none() {
        return Err(BladApi::NieprawidloweDane(format!(
            "nie ma klienta {} (inwestor musi być w bazie klientów)",
            d.klient_id
        )));
    }
    let dzis = dzis();
    let waznosc_dni: i64 = ustawienie(&mut tx, "oferta_waznosc_dni")
        .await?
        .and_then(|v| v.parse().ok())
        .unwrap_or(30);
    let stawka: i64 = d.stawka_vat.unwrap_or(
        ustawienie(&mut tx, "stawka_vat_domyslna")
            .await?
            .and_then(|v| v.parse().ok())
            .unwrap_or(23),
    );
    let termin = match oczysc(d.termin_realizacji) {
        Some(t) => Some(t),
        None => oczysc(ustawienie(&mut tx, "oferta_termin_realizacji").await?),
    };
    let klauzule: Vec<String> = sqlx::query_scalar(
        "SELECT wartosc FROM pozycje_slownika WHERE slownik_kod = 'klauzula' AND aktywna = 1 ORDER BY kolejnosc, id",
    )
    .fetch_all(&mut *tx)
    .await?;
    let dane = DaneWersji {
        data_oferty: Some(
            d.data_oferty
                .unwrap_or_else(|| dzis.format("%Y-%m-%d").to_string()),
        ),
        termin_realizacji: termin,
        data_waznosci_cen: Some(d.data_waznosci_cen.unwrap_or_else(|| {
            (dzis + chrono::Days::new(waznosc_dni.max(0) as u64))
                .format("%Y-%m-%d")
                .to_string()
        })),
        adres_inwestycji: d.adres_inwestycji,
        poziom_wyceny: d.poziom_wyceny.unwrap_or_else(|| "sekcja".into()),
        stawka_vat: stawka,
        tryb_cen: "brutto".into(),
        klauzule,
        ..Default::default()
    }
    .oczysc()?;

    let numer = nowy_numer(&mut tx, chrono::Datelike::year(&dzis)).await?;
    let oferta_id = sqlx::query("INSERT INTO oferty (numer, klient_id) VALUES (?1, ?2)")
        .bind(&numer)
        .bind(d.klient_id)
        .execute(&mut *tx)
        .await?
        .last_insert_rowid();
    let spec = specyfikacje::utworz_w(&mut tx).await?;
    let wersja = wstaw_wersje(&mut tx, oferta_id, 1, spec, &dane).await?;
    specyfikacje::dziennik(&mut tx, "oferty", oferta_id, "dodanie").await?;
    let w = pelna_wersja_w(&mut tx, wersja).await?;
    tx.commit().await?;
    Ok(w)
}

async fn wstaw_wersje(
    db: &mut SqliteConnection,
    oferta_id: i64,
    nr: i64,
    spec: i64,
    d: &DaneWersji,
) -> Wynik<i64> {
    Ok(sqlx::query(
        "INSERT INTO wersje_oferty (oferta_id, nr_wersji, specyfikacja_id, data_oferty, termin_realizacji, \
           data_waznosci_cen, adres_inwestycji, poziom_wyceny, cena_calosc_gr, rabat_termin_gr, rabat_calosc_bp, \
           stawka_vat, tryb_cen, koszt_wlasny_gr, klauzule, uwagi) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
    )
    .bind(oferta_id)
    .bind(nr)
    .bind(spec)
    .bind(&d.data_oferty)
    .bind(&d.termin_realizacji)
    .bind(&d.data_waznosci_cen)
    .bind(&d.adres_inwestycji)
    .bind(&d.poziom_wyceny)
    .bind(d.cena_calosc_gr)
    .bind(d.rabat_termin_gr)
    .bind(d.rabat_calosc_bp)
    .bind(d.stawka_vat)
    .bind(&d.tryb_cen)
    .bind(d.koszt_wlasny_gr)
    .bind(serde_json::to_string(&d.klauzule).unwrap_or_else(|_| "[]".into()))
    .bind(&d.uwagi)
    .execute(&mut *db)
    .await?
    .last_insert_rowid())
}

/// Dane wersji w postaci do zmiany (PUT).
pub fn dane_wersji(w: &WersjaOferty) -> DaneWersji {
    DaneWersji {
        data_oferty: w.data_oferty.clone(),
        termin_realizacji: w.termin_realizacji.clone(),
        data_waznosci_cen: w.data_waznosci_cen.clone(),
        adres_inwestycji: w.adres_inwestycji.clone(),
        poziom_wyceny: w.poziom_wyceny.clone(),
        cena_calosc_gr: w.cena_calosc_gr,
        rabat_termin_gr: w.rabat_termin_gr,
        rabat_calosc_bp: w.rabat_calosc_bp,
        stawka_vat: w.stawka_vat,
        tryb_cen: w.tryb_cen.clone(),
        koszt_wlasny_gr: w.koszt_wlasny_gr,
        klauzule: w.klauzule.clone(),
        uwagi: w.uwagi.clone(),
    }
}

fn sprawdz_edycje(w: &WersjaOferty) -> Wynik<()> {
    match &w.powod_blokady {
        Some(p) => Err(BladApi::NieprawidloweDane(p.clone())),
        None => Ok(()),
    }
}

pub async fn zmien_wersje(pula: &SqlitePool, id: i64, d: DaneWersji) -> Wynik<PelnaWersja> {
    let d = d.oczysc()?;
    let mut tx = crate::baza::transakcja_zapisu(pula).await?;
    let w = pobierz_wersje_w(&mut tx, id).await?;
    sprawdz_edycje(&w)?;
    let p = zapisz_wersje_w(&mut tx, &w, &d).await?;
    tx.commit().await?;
    Ok(p)
}

/// Zmiana częściowa parametrów wersji z wykrywaniem konfliktów (`PUT /wersje-oferty/{id}`).
pub async fn zmien_wersje_czesciowo(
    pula: &SqlitePool,
    id: i64,
    z: ZmianaCzesciowa,
) -> Wynik<PelnaWersja> {
    let mut tx = crate::baza::transakcja_zapisu(pula).await?;
    let w = pobierz_wersje_w(&mut tx, id).await?;
    sprawdz_edycje(&w)?;
    let baza = dane_wersji(&w);
    let aktualny = serde_json::to_value(&baza).unwrap_or_default();
    z.sprawdz(
        &aktualny,
        serde_json::to_value(&w).unwrap_or_default(),
        &nazwa_pola_wersji,
    )?;
    let d = z.zastosuj(&baza)?.oczysc()?;
    let p = zapisz_wersje_w(&mut tx, &w, &d).await?;
    tx.commit().await?;
    Ok(p)
}

async fn zapisz_wersje_w(
    tx: &mut SqliteConnection,
    w: &WersjaOferty,
    d: &DaneWersji,
) -> Wynik<PelnaWersja> {
    let id = w.id;
    sqlx::query(
        "UPDATE wersje_oferty SET data_oferty = ?1, termin_realizacji = ?2, data_waznosci_cen = ?3, \
           adres_inwestycji = ?4, poziom_wyceny = ?5, cena_calosc_gr = ?6, rabat_termin_gr = ?7, \
           rabat_calosc_bp = ?8, stawka_vat = ?9, tryb_cen = ?10, koszt_wlasny_gr = ?11, klauzule = ?12, \
           uwagi = ?13, zmieniono = strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), wersja = wersja + 1 WHERE id = ?14",
    )
    .bind(&d.data_oferty)
    .bind(&d.termin_realizacji)
    .bind(&d.data_waznosci_cen)
    .bind(&d.adres_inwestycji)
    .bind(&d.poziom_wyceny)
    .bind(d.cena_calosc_gr)
    .bind(d.rabat_termin_gr)
    .bind(d.rabat_calosc_bp)
    .bind(d.stawka_vat)
    .bind(&d.tryb_cen)
    .bind(d.koszt_wlasny_gr)
    .bind(serde_json::to_string(&d.klauzule).unwrap_or_else(|_| "[]".into()))
    .bind(&d.uwagi)
    .bind(id)
    .execute(&mut *tx)
    .await?;
    dotknij_oferte(tx, w.oferta_id).await?;
    specyfikacje::dziennik(tx, "wersje_oferty", id, "zmiana").await?;
    pelna_wersja_w(tx, id).await
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct DaneOferty {
    pub klient_id: i64,
}

pub async fn zmien_klienta(pula: &SqlitePool, id: i64, d: DaneOferty) -> Wynik<SzczegolyOferty> {
    let mut tx = crate::baza::transakcja_zapisu(pula).await?;
    let o = pobierz_oferte_w(&mut tx, id).await?;
    if o.status != "szkic" {
        return Err(BladApi::NieprawidloweDane(
            "inwestora można zmienić tylko w szkicu oferty – utwórz nową wersję".into(),
        ));
    }
    let n = sqlx::query(
        "UPDATE oferty SET klient_id = ?1, zmieniono = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = ?2 \
         AND EXISTS (SELECT 1 FROM klienci WHERE id = ?1 AND usunieto IS NULL)",
    )
    .bind(d.klient_id)
    .bind(id)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if n == 0 {
        pobierz_oferte_w(&mut tx, id).await?;
        return Err(BladApi::NieprawidloweDane(format!(
            "nie ma klienta {} (albo został usunięty)",
            d.klient_id
        )));
    }
    specyfikacje::dziennik(&mut tx, "oferty", id, "zmiana").await?;
    let wynik = SzczegolyOferty {
        oferta: pobierz_oferte_w(&mut tx, id).await?,
        wersje: wersje_oferty(&mut tx, id).await?,
    };
    tx.commit().await?;
    Ok(wynik)
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct ZmianaStatusu {
    pub status: String,
}

pub async fn zmien_status(pula: &SqlitePool, id: i64, z: ZmianaStatusu) -> Wynik<SzczegolyOferty> {
    if !STATUSY_OFERTY.contains(&z.status.as_str()) {
        return Err(BladApi::NieprawidloweDane(format!(
            "nieznany status „{}” (dozwolone: {})",
            z.status,
            STATUSY_OFERTY.join(", ")
        )));
    }
    let mut tx = crate::baza::transakcja_zapisu(pula).await?;
    let o = pobierz_oferte_w(&mut tx, id).await?;
    if o.status != "szkic" && z.status == "szkic" {
        return Err(BladApi::NieprawidloweDane(
            "oferty, która wyszła ze szkicu, nie można przywrócić do szkicu – utwórz nową wersję"
                .into(),
        ));
    }
    if o.status == "szkic" && z.status != "szkic" {
        // Wersja wychodzi ze szkicu: zamrażamy dane stron do dokumentu.
        let migawka = migawka_w(&mut tx, id).await?;
        sqlx::query(
            "UPDATE wersje_oferty SET migawka = ?1, wyslano = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') \
             WHERE oferta_id = ?2 AND nr_wersji = (SELECT MAX(nr_wersji) FROM wersje_oferty WHERE oferta_id = ?2)",
        )
        .bind(serde_json::to_string(&migawka).unwrap_or_default())
        .bind(id)
        .execute(&mut *tx)
        .await?;
    }
    sqlx::query(
        "UPDATE oferty SET status = ?1, \
           data_akceptacji = CASE WHEN ?1 = 'zaakceptowana' THEN COALESCE(data_akceptacji, ?2) ELSE NULL END, \
           zmieniono = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = ?3",
    )
    .bind(&z.status)
    .bind(dzis().format("%Y-%m-%d").to_string())
    .bind(id)
    .execute(&mut *tx)
    .await?;
    specyfikacje::dziennik(&mut tx, "oferty", id, "zmiana").await?;
    let wynik = SzczegolyOferty {
        oferta: pobierz_oferte_w(&mut tx, id).await?,
        wersje: wersje_oferty(&mut tx, id).await?,
    };
    tx.commit().await?;
    Ok(wynik)
}

/// Nowa wersja = kopia wskazanej (domyślnie najnowszej) wraz ze specyfikacją i parametrami.
pub async fn nowa_wersja(
    pula: &SqlitePool,
    oferta_id: i64,
    d: DaneNowejWersji,
) -> Wynik<PelnaWersja> {
    let mut tx = crate::baza::transakcja_zapisu(pula).await?;
    let oferta = pobierz_oferte_w(&mut tx, oferta_id).await?;
    if oferta.klient_usuniety {
        return Err(BladApi::NieprawidloweDane(format!(
            "klient „{}” został usunięty – przywróć go w bazie klientów, aby tworzyć nowe wersje oferty",
            oferta.klient_nazwa
        )));
    }
    let wersje = wersje_oferty(&mut tx, oferta_id).await?;
    let zrodlo_id = match d.z_wersji_id {
        Some(z) => z,
        None => wersje
            .last()
            .map(|w| w.id)
            .ok_or_else(|| BladApi::NieZnaleziono("wersja oferty".into()))?,
    };
    let zrodlo = pobierz_wersje_w(&mut tx, zrodlo_id).await?;
    if zrodlo.oferta_id != oferta_id {
        return Err(BladApi::NieprawidloweDane(
            "wersja należy do innej oferty".into(),
        ));
    }
    let nr = wersje.iter().map(|w| w.nr_wersji).max().unwrap_or(0) + 1;
    let spec = specyfikacje::kopiuj_specyfikacje_w(&mut tx, zrodlo.specyfikacja_id).await?;
    let waznosc_dni: i64 = ustawienie(&mut tx, "oferta_waznosc_dni")
        .await?
        .and_then(|v| v.parse().ok())
        .unwrap_or(30);
    let dzis = dzis();
    let dane = DaneWersji {
        data_oferty: Some(dzis.format("%Y-%m-%d").to_string()),
        termin_realizacji: zrodlo.termin_realizacji.clone(),
        // Nowa wersja = nowa oferta dla klienta: ważność od dziś (nie kopia starej daty).
        data_waznosci_cen: Some(
            (dzis + chrono::Days::new(waznosc_dni.clamp(0, 365) as u64))
                .format("%Y-%m-%d")
                .to_string(),
        ),
        adres_inwestycji: zrodlo.adres_inwestycji.clone(),
        poziom_wyceny: zrodlo.poziom_wyceny.clone(),
        cena_calosc_gr: zrodlo.cena_calosc_gr,
        rabat_termin_gr: zrodlo.rabat_termin_gr,
        rabat_calosc_bp: zrodlo.rabat_calosc_bp,
        stawka_vat: zrodlo.stawka_vat,
        tryb_cen: zrodlo.tryb_cen.clone(),
        koszt_wlasny_gr: zrodlo.koszt_wlasny_gr,
        klauzule: zrodlo.klauzule.clone(),
        uwagi: zrodlo.uwagi.clone(),
    };
    let id = wstaw_wersje(&mut tx, oferta_id, nr, spec, &dane).await?;
    // Nowa wersja jest szkicem – oferta wraca do statusu „szkic” (poprzednia wersja zostaje
    // zamrożona z migawką danych, jeśli była wysłana).
    sqlx::query(
        "UPDATE oferty SET status = 'szkic', data_akceptacji = NULL, \
           zmieniono = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = ?1",
    )
    .bind(oferta_id)
    .execute(&mut *tx)
    .await?;
    specyfikacje::dziennik(&mut tx, "wersje_oferty", id, "dodanie").await?;
    let p = pelna_wersja_w(&mut tx, id).await?;
    tx.commit().await?;
    Ok(p)
}

/// Migawka bieżących danych inwestora oferty i wykonawcy (firma z ustawień).
pub async fn migawka_w(db: &mut SqliteConnection, oferta_id: i64) -> Wynik<MigawkaStron> {
    #[allow(clippy::type_complexity)]
    let k: Option<(
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    )> = sqlx::query_as(
        "SELECT k.nazwa, k.adres_ulica, k.adres_kod, k.adres_miasto, k.nip, k.email, k.telefon \
             FROM oferty o JOIN klienci k ON k.id = o.klient_id WHERE o.id = ?1",
    )
    .bind(oferta_id)
    .fetch_optional(&mut *db)
    .await?;
    let (nazwa, adres_ulica, adres_kod, adres_miasto, nip, email, telefon) =
        k.ok_or_else(|| BladApi::NieZnaleziono(format!("oferta {oferta_id}")))?;
    let f = crate::firma::pobierz_w(db).await?;
    Ok(MigawkaStron {
        inwestor: DaneInwestora {
            nazwa,
            adres_ulica,
            adres_kod,
            adres_miasto,
            nip,
            email,
            telefon,
        },
        wykonawca: DaneWykonawcy {
            nazwa: f.firma_nazwa,
            adres: f.firma_adres,
            nip: f.firma_nip,
            telefon: f.firma_telefon,
            email: f.firma_email,
        },
    })
}

/// Usunięcie oferty ze wszystkimi wersjami i specyfikacjami (nie gdy jest z niej umowa).
pub async fn usun(pula: &SqlitePool, id: i64) -> Wynik<()> {
    let mut tx = crate::baza::transakcja_zapisu(pula).await?;
    pobierz_oferte_w(&mut tx, id).await?;
    let umowy: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM umowy WHERE wersja_oferty_id IN (SELECT id FROM wersje_oferty WHERE oferta_id = ?1)",
    )
    .bind(id)
    .fetch_one(&mut *tx)
    .await?;
    if umowy > 0 {
        return Err(BladApi::NieprawidloweDane(
            "z tej oferty utworzono umowę – nie można jej usunąć".into(),
        ));
    }
    let specy: Vec<i64> =
        sqlx::query_scalar("SELECT specyfikacja_id FROM wersje_oferty WHERE oferta_id = ?1")
            .bind(id)
            .fetch_all(&mut *tx)
            .await?;
    sqlx::query("DELETE FROM wersje_oferty WHERE oferta_id = ?1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM oferty WHERE id = ?1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    for s in specy {
        specyfikacje::usun_specyfikacje_w(&mut tx, s).await?;
    }
    specyfikacje::dziennik(&mut tx, "oferty", id, "usuniecie").await?;
    tx.commit().await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Porównanie wersji
// ---------------------------------------------------------------------------

fn roznice_naglowka(a: &WersjaOferty, b: &WersjaOferty, wa: &Wycena, wb: &Wycena) -> Vec<Roznica> {
    let poziom = |p: &str| match p {
        "czesc" => "części",
        "calosc" => "całości",
        _ => "sekcji",
    };
    let naglowek: Vec<_> = [
        pole("data oferty", a.data_oferty.clone(), b.data_oferty.clone()),
        pole(
            "termin realizacji",
            a.termin_realizacji.clone(),
            b.termin_realizacji.clone(),
        ),
        pole(
            "ważność cen",
            a.data_waznosci_cen.clone(),
            b.data_waznosci_cen.clone(),
        ),
        pole(
            "adres inwestycji",
            a.adres_inwestycji.clone(),
            b.adres_inwestycji.clone(),
        ),
        pole(
            "klauzule",
            Some(a.klauzule.join(" | ")),
            Some(b.klauzule.join(" | ")),
        ),
    ]
    .into_iter()
    .flatten()
    .collect();
    let wycena: Vec<_> = [
        pole(
            "wycena na poziomie",
            Some(poziom(&a.poziom_wyceny).into()),
            Some(poziom(&b.poziom_wyceny).into()),
        ),
        pole(
            "cena za całość",
            a.cena_calosc_gr.map(formaty::kwota),
            b.cena_calosc_gr.map(formaty::kwota),
        ),
        pole(
            "rabat za termin",
            Some(formaty::kwota(a.rabat_termin_gr)),
            Some(formaty::kwota(b.rabat_termin_gr)),
        ),
        pole(
            "rabat za całość",
            Some(formaty::procent_bp(a.rabat_calosc_bp)),
            Some(formaty::procent_bp(b.rabat_calosc_bp)),
        ),
        pole(
            "stawka VAT",
            Some(format!("{}%", a.stawka_vat)),
            Some(format!("{}%", b.stawka_vat)),
        ),
        pole(
            "ceny wpisane",
            Some(a.tryb_cen.clone()),
            Some(b.tryb_cen.clone()),
        ),
        pole(
            "suma za meble",
            Some(formaty::kwota(wa.suma_meble_gr)),
            Some(formaty::kwota(wb.suma_meble_gr)),
        ),
        pole(
            "sprzęt AGD",
            Some(formaty::kwota(wa.suma_agd_gr)),
            Some(formaty::kwota(wb.suma_agd_gr)),
        ),
        pole(
            "SUMA brutto",
            Some(formaty::kwota(wa.suma_brutto_gr)),
            Some(formaty::kwota(wb.suma_brutto_gr)),
        ),
    ]
    .into_iter()
    .flatten()
    .collect();
    let mut r = Vec::new();
    if !naglowek.is_empty() {
        r.push(Roznica {
            poziom: "naglowek".into(),
            zmiana: "zmieniono".into(),
            sciezka: "Nagłówek oferty".into(),
            pola: naglowek,
        });
    }
    if !wycena.is_empty() {
        r.push(Roznica {
            poziom: "wycena".into(),
            zmiana: "zmieniono".into(),
            sciezka: "Wycena".into(),
            pola: wycena,
        });
    }
    r
}

pub async fn porownaj(pula: &SqlitePool, oferta_id: i64, z: i64, do_: i64) -> Wynik<Porownanie> {
    let mut db = pula.acquire().await?;
    let a = pelna_wersja_w(&mut db, z).await?;
    let b = pelna_wersja_w(&mut db, do_).await?;
    if a.wersja.oferta_id != oferta_id || b.wersja.oferta_id != oferta_id {
        return Err(BladApi::NieprawidloweDane(
            "porównywane wersje muszą należeć do tej oferty".into(),
        ));
    }
    let mut roznice = roznice_naglowka(&a.wersja, &b.wersja, &a.wycena, &b.wycena);
    roznice.extend(roznice::porownaj_specyfikacje(
        &a.specyfikacja,
        &b.specyfikacja,
    ));
    Ok(Porownanie {
        z_wersji: a.wersja.nr_wersji,
        do_wersji: b.wersja.nr_wersji,
        roznice,
    })
}

#[cfg(test)]
mod testy {
    use super::*;
    use crate::baza::otworz_w_pamieci;
    use crate::klienci::{self, DaneKlienta};
    use crate::specyfikacje::{DaneCzesci, DaneSekcji, DaneWiersza};

    async fn klient(pula: &SqlitePool) -> i64 {
        klienci::dodaj(
            pula,
            DaneKlienta {
                nazwa: "Klient Testowy".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap()
        .id
    }

    fn dane_z(w: &WersjaOferty) -> DaneWersji {
        DaneWersji {
            data_oferty: w.data_oferty.clone(),
            termin_realizacji: w.termin_realizacji.clone(),
            data_waznosci_cen: w.data_waznosci_cen.clone(),
            adres_inwestycji: w.adres_inwestycji.clone(),
            poziom_wyceny: w.poziom_wyceny.clone(),
            cena_calosc_gr: w.cena_calosc_gr,
            rabat_termin_gr: w.rabat_termin_gr,
            rabat_calosc_bp: w.rabat_calosc_bp,
            stawka_vat: w.stawka_vat,
            tryb_cen: w.tryb_cen.clone(),
            koszt_wlasny_gr: w.koszt_wlasny_gr,
            klauzule: w.klauzule.clone(),
            uwagi: w.uwagi.clone(),
        }
    }

    #[tokio::test]
    async fn oferta_wersje_wycena_i_porownanie() {
        let pula = otworz_w_pamieci().await.unwrap();
        let k = klient(&pula).await;
        let p = dodaj(
            &pula,
            DaneNowejOferty {
                klient_id: k,
                stawka_vat: Some(8),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert!(p.oferta.numer.starts_with("OF/"));
        assert_eq!(p.wersja.nr_wersji, 1);
        assert_eq!(p.wersja.stawka_vat, 8);
        assert_eq!(p.wersja.klauzule.len(), 4);
        assert!(p.wersja.najnowsza);

        let spec = p.wersja.specyfikacja_id;
        let s = specyfikacje::dodaj_sekcje(
            &pula,
            spec,
            DaneSekcji {
                nazwa: "Kuchnia".into(),
                cena_gr: Some(2_160_000),
                ..Default::default()
            },
        )
        .await
        .unwrap()
        .id;
        let c = specyfikacje::dodaj_czesc(
            &pula,
            s,
            DaneCzesci {
                nazwa: "Słupki".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap()
        .id;
        let w = specyfikacje::dodaj_wiersz(
            &pula,
            c,
            DaneWiersza {
                rodzaj: Some("Korpus".into()),
                material: Some("Płyta A".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap()
        .id;

        let mut d = dane_z(&p.wersja);
        d.rabat_calosc_bp = 1000;
        let p1 = zmien_wersje(&pula, p.wersja.id, d).await.unwrap();
        assert_eq!(p1.wycena.suma_brutto_gr, 1_944_000);

        let p2 = nowa_wersja(&pula, p.oferta.id, DaneNowejWersji::default())
            .await
            .unwrap();
        assert_eq!(p2.wersja.nr_wersji, 2);
        assert_eq!(p2.wycena.suma_brutto_gr, 1_944_000);
        assert_ne!(p2.wersja.specyfikacja_id, spec);
        assert_eq!(p2.wersje.len(), 2);

        // v1 zamrożona
        assert!(zmien_wersje(&pula, p.wersja.id, dane_z(&p1.wersja))
            .await
            .is_err());
        assert!(specyfikacje::zmien_wiersz(&pula, w, DaneWiersza::default())
            .await
            .is_err());

        // zmiana w v2 i porównanie
        let w2 = p2.specyfikacja.sekcje[0].czesci[0].wiersze[0].id;
        specyfikacje::zmien_wiersz(
            &pula,
            w2,
            DaneWiersza {
                rodzaj: Some("Korpus".into()),
                material: Some("Płyta B".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        specyfikacje::dodaj_czesc(
            &pula,
            p2.specyfikacja.sekcje[0].id,
            DaneCzesci {
                nazwa: "Wyspa".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let por = porownaj(&pula, p.oferta.id, p.wersja.id, p2.wersja.id)
            .await
            .unwrap();
        assert!(por.roznice.iter().any(|r| r.poziom == "wiersz"
            && r.zmiana == "zmieniono"
            && r.pola
                .iter()
                .any(|f| f.pole == "materiał" && f.po.as_deref() == Some("Płyta B"))));
        assert!(por
            .roznice
            .iter()
            .any(|r| r.poziom == "czesc" && r.zmiana == "dodano" && r.sciezka.ends_with("Wyspa")));
        assert_eq!(por.z_wersji, 1);
        assert_eq!(por.do_wersji, 2);
    }

    #[tokio::test]
    async fn statusy_lista_i_usuwanie() {
        let pula = otworz_w_pamieci().await.unwrap();
        let k = klient(&pula).await;
        let a = dodaj(
            &pula,
            DaneNowejOferty {
                klient_id: k,
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let b = dodaj(
            &pula,
            DaneNowejOferty {
                klient_id: k,
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert_ne!(a.oferta.numer, b.oferta.numer);
        assert!(dodaj(
            &pula,
            DaneNowejOferty {
                klient_id: 999,
                ..Default::default()
            }
        )
        .await
        .is_err());

        let s = zmien_status(
            &pula,
            a.oferta.id,
            ZmianaStatusu {
                status: "zaakceptowana".into(),
            },
        )
        .await
        .unwrap();
        assert!(s.oferta.data_akceptacji.is_some());
        assert!(zmien_status(
            &pula,
            a.oferta.id,
            ZmianaStatusu {
                status: "zła".into()
            }
        )
        .await
        .is_err());

        let l = lista(
            &pula,
            FiltrOfert {
                status: Some("zaakceptowana".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].id, a.oferta.id);
        let l = lista(
            &pula,
            FiltrOfert {
                klient_id: Some(k),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(l.len(), 2);

        nowa_wersja(&pula, b.oferta.id, DaneNowejWersji::default())
            .await
            .unwrap();
        usun(&pula, b.oferta.id).await.unwrap();
        let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM specyfikacje")
            .fetch_one(&pula)
            .await
            .unwrap();
        assert_eq!(n, 1);
    }
}
