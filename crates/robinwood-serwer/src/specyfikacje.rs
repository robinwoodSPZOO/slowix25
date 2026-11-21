//! Specyfikacja mebli: sekcja (pomieszczenie) → część (moduł) → wiersze.
//!
//! Wspólna dla wersji oferty, umowy i aneksu – moduły ofert i umów wskazują na `specyfikacje.id`
//! i używają tych samych funkcji oraz tras `/api/v1/specyfikacje/...`, `/sekcje/...`,
//! `/czesci/...`, `/wiersze/...`.
//!
//! Każda zmiana zwraca [`WynikZmiany`] – identyfikator dotkniętego elementu i całe aktualne
//! drzewo specyfikacji (zmiana może dotknąć sąsiednich wierszy, np. przy dziedziczeniu
//! materiału korpusu).

use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqliteConnection, SqlitePool};

use crate::bledy::{BladApi, Wynik};

pub const PODGRUPY: &[&str] = &["glowna", "wyposazenie", "frezowanie"];
pub const STATUSY_WIERSZA: &[&str] = &[
    "ustalone",
    "do_potwierdzenia",
    "po_stronie_klienta",
    "poza_wycena",
];
pub const TYPY_SEKCJI: &[&str] = &["meble", "agd"];
/// Rodzaj wiersza, którego materiał „spływa” na wiersze zależne.
pub const RODZAJ_KORPUS: &str = "Korpus";
/// Rodzaje, które domyślnie dziedziczą materiał korpusu, dopóki nie zostaną zmienione ręcznie.
pub const RODZAJE_Z_MATERIALEM_KORPUSU: &[&str] = &["Półki", "Cokół", "Plecy"];

// ---------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, FromRow, PartialEq)]
pub struct Wiersz {
    pub id: i64,
    pub czesc_id: i64,
    pub ident: Option<String>,
    pub podgrupa: String,
    pub rodzaj: Option<String>,
    pub material: Option<String>,
    pub opis: Option<String>,
    pub ilosc: Option<f64>,
    pub jednostka: Option<String>,
    pub status: String,
    pub termin_potwierdzenia: Option<String>,
    pub kategoria_materialu_id: Option<i64>,
    pub pozycja_slownika_id: Option<i64>,
    pub cena_gr: Option<i64>,
    pub material_z_korpusu: bool,
    pub kolejnosc: i64,
}

#[derive(Debug, Clone, Serialize, FromRow, PartialEq)]
pub struct Czesc {
    pub id: i64,
    pub sekcja_id: i64,
    pub ident: Option<String>,
    pub nazwa: String,
    pub cena_gr: Option<i64>,
    pub kolejnosc: i64,
    #[sqlx(skip)]
    pub wiersze: Vec<Wiersz>,
}

#[derive(Debug, Clone, Serialize, FromRow, PartialEq)]
pub struct Sekcja {
    pub id: i64,
    pub specyfikacja_id: i64,
    pub ident: Option<String>,
    pub numer: Option<String>,
    pub nazwa: String,
    pub typ: String,
    pub wariant: Option<String>,
    pub cena_gr: Option<i64>,
    pub stawka_vat: Option<i64>,
    pub kolejnosc: i64,
    #[sqlx(skip)]
    pub czesci: Vec<Czesc>,
}

#[derive(Debug, Clone, Serialize, FromRow, PartialEq)]
pub struct Zdjecie {
    pub id: i64,
    pub specyfikacja_id: i64,
    pub sekcja_id: Option<i64>,
    pub czesc_id: Option<i64>,
    pub plik_id: i64,
    pub podpis: Option<String>,
    pub kolejnosc: i64,
    pub nazwa: String,
    pub typ_mime: Option<String>,
}

/// Do czego należy specyfikacja (np. wersja oferty) – decyduje o edytowalności.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Wlasciciel {
    pub rodzaj: String,
    pub id: i64,
    pub nadrzedny_id: i64,
    pub opis: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Specyfikacja {
    pub id: i64,
    pub zrodlo_id: Option<i64>,
    pub sekcje: Vec<Sekcja>,
    pub zdjecia: Vec<Zdjecie>,
    pub wlasciciel: Option<Wlasciciel>,
    /// Starsza wersja oferty – tylko do odczytu (zmiany przez nową wersję).
    pub tylko_do_odczytu: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct WynikZmiany {
    pub id: i64,
    pub specyfikacja: Specyfikacja,
}

// ---------------------------------------------------------------------------
// Dane wejściowe
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct DaneSekcji {
    pub nazwa: String,
    pub typ: Option<String>,
    pub wariant: Option<String>,
    pub cena_gr: Option<i64>,
    pub stawka_vat: Option<i64>,
    /// Przy dodawaniu: wstaw za tą sekcją (inaczej na końcu).
    pub po_sekcji_id: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct DaneCzesci {
    pub nazwa: String,
    pub cena_gr: Option<i64>,
    pub po_czesci_id: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct DaneWiersza {
    pub podgrupa: Option<String>,
    pub rodzaj: Option<String>,
    pub material: Option<String>,
    pub opis: Option<String>,
    pub ilosc: Option<f64>,
    pub jednostka: Option<String>,
    pub status: Option<String>,
    pub termin_potwierdzenia: Option<String>,
    pub kategoria_materialu_id: Option<i64>,
    pub pozycja_slownika_id: Option<i64>,
    pub cena_gr: Option<i64>,
    /// `None` = ustal automatycznie (ręczna zmiana materiału zeruje dziedziczenie).
    pub material_z_korpusu: Option<bool>,
    /// Przy dodawaniu: wstaw za tym wierszem (Enter w tabeli).
    pub po_wierszu_id: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct Kolejnosc {
    /// Pełna lista identyfikatorów w nowej kolejności. Elementy z innego rodzica tej samej
    /// specyfikacji są przenoszone (np. część do innej sekcji).
    pub ids: Vec<i64>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct DaneKopii {
    /// Część: sekcja docelowa (domyślnie ta sama). Sekcja: specyfikacja docelowa.
    pub cel_id: Option<i64>,
}

fn oczysc_tekst(t: Option<String>) -> Option<String> {
    t.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

fn sprawdz_vat(v: Option<i64>) -> Wynik<()> {
    match v {
        None | Some(8) | Some(23) => Ok(()),
        Some(x) => Err(BladApi::NieprawidloweDane(format!(
            "stawka VAT {x}% – dozwolone 8 lub 23"
        ))),
    }
}

fn sprawdz_cene(c: Option<i64>) -> Wynik<()> {
    match c {
        Some(x) if x < 0 => Err(BladApi::NieprawidloweDane(
            "cena nie może być ujemna".into(),
        )),
        _ => Ok(()),
    }
}

impl DaneSekcji {
    fn oczysc(mut self) -> Wynik<Self> {
        self.nazwa = self.nazwa.trim().to_string();
        if self.nazwa.is_empty() {
            return Err(BladApi::NieprawidloweDane(
                "nazwa sekcji jest wymagana".into(),
            ));
        }
        let typ = self.typ.take().unwrap_or_else(|| "meble".into());
        if !TYPY_SEKCJI.contains(&typ.as_str()) {
            return Err(BladApi::NieprawidloweDane(format!(
                "nieznany typ sekcji „{typ}”"
            )));
        }
        self.typ = Some(typ);
        self.wariant = oczysc_tekst(self.wariant);
        sprawdz_vat(self.stawka_vat)?;
        sprawdz_cene(self.cena_gr)?;
        Ok(self)
    }
}

impl DaneCzesci {
    fn oczysc(mut self) -> Wynik<Self> {
        self.nazwa = self.nazwa.trim().to_string();
        if self.nazwa.is_empty() {
            return Err(BladApi::NieprawidloweDane(
                "nazwa części jest wymagana".into(),
            ));
        }
        sprawdz_cene(self.cena_gr)?;
        Ok(self)
    }
}

impl DaneWiersza {
    fn oczysc(mut self) -> Wynik<Self> {
        let podgrupa = self.podgrupa.take().unwrap_or_else(|| "glowna".into());
        if !PODGRUPY.contains(&podgrupa.as_str()) {
            return Err(BladApi::NieprawidloweDane(format!(
                "nieznana podgrupa „{podgrupa}”"
            )));
        }
        self.podgrupa = Some(podgrupa);
        let status = self.status.take().unwrap_or_else(|| "ustalone".into());
        if !STATUSY_WIERSZA.contains(&status.as_str()) {
            return Err(BladApi::NieprawidloweDane(format!(
                "nieznany status wiersza „{status}” (dozwolone: {})",
                STATUSY_WIERSZA.join(", ")
            )));
        }
        self.status = Some(status);
        self.rodzaj = oczysc_tekst(self.rodzaj);
        self.material = oczysc_tekst(self.material);
        self.opis = oczysc_tekst(self.opis);
        self.jednostka = oczysc_tekst(self.jednostka);
        self.termin_potwierdzenia = oczysc_tekst(self.termin_potwierdzenia);
        if let Some(i) = self.ilosc {
            if !i.is_finite() || i < 0.0 {
                return Err(BladApi::NieprawidloweDane(
                    "ilość musi być liczbą nieujemną".into(),
                ));
            }
        }
        sprawdz_cene(self.cena_gr)?;
        Ok(self)
    }
}

pub fn nowy_ident() -> String {
    use rand::Rng;
    format!("{:016x}", rand::thread_rng().gen::<u64>())
}

fn rowne_bez_wielkosci(a: Option<&str>, b: &str) -> bool {
    a.map(|a| a.trim().to_lowercase() == b.to_lowercase())
        .unwrap_or(false)
}

pub fn dziedziczy_material_korpusu(rodzaj: Option<&str>) -> bool {
    RODZAJE_Z_MATERIALEM_KORPUSU
        .iter()
        .any(|r| rowne_bez_wielkosci(rodzaj, r))
}

// ---------------------------------------------------------------------------
// Odczyt
// ---------------------------------------------------------------------------

const KOLUMNY_SEKCJI: &str =
    "s.id, s.specyfikacja_id, s.ident, s.numer, s.nazwa, s.typ, s.wariant, s.cena_gr, s.stawka_vat, s.kolejnosc";
const KOLUMNY_CZESCI: &str = "c.id, c.sekcja_id, c.ident, c.nazwa, c.cena_gr, c.kolejnosc";
const KOLUMNY_WIERSZA: &str = "w.id, w.czesc_id, w.ident, w.podgrupa, w.rodzaj, w.material, w.opis, w.ilosc, \
    w.jednostka, w.status, w.termin_potwierdzenia, w.kategoria_materialu_id, w.pozycja_slownika_id, \
    w.cena_gr, w.material_z_korpusu, w.kolejnosc";

async fn wlasciciel(db: &mut SqliteConnection, spec_id: i64) -> Wynik<(Option<Wlasciciel>, bool)> {
    let wiersz: Option<(i64, i64, i64, i64, String)> = sqlx::query_as(
        "SELECT w.id, w.oferta_id, w.nr_wersji, \
           (SELECT MAX(nr_wersji) FROM wersje_oferty WHERE oferta_id = w.oferta_id), o.numer \
         FROM wersje_oferty w JOIN oferty o ON o.id = w.oferta_id WHERE w.specyfikacja_id = ?1",
    )
    .bind(spec_id)
    .fetch_optional(&mut *db)
    .await?;
    Ok(match wiersz {
        Some((wid, oid, nr, max, numer)) => (
            Some(Wlasciciel {
                rodzaj: "wersja_oferty".into(),
                id: wid,
                nadrzedny_id: oid,
                opis: format!("Oferta {numer} v{nr}"),
            }),
            nr < max,
        ),
        None => (None, false),
    })
}

pub async fn pobierz_w(db: &mut SqliteConnection, id: i64) -> Wynik<Specyfikacja> {
    let zrodlo: Option<(Option<i64>,)> =
        sqlx::query_as("SELECT zrodlo_id FROM specyfikacje WHERE id = ?1")
            .bind(id)
            .fetch_optional(&mut *db)
            .await?;
    let Some((zrodlo_id,)) = zrodlo else {
        return Err(BladApi::NieZnaleziono(format!("specyfikacja {id}")));
    };
    let mut sekcje: Vec<Sekcja> = sqlx::query_as(&format!(
        "SELECT {KOLUMNY_SEKCJI} FROM sekcje_specyfikacji s WHERE s.specyfikacja_id = ?1 \
         ORDER BY s.kolejnosc, s.id"
    ))
    .bind(id)
    .fetch_all(&mut *db)
    .await?;
    let mut czesci: Vec<Czesc> = sqlx::query_as(&format!(
        "SELECT {KOLUMNY_CZESCI} FROM czesci_specyfikacji c \
         JOIN sekcje_specyfikacji s ON s.id = c.sekcja_id WHERE s.specyfikacja_id = ?1 \
         ORDER BY c.kolejnosc, c.id"
    ))
    .bind(id)
    .fetch_all(&mut *db)
    .await?;
    let wiersze: Vec<Wiersz> = sqlx::query_as(&format!(
        "SELECT {KOLUMNY_WIERSZA} FROM wiersze_specyfikacji w \
         JOIN czesci_specyfikacji c ON c.id = w.czesc_id \
         JOIN sekcje_specyfikacji s ON s.id = c.sekcja_id WHERE s.specyfikacja_id = ?1 \
         ORDER BY w.kolejnosc, w.id"
    ))
    .bind(id)
    .fetch_all(&mut *db)
    .await?;
    let zdjecia: Vec<Zdjecie> = sqlx::query_as(
        "SELECT z.id, z.specyfikacja_id, z.sekcja_id, z.czesc_id, z.plik_id, z.podpis, z.kolejnosc, \
           p.nazwa, p.typ_mime \
         FROM zdjecia_specyfikacji z JOIN pliki p ON p.id = z.plik_id \
         WHERE z.specyfikacja_id = ?1 ORDER BY z.kolejnosc, z.id",
    )
    .bind(id)
    .fetch_all(&mut *db)
    .await?;

    for w in wiersze {
        if let Some(c) = czesci.iter_mut().find(|c| c.id == w.czesc_id) {
            c.wiersze.push(w);
        }
    }
    for c in czesci {
        if let Some(s) = sekcje.iter_mut().find(|s| s.id == c.sekcja_id) {
            s.czesci.push(c);
        }
    }
    let (wlasciciel, tylko_do_odczytu) = wlasciciel(db, id).await?;
    Ok(Specyfikacja {
        id,
        zrodlo_id,
        sekcje,
        zdjecia,
        wlasciciel,
        tylko_do_odczytu,
    })
}

pub async fn pobierz(pula: &SqlitePool, id: i64) -> Wynik<Specyfikacja> {
    let mut db = pula.acquire().await?;
    pobierz_w(&mut db, id).await
}

/// Nowa, pusta specyfikacja (np. dla nowej oferty albo umowy „od zera”).
pub async fn utworz_w(db: &mut SqliteConnection) -> Wynik<i64> {
    Ok(sqlx::query("INSERT INTO specyfikacje DEFAULT VALUES")
        .execute(&mut *db)
        .await?
        .last_insert_rowid())
}

// ---------------------------------------------------------------------------
// Pomocnicze: rodzice, blokada zapisu, dziennik, numeracja
// ---------------------------------------------------------------------------

async fn spec_sekcji(db: &mut SqliteConnection, sekcja_id: i64) -> Wynik<i64> {
    sqlx::query_scalar("SELECT specyfikacja_id FROM sekcje_specyfikacji WHERE id = ?1")
        .bind(sekcja_id)
        .fetch_optional(&mut *db)
        .await?
        .ok_or_else(|| BladApi::NieZnaleziono(format!("sekcja {sekcja_id}")))
}

async fn rodzic_czesci(db: &mut SqliteConnection, czesc_id: i64) -> Wynik<(i64, i64)> {
    sqlx::query_as(
        "SELECT c.sekcja_id, s.specyfikacja_id FROM czesci_specyfikacji c \
         JOIN sekcje_specyfikacji s ON s.id = c.sekcja_id WHERE c.id = ?1",
    )
    .bind(czesc_id)
    .fetch_optional(&mut *db)
    .await?
    .ok_or_else(|| BladApi::NieZnaleziono(format!("część {czesc_id}")))
}

async fn pobierz_wiersz(db: &mut SqliteConnection, id: i64) -> Wynik<Wiersz> {
    sqlx::query_as(&format!(
        "SELECT {KOLUMNY_WIERSZA} FROM wiersze_specyfikacji w WHERE w.id = ?1"
    ))
    .bind(id)
    .fetch_optional(&mut *db)
    .await?
    .ok_or_else(|| BladApi::NieZnaleziono(format!("wiersz {id}")))
}

/// Specyfikacja istnieje i można ją zmieniać (starsze wersje oferty są zamrożone).
pub async fn sprawdz_zapis(db: &mut SqliteConnection, spec_id: i64) -> Wynik<()> {
    let istnieje: Option<i64> = sqlx::query_scalar("SELECT id FROM specyfikacje WHERE id = ?1")
        .bind(spec_id)
        .fetch_optional(&mut *db)
        .await?;
    if istnieje.is_none() {
        return Err(BladApi::NieZnaleziono(format!("specyfikacja {spec_id}")));
    }
    let (_, tylko_odczyt) = wlasciciel(db, spec_id).await?;
    if tylko_odczyt {
        return Err(BladApi::NieprawidloweDane(
            "starsza wersja oferty jest tylko do odczytu – zmiany wprowadź w najnowszej wersji"
                .into(),
        ));
    }
    Ok(())
}

pub async fn dziennik(
    db: &mut SqliteConnection,
    tabela: &str,
    id: i64,
    operacja: &str,
) -> Wynik<()> {
    sqlx::query("INSERT INTO dziennik_zmian (tabela, rekord_id, operacja) VALUES (?1, ?2, ?3)")
        .bind(tabela)
        .bind(id)
        .bind(operacja)
        .execute(&mut *db)
        .await?;
    Ok(())
}

/// Sekcje mebli numerowane „01”, „02”… wg kolejności; sekcje AGD bez numeru.
async fn przenumeruj(db: &mut SqliteConnection, spec_id: i64) -> Wynik<()> {
    let sekcje: Vec<(i64, String)> = sqlx::query_as(
        "SELECT id, typ FROM sekcje_specyfikacji WHERE specyfikacja_id = ?1 ORDER BY kolejnosc, id",
    )
    .bind(spec_id)
    .fetch_all(&mut *db)
    .await?;
    let mut nr = 0;
    for (i, (id, typ)) in sekcje.iter().enumerate() {
        let numer = if typ == "meble" {
            nr += 1;
            Some(format!("{nr:02}"))
        } else {
            None
        };
        sqlx::query("UPDATE sekcje_specyfikacji SET numer = ?1, kolejnosc = ?2 WHERE id = ?3")
            .bind(numer)
            .bind(i as i64)
            .bind(id)
            .execute(&mut *db)
            .await?;
    }
    Ok(())
}

/// Ustala kolejność nowego elementu: za wskazanym (przesuwając następne) albo na końcu.
async fn miejsce(
    db: &mut SqliteConnection,
    tabela: &str,
    kolumna_rodzica: &str,
    rodzic_id: i64,
    po_id: Option<i64>,
) -> Wynik<i64> {
    if let Some(po) = po_id {
        let k: Option<i64> = sqlx::query_scalar(&format!(
            "SELECT kolejnosc FROM {tabela} WHERE id = ?1 AND {kolumna_rodzica} = ?2"
        ))
        .bind(po)
        .bind(rodzic_id)
        .fetch_optional(&mut *db)
        .await?;
        if let Some(k) = k {
            sqlx::query(&format!(
                "UPDATE {tabela} SET kolejnosc = kolejnosc + 1 WHERE {kolumna_rodzica} = ?1 AND kolejnosc > ?2"
            ))
            .bind(rodzic_id)
            .bind(k)
            .execute(&mut *db)
            .await?;
            return Ok(k + 1);
        }
    }
    let max: Option<i64> = sqlx::query_scalar(&format!(
        "SELECT MAX(kolejnosc) FROM {tabela} WHERE {kolumna_rodzica} = ?1"
    ))
    .bind(rodzic_id)
    .fetch_one(&mut *db)
    .await?;
    Ok(max.map(|m| m + 1).unwrap_or(0))
}

async fn wynik(db: &mut SqliteConnection, id: i64, spec_id: i64) -> Wynik<WynikZmiany> {
    Ok(WynikZmiany {
        id,
        specyfikacja: pobierz_w(db, spec_id).await?,
    })
}

// ---------------------------------------------------------------------------
// Sekcje
// ---------------------------------------------------------------------------

pub async fn dodaj_sekcje(pula: &SqlitePool, spec_id: i64, dane: DaneSekcji) -> Wynik<WynikZmiany> {
    let d = dane.oczysc()?;
    let mut tx = pula.begin().await?;
    sprawdz_zapis(&mut tx, spec_id).await?;
    let id = wstaw_sekcje(&mut tx, spec_id, &d, None).await?;
    przenumeruj(&mut tx, spec_id).await?;
    dziennik(&mut tx, "sekcje_specyfikacji", id, "dodanie").await?;
    let w = wynik(&mut tx, id, spec_id).await?;
    tx.commit().await?;
    Ok(w)
}

async fn wstaw_sekcje(
    db: &mut SqliteConnection,
    spec_id: i64,
    d: &DaneSekcji,
    ident: Option<&str>,
) -> Wynik<i64> {
    let kolejnosc = miejsce(
        db,
        "sekcje_specyfikacji",
        "specyfikacja_id",
        spec_id,
        d.po_sekcji_id,
    )
    .await?;
    Ok(sqlx::query(
        "INSERT INTO sekcje_specyfikacji (specyfikacja_id, ident, nazwa, typ, wariant, cena_gr, stawka_vat, kolejnosc) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
    )
    .bind(spec_id)
    .bind(ident.map(str::to_owned).unwrap_or_else(nowy_ident))
    .bind(&d.nazwa)
    .bind(d.typ.as_deref().unwrap_or("meble"))
    .bind(&d.wariant)
    .bind(d.cena_gr)
    .bind(d.stawka_vat)
    .bind(kolejnosc)
    .execute(&mut *db)
    .await?
    .last_insert_rowid())
}

pub async fn zmien_sekcje(pula: &SqlitePool, id: i64, dane: DaneSekcji) -> Wynik<WynikZmiany> {
    let d = dane.oczysc()?;
    let mut tx = pula.begin().await?;
    let spec_id = spec_sekcji(&mut tx, id).await?;
    sprawdz_zapis(&mut tx, spec_id).await?;
    sqlx::query(
        "UPDATE sekcje_specyfikacji SET nazwa = ?1, typ = ?2, wariant = ?3, cena_gr = ?4, stawka_vat = ?5 WHERE id = ?6",
    )
    .bind(&d.nazwa)
    .bind(d.typ.as_deref().unwrap_or("meble"))
    .bind(&d.wariant)
    .bind(d.cena_gr)
    .bind(d.stawka_vat)
    .bind(id)
    .execute(&mut *tx)
    .await?;
    przenumeruj(&mut tx, spec_id).await?;
    dziennik(&mut tx, "sekcje_specyfikacji", id, "zmiana").await?;
    let w = wynik(&mut tx, id, spec_id).await?;
    tx.commit().await?;
    Ok(w)
}

pub async fn usun_sekcje(pula: &SqlitePool, id: i64) -> Wynik<WynikZmiany> {
    let mut tx = pula.begin().await?;
    let spec_id = spec_sekcji(&mut tx, id).await?;
    sprawdz_zapis(&mut tx, spec_id).await?;
    sqlx::query("DELETE FROM sekcje_specyfikacji WHERE id = ?1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    przenumeruj(&mut tx, spec_id).await?;
    dziennik(&mut tx, "sekcje_specyfikacji", id, "usuniecie").await?;
    let w = wynik(&mut tx, id, spec_id).await?;
    tx.commit().await?;
    Ok(w)
}

pub async fn kolejnosc_sekcji(pula: &SqlitePool, spec_id: i64, k: Kolejnosc) -> Wynik<WynikZmiany> {
    let mut tx = pula.begin().await?;
    sprawdz_zapis(&mut tx, spec_id).await?;
    for (i, id) in k.ids.iter().enumerate() {
        let n = sqlx::query(
            "UPDATE sekcje_specyfikacji SET kolejnosc = ?1 WHERE id = ?2 AND specyfikacja_id = ?3",
        )
        .bind(i as i64)
        .bind(id)
        .bind(spec_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if n == 0 {
            return Err(BladApi::NieprawidloweDane(format!(
                "sekcja {id} nie należy do specyfikacji {spec_id}"
            )));
        }
    }
    przenumeruj(&mut tx, spec_id).await?;
    let w = wynik(&mut tx, spec_id, spec_id).await?;
    tx.commit().await?;
    Ok(w)
}

/// Kopia sekcji (z częściami, wierszami i zdjęciami) – na końcu tej samej albo innej specyfikacji.
pub async fn kopiuj_sekcje(pula: &SqlitePool, id: i64, dane: DaneKopii) -> Wynik<WynikZmiany> {
    let mut tx = pula.begin().await?;
    let zrodlo_spec = spec_sekcji(&mut tx, id).await?;
    let cel = dane.cel_id.unwrap_or(zrodlo_spec);
    sprawdz_zapis(&mut tx, cel).await?;
    let nowa = kopiuj_sekcje_w(&mut tx, id, cel, false, cel == zrodlo_spec).await?;
    przenumeruj(&mut tx, cel).await?;
    dziennik(&mut tx, "sekcje_specyfikacji", nowa, "dodanie").await?;
    let w = wynik(&mut tx, nowa, cel).await?;
    tx.commit().await?;
    Ok(w)
}

async fn kopiuj_sekcje_w(
    db: &mut SqliteConnection,
    sekcja_id: i64,
    cel_spec: i64,
    zachowaj_ident: bool,
    dopisz_kopia: bool,
) -> Wynik<i64> {
    let s: Sekcja = sqlx::query_as(&format!(
        "SELECT {KOLUMNY_SEKCJI} FROM sekcje_specyfikacji s WHERE s.id = ?1"
    ))
    .bind(sekcja_id)
    .fetch_one(&mut *db)
    .await?;
    let kolejnosc = if zachowaj_ident {
        s.kolejnosc
    } else {
        miejsce(db, "sekcje_specyfikacji", "specyfikacja_id", cel_spec, None).await?
    };
    let nazwa = if dopisz_kopia {
        format!("{} (kopia)", s.nazwa)
    } else {
        s.nazwa.clone()
    };
    let ident = if zachowaj_ident {
        s.ident.clone().unwrap_or_else(nowy_ident)
    } else {
        nowy_ident()
    };
    let nowa = sqlx::query(
        "INSERT INTO sekcje_specyfikacji (specyfikacja_id, ident, numer, nazwa, typ, wariant, cena_gr, stawka_vat, kolejnosc) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
    )
    .bind(cel_spec)
    .bind(ident)
    .bind(&s.numer)
    .bind(nazwa)
    .bind(&s.typ)
    .bind(&s.wariant)
    .bind(s.cena_gr)
    .bind(s.stawka_vat)
    .bind(kolejnosc)
    .execute(&mut *db)
    .await?
    .last_insert_rowid();

    let czesci: Vec<i64> = sqlx::query_scalar(
        "SELECT id FROM czesci_specyfikacji WHERE sekcja_id = ?1 ORDER BY kolejnosc, id",
    )
    .bind(sekcja_id)
    .fetch_all(&mut *db)
    .await?;
    for c in czesci {
        kopiuj_czesc_w(db, c, nowa, cel_spec, zachowaj_ident, false).await?;
    }
    sqlx::query(
        "INSERT INTO zdjecia_specyfikacji (specyfikacja_id, sekcja_id, czesc_id, plik_id, podpis, kolejnosc) \
         SELECT ?1, ?2, NULL, plik_id, podpis, kolejnosc FROM zdjecia_specyfikacji \
         WHERE sekcja_id = ?3 AND czesc_id IS NULL",
    )
    .bind(cel_spec)
    .bind(nowa)
    .bind(sekcja_id)
    .execute(&mut *db)
    .await?;
    Ok(nowa)
}

// ---------------------------------------------------------------------------
// Części
// ---------------------------------------------------------------------------

pub async fn dodaj_czesc(
    pula: &SqlitePool,
    sekcja_id: i64,
    dane: DaneCzesci,
) -> Wynik<WynikZmiany> {
    let d = dane.oczysc()?;
    let mut tx = pula.begin().await?;
    let spec_id = spec_sekcji(&mut tx, sekcja_id).await?;
    sprawdz_zapis(&mut tx, spec_id).await?;
    let id = wstaw_czesc(&mut tx, sekcja_id, &d.nazwa, d.cena_gr, d.po_czesci_id).await?;
    dziennik(&mut tx, "czesci_specyfikacji", id, "dodanie").await?;
    let w = wynik(&mut tx, id, spec_id).await?;
    tx.commit().await?;
    Ok(w)
}

async fn wstaw_czesc(
    db: &mut SqliteConnection,
    sekcja_id: i64,
    nazwa: &str,
    cena_gr: Option<i64>,
    po: Option<i64>,
) -> Wynik<i64> {
    let kolejnosc = miejsce(db, "czesci_specyfikacji", "sekcja_id", sekcja_id, po).await?;
    Ok(sqlx::query(
        "INSERT INTO czesci_specyfikacji (sekcja_id, ident, nazwa, cena_gr, kolejnosc) VALUES (?1, ?2, ?3, ?4, ?5)",
    )
    .bind(sekcja_id)
    .bind(nowy_ident())
    .bind(nazwa)
    .bind(cena_gr)
    .bind(kolejnosc)
    .execute(&mut *db)
    .await?
    .last_insert_rowid())
}

pub async fn zmien_czesc(pula: &SqlitePool, id: i64, dane: DaneCzesci) -> Wynik<WynikZmiany> {
    let d = dane.oczysc()?;
    let mut tx = pula.begin().await?;
    let (_, spec_id) = rodzic_czesci(&mut tx, id).await?;
    sprawdz_zapis(&mut tx, spec_id).await?;
    sqlx::query("UPDATE czesci_specyfikacji SET nazwa = ?1, cena_gr = ?2 WHERE id = ?3")
        .bind(&d.nazwa)
        .bind(d.cena_gr)
        .bind(id)
        .execute(&mut *tx)
        .await?;
    dziennik(&mut tx, "czesci_specyfikacji", id, "zmiana").await?;
    let w = wynik(&mut tx, id, spec_id).await?;
    tx.commit().await?;
    Ok(w)
}

pub async fn usun_czesc(pula: &SqlitePool, id: i64) -> Wynik<WynikZmiany> {
    let mut tx = pula.begin().await?;
    let (_, spec_id) = rodzic_czesci(&mut tx, id).await?;
    sprawdz_zapis(&mut tx, spec_id).await?;
    sqlx::query("DELETE FROM czesci_specyfikacji WHERE id = ?1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    dziennik(&mut tx, "czesci_specyfikacji", id, "usuniecie").await?;
    let w = wynik(&mut tx, id, spec_id).await?;
    tx.commit().await?;
    Ok(w)
}

/// Kolejność części w sekcji; części z innych sekcji tej samej specyfikacji są przenoszone.
pub async fn kolejnosc_czesci(
    pula: &SqlitePool,
    sekcja_id: i64,
    k: Kolejnosc,
) -> Wynik<WynikZmiany> {
    let mut tx = pula.begin().await?;
    let spec_id = spec_sekcji(&mut tx, sekcja_id).await?;
    sprawdz_zapis(&mut tx, spec_id).await?;
    for (i, id) in k.ids.iter().enumerate() {
        let (_, spec_czesci) = rodzic_czesci(&mut tx, *id).await?;
        if spec_czesci != spec_id {
            return Err(BladApi::NieprawidloweDane(format!(
                "część {id} należy do innej specyfikacji"
            )));
        }
        sqlx::query("UPDATE czesci_specyfikacji SET sekcja_id = ?1, kolejnosc = ?2 WHERE id = ?3")
            .bind(sekcja_id)
            .bind(i as i64)
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    sqlx::query(
        "UPDATE zdjecia_specyfikacji SET sekcja_id = \
           (SELECT sekcja_id FROM czesci_specyfikacji WHERE id = zdjecia_specyfikacji.czesc_id) \
         WHERE czesc_id IS NOT NULL AND specyfikacja_id = ?1",
    )
    .bind(spec_id)
    .execute(&mut *tx)
    .await?;
    let w = wynik(&mut tx, sekcja_id, spec_id).await?;
    tx.commit().await?;
    Ok(w)
}

/// Kopia części z wierszami – w tej samej sekcji (zaraz za oryginałem) albo w innej.
pub async fn kopiuj_czesc(pula: &SqlitePool, id: i64, dane: DaneKopii) -> Wynik<WynikZmiany> {
    let mut tx = pula.begin().await?;
    let (sekcja_zrodlowa, _) = rodzic_czesci(&mut tx, id).await?;
    let cel = dane.cel_id.unwrap_or(sekcja_zrodlowa);
    let spec_cel = spec_sekcji(&mut tx, cel).await?;
    sprawdz_zapis(&mut tx, spec_cel).await?;
    let nowa = kopiuj_czesc_w(&mut tx, id, cel, spec_cel, false, cel == sekcja_zrodlowa).await?;
    dziennik(&mut tx, "czesci_specyfikacji", nowa, "dodanie").await?;
    let w = wynik(&mut tx, nowa, spec_cel).await?;
    tx.commit().await?;
    Ok(w)
}

async fn kopiuj_czesc_w(
    db: &mut SqliteConnection,
    czesc_id: i64,
    cel_sekcja: i64,
    cel_spec: i64,
    zachowaj_ident: bool,
    obok_oryginalu: bool,
) -> Wynik<i64> {
    let c: Czesc = sqlx::query_as(&format!(
        "SELECT {KOLUMNY_CZESCI} FROM czesci_specyfikacji c WHERE c.id = ?1"
    ))
    .bind(czesc_id)
    .fetch_one(&mut *db)
    .await?;
    let (nazwa, kolejnosc, ident) = if zachowaj_ident {
        (
            c.nazwa.clone(),
            c.kolejnosc,
            c.ident.clone().unwrap_or_else(nowy_ident),
        )
    } else if obok_oryginalu {
        (
            format!("{} (kopia)", c.nazwa),
            miejsce(
                db,
                "czesci_specyfikacji",
                "sekcja_id",
                cel_sekcja,
                Some(czesc_id),
            )
            .await?,
            nowy_ident(),
        )
    } else {
        (
            c.nazwa.clone(),
            miejsce(db, "czesci_specyfikacji", "sekcja_id", cel_sekcja, None).await?,
            nowy_ident(),
        )
    };
    let nowa = sqlx::query(
        "INSERT INTO czesci_specyfikacji (sekcja_id, ident, nazwa, cena_gr, kolejnosc) VALUES (?1, ?2, ?3, ?4, ?5)",
    )
    .bind(cel_sekcja)
    .bind(ident)
    .bind(nazwa)
    .bind(c.cena_gr)
    .bind(kolejnosc)
    .execute(&mut *db)
    .await?
    .last_insert_rowid();
    let nowy_ident_sql = if zachowaj_ident {
        "COALESCE(ident, lower(hex(randomblob(8))))"
    } else {
        "lower(hex(randomblob(8)))"
    };
    sqlx::query(&format!(
        "INSERT INTO wiersze_specyfikacji (czesc_id, ident, podgrupa, rodzaj, material, opis, ilosc, jednostka, \
           status, termin_potwierdzenia, kategoria_materialu_id, pozycja_slownika_id, cena_gr, material_z_korpusu, kolejnosc) \
         SELECT ?1, {nowy_ident_sql}, podgrupa, rodzaj, material, opis, ilosc, jednostka, status, termin_potwierdzenia, \
           kategoria_materialu_id, pozycja_slownika_id, cena_gr, material_z_korpusu, kolejnosc \
         FROM wiersze_specyfikacji WHERE czesc_id = ?2 ORDER BY kolejnosc, id"
    ))
    .bind(nowa)
    .bind(czesc_id)
    .execute(&mut *db)
    .await?;
    sqlx::query(
        "INSERT INTO zdjecia_specyfikacji (specyfikacja_id, sekcja_id, czesc_id, plik_id, podpis, kolejnosc) \
         SELECT ?1, ?2, ?3, plik_id, podpis, kolejnosc FROM zdjecia_specyfikacji WHERE czesc_id = ?4",
    )
    .bind(cel_spec)
    .bind(cel_sekcja)
    .bind(nowa)
    .bind(czesc_id)
    .execute(&mut *db)
    .await?;
    Ok(nowa)
}

// ---------------------------------------------------------------------------
// Wiersze
// ---------------------------------------------------------------------------

/// Materiał i kategoria pierwszego korpusu w części (podgrupa główna).
async fn material_korpusu(
    db: &mut SqliteConnection,
    czesc_id: i64,
) -> Wynik<Option<(String, Option<i64>)>> {
    let r: Option<(Option<String>, Option<i64>)> = sqlx::query_as(
        "SELECT material, kategoria_materialu_id FROM wiersze_specyfikacji \
         WHERE czesc_id = ?1 AND podgrupa = 'glowna' AND lower(trim(rodzaj)) = lower(?2) \
         ORDER BY kolejnosc, id LIMIT 1",
    )
    .bind(czesc_id)
    .bind(RODZAJ_KORPUS)
    .fetch_optional(&mut *db)
    .await?;
    Ok(r.and_then(|(m, k)| m.map(|m| (m, k))))
}

/// Materiał korpusu spływa na wiersze z flagą `material_z_korpusu`.
async fn rozlej_material_korpusu(db: &mut SqliteConnection, czesc_id: i64) -> Wynik<()> {
    let korpus: Option<(i64, Option<String>, Option<i64>)> = sqlx::query_as(
        "SELECT id, material, kategoria_materialu_id FROM wiersze_specyfikacji \
         WHERE czesc_id = ?1 AND podgrupa = 'glowna' AND lower(trim(rodzaj)) = lower(?2) \
         ORDER BY kolejnosc, id LIMIT 1",
    )
    .bind(czesc_id)
    .bind(RODZAJ_KORPUS)
    .fetch_optional(&mut *db)
    .await?;
    if let Some((kid, material, kategoria)) = korpus {
        sqlx::query(
            "UPDATE wiersze_specyfikacji SET material = ?1, kategoria_materialu_id = ?2 \
             WHERE czesc_id = ?3 AND id != ?4 AND podgrupa = 'glowna' AND material_z_korpusu = 1",
        )
        .bind(material)
        .bind(kategoria)
        .bind(czesc_id)
        .bind(kid)
        .execute(&mut *db)
        .await?;
    }
    Ok(())
}

pub async fn dodaj_wiersz(
    pula: &SqlitePool,
    czesc_id: i64,
    dane: DaneWiersza,
) -> Wynik<WynikZmiany> {
    let d = dane.oczysc()?;
    let mut tx = pula.begin().await?;
    let (_, spec_id) = rodzic_czesci(&mut tx, czesc_id).await?;
    sprawdz_zapis(&mut tx, spec_id).await?;
    let id = wstaw_wiersz(&mut tx, czesc_id, d).await?;
    dziennik(&mut tx, "wiersze_specyfikacji", id, "dodanie").await?;
    let w = wynik(&mut tx, id, spec_id).await?;
    tx.commit().await?;
    Ok(w)
}

async fn wstaw_wiersz(db: &mut SqliteConnection, czesc_id: i64, mut d: DaneWiersza) -> Wynik<i64> {
    let kolejnosc = miejsce(
        db,
        "wiersze_specyfikacji",
        "czesc_id",
        czesc_id,
        d.po_wierszu_id,
    )
    .await?;
    let mut z_korpusu = d.material_z_korpusu.unwrap_or(false);
    if d.podgrupa.as_deref() == Some("glowna")
        && d.material.is_none()
        && d.material_z_korpusu != Some(false)
        && dziedziczy_material_korpusu(d.rodzaj.as_deref())
    {
        z_korpusu = true;
        if let Some((m, k)) = material_korpusu(db, czesc_id).await? {
            d.material = Some(m);
            if d.kategoria_materialu_id.is_none() {
                d.kategoria_materialu_id = k;
            }
        }
    }
    let id = sqlx::query(
        "INSERT INTO wiersze_specyfikacji (czesc_id, ident, podgrupa, rodzaj, material, opis, ilosc, jednostka, status, \
           termin_potwierdzenia, kategoria_materialu_id, pozycja_slownika_id, cena_gr, material_z_korpusu, kolejnosc) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
    )
    .bind(czesc_id)
    .bind(nowy_ident())
    .bind(d.podgrupa.as_deref().unwrap_or("glowna"))
    .bind(&d.rodzaj)
    .bind(&d.material)
    .bind(&d.opis)
    .bind(d.ilosc)
    .bind(&d.jednostka)
    .bind(d.status.as_deref().unwrap_or("ustalone"))
    .bind(&d.termin_potwierdzenia)
    .bind(d.kategoria_materialu_id)
    .bind(d.pozycja_slownika_id)
    .bind(d.cena_gr)
    .bind(z_korpusu)
    .bind(kolejnosc)
    .execute(&mut *db)
    .await?
    .last_insert_rowid();
    if rowne_bez_wielkosci(d.rodzaj.as_deref(), RODZAJ_KORPUS) {
        rozlej_material_korpusu(db, czesc_id).await?;
    }
    Ok(id)
}

pub async fn zmien_wiersz(pula: &SqlitePool, id: i64, dane: DaneWiersza) -> Wynik<WynikZmiany> {
    let mut d = dane.oczysc()?;
    let mut tx = pula.begin().await?;
    let stary = pobierz_wiersz(&mut tx, id).await?;
    let (_, spec_id) = rodzic_czesci(&mut tx, stary.czesc_id).await?;
    sprawdz_zapis(&mut tx, spec_id).await?;

    let zmiana_materialu = d.material != stary.material;
    let mut z_korpusu = match d.material_z_korpusu {
        Some(v) => v,
        None if zmiana_materialu => false,
        None => stary.material_z_korpusu,
    };
    // Wiersz zmienił rodzaj na zależny i nie ma materiału – podpinamy do korpusu.
    if d.material.is_none()
        && d.material_z_korpusu != Some(false)
        && d.podgrupa.as_deref() == Some("glowna")
        && dziedziczy_material_korpusu(d.rodzaj.as_deref())
    {
        z_korpusu = true;
    }
    if z_korpusu && d.material_z_korpusu == Some(true) || (z_korpusu && d.material.is_none()) {
        if let Some((m, k)) = material_korpusu(&mut tx, stary.czesc_id).await? {
            d.material = Some(m);
            d.kategoria_materialu_id = d.kategoria_materialu_id.or(k);
        }
    }

    sqlx::query(
        "UPDATE wiersze_specyfikacji SET podgrupa = ?1, rodzaj = ?2, material = ?3, opis = ?4, ilosc = ?5, \
           jednostka = ?6, status = ?7, termin_potwierdzenia = ?8, kategoria_materialu_id = ?9, \
           pozycja_slownika_id = ?10, cena_gr = ?11, material_z_korpusu = ?12 WHERE id = ?13",
    )
    .bind(d.podgrupa.as_deref().unwrap_or("glowna"))
    .bind(&d.rodzaj)
    .bind(&d.material)
    .bind(&d.opis)
    .bind(d.ilosc)
    .bind(&d.jednostka)
    .bind(d.status.as_deref().unwrap_or("ustalone"))
    .bind(&d.termin_potwierdzenia)
    .bind(d.kategoria_materialu_id)
    .bind(d.pozycja_slownika_id)
    .bind(d.cena_gr)
    .bind(z_korpusu)
    .bind(id)
    .execute(&mut *tx)
    .await?;

    if rowne_bez_wielkosci(d.rodzaj.as_deref(), RODZAJ_KORPUS)
        || rowne_bez_wielkosci(stary.rodzaj.as_deref(), RODZAJ_KORPUS)
    {
        rozlej_material_korpusu(&mut tx, stary.czesc_id).await?;
    }
    dziennik(&mut tx, "wiersze_specyfikacji", id, "zmiana").await?;
    let w = wynik(&mut tx, id, spec_id).await?;
    tx.commit().await?;
    Ok(w)
}

pub async fn usun_wiersz(pula: &SqlitePool, id: i64) -> Wynik<WynikZmiany> {
    let mut tx = pula.begin().await?;
    let stary = pobierz_wiersz(&mut tx, id).await?;
    let (_, spec_id) = rodzic_czesci(&mut tx, stary.czesc_id).await?;
    sprawdz_zapis(&mut tx, spec_id).await?;
    sqlx::query("DELETE FROM wiersze_specyfikacji WHERE id = ?1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    dziennik(&mut tx, "wiersze_specyfikacji", id, "usuniecie").await?;
    let w = wynik(&mut tx, stary.czesc_id, spec_id).await?;
    tx.commit().await?;
    Ok(w)
}

/// Kolejność wierszy w części; wiersze z innych części tej samej specyfikacji są przenoszone.
pub async fn kolejnosc_wierszy(
    pula: &SqlitePool,
    czesc_id: i64,
    k: Kolejnosc,
) -> Wynik<WynikZmiany> {
    let mut tx = pula.begin().await?;
    let (_, spec_id) = rodzic_czesci(&mut tx, czesc_id).await?;
    sprawdz_zapis(&mut tx, spec_id).await?;
    for (i, id) in k.ids.iter().enumerate() {
        let w = pobierz_wiersz(&mut tx, *id).await?;
        let (_, s) = rodzic_czesci(&mut tx, w.czesc_id).await?;
        if s != spec_id {
            return Err(BladApi::NieprawidloweDane(format!(
                "wiersz {id} należy do innej specyfikacji"
            )));
        }
        sqlx::query("UPDATE wiersze_specyfikacji SET czesc_id = ?1, kolejnosc = ?2 WHERE id = ?3")
            .bind(czesc_id)
            .bind(i as i64)
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    let w = wynik(&mut tx, czesc_id, spec_id).await?;
    tx.commit().await?;
    Ok(w)
}

/// „Kopiuj materiał w dół”: materiał (i kategoria) wiersza trafia do kolejnych wierszy tej samej
/// części i podgrupy, które nie mają materiału albo mają materiał wypełniony automatycznie.
pub async fn kopiuj_material_w_dol(pula: &SqlitePool, id: i64) -> Wynik<WynikZmiany> {
    let mut tx = pula.begin().await?;
    let w = pobierz_wiersz(&mut tx, id).await?;
    let (_, spec_id) = rodzic_czesci(&mut tx, w.czesc_id).await?;
    sprawdz_zapis(&mut tx, spec_id).await?;
    if w.material.is_none() {
        return Err(BladApi::NieprawidloweDane(
            "wiersz nie ma materiału do skopiowania".into(),
        ));
    }
    sqlx::query(
        "UPDATE wiersze_specyfikacji SET material = ?1, kategoria_materialu_id = ?2, material_z_korpusu = 1 \
         WHERE czesc_id = ?3 AND podgrupa = ?4 AND id != ?5 \
           AND (kolejnosc > ?6 OR (kolejnosc = ?6 AND id > ?5)) \
           AND (material IS NULL OR material_z_korpusu = 1)",
    )
    .bind(&w.material)
    .bind(w.kategoria_materialu_id)
    .bind(w.czesc_id)
    .bind(&w.podgrupa)
    .bind(id)
    .bind(w.kolejnosc)
    .execute(&mut *tx)
    .await?;
    dziennik(&mut tx, "wiersze_specyfikacji", id, "zmiana").await?;
    let wy = wynik(&mut tx, id, spec_id).await?;
    tx.commit().await?;
    Ok(wy)
}

// ---------------------------------------------------------------------------
// Szablony: wstawianie części z wierszami
// ---------------------------------------------------------------------------

/// Wiersz szablonu (format wewnętrzny, patrz migracja 0002 i `szablony.rs`).
#[derive(Debug, Clone, Deserialize, Serialize, Default, PartialEq)]
#[serde(default)]
pub struct WierszSzablonu {
    pub podgrupa: Option<String>,
    pub rodzaj: Option<String>,
    pub material: Option<String>,
    pub opis: Option<String>,
    pub kategoria: Option<String>,
    pub udzial: Option<f64>,
}

/// Wstawia część o podanej nazwie z wierszami szablonu (w transakcji wywołującego).
pub async fn wstaw_czesc_z_wierszami(
    db: &mut SqliteConnection,
    sekcja_id: i64,
    nazwa: &str,
    wiersze: &[WierszSzablonu],
    po_czesci_id: Option<i64>,
) -> Wynik<i64> {
    let czesc = wstaw_czesc(db, sekcja_id, nazwa, None, po_czesci_id).await?;
    // Wiersze zależne (półki, cokół, plecy) dostaną materiał korpusu po wstawieniu wszystkich.
    for w in wiersze {
        let kategoria: Option<i64> =
            match &w.kategoria {
                Some(k) if !k.trim().is_empty() => sqlx::query_scalar(
                    "SELECT id FROM kategorie_materialow WHERE lower(nazwa) = lower(?1) LIMIT 1",
                )
                .bind(k.trim())
                .fetch_optional(&mut *db)
                .await?,
                _ => None,
            };
        let d = DaneWiersza {
            podgrupa: w.podgrupa.clone(),
            rodzaj: w.rodzaj.clone(),
            material: w.material.clone(),
            opis: w.opis.clone(),
            kategoria_materialu_id: kategoria,
            ..Default::default()
        }
        .oczysc()?;
        wstaw_wiersz(db, czesc, d).await?;
    }
    rozlej_material_korpusu(db, czesc).await?;
    Ok(czesc)
}

pub async fn dodaj_sekcje_w(
    db: &mut SqliteConnection,
    spec_id: i64,
    nazwa: &str,
    typ: &str,
) -> Wynik<i64> {
    let d = DaneSekcji {
        nazwa: nazwa.into(),
        typ: Some(typ.into()),
        ..Default::default()
    }
    .oczysc()?;
    let id = wstaw_sekcje(db, spec_id, &d, None).await?;
    przenumeruj(db, spec_id).await?;
    Ok(id)
}

pub async fn spec_sekcji_pub(db: &mut SqliteConnection, sekcja_id: i64) -> Wynik<i64> {
    spec_sekcji(db, sekcja_id).await
}

pub async fn pobierz_w_wynik(
    db: &mut SqliteConnection,
    id: i64,
    spec_id: i64,
) -> Wynik<WynikZmiany> {
    wynik(db, id, spec_id).await
}

// ---------------------------------------------------------------------------
// Kopia całej specyfikacji (nowa wersja oferty, oferta → umowa, aneks)
// ---------------------------------------------------------------------------

/// Głęboka kopia specyfikacji z zachowaniem `ident` (do porównań) i `zrodlo_id`.
pub async fn kopiuj_specyfikacje_w(db: &mut SqliteConnection, zrodlo: i64) -> Wynik<i64> {
    let nowa = sqlx::query("INSERT INTO specyfikacje (zrodlo_id) VALUES (?1)")
        .bind(zrodlo)
        .execute(&mut *db)
        .await?
        .last_insert_rowid();
    let sekcje: Vec<i64> = sqlx::query_scalar(
        "SELECT id FROM sekcje_specyfikacji WHERE specyfikacja_id = ?1 ORDER BY kolejnosc, id",
    )
    .bind(zrodlo)
    .fetch_all(&mut *db)
    .await?;
    for s in sekcje {
        kopiuj_sekcje_w(db, s, nowa, true, false).await?;
    }
    sqlx::query(
        "INSERT INTO zdjecia_specyfikacji (specyfikacja_id, sekcja_id, czesc_id, plik_id, podpis, kolejnosc) \
         SELECT ?1, NULL, NULL, plik_id, podpis, kolejnosc FROM zdjecia_specyfikacji \
         WHERE specyfikacja_id = ?2 AND sekcja_id IS NULL AND czesc_id IS NULL",
    )
    .bind(nowa)
    .bind(zrodlo)
    .execute(&mut *db)
    .await?;
    Ok(nowa)
}

/// Usuwa specyfikację z całą zawartością (wywołujący odpowiada za usunięcie odwołań).
pub async fn usun_specyfikacje_w(db: &mut SqliteConnection, id: i64) -> Wynik<()> {
    sqlx::query("DELETE FROM zdjecia_specyfikacji WHERE specyfikacja_id = ?1")
        .bind(id)
        .execute(&mut *db)
        .await?;
    sqlx::query("UPDATE specyfikacje SET zrodlo_id = NULL WHERE zrodlo_id = ?1")
        .bind(id)
        .execute(&mut *db)
        .await?;
    sqlx::query("DELETE FROM specyfikacje WHERE id = ?1")
        .bind(id)
        .execute(&mut *db)
        .await?;
    Ok(())
}

#[cfg(test)]
mod testy {
    use super::*;
    use crate::baza::otworz_w_pamieci;

    async fn nowa_spec(pula: &SqlitePool) -> i64 {
        let mut db = pula.acquire().await.unwrap();
        utworz_w(&mut db).await.unwrap()
    }

    fn wiersz(rodzaj: &str, material: Option<&str>) -> DaneWiersza {
        DaneWiersza {
            rodzaj: Some(rodzaj.into()),
            material: material.map(Into::into),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn drzewo_numeracja_i_kolejnosc() {
        let pula = otworz_w_pamieci().await.unwrap();
        let spec = nowa_spec(&pula).await;
        let a = dodaj_sekcje(
            &pula,
            spec,
            DaneSekcji {
                nazwa: "Zabudowa Kuchni".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap()
        .id;
        let agd = dodaj_sekcje(
            &pula,
            spec,
            DaneSekcji {
                nazwa: "Sprzęt AGD".into(),
                typ: Some("agd".into()),
                stawka_vat: Some(23),
                ..Default::default()
            },
        )
        .await
        .unwrap()
        .id;
        let b = dodaj_sekcje(
            &pula,
            spec,
            DaneSekcji {
                nazwa: "Szafa".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let numery: Vec<_> = b
            .specyfikacja
            .sekcje
            .iter()
            .map(|s| s.numer.clone())
            .collect();
        assert_eq!(numery, vec![Some("01".into()), None, Some("02".into())]);

        let w = kolejnosc_sekcji(
            &pula,
            spec,
            Kolejnosc {
                ids: vec![b.id, a, agd],
            },
        )
        .await
        .unwrap();
        assert_eq!(w.specyfikacja.sekcje[0].nazwa, "Szafa");
        assert_eq!(w.specyfikacja.sekcje[0].numer.as_deref(), Some("01"));

        // Wstawianie wiersza za wskazanym (Enter w tabeli)
        let c = dodaj_czesc(
            &pula,
            a,
            DaneCzesci {
                nazwa: "Słupki".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap()
        .id;
        let w1 = dodaj_wiersz(&pula, c, wiersz("Fronty", Some("MDF")))
            .await
            .unwrap()
            .id;
        let _w2 = dodaj_wiersz(&pula, c, wiersz("Blat", None))
            .await
            .unwrap()
            .id;
        let mut srodek = wiersz("Uchwyty", None);
        srodek.po_wierszu_id = Some(w1);
        let r = dodaj_wiersz(&pula, c, srodek).await.unwrap();
        let rodzaje: Vec<_> = r.specyfikacja.sekcje[1].czesci[0]
            .wiersze
            .iter()
            .map(|w| w.rodzaj.clone().unwrap())
            .collect();
        assert_eq!(rodzaje, vec!["Fronty", "Uchwyty", "Blat"]);

        let zly = DaneWiersza {
            status: Some("może".into()),
            ..Default::default()
        };
        assert!(matches!(
            dodaj_wiersz(&pula, c, zly).await,
            Err(BladApi::NieprawidloweDane(_))
        ));
    }

    #[tokio::test]
    async fn material_korpusu_splywa_dopoki_nie_zmieniony() {
        let pula = otworz_w_pamieci().await.unwrap();
        let spec = nowa_spec(&pula).await;
        let s = dodaj_sekcje(
            &pula,
            spec,
            DaneSekcji {
                nazwa: "K".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap()
        .id;
        let c = dodaj_czesc(
            &pula,
            s,
            DaneCzesci {
                nazwa: "Szafki".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap()
        .id;
        let korpus = dodaj_wiersz(&pula, c, wiersz("Korpus", Some("Płyta Testowa Biała")))
            .await
            .unwrap()
            .id;
        let polki = dodaj_wiersz(&pula, c, wiersz("Półki", None)).await.unwrap();
        let p = polki.specyfikacja.sekcje[0].czesci[0].wiersze[1].clone();
        assert_eq!(p.material.as_deref(), Some("Płyta Testowa Biała"));
        assert!(p.material_z_korpusu);
        let cokol = dodaj_wiersz(&pula, c, wiersz("Cokół", None))
            .await
            .unwrap()
            .id;

        // Ręczna zmiana cokołu zrywa dziedziczenie
        let r = zmien_wiersz(&pula, cokol, wiersz("Cokół", Some("Aluminium")))
            .await
            .unwrap();
        assert!(!r.specyfikacja.sekcje[0].czesci[0].wiersze[2].material_z_korpusu);

        // Zmiana korpusu: półki podążają, cokół nie
        let r = zmien_wiersz(&pula, korpus, wiersz("Korpus", Some("Płyta Testowa Szara")))
            .await
            .unwrap();
        let ws = &r.specyfikacja.sekcje[0].czesci[0].wiersze;
        assert_eq!(ws[1].material.as_deref(), Some("Płyta Testowa Szara"));
        assert_eq!(ws[2].material.as_deref(), Some("Aluminium"));

        // Zapis półek z tym samym materiałem (np. zmiana opisu) nie zrywa dziedziczenia
        let mut d = wiersz("Półki", Some("Płyta Testowa Szara"));
        d.opis = Some("2 szt.".into());
        let r = zmien_wiersz(&pula, p.id, d).await.unwrap();
        assert!(r.specyfikacja.sekcje[0].czesci[0].wiersze[1].material_z_korpusu);

        // Kopiuj w dół z korpusu: cokół zmieniony ręcznie zostaje
        let blat = dodaj_wiersz(&pula, c, wiersz("Blenda", None))
            .await
            .unwrap()
            .id;
        let r = kopiuj_material_w_dol(&pula, korpus).await.unwrap();
        let ws = &r.specyfikacja.sekcje[0].czesci[0].wiersze;
        assert_eq!(
            ws.iter()
                .find(|w| w.id == blat)
                .unwrap()
                .material
                .as_deref(),
            Some("Płyta Testowa Szara")
        );
        assert_eq!(ws[2].material.as_deref(), Some("Aluminium"));
    }

    #[tokio::test]
    async fn kopiowanie_czesci_sekcji_i_specyfikacji() {
        let pula = otworz_w_pamieci().await.unwrap();
        let spec = nowa_spec(&pula).await;
        let s = dodaj_sekcje(
            &pula,
            spec,
            DaneSekcji {
                nazwa: "Kuchnia".into(),
                cena_gr: Some(100),
                ..Default::default()
            },
        )
        .await
        .unwrap()
        .id;
        let c = dodaj_czesc(
            &pula,
            s,
            DaneCzesci {
                nazwa: "Słupki".into(),
                cena_gr: Some(50),
                ..Default::default()
            },
        )
        .await
        .unwrap()
        .id;
        dodaj_czesc(
            &pula,
            s,
            DaneCzesci {
                nazwa: "Wiszące".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        dodaj_wiersz(&pula, c, wiersz("Korpus", Some("X")))
            .await
            .unwrap();
        dodaj_wiersz(&pula, c, wiersz("Półki", None)).await.unwrap();

        let r = kopiuj_czesc(&pula, c, DaneKopii::default()).await.unwrap();
        let cz = &r.specyfikacja.sekcje[0].czesci;
        assert_eq!(cz.len(), 3);
        assert_eq!(cz[1].nazwa, "Słupki (kopia)");
        assert_eq!(cz[1].wiersze.len(), 2);
        assert_ne!(cz[1].ident, cz[0].ident);
        assert_ne!(cz[1].wiersze[0].ident, cz[0].wiersze[0].ident);

        let r = kopiuj_sekcje(&pula, s, DaneKopii::default()).await.unwrap();
        assert_eq!(r.specyfikacja.sekcje.len(), 2);
        assert_eq!(r.specyfikacja.sekcje[1].numer.as_deref(), Some("02"));
        assert_eq!(r.specyfikacja.sekcje[1].czesci.len(), 3);

        let mut db = pula.acquire().await.unwrap();
        let kopia = kopiuj_specyfikacje_w(&mut db, spec).await.unwrap();
        let a = pobierz_w(&mut db, spec).await.unwrap();
        let b = pobierz_w(&mut db, kopia).await.unwrap();
        assert_eq!(b.zrodlo_id, Some(spec));
        assert_eq!(a.sekcje.len(), b.sekcje.len());
        assert_eq!(a.sekcje[0].ident, b.sekcje[0].ident);
        assert_eq!(
            a.sekcje[0].czesci[0].wiersze[1].ident,
            b.sekcje[0].czesci[0].wiersze[1].ident
        );
        assert_eq!(
            a.sekcje[0].czesci[0].wiersze[1].material,
            b.sekcje[0].czesci[0].wiersze[1].material
        );
    }

    #[tokio::test]
    async fn przenoszenie_czesci_miedzy_sekcjami() {
        let pula = otworz_w_pamieci().await.unwrap();
        let spec = nowa_spec(&pula).await;
        let s1 = dodaj_sekcje(
            &pula,
            spec,
            DaneSekcji {
                nazwa: "A".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap()
        .id;
        let s2 = dodaj_sekcje(
            &pula,
            spec,
            DaneSekcji {
                nazwa: "B".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap()
        .id;
        let c1 = dodaj_czesc(
            &pula,
            s1,
            DaneCzesci {
                nazwa: "c1".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap()
        .id;
        let c2 = dodaj_czesc(
            &pula,
            s2,
            DaneCzesci {
                nazwa: "c2".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap()
        .id;
        let r = kolejnosc_czesci(&pula, s2, Kolejnosc { ids: vec![c1, c2] })
            .await
            .unwrap();
        assert!(r.specyfikacja.sekcje[0].czesci.is_empty());
        assert_eq!(r.specyfikacja.sekcje[1].czesci[0].nazwa, "c1");

        let inna = nowa_spec(&pula).await;
        let s3 = dodaj_sekcje(
            &pula,
            inna,
            DaneSekcji {
                nazwa: "C".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap()
        .id;
        assert!(kolejnosc_czesci(&pula, s3, Kolejnosc { ids: vec![c1] })
            .await
            .is_err());
    }
}
