//! Generator Umów: umowy z oferty albo od zera, numeracja, skrócony formularz (zmiany
//! częściowe z wykrywaniem konfliktów), statusy z zamrożeniem danych, plan płatności A/B,
//! harmonogram etapów oraz powiązanie z kalendarzem (zlecenie, materiały, harmonogram).
//!
//! Specyfikacja umowy to osobny obiekt (`specyfikacje`), edytowany wspólnym edytorem –
//! właściciel „umowa” blokuje zapis poza szkicem (`specyfikacje::wlasciciel`).

use std::collections::{BTreeMap, BTreeSet};

use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{FromRow, SqliteConnection, SqlitePool};

use crate::bledy::{BladApi, Wynik};
use crate::limity::{self, MAKS_DLUGI, MAKS_NAZWA};
use crate::oferty::{DaneInwestora, DaneWykonawcy};
use crate::platnosci::{self, KontrolaEtapow, PlanPlatnosci, UstawieniaPlatnosci, WejsciePlanu};
use crate::roznice::Roznica;
use crate::specyfikacje::{self, Specyfikacja};
use crate::zmiany::ZmianaCzesciowa;

pub const STATUSY_UMOWY: &[&str] = &[
    "szkic",
    "wyslana",
    "podpisana",
    "w_realizacji",
    "odebrana",
    "zamknieta",
    "anulowana",
];
pub const ZALACZNIKI: &[&str] = &["1", "2", "3", "4", "5", "6"];
/// Załączniki dołączane zawsze: specyfikacja, standard wykonania, protokół odbioru.
pub const ZALACZNIKI_STALE: &[&str] = &["1", "4", "5"];
pub const RODZAJE_NIERUCHOMOSCI: &[&str] = &["lokal_mieszkalny", "dom_jednorodzinny", "inny"];

pub fn nazwa_statusu(s: &str) -> &'static str {
    match s {
        "szkic" => "szkic",
        "wyslana" => "wysłana do klienta",
        "podpisana" => "podpisana",
        "w_realizacji" => "w realizacji",
        "odebrana" => "odebrana",
        "zamknieta" => "zamknięta",
        "anulowana" => "anulowana",
        _ => "nieznany",
    }
}

pub fn nazwa_rodzaju_nieruchomosci(s: &str) -> &'static str {
    match s {
        "lokal_mieszkalny" => "lokal mieszkalny",
        "dom_jednorodzinny" => "budynek mieszkalny jednorodzinny",
        "inny" => "inny",
        _ => "",
    }
}

// ---------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------

/// Pola skróconego formularza umowy (wszystkie edytowalne w szkicu). `PUT /umowy/{id}`
/// przyjmuje dowolny ich podzbiór (+ `poprzednie`).
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct DaneUmowy {
    pub numer: Option<String>,
    pub miejscowosc: Option<String>,
    pub data_zawarcia: Option<String>,
    pub klient_id: Option<i64>,
    pub wartosc_brutto_gr: Option<i64>,
    pub stawka_vat: Option<i64>,
    pub zadatek_gr: Option<i64>,
    pub termin_zadatku: Option<String>,
    pub wariant_platnosci: Option<String>,
    pub kwota_przed_montazem_gr: Option<i64>,
    pub montaz_do: Option<String>,
    pub dni_na_brakujace_informacje: Option<i64>,
    pub vat_rodzaj_nieruchomosci: Option<String>,
    pub vat_powierzchnia_m2: Option<f64>,
    pub vat_podstawa_powierzchni: Option<String>,
    pub zalaczniki: Option<Vec<String>>,
    pub czas_warsztat_dni: Option<i64>,
    pub czas_montazu_dni: Option<i64>,
    pub uwagi: Option<String>,
    // Miejsce montażu (zapisywane w inwestycji klienta)
    pub montaz_ulica: Option<String>,
    pub montaz_kod: Option<String>,
    pub montaz_miasto: Option<String>,
    pub montaz_pietro: Option<String>,
    pub montaz_kod_domofonu: Option<String>,
    pub montaz_parking: Option<String>,
    pub montaz_winda_wniesienie: Option<String>,
}

const POLA_UMOWY: &[&str] = &[
    "numer",
    "miejscowosc",
    "data_zawarcia",
    "klient_id",
    "wartosc_brutto_gr",
    "stawka_vat",
    "zadatek_gr",
    "termin_zadatku",
    "wariant_platnosci",
    "kwota_przed_montazem_gr",
    "montaz_do",
    "dni_na_brakujace_informacje",
    "vat_rodzaj_nieruchomosci",
    "vat_powierzchnia_m2",
    "vat_podstawa_powierzchni",
    "zalaczniki",
    "czas_warsztat_dni",
    "czas_montazu_dni",
    "uwagi",
    "montaz_ulica",
    "montaz_kod",
    "montaz_miasto",
    "montaz_pietro",
    "montaz_kod_domofonu",
    "montaz_parking",
    "montaz_winda_wniesienie",
];
/// Pola wewnętrzne, które można zmieniać także po wyjściu ze szkicu (nie trafiają do dokumentu).
const POLA_WEWNETRZNE: &[&str] = &["czas_warsztat_dni", "czas_montazu_dni", "uwagi"];

pub fn nazwa_pola(p: &str) -> String {
    match p {
        "numer" => "numer umowy",
        "miejscowosc" => "miejscowość",
        "data_zawarcia" => "data umowy",
        "klient_id" => "klient",
        "wartosc_brutto_gr" => "wartość brutto",
        "stawka_vat" => "stawka VAT",
        "zadatek_gr" => "zadatek",
        "termin_zadatku" => "termin zadatku",
        "wariant_platnosci" => "wariant płatności",
        "kwota_przed_montazem_gr" => "kwota przed montażem",
        "montaz_do" => "termin rozpoczęcia montażu",
        "dni_na_brakujace_informacje" => "dni na brakujące informacje",
        "vat_rodzaj_nieruchomosci" => "rodzaj nieruchomości",
        "vat_powierzchnia_m2" => "powierzchnia użytkowa",
        "vat_podstawa_powierzchni" => "podstawa określenia powierzchni",
        "zalaczniki" => "załączniki",
        "czas_warsztat_dni" => "czas warsztatu",
        "czas_montazu_dni" => "czas montażu",
        "uwagi" => "uwagi",
        "montaz_ulica" => "adres montażu",
        "montaz_kod" => "kod pocztowy montażu",
        "montaz_miasto" => "miejscowość montażu",
        "montaz_pietro" => "piętro",
        "montaz_kod_domofonu" => "kod domofonu",
        "montaz_parking" => "parking",
        "montaz_winda_wniesienie" => "winda / wniesienie",
        inne => inne,
    }
    .to_string()
}

fn oczysc_tekst(t: Option<String>) -> Option<String> {
    t.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

pub fn sprawdz_date(nazwa: &str, d: &Option<String>) -> Wynik<Option<NaiveDate>> {
    match d {
        None => Ok(None),
        Some(t) => {
            let d = NaiveDate::parse_from_str(t, "%Y-%m-%d").map_err(|_| {
                BladApi::NieprawidloweDane(format!("{nazwa}: nieprawidłowa data „{t}” (RRRR-MM-DD)"))
            })?;
            if !(1990..=2100).contains(&d.year()) {
                return Err(BladApi::NieprawidloweDane(format!(
                    "{nazwa}: rok spoza zakresu 1990–2100"
                )));
            }
            Ok(Some(d))
        }
    }
}

fn dni_opc(nazwa: &str, v: Option<i64>, maks: i64) -> Wynik<()> {
    match v {
        Some(x) if !(0..=maks).contains(&x) => Err(BladApi::NieprawidloweDane(format!(
            "{nazwa}: od 0 do {maks} dni"
        ))),
        _ => Ok(()),
    }
}

impl DaneUmowy {
    pub fn oczysc(mut self) -> Wynik<Self> {
        for (n, t, m) in [
            ("numer umowy", &mut self.numer, 50),
            ("miejscowość", &mut self.miejscowosc, MAKS_NAZWA),
            ("wariant płatności", &mut self.wariant_platnosci, 1),
            ("rodzaj nieruchomości", &mut self.vat_rodzaj_nieruchomosci, 40),
            ("podstawa powierzchni", &mut self.vat_podstawa_powierzchni, 1000),
            ("uwagi", &mut self.uwagi, MAKS_DLUGI),
            ("adres montażu", &mut self.montaz_ulica, MAKS_NAZWA),
            ("kod pocztowy", &mut self.montaz_kod, 20),
            ("miejscowość montażu", &mut self.montaz_miasto, MAKS_NAZWA),
            ("piętro", &mut self.montaz_pietro, 100),
            ("kod domofonu", &mut self.montaz_kod_domofonu, 100),
            ("parking", &mut self.montaz_parking, 1000),
            ("winda / wniesienie", &mut self.montaz_winda_wniesienie, 1000),
        ] {
            *t = oczysc_tekst(t.take());
            limity::tekst(n, t.as_deref(), m)?;
        }
        if let Some(w) = &mut self.wariant_platnosci {
            *w = w.to_uppercase();
            if w != "A" && w != "B" {
                return Err(BladApi::NieprawidloweDane(
                    "wariant płatności: A albo B".into(),
                ));
            }
        }
        sprawdz_date("data umowy", &self.data_zawarcia)?;
        sprawdz_date("termin zadatku", &self.termin_zadatku)?;
        sprawdz_date("termin rozpoczęcia montażu", &self.montaz_do)?;
        limity::kwota("wartość umowy", self.wartosc_brutto_gr)?;
        limity::kwota("zadatek", self.zadatek_gr)?;
        limity::kwota("kwota przed montażem", self.kwota_przed_montazem_gr)?;
        if let Some(s) = self.stawka_vat {
            if s != 8 && s != 23 {
                return Err(BladApi::NieprawidloweDane(
                    "stawka VAT: 8 albo 23".into(),
                ));
            }
        }
        if let Some(r) = &self.vat_rodzaj_nieruchomosci {
            if !RODZAJE_NIERUCHOMOSCI.contains(&r.as_str()) {
                return Err(BladApi::NieprawidloweDane(format!(
                    "rodzaj nieruchomości: {}",
                    RODZAJE_NIERUCHOMOSCI.join(", ")
                )));
            }
        }
        if let Some(p) = self.vat_powierzchnia_m2 {
            if !p.is_finite() || !(0.0..=100_000.0).contains(&p) {
                return Err(BladApi::NieprawidloweDane(
                    "powierzchnia użytkowa: od 0 do 100 000 m²".into(),
                ));
            }
        }
        dni_opc("dni na brakujące informacje", self.dni_na_brakujace_informacje, 365)?;
        dni_opc("czas warsztatu", self.czas_warsztat_dni, 365)?;
        dni_opc("czas montażu", self.czas_montazu_dni, 365)?;
        if self.czas_montazu_dni == Some(0) {
            return Err(BladApi::NieprawidloweDane(
                "czas montażu: co najmniej 1 dzień".into(),
            ));
        }
        let mut z: BTreeSet<String> = self
            .zalaczniki
            .take()
            .unwrap_or_default()
            .into_iter()
            .map(|s| s.trim().to_string())
            .collect();
        for s in &z {
            if !ZALACZNIKI.contains(&s.as_str()) {
                return Err(BladApi::NieprawidloweDane(format!(
                    "nieznany załącznik „{s}” (1–6)"
                )));
            }
        }
        for s in ZALACZNIKI_STALE {
            z.insert((*s).to_string());
        }
        self.zalaczniki = Some(z.into_iter().collect());
        Ok(self)
    }
}

/// Miejsce montażu i dane stron zamrożone przy wyjściu umowy ze szkicu.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default)]
pub struct MigawkaUmowy {
    pub zamawiajacy: DaneInwestora,
    pub osoby_kontaktowe: Vec<OsobaKontaktowa>,
    pub wykonawca: DaneWykonawcy,
    pub wykonawca_bank: String,
    pub wykonawca_rachunek: String,
    pub wykonawca_swift: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, FromRow)]
#[serde(default)]
pub struct OsobaKontaktowa {
    #[serde(skip_deserializing)]
    pub id: i64,
    pub imie_nazwisko: String,
    pub rola: Option<String>,
    pub email: Option<String>,
    pub telefon: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Umowa {
    pub id: i64,
    pub rok: i64,
    pub nr_kolejny: i64,
    /// Status widoczny dla użytkownika (z „anulowana”).
    pub status: String,
    pub klient_nazwa: Option<String>,
    pub klient_usuniety: bool,
    pub inwestycja_id: Option<i64>,
    pub wersja_oferty_id: Option<i64>,
    pub oferta_id: Option<i64>,
    pub oferta_numer: Option<String>,
    pub nr_wersji_oferty: Option<i64>,
    pub specyfikacja_id: i64,
    pub specyfikacja_pierwotna_id: Option<i64>,
    pub wersja: i64,
    pub utworzono: String,
    pub zmieniono: String,
    pub wyslano: Option<String>,
    pub data_podpisania: Option<String>,
    pub anulowano: Option<String>,
    pub anulowanie_powod: Option<String>,
    pub migawka: Option<MigawkaUmowy>,
    pub tylko_do_odczytu: bool,
    pub powod_blokady: Option<String>,
    #[serde(flatten)]
    pub dane: DaneUmowy,
}

#[derive(FromRow)]
struct UmowaBaza {
    id: i64,
    rok: i64,
    nr_kolejny: i64,
    numer: String,
    klient_id: i64,
    klient_nazwa: Option<String>,
    klient_usuniety: bool,
    inwestycja_id: Option<i64>,
    wersja_oferty_id: Option<i64>,
    oferta_id: Option<i64>,
    oferta_numer: Option<String>,
    nr_wersji_oferty: Option<i64>,
    specyfikacja_id: i64,
    specyfikacja_pierwotna_id: Option<i64>,
    miejscowosc: Option<String>,
    data_zawarcia: Option<String>,
    status: String,
    anulowano: Option<String>,
    anulowanie_powod: Option<String>,
    wartosc_brutto_gr: Option<i64>,
    stawka_vat: Option<i64>,
    zadatek_gr: Option<i64>,
    termin_zadatku: Option<String>,
    wariant_platnosci: Option<String>,
    kwota_przed_montazem_gr: Option<i64>,
    montaz_do: Option<String>,
    dni_na_brakujace_informacje: Option<i64>,
    vat_rodzaj_nieruchomosci: Option<String>,
    vat_powierzchnia_m2: Option<f64>,
    vat_podstawa_powierzchni: Option<String>,
    zalaczniki: Option<String>,
    czas_warsztat_dni: Option<i64>,
    czas_montazu_dni: Option<i64>,
    uwagi: Option<String>,
    wersja: i64,
    utworzono: String,
    zmieniono: String,
    wyslano: Option<String>,
    data_podpisania: Option<String>,
    migawka: Option<String>,
    montaz_ulica: Option<String>,
    montaz_kod: Option<String>,
    montaz_miasto: Option<String>,
    montaz_pietro: Option<String>,
    montaz_kod_domofonu: Option<String>,
    montaz_parking: Option<String>,
    montaz_winda_wniesienie: Option<String>,
}

const SQL_UMOWY: &str = "SELECT u.id, u.rok, u.nr_kolejny, u.numer, u.klient_id, k.nazwa AS klient_nazwa, \
    (k.usunieto IS NOT NULL) AS klient_usuniety, u.inwestycja_id, u.wersja_oferty_id, w.oferta_id, o.numer AS oferta_numer, \
    w.nr_wersji AS nr_wersji_oferty, u.specyfikacja_id, u.specyfikacja_pierwotna_id, u.miejscowosc, u.data_zawarcia, \
    u.status, u.anulowano, u.anulowanie_powod, u.wartosc_brutto_gr, u.stawka_vat, u.zadatek_gr, u.termin_zadatku, \
    u.wariant_platnosci, u.kwota_przed_montazem_gr, u.montaz_do, u.dni_na_brakujace_informacje, \
    u.vat_rodzaj_nieruchomosci, u.vat_powierzchnia_m2, u.vat_podstawa_powierzchni, u.zalaczniki, \
    u.czas_warsztat_dni, u.czas_montazu_dni, u.uwagi, u.wersja, u.utworzono, u.zmieniono, u.wyslano, \
    u.data_podpisania, u.migawka, i.adres_ulica AS montaz_ulica, i.adres_kod AS montaz_kod, \
    i.adres_miasto AS montaz_miasto, i.pietro AS montaz_pietro, i.kod_domofonu AS montaz_kod_domofonu, \
    i.parking AS montaz_parking, i.winda_wniesienie AS montaz_winda_wniesienie \
    FROM umowy u LEFT JOIN klienci k ON k.id = u.klient_id LEFT JOIN wersje_oferty w ON w.id = u.wersja_oferty_id \
    LEFT JOIN oferty o ON o.id = w.oferta_id LEFT JOIN inwestycje i ON i.id = u.inwestycja_id";

impl From<UmowaBaza> for Umowa {
    fn from(b: UmowaBaza) -> Self {
        let status = if b.anulowano.is_some() {
            "anulowana".to_string()
        } else {
            b.status.clone()
        };
        let powod_blokady = if b.anulowano.is_some() {
            Some("umowa jest anulowana – tylko do odczytu".to_string())
        } else if b.status != "szkic" {
            Some(format!(
                "umowa ma status „{}” – dane i specyfikacja są zamrożone; zmiany przez aneks",
                nazwa_statusu(&b.status)
            ))
        } else {
            None
        };
        Umowa {
            id: b.id,
            rok: b.rok,
            nr_kolejny: b.nr_kolejny,
            status,
            klient_nazwa: b.klient_nazwa,
            klient_usuniety: b.klient_usuniety,
            inwestycja_id: b.inwestycja_id,
            wersja_oferty_id: b.wersja_oferty_id,
            oferta_id: b.oferta_id,
            oferta_numer: b.oferta_numer,
            nr_wersji_oferty: b.nr_wersji_oferty,
            specyfikacja_id: b.specyfikacja_id,
            specyfikacja_pierwotna_id: b.specyfikacja_pierwotna_id,
            wersja: b.wersja,
            utworzono: b.utworzono,
            zmieniono: b.zmieniono,
            wyslano: b.wyslano,
            data_podpisania: b.data_podpisania,
            anulowano: b.anulowano,
            anulowanie_powod: b.anulowanie_powod,
            migawka: b.migawka.and_then(|m| serde_json::from_str(&m).ok()),
            tylko_do_odczytu: powod_blokady.is_some(),
            powod_blokady,
            dane: DaneUmowy {
                numer: Some(b.numer),
                miejscowosc: b.miejscowosc,
                data_zawarcia: b.data_zawarcia,
                klient_id: Some(b.klient_id),
                wartosc_brutto_gr: b.wartosc_brutto_gr,
                stawka_vat: b.stawka_vat,
                zadatek_gr: b.zadatek_gr,
                termin_zadatku: b.termin_zadatku,
                wariant_platnosci: b.wariant_platnosci,
                kwota_przed_montazem_gr: b.kwota_przed_montazem_gr,
                montaz_do: b.montaz_do,
                dni_na_brakujace_informacje: b.dni_na_brakujace_informacje,
                vat_rodzaj_nieruchomosci: b.vat_rodzaj_nieruchomosci,
                vat_powierzchnia_m2: b.vat_powierzchnia_m2,
                vat_podstawa_powierzchni: b.vat_podstawa_powierzchni,
                zalaczniki: Some(
                    b.zalaczniki
                        .and_then(|z| serde_json::from_str(&z).ok())
                        .unwrap_or_else(|| {
                            ZALACZNIKI_STALE.iter().map(|s| s.to_string()).collect()
                        }),
                ),
                czas_warsztat_dni: b.czas_warsztat_dni,
                czas_montazu_dni: b.czas_montazu_dni,
                uwagi: b.uwagi,
                montaz_ulica: b.montaz_ulica,
                montaz_kod: b.montaz_kod,
                montaz_miasto: b.montaz_miasto,
                montaz_pietro: b.montaz_pietro,
                montaz_kod_domofonu: b.montaz_kod_domofonu,
                montaz_parking: b.montaz_parking,
                montaz_winda_wniesienie: b.montaz_winda_wniesienie,
            },
        }
    }
}

impl Umowa {
    pub fn numer(&self) -> &str {
        self.dane.numer.as_deref().unwrap_or("")
    }
    /// Status zapisany w bazie (bez „anulowana”).
    pub fn szkic(&self) -> bool {
        self.status == "szkic"
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, FromRow)]
pub struct PlatnoscPlanowana {
    pub id: i64,
    pub umowa_id: i64,
    pub rodzaj: String,
    pub opis: Option<String>,
    pub kwota_gr: i64,
    pub termin: Option<String>,
    pub kolejnosc: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, FromRow)]
pub struct Etap {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub lp: i64,
    pub nazwa: String,
    #[serde(default)]
    pub termin: Option<String>,
    #[serde(default)]
    pub kwota_gr: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, FromRow)]
pub struct Wplata {
    pub id: i64,
    pub umowa_id: i64,
    pub platnosc_planowana_id: Option<i64>,
    pub data: String,
    pub kwota_gr: i64,
    pub sposob: Option<String>,
    pub uwagi: Option<String>,
    pub utworzono: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Default)]
pub struct PozycjaSalda {
    pub opis: String,
    pub termin: Option<String>,
    pub kwota_gr: i64,
    /// Część jeszcze niezapłacona.
    pub do_zaplaty_gr: i64,
    pub zalegla: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Default)]
pub struct Saldo {
    pub wartosc_gr: i64,
    pub wplacono_gr: i64,
    /// Wartość − wpłaty (ujemne = nadpłata).
    pub pozostalo_gr: i64,
    /// Suma rat po terminie, niepokrytych wpłatami.
    pub zalegle_gr: i64,
    pub pozycje: Vec<PozycjaSalda>,
    /// Najbliższa niezapłacona rata (termin płatności do protokołu odbioru).
    pub nastepna: Option<PozycjaSalda>,
}

#[derive(Debug, Clone, Serialize, PartialEq, FromRow)]
pub struct Protokol {
    pub id: i64,
    pub umowa_id: i64,
    pub data: String,
    pub uwagi: Option<String>,
    pub bez_zastrzezen: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, FromRow)]
pub struct AneksKrotko {
    pub id: i64,
    pub nr: i64,
    pub data: Option<String>,
    pub status: String,
    pub zmiana_ceny_gr: i64,
    pub nowy_termin_montazu: Option<String>,
    pub opis_zmian: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, FromRow)]
pub struct ZlecenieUmowy {
    pub id: i64,
    pub etap: String,
    pub montaz_od: Option<String>,
    pub harmonogram_nieaktualny_powod: Option<String>,
    pub zadania: i64,
    pub materialy: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct PelnaUmowa {
    pub umowa: Umowa,
    pub specyfikacja: Specyfikacja,
    /// Plan płatności wyliczony z bieżących danych (podgląd w szkicu).
    pub plan: PlanPlatnosci,
    /// Płatności zapisane (od wyjścia ze szkicu) – z nich silnik terminów tworzy zadania.
    pub platnosci: Vec<PlatnoscPlanowana>,
    pub wplaty: Vec<Wplata>,
    pub saldo: Saldo,
    pub etapy: Vec<Etap>,
    pub kontrola_etapow: KontrolaEtapow,
    pub aneksy: Vec<AneksKrotko>,
    pub protokol: Option<Protokol>,
    pub zlecenie: Option<ZlecenieUmowy>,
    /// Ostrzeżenia formularza (VAT, brakujące dane).
    pub ostrzezenia: Vec<String>,
    /// Czego brakuje do wyjścia ze szkicu.
    pub braki: Vec<String>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct UmowaNaLiscie {
    pub id: i64,
    pub numer: String,
    pub rok: i64,
    pub klient_id: i64,
    pub klient_nazwa: Option<String>,
    pub status: String,
    pub wartosc_brutto_gr: Option<i64>,
    pub wplacono_gr: i64,
    pub saldo_gr: i64,
    pub montaz_do: Option<String>,
    pub data_zawarcia: Option<String>,
    pub oferta_numer: Option<String>,
    pub zmieniono: String,
}

// ---------------------------------------------------------------------------
// Ustawienia umów
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct UstawieniaUmow {
    pub format_numeru: String,
    pub miejscowosc: String,
    pub dni_na_informacje: i64,
    pub termin_zadatku_dni: i64,
    pub zadatek_procent: i64,
    pub wariant_a_dni_przed_montazem: i64,
    pub wariant_b_dni_przed_montazem: i64,
    pub wariant_b_dni_po_odbiorze: i64,
    pub czas_warsztat_dni: i64,
    pub czas_montazu_dni: i64,
}

impl Default for UstawieniaUmow {
    fn default() -> Self {
        Self {
            format_numeru: "{NN}/{RRRR}".into(),
            miejscowosc: String::new(),
            dni_na_informacje: 14,
            termin_zadatku_dni: 3,
            zadatek_procent: 30,
            wariant_a_dni_przed_montazem: 3,
            wariant_b_dni_przed_montazem: 3,
            wariant_b_dni_po_odbiorze: 3,
            czas_warsztat_dni: 10,
            czas_montazu_dni: 2,
        }
    }
}

impl UstawieniaUmow {
    pub fn platnosci(&self) -> UstawieniaPlatnosci {
        let u = |v: i64| v.clamp(0, 365) as u32;
        UstawieniaPlatnosci {
            termin_zadatku_dni: u(self.termin_zadatku_dni),
            a_dni_przed_montazem: u(self.wariant_a_dni_przed_montazem),
            b_dni_przed_montazem: u(self.wariant_b_dni_przed_montazem),
            b_dni_po_odbiorze: u(self.wariant_b_dni_po_odbiorze),
        }
    }
}

const KLUCZE_USTAWIEN: &[&str] = &[
    "umowa_format_numeru",
    "umowa_miejscowosc",
    "umowa_dni_na_informacje",
    "umowa_termin_zadatku_dni",
    "umowa_zadatek_procent",
    "umowa_wariant_a_dni_przed_montazem",
    "umowa_wariant_b_dni_przed_montazem",
    "umowa_wariant_b_dni_po_odbiorze",
    "umowa_czas_warsztat_dni",
    "umowa_czas_montazu_dni",
];

pub async fn ustawienia_w(db: &mut SqliteConnection) -> Wynik<UstawieniaUmow> {
    let wiersze: Vec<(String, String)> =
        sqlx::query_as("SELECT klucz, wartosc FROM ustawienia WHERE klucz LIKE 'umowa_%'")
            .fetch_all(&mut *db)
            .await?;
    let mut u = UstawieniaUmow::default();
    for (k, v) in wiersze {
        let n = |d: i64| v.trim().parse::<i64>().unwrap_or(d);
        match k.as_str() {
            "umowa_format_numeru" if !v.trim().is_empty() => u.format_numeru = v.clone(),
            "umowa_miejscowosc" => u.miejscowosc = v.clone(),
            "umowa_dni_na_informacje" => u.dni_na_informacje = n(u.dni_na_informacje),
            "umowa_termin_zadatku_dni" => u.termin_zadatku_dni = n(u.termin_zadatku_dni),
            "umowa_zadatek_procent" => u.zadatek_procent = n(u.zadatek_procent),
            "umowa_wariant_a_dni_przed_montazem" => {
                u.wariant_a_dni_przed_montazem = n(u.wariant_a_dni_przed_montazem)
            }
            "umowa_wariant_b_dni_przed_montazem" => {
                u.wariant_b_dni_przed_montazem = n(u.wariant_b_dni_przed_montazem)
            }
            "umowa_wariant_b_dni_po_odbiorze" => {
                u.wariant_b_dni_po_odbiorze = n(u.wariant_b_dni_po_odbiorze)
            }
            "umowa_czas_warsztat_dni" => u.czas_warsztat_dni = n(u.czas_warsztat_dni),
            "umowa_czas_montazu_dni" => u.czas_montazu_dni = n(u.czas_montazu_dni),
            _ => {}
        }
    }
    Ok(u)
}

pub async fn ustawienia(pula: &SqlitePool) -> Wynik<UstawieniaUmow> {
    let mut db = pula.acquire().await?;
    ustawienia_w(&mut db).await
}

pub async fn zapisz_ustawienia(pula: &SqlitePool, u: UstawieniaUmow) -> Wynik<UstawieniaUmow> {
    let f = u.format_numeru.trim().to_string();
    if !(f.contains("{N}") || f.contains("{NN}") || f.contains("{NNN}")) {
        return Err(BladApi::NieprawidloweDane(
            "format numeru musi zawierać numer kolejny: {N}, {NN} albo {NNN}".into(),
        ));
    }
    limity::tekst("format numeru", Some(&f), 40)?;
    limity::tekst("miejscowość", Some(&u.miejscowosc), MAKS_NAZWA)?;
    for (n, v, maks) in [
        ("dni na brakujące informacje", u.dni_na_informacje, 365),
        ("termin zadatku", u.termin_zadatku_dni, 365),
        ("zadatek %", u.zadatek_procent, 100),
        ("wariant A – dni przed montażem", u.wariant_a_dni_przed_montazem, 60),
        ("wariant B – dni przed montażem", u.wariant_b_dni_przed_montazem, 60),
        ("wariant B – dni po odbiorze", u.wariant_b_dni_po_odbiorze, 60),
        ("czas warsztatu", u.czas_warsztat_dni, 365),
        ("czas montażu", u.czas_montazu_dni, 365),
    ] {
        dni_opc(n, Some(v), maks)?;
    }
    let wartosci = [
        f,
        u.miejscowosc.trim().to_string(),
        u.dni_na_informacje.to_string(),
        u.termin_zadatku_dni.to_string(),
        u.zadatek_procent.to_string(),
        u.wariant_a_dni_przed_montazem.to_string(),
        u.wariant_b_dni_przed_montazem.to_string(),
        u.wariant_b_dni_po_odbiorze.to_string(),
        u.czas_warsztat_dni.max(0).to_string(),
        u.czas_montazu_dni.max(1).to_string(),
    ];
    let mut tx = crate::baza::transakcja_zapisu(pula).await?;
    for (k, v) in KLUCZE_USTAWIEN.iter().zip(wartosci) {
        sqlx::query(
            "INSERT INTO ustawienia (klucz, wartosc) VALUES (?1, ?2) \
             ON CONFLICT(klucz) DO UPDATE SET wartosc = excluded.wartosc",
        )
        .bind(k)
        .bind(v)
        .execute(&mut *tx)
        .await?;
    }
    specyfikacje::dziennik(&mut tx, "ustawienia", 0, "zmiana").await?;
    let w = ustawienia_w(&mut tx).await?;
    tx.commit().await?;
    Ok(w)
}

// ---------------------------------------------------------------------------
// Numeracja
// ---------------------------------------------------------------------------

/// Numer z formatu: `{N}`/`{NN}`/`{NNN}` – numer kolejny (z zerami), `{RRRR}`/`{RR}` – rok.
pub fn numer_z_formatu(format: &str, rok: i64, nr: i64) -> String {
    format
        .replace("{NNN}", &format!("{nr:03}"))
        .replace("{NN}", &format!("{nr:02}"))
        .replace("{N}", &nr.to_string())
        .replace("{RRRR}", &format!("{rok:04}"))
        .replace("{RR}", &format!("{:02}", rok % 100))
}

/// Odczyt roku i numeru kolejnego z numeru zapisanego w formacie (np. ręcznie zmienionego).
pub fn parsuj_numer(format: &str, numer: &str) -> Option<(i64, i64)> {
    let mut f = format;
    let mut n = numer;
    let mut rok = None;
    let mut nr = None;
    let cyfry = |s: &str, ile: Option<usize>| -> Option<(i64, usize)> {
        let dl = s.bytes().take_while(u8::is_ascii_digit).count();
        let dl = match ile {
            Some(k) if dl >= k => k,
            Some(_) => return None,
            None if dl == 0 => return None,
            None => dl,
        };
        s[..dl].parse().ok().map(|v| (v, dl))
    };
    while !f.is_empty() {
        let (token, dl_f) = if f.starts_with("{NNN}") || f.starts_with("{RRRR}") {
            (&f[..if f.starts_with("{NNN}") { 5 } else { 6 }], if f.starts_with("{NNN}") { 5 } else { 6 })
        } else if f.starts_with("{NN}") || f.starts_with("{RR}") {
            (&f[..4], 4)
        } else if f.starts_with("{N}") {
            (&f[..3], 3)
        } else {
            let c = f.chars().next()?;
            if !n.starts_with(c) {
                return None;
            }
            f = &f[c.len_utf8()..];
            n = &n[c.len_utf8()..];
            continue;
        };
        let (v, dl) = match token {
            "{RRRR}" => cyfry(n, Some(4))?,
            "{RR}" => {
                let (v, d) = cyfry(n, Some(2))?;
                (2000 + v, d)
            }
            _ => cyfry(n, None)?,
        };
        if token.starts_with("{R") {
            rok = Some(v);
        } else {
            nr = Some(v);
        }
        f = &f[dl_f..];
        n = &n[dl..];
    }
    if !n.is_empty() {
        return None;
    }
    Some((rok?, nr?))
}

/// Kolejny wolny numer w roku (wywoływać w transakcji zapisu – BEGIN IMMEDIATE wyklucza kolizje
/// przy równoczesnym tworzeniu na kilku stanowiskach).
async fn nowy_numer(db: &mut SqliteConnection, format: &str, rok: i64) -> Wynik<(i64, String)> {
    let max: Option<i64> = sqlx::query_scalar("SELECT MAX(nr_kolejny) FROM umowy WHERE rok = ?1")
        .bind(rok)
        .fetch_one(&mut *db)
        .await?;
    let mut nr = max.unwrap_or(0) + 1;
    loop {
        let numer = numer_z_formatu(format, rok, nr);
        let zajety: Option<i64> =
            sqlx::query_scalar("SELECT id FROM umowy WHERE lower(numer) = lower(?1)")
                .bind(&numer)
                .fetch_optional(&mut *db)
                .await?;
        if zajety.is_none() {
            return Ok((nr, numer));
        }
        nr += 1;
    }
}

// ---------------------------------------------------------------------------
// Odczyt
// ---------------------------------------------------------------------------

pub async fn pobierz_w(db: &mut SqliteConnection, id: i64) -> Wynik<Umowa> {
    let b: UmowaBaza = sqlx::query_as(&format!("{SQL_UMOWY} WHERE u.id = ?1"))
        .bind(id)
        .fetch_optional(&mut *db)
        .await?
        .ok_or_else(|| BladApi::NieZnaleziono(format!("umowa {id}")))?;
    Ok(b.into())
}

pub async fn platnosci_w(db: &mut SqliteConnection, id: i64) -> Wynik<Vec<PlatnoscPlanowana>> {
    Ok(sqlx::query_as(
        "SELECT id, umowa_id, rodzaj, opis, kwota_gr, termin, kolejnosc FROM platnosci_planowane \
         WHERE umowa_id = ?1 ORDER BY kolejnosc, id",
    )
    .bind(id)
    .fetch_all(&mut *db)
    .await?)
}

pub async fn wplaty_w(db: &mut SqliteConnection, id: i64) -> Wynik<Vec<Wplata>> {
    Ok(sqlx::query_as(
        "SELECT id, umowa_id, platnosc_planowana_id, data, kwota_gr, sposob, uwagi, utworzono \
         FROM wplaty WHERE umowa_id = ?1 ORDER BY data, id",
    )
    .bind(id)
    .fetch_all(&mut *db)
    .await?)
}

pub async fn etapy_w(db: &mut SqliteConnection, id: i64) -> Wynik<Vec<Etap>> {
    Ok(sqlx::query_as(
        "SELECT id, lp, nazwa, termin, kwota_gr FROM etapy_umowy WHERE umowa_id = ?1 ORDER BY lp, id",
    )
    .bind(id)
    .fetch_all(&mut *db)
    .await?)
}

pub async fn protokol_w(db: &mut SqliteConnection, id: i64) -> Wynik<Option<Protokol>> {
    Ok(sqlx::query_as(
        "SELECT id, umowa_id, data, uwagi, bez_zastrzezen FROM protokoly_odbioru WHERE umowa_id = ?1 \
         ORDER BY data DESC, id DESC LIMIT 1",
    )
    .bind(id)
    .fetch_optional(&mut *db)
    .await?)
}

pub async fn osoby_kontaktowe_w(db: &mut SqliteConnection, klient_id: i64) -> Wynik<Vec<OsobaKontaktowa>> {
    Ok(sqlx::query_as(
        "SELECT id, imie_nazwisko, rola, email, telefon FROM osoby_kontaktowe WHERE klient_id = ?1 ORDER BY id",
    )
    .bind(klient_id)
    .fetch_all(&mut *db)
    .await?)
}

async fn dni_wolne_daty(db: &mut SqliteConnection) -> Wynik<Vec<NaiveDate>> {
    let d: Vec<String> = sqlx::query_scalar("SELECT data FROM dni_wolne")
        .fetch_all(&mut *db)
        .await?;
    Ok(d.iter()
        .filter_map(|t| NaiveDate::parse_from_str(t, "%Y-%m-%d").ok())
        .collect())
}

fn data_opc(t: &Option<String>) -> Option<NaiveDate> {
    t.as_deref()
        .and_then(|t| NaiveDate::parse_from_str(t, "%Y-%m-%d").ok())
}

pub fn wejscie_planu(u: &Umowa, data_odbioru: Option<NaiveDate>) -> WejsciePlanu {
    WejsciePlanu {
        wartosc_gr: u.dane.wartosc_brutto_gr,
        zadatek_gr: u.dane.zadatek_gr,
        termin_zadatku: data_opc(&u.dane.termin_zadatku),
        data_zawarcia: data_opc(&u.dane.data_zawarcia),
        wariant: u.dane.wariant_platnosci.clone(),
        kwota_przed_montazem_gr: u.dane.kwota_przed_montazem_gr,
        montaz_do: data_opc(&u.dane.montaz_do),
        czas_montazu_dni: u.dane.czas_montazu_dni.map(|v| v.clamp(1, 365) as u32),
        data_odbioru,
    }
}

pub async fn plan_w(db: &mut SqliteConnection, u: &Umowa) -> Wynik<PlanPlatnosci> {
    let ust = ustawienia_w(db).await?;
    let wolne = dni_wolne_daty(db).await?;
    let odbior = protokol_w(db, u.id).await?.and_then(|p| data_opc(&Some(p.data)));
    Ok(platnosci::wylicz_plan(
        &wejscie_planu(u, odbior),
        &ust.platnosci(),
        &wolne,
    ))
}

pub fn dzis() -> NaiveDate {
    chrono::Local::now().date_naive()
}

/// Saldo: wpłaty pokrywają raty po kolei; raty po terminie niepokryte = zaległość.
pub fn policz_saldo(
    wartosc: i64,
    raty: &[(String, Option<String>, i64)],
    wplacono: i64,
    dzis: NaiveDate,
) -> Saldo {
    let mut s = Saldo {
        wartosc_gr: wartosc,
        wplacono_gr: wplacono,
        pozostalo_gr: wartosc - wplacono,
        ..Default::default()
    };
    let mut narastajaco = 0i64;
    for (opis, termin, kwota) in raty {
        narastajaco += kwota;
        let do_zaplaty = (narastajaco - wplacono).clamp(0, *kwota);
        let zalegla = do_zaplaty > 0
            && termin
                .as_deref()
                .and_then(|t| NaiveDate::parse_from_str(t, "%Y-%m-%d").ok())
                .is_some_and(|t| t < dzis);
        if zalegla {
            s.zalegle_gr += do_zaplaty;
        }
        let p = PozycjaSalda {
            opis: opis.clone(),
            termin: termin.clone(),
            kwota_gr: *kwota,
            do_zaplaty_gr: do_zaplaty,
            zalegla,
        };
        if do_zaplaty > 0 && s.nastepna.is_none() {
            s.nastepna = Some(p.clone());
        }
        s.pozycje.push(p);
    }
    s
}

/// Ostrzeżenia formularza: VAT 8% (limity powierzchni 150/300 m²), brakujące dane.
pub fn ostrzezenia_vat(d: &DaneUmowy) -> Vec<String> {
    let mut w = Vec::new();
    if d.stawka_vat == Some(8) {
        match d.vat_rodzaj_nieruchomosci.as_deref() {
            None => w.push(
                "stawka 8%: uzupełnij oświadczenie o rodzaju nieruchomości i powierzchni (§9)".into(),
            ),
            Some("inny") => w.push(
                "stawka 8% dotyczy budownictwa mieszkaniowego – dla innej nieruchomości sprawdź, czy przysługuje".into(),
            ),
            _ => {}
        }
        if d.vat_powierzchnia_m2.is_none() {
            w.push("stawka 8%: wpisz powierzchnię użytkową nieruchomości".into());
        }
    }
    if let (Some(r), Some(p)) = (d.vat_rodzaj_nieruchomosci.as_deref(), d.vat_powierzchnia_m2) {
        let limit = match r {
            "lokal_mieszkalny" => Some(150.0),
            "dom_jednorodzinny" => Some(300.0),
            _ => None,
        };
        if let Some(l) = limit {
            if p > l {
                w.push(format!(
                    "powierzchnia {} m² przekracza limit {l} m² dla: {} – stawka 8% tylko proporcjonalnie do limitu (nadwyżka 23%)",
                    crate::formaty::ilosc(p),
                    nazwa_rodzaju_nieruchomosci(r)
                ));
            }
        }
    }
    w
}

pub async fn pelna_w(db: &mut SqliteConnection, id: i64) -> Wynik<PelnaUmowa> {
    let umowa = pobierz_w(db, id).await?;
    let specyfikacja = specyfikacje::pobierz_w(db, umowa.specyfikacja_id).await?;
    let plan = plan_w(db, &umowa).await?;
    let platnosci = platnosci_w(db, id).await?;
    let wplaty = wplaty_w(db, id).await?;
    let etapy = etapy_w(db, id).await?;
    let protokol = protokol_w(db, id).await?;
    let aneksy: Vec<AneksKrotko> = sqlx::query_as(
        "SELECT id, nr, data, status, zmiana_ceny_gr, nowy_termin_montazu, opis_zmian FROM aneksy \
         WHERE umowa_id = ?1 ORDER BY nr",
    )
    .bind(id)
    .fetch_all(&mut *db)
    .await?;
    let zlecenie: Option<ZlecenieUmowy> = sqlx::query_as(
        "SELECT z.id, z.etap, z.montaz_od, z.harmonogram_nieaktualny_powod, \
           (SELECT COUNT(*) FROM zadania_kalendarza t WHERE t.zlecenie_id = z.id) AS zadania, \
           (SELECT COUNT(*) FROM zamowienia_materialow m WHERE m.zlecenie_id = z.id) AS materialy \
         FROM zlecenia z WHERE z.umowa_id = ?1",
    )
    .bind(id)
    .fetch_optional(&mut *db)
    .await?;
    let wplacono: i64 = wplaty.iter().map(|w| w.kwota_gr).sum();
    let raty: Vec<(String, Option<String>, i64)> = if platnosci.is_empty() {
        plan.pozycje
            .iter()
            .map(|p| (p.opis.clone(), p.termin.clone(), p.kwota_gr))
            .collect()
    } else {
        platnosci
            .iter()
            .map(|p| {
                (
                    p.opis.clone().unwrap_or_else(|| p.rodzaj.clone()),
                    p.termin.clone(),
                    p.kwota_gr,
                )
            })
            .collect()
    };
    let saldo = policz_saldo(
        umowa.dane.wartosc_brutto_gr.unwrap_or(0),
        &raty,
        wplacono,
        dzis(),
    );
    let kontrola_etapow = platnosci::kontrola_etapow(
        &etapy.iter().map(|e| e.kwota_gr).collect::<Vec<_>>(),
        umowa.dane.wartosc_brutto_gr,
    );
    let mut ostrzezenia = ostrzezenia_vat(&umowa.dane);
    ostrzezenia.extend(plan.ostrzezenia.iter().cloned());
    if umowa.dane.zalaczniki.as_ref().is_some_and(|z| z.iter().any(|s| s == "2"))
        && !etapy.is_empty()
        && !kontrola_etapow.zgodna
    {
        ostrzezenia.push(format!(
            "harmonogram etapów: suma {} różni się od wartości umowy o {}",
            crate::formaty::kwota(kontrola_etapow.suma_gr),
            crate::formaty::kwota(kontrola_etapow.roznica_gr)
        ));
    }
    let braki = braki(&umowa, &plan, &etapy, &kontrola_etapow);
    Ok(PelnaUmowa {
        umowa,
        specyfikacja,
        plan,
        platnosci,
        wplaty,
        saldo,
        etapy,
        kontrola_etapow,
        aneksy,
        protokol,
        zlecenie,
        ostrzezenia,
        braki,
    })
}

pub async fn pelna(pula: &SqlitePool, id: i64) -> Wynik<PelnaUmowa> {
    let mut db = pula.acquire().await?;
    pelna_w(&mut db, id).await
}

fn braki(u: &Umowa, plan: &PlanPlatnosci, etapy: &[Etap], k: &KontrolaEtapow) -> Vec<String> {
    let d = &u.dane;
    let mut b = Vec::new();
    if u.klient_usuniety {
        b.push("klient jest usunięty – przywróć go w bazie klientów".into());
    }
    if d.data_zawarcia.is_none() {
        b.push("data umowy".into());
    }
    if d.miejscowosc.is_none() {
        b.push("miejscowość zawarcia umowy".into());
    }
    if d.montaz_do.is_none() {
        b.push("termin rozpoczęcia montażu".into());
    }
    if d.montaz_ulica.is_none() && d.montaz_miasto.is_none() {
        b.push("adres montażu".into());
    }
    if d.stawka_vat.is_none() {
        b.push("stawka VAT".into());
    }
    for e in &plan.bledy {
        b.push(format!("płatności: {e}"));
    }
    if d.zalaczniki.as_ref().is_some_and(|z| z.iter().any(|s| s == "2"))
        && !etapy.is_empty()
        && !k.zgodna
    {
        b.push("harmonogram etapów: suma kwot musi równać się wartości umowy".into());
    }
    b
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct FiltrUmow {
    pub status: Option<String>,
    pub klient_id: Option<i64>,
    pub rok: Option<i64>,
    pub szukaj: Option<String>,
    pub limit: Option<i64>,
    pub od: Option<i64>,
}

pub async fn lista(pula: &SqlitePool, f: FiltrUmow) -> Wynik<Vec<UmowaNaLiscie>> {
    if let Some(s) = &f.status {
        if !STATUSY_UMOWY.contains(&s.as_str()) {
            return Err(BladApi::NieprawidloweDane(format!(
                "nieznany status „{s}” (dozwolone: {})",
                STATUSY_UMOWY.join(", ")
            )));
        }
    }
    let szukaj = f
        .szukaj
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| format!("%{}%", s.to_lowercase()));
    Ok(sqlx::query_as(
        "SELECT u.id, u.numer, u.rok, u.klient_id, k.nazwa AS klient_nazwa, \
           CASE WHEN u.anulowano IS NOT NULL THEN 'anulowana' ELSE u.status END AS status, \
           u.wartosc_brutto_gr, COALESCE((SELECT SUM(kwota_gr) FROM wplaty w WHERE w.umowa_id = u.id), 0) AS wplacono_gr, \
           COALESCE(u.wartosc_brutto_gr, 0) - COALESCE((SELECT SUM(kwota_gr) FROM wplaty w WHERE w.umowa_id = u.id), 0) AS saldo_gr, \
           u.montaz_do, u.data_zawarcia, o.numer AS oferta_numer, u.zmieniono \
         FROM umowy u LEFT JOIN klienci k ON k.id = u.klient_id \
         LEFT JOIN wersje_oferty wo ON wo.id = u.wersja_oferty_id LEFT JOIN oferty o ON o.id = wo.oferta_id \
         WHERE (?1 IS NULL OR (CASE WHEN u.anulowano IS NOT NULL THEN 'anulowana' ELSE u.status END) = ?1) \
           AND (?2 IS NULL OR u.klient_id = ?2) AND (?3 IS NULL OR u.rok = ?3) \
           AND (?4 IS NULL OR lower(u.numer) LIKE ?4 OR lower(k.nazwa) LIKE ?4 OR lower(COALESCE(o.numer, '')) LIKE ?4) \
         ORDER BY u.rok DESC, u.nr_kolejny DESC, u.id DESC LIMIT ?5 OFFSET ?6",
    )
    .bind(&f.status)
    .bind(f.klient_id)
    .bind(f.rok)
    .bind(szukaj)
    .bind(f.limit.unwrap_or(500).clamp(1, 5000))
    .bind(f.od.unwrap_or(0).max(0))
    .fetch_all(pula)
    .await?)
}

// ---------------------------------------------------------------------------
// Tworzenie
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct DaneNowejUmowy {
    /// Umowa z oferty: wersja wysłana albo zaakceptowana.
    pub wersja_oferty_id: Option<i64>,
    /// Umowa od zera: klient z bazy.
    pub klient_id: Option<i64>,
    pub data_zawarcia: Option<String>,
}

pub async fn dodaj(pula: &SqlitePool, d: DaneNowejUmowy) -> Wynik<PelnaUmowa> {
    let data = sprawdz_date("data umowy", &d.data_zawarcia)?.unwrap_or_else(dzis);
    let mut tx = crate::baza::transakcja_zapisu(pula).await?;
    let ust = ustawienia_w(&mut tx).await?;
    let (klient_id, spec, wersja_oferty_id, wartosc, stawka, inwestycja) = match (
        d.wersja_oferty_id,
        d.klient_id,
    ) {
        (Some(wid), _) => {
            let p = crate::oferty::pelna_wersja_w(&mut tx, wid).await?;
            if p.wersja.wyslano.is_none() {
                return Err(BladApi::NieprawidloweDane(format!(
                    "oferta {} v{} jest szkicem – umowę tworzy się z wersji wysłanej albo zaakceptowanej",
                    p.oferta.numer, p.wersja.nr_wersji
                )));
            }
            if p.oferta.status == "odrzucona" {
                return Err(BladApi::NieprawidloweDane(format!(
                    "oferta {} jest odrzucona",
                    p.oferta.numer
                )));
            }
            if p.oferta.klient_usuniety {
                return Err(BladApi::NieprawidloweDane(format!(
                    "klient „{}” jest usunięty – przywróć go w bazie klientów",
                    p.oferta.klient_nazwa
                )));
            }
            if !p.wycena.bledy.is_empty() {
                return Err(BladApi::NieprawidloweDane(format!(
                    "wycena oferty zawiera błędy: {}",
                    p.wycena.bledy.join("; ")
                )));
            }
            let istniejaca: Option<String> = sqlx::query_scalar(
                "SELECT numer FROM umowy WHERE wersja_oferty_id = ?1 AND anulowano IS NULL",
            )
            .bind(wid)
            .fetch_optional(&mut *tx)
            .await?;
            if let Some(n) = istniejaca {
                return Err(BladApi::NieprawidloweDane(format!(
                    "z tej wersji oferty powstała już umowa {n}"
                )));
            }
            let spec = specyfikacje::kopiuj_specyfikacje_w(&mut tx, p.wersja.specyfikacja_id).await?;
            // Miejsce montażu: inwestycja oferty albo nowa – z adresu inwestycji z oferty.
            let oferta_inw: Option<Option<i64>> =
                sqlx::query_scalar("SELECT inwestycja_id FROM oferty WHERE id = ?1")
                    .bind(p.oferta.id)
                    .fetch_optional(&mut *tx)
                    .await?;
            let inw = match oferta_inw.flatten() {
                Some(i) => Some(i),
                None => Some(
                    nowa_inwestycja(&mut tx, p.oferta.klient_id, p.wersja.adres_inwestycji.as_deref())
                        .await?,
                ),
            };
            (
                p.oferta.klient_id,
                spec,
                Some(wid),
                Some(p.wycena.suma_brutto_gr),
                Some(p.wersja.stawka_vat),
                inw,
            )
        }
        (None, Some(kid)) => {
            let ok: Option<i64> =
                sqlx::query_scalar("SELECT id FROM klienci WHERE id = ?1 AND usunieto IS NULL")
                    .bind(kid)
                    .fetch_optional(&mut *tx)
                    .await?;
            if ok.is_none() {
                return Err(BladApi::NieprawidloweDane(format!(
                    "nie ma klienta {kid} (zamawiający musi być w bazie klientów)"
                )));
            }
            let spec = specyfikacje::utworz_w(&mut tx).await?;
            let inw = nowa_inwestycja(&mut tx, kid, None).await?;
            let stawka: Option<i64> = sqlx::query_scalar(
                "SELECT CAST(wartosc AS INTEGER) FROM ustawienia WHERE klucz = 'stawka_vat_domyslna'",
            )
            .fetch_optional(&mut *tx)
            .await?;
            (kid, spec, None, None, stawka.filter(|s| *s == 8 || *s == 23), Some(inw))
        }
        (None, None) => {
            return Err(BladApi::NieprawidloweDane(
                "podaj wersję oferty (wersja_oferty_id) albo klienta (klient_id)".into(),
            ))
        }
    };
    let rok = i64::from(data.year());
    let (nr, numer) = nowy_numer(&mut tx, &ust.format_numeru, rok).await?;
    let zadatek = wartosc.map(|w| {
        // zaokrąglenie do pełnych złotych
        let z = (i128::from(w) * i128::from(ust.zadatek_procent.clamp(0, 100)) / 100) as i64;
        (z + 50) / 100 * 100
    });
    let zal = serde_json::to_string(&ZALACZNIKI_STALE).unwrap_or_default();
    let id = sqlx::query(
        "INSERT INTO umowy (rok, nr_kolejny, numer, klient_id, inwestycja_id, wersja_oferty_id, specyfikacja_id, \
           miejscowosc, data_zawarcia, wartosc_brutto_gr, stawka_vat, zadatek_gr, wariant_platnosci, \
           dni_na_brakujace_informacje, zalaczniki, czas_warsztat_dni, czas_montazu_dni) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, 'A', ?13, ?14, ?15, ?16)",
    )
    .bind(rok)
    .bind(nr)
    .bind(&numer)
    .bind(klient_id)
    .bind(inwestycja)
    .bind(wersja_oferty_id)
    .bind(spec)
    .bind(oczysc_tekst(Some(ust.miejscowosc.clone())))
    .bind(data.format("%Y-%m-%d").to_string())
    .bind(wartosc)
    .bind(stawka)
    .bind(zadatek)
    .bind(ust.dni_na_informacje)
    .bind(zal)
    .bind(ust.czas_warsztat_dni)
    .bind(ust.czas_montazu_dni.max(1))
    .execute(&mut *tx)
    .await?
    .last_insert_rowid();
    specyfikacje::dziennik(&mut tx, "umowy", id, "dodanie").await?;
    let p = pelna_w(&mut tx, id).await?;
    tx.commit().await?;
    Ok(p)
}

async fn nowa_inwestycja(db: &mut SqliteConnection, klient_id: i64, adres: Option<&str>) -> Wynik<i64> {
    let adres = adres.map(str::trim).filter(|a| !a.is_empty());
    Ok(sqlx::query(
        "INSERT INTO inwestycje (klient_id, nazwa, adres_ulica) VALUES (?1, ?2, ?3)",
    )
    .bind(klient_id)
    .bind(adres.unwrap_or("Miejsce montażu"))
    .bind(adres)
    .execute(&mut *db)
    .await?
    .last_insert_rowid())
}

// ---------------------------------------------------------------------------
// Zmiana danych (skrócony formularz)
// ---------------------------------------------------------------------------

fn do_json<T: Serialize>(t: &T) -> Value {
    serde_json::to_value(t).unwrap_or(Value::Null)
}

pub async fn zmien(pula: &SqlitePool, id: i64, tresc: Value) -> Wynik<PelnaUmowa> {
    let z = ZmianaCzesciowa::z_json(tresc, POLA_UMOWY)?;
    let mut tx = crate::baza::transakcja_zapisu(pula).await?;
    let u = pobierz_w(&mut tx, id).await?;
    if u.tylko_do_odczytu {
        if let Some(pole) = z.zmiany.keys().find(|k| !POLA_WEWNETRZNE.contains(&k.as_str())) {
            return Err(BladApi::NieprawidloweDane(format!(
                "{} – nie można zmienić pola „{}”",
                u.powod_blokady.clone().unwrap_or_default(),
                nazwa_pola(pole)
            )));
        }
    }
    let baza = u.dane.clone();
    z.sprawdz(&do_json(&baza), do_json(&u), &nazwa_pola)?;
    let d = z.zastosuj(&baza)?.oczysc()?;
    if d.numer.is_none() {
        return Err(BladApi::NieprawidloweDane("numer umowy jest wymagany".into()));
    }
    let klient_id = d.klient_id.ok_or_else(|| BladApi::NieprawidloweDane("klient jest wymagany".into()))?;
    if d.klient_id != baza.klient_id {
        let ok: Option<i64> =
            sqlx::query_scalar("SELECT id FROM klienci WHERE id = ?1 AND usunieto IS NULL")
                .bind(klient_id)
                .fetch_optional(&mut *tx)
                .await?;
        if ok.is_none() {
            return Err(BladApi::NieprawidloweDane(format!("nie ma klienta {klient_id}")));
        }
    }
    // Numer: unikalny (bez względu na wielkość liter); rok i numer kolejny z formatu, jeśli pasuje.
    let (mut rok, mut nr) = (u.rok, u.nr_kolejny);
    let numer = d.numer.clone().unwrap_or_default();
    if d.numer != baza.numer {
        let zajety: Option<String> = sqlx::query_scalar(
            "SELECT numer FROM umowy WHERE lower(numer) = lower(?1) AND id <> ?2",
        )
        .bind(&numer)
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;
        if let Some(n) = zajety {
            return Err(BladApi::NieprawidloweDane(format!(
                "numer umowy „{n}” jest już zajęty"
            )));
        }
        let ust = ustawienia_w(&mut tx).await?;
        if let Some((r, n)) = parsuj_numer(&ust.format_numeru, &numer) {
            let kolizja: Option<i64> = sqlx::query_scalar(
                "SELECT id FROM umowy WHERE rok = ?1 AND nr_kolejny = ?2 AND id <> ?3",
            )
            .bind(r)
            .bind(n)
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;
            if kolizja.is_none() {
                rok = r;
                nr = n;
            }
        }
    }
    sqlx::query(
        "UPDATE umowy SET numer = ?1, rok = ?2, nr_kolejny = ?3, klient_id = ?4, miejscowosc = ?5, data_zawarcia = ?6, \
           wartosc_brutto_gr = ?7, stawka_vat = ?8, zadatek_gr = ?9, termin_zadatku = ?10, wariant_platnosci = ?11, \
           kwota_przed_montazem_gr = ?12, montaz_do = ?13, dni_na_brakujace_informacje = ?14, \
           vat_rodzaj_nieruchomosci = ?15, vat_powierzchnia_m2 = ?16, vat_podstawa_powierzchni = ?17, zalaczniki = ?18, \
           czas_warsztat_dni = ?19, czas_montazu_dni = ?20, uwagi = ?21, wersja = wersja + 1, \
           zmieniono = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = ?22",
    )
    .bind(&numer)
    .bind(rok)
    .bind(nr)
    .bind(klient_id)
    .bind(&d.miejscowosc)
    .bind(&d.data_zawarcia)
    .bind(d.wartosc_brutto_gr)
    .bind(d.stawka_vat)
    .bind(d.zadatek_gr)
    .bind(&d.termin_zadatku)
    .bind(&d.wariant_platnosci)
    .bind(d.kwota_przed_montazem_gr)
    .bind(&d.montaz_do)
    .bind(d.dni_na_brakujace_informacje)
    .bind(&d.vat_rodzaj_nieruchomosci)
    .bind(d.vat_powierzchnia_m2)
    .bind(&d.vat_podstawa_powierzchni)
    .bind(serde_json::to_string(&d.zalaczniki).unwrap_or_default())
    .bind(d.czas_warsztat_dni)
    .bind(d.czas_montazu_dni)
    .bind(&d.uwagi)
    .bind(id)
    .execute(&mut *tx)
    .await?;
    // Miejsce montażu → inwestycja klienta.
    let montaz = [
        &d.montaz_ulica,
        &d.montaz_kod,
        &d.montaz_miasto,
        &d.montaz_pietro,
        &d.montaz_kod_domofonu,
        &d.montaz_parking,
        &d.montaz_winda_wniesienie,
    ];
    let montaz_przed = [
        &baza.montaz_ulica,
        &baza.montaz_kod,
        &baza.montaz_miasto,
        &baza.montaz_pietro,
        &baza.montaz_kod_domofonu,
        &baza.montaz_parking,
        &baza.montaz_winda_wniesienie,
    ];
    if montaz != montaz_przed || d.klient_id != baza.klient_id {
        let inw = match u.inwestycja_id {
            Some(i) if d.klient_id == baza.klient_id => i,
            _ => {
                let i = nowa_inwestycja(&mut tx, klient_id, d.montaz_ulica.as_deref()).await?;
                sqlx::query("UPDATE umowy SET inwestycja_id = ?1 WHERE id = ?2")
                    .bind(i)
                    .bind(id)
                    .execute(&mut *tx)
                    .await?;
                i
            }
        };
        sqlx::query(
            "UPDATE inwestycje SET nazwa = COALESCE(?1, nazwa), adres_ulica = ?1, adres_kod = ?2, adres_miasto = ?3, pietro = ?4, \
             kod_domofonu = ?5, parking = ?6, winda_wniesienie = ?7 WHERE id = ?8",
        )
        .bind(&d.montaz_ulica)
        .bind(&d.montaz_kod)
        .bind(&d.montaz_miasto)
        .bind(&d.montaz_pietro)
        .bind(&d.montaz_kod_domofonu)
        .bind(&d.montaz_parking)
        .bind(&d.montaz_winda_wniesienie)
        .bind(inw)
        .execute(&mut *tx)
        .await?;
        specyfikacje::dziennik(&mut tx, "inwestycje", inw, "zmiana").await?;
    }
    specyfikacje::dziennik(&mut tx, "umowy", id, "zmiana").await?;
    // Czasy produkcji po podpisaniu → zlecenie i przeliczenie harmonogramu.
    if !u.szkic()
        && (d.czas_warsztat_dni != baza.czas_warsztat_dni
            || d.czas_montazu_dni != baza.czas_montazu_dni)
    {
        let zid: Option<i64> = sqlx::query_scalar("SELECT id FROM zlecenia WHERE umowa_id = ?1")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;
        if let Some(zid) = zid {
            sqlx::query(
                "UPDATE zlecenia SET czas_warsztat_dni = ?1, czas_montazu_dni = ?2 WHERE id = ?3",
            )
            .bind(d.czas_warsztat_dni)
            .bind(d.czas_montazu_dni)
            .bind(zid)
            .execute(&mut *tx)
            .await?;
            specyfikacje::dziennik(&mut tx, "zlecenia", zid, "zmiana").await?;
            crate::terminarz::przelicz_jesli_jest_w(&mut tx, zid).await?;
        }
    }
    let p = pelna_w(&mut tx, id).await?;
    tx.commit().await?;
    Ok(p)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DaneEtapow {
    pub etapy: Vec<Etap>,
}

pub async fn zapisz_etapy(pula: &SqlitePool, id: i64, d: DaneEtapow) -> Wynik<PelnaUmowa> {
    if d.etapy.len() > 100 {
        return Err(BladApi::NieprawidloweDane("najwyżej 100 etapów".into()));
    }
    for (i, e) in d.etapy.iter().enumerate() {
        if e.nazwa.trim().is_empty() {
            return Err(BladApi::NieprawidloweDane(format!(
                "etap {}: wpisz nazwę pracy",
                i + 1
            )));
        }
        limity::tekst("nazwa etapu", Some(&e.nazwa), limity::MAKS_TEKST)?;
        limity::kwota("kwota etapu", Some(e.kwota_gr))?;
        sprawdz_date("termin etapu", &e.termin)?;
    }
    let mut tx = crate::baza::transakcja_zapisu(pula).await?;
    let u = pobierz_w(&mut tx, id).await?;
    if let Some(p) = &u.powod_blokady {
        return Err(BladApi::NieprawidloweDane(p.clone()));
    }
    sqlx::query("DELETE FROM etapy_umowy WHERE umowa_id = ?1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    for (i, e) in d.etapy.iter().enumerate() {
        sqlx::query("INSERT INTO etapy_umowy (umowa_id, lp, nazwa, termin, kwota_gr) VALUES (?1, ?2, ?3, ?4, ?5)")
            .bind(id)
            .bind(i as i64 + 1)
            .bind(e.nazwa.trim())
            .bind(&e.termin)
            .bind(e.kwota_gr)
            .execute(&mut *tx)
            .await?;
    }
    specyfikacje::dziennik(&mut tx, "etapy_umowy", id, "zmiana").await?;
    let p = pelna_w(&mut tx, id).await?;
    tx.commit().await?;
    Ok(p)
}

// ---------------------------------------------------------------------------
// Statusy
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct ZmianaStatusuUmowy {
    pub status: String,
    /// Data podpisania (dla „podpisana”; domyślnie data umowy) albo odbioru.
    pub data: Option<String>,
    /// Powód anulowania.
    pub powod: Option<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct WynikHarmonogramu {
    pub zlecenie_id: i64,
    pub wygenerowano: bool,
    pub powod: Option<String>,
    pub dodane: usize,
    pub zmienione: usize,
    pub materialy_dodane: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct WynikStatusu {
    #[serde(flatten)]
    pub umowa: PelnaUmowa,
    pub harmonogram: Option<WynikHarmonogramu>,
}

fn dozwolone_przejscie(z: &str, na: &str) -> bool {
    matches!(
        (z, na),
        ("szkic", "wyslana")
            | ("szkic", "podpisana")
            | ("wyslana", "szkic")
            | ("wyslana", "podpisana")
            | ("podpisana", "w_realizacji")
            | ("podpisana", "odebrana")
            | ("w_realizacji", "odebrana")
            | ("odebrana", "zamknieta")
            | ("odebrana", "w_realizacji")
    ) || (na == "anulowana" && z != "zamknieta" && z != "anulowana")
}

/// Migawka danych stron (zamawiający z bazy klientów, osoby kontaktowe, wykonawca z ustawień).
pub async fn migawka_w(db: &mut SqliteConnection, u: &Umowa) -> Wynik<MigawkaUmowy> {
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
        "SELECT nazwa, adres_ulica, adres_kod, adres_miasto, nip, email, telefon FROM klienci WHERE id = ?1",
    )
    .bind(u.dane.klient_id)
    .fetch_optional(&mut *db)
    .await?;
    let (nazwa, adres_ulica, adres_kod, adres_miasto, nip, email, telefon) =
        k.ok_or_else(|| BladApi::NieZnaleziono("klient umowy".into()))?;
    let f = crate::firma::pobierz_w(db).await?;
    Ok(MigawkaUmowy {
        zamawiajacy: DaneInwestora {
            nazwa,
            adres_ulica,
            adres_kod,
            adres_miasto,
            nip,
            email,
            telefon,
        },
        osoby_kontaktowe: osoby_kontaktowe_w(db, u.dane.klient_id.unwrap_or(0)).await?,
        wykonawca: DaneWykonawcy {
            nazwa: f.firma_nazwa,
            adres: f.firma_adres,
            nip: f.firma_nip,
            telefon: f.firma_telefon,
            email: f.firma_email,
        },
        wykonawca_bank: f.firma_bank,
        wykonawca_rachunek: f.firma_rachunek,
        wykonawca_swift: f.firma_swift,
    })
}

/// Zapis planu do `platnosci_planowane` – w miejscu (po rodzaju i kolejności), żeby zadania
/// kalendarza `platnosc:<id>` zachowały identyfikatory i stan wykonania.
pub async fn zapisz_plan_w(db: &mut SqliteConnection, umowa_id: i64, plan: &PlanPlatnosci) -> Wynik<()> {
    let stare = platnosci_w(db, umowa_id).await?;
    let mut uzyte = BTreeSet::new();
    for (i, p) in plan.pozycje.iter().enumerate() {
        let opis = if p.szacunkowy {
            format!("{} – {} (termin szacunkowy)", p.opis, p.termin_opis)
        } else {
            p.opis.clone()
        };
        let istn = stare
            .iter()
            .find(|s| s.rodzaj == p.rodzaj && !uzyte.contains(&s.id));
        match istn {
            Some(s) => {
                uzyte.insert(s.id);
                sqlx::query(
                    "UPDATE platnosci_planowane SET opis = ?1, kwota_gr = ?2, termin = ?3, kolejnosc = ?4 WHERE id = ?5",
                )
                .bind(&opis)
                .bind(p.kwota_gr)
                .bind(&p.termin)
                .bind(i as i64)
                .bind(s.id)
                .execute(&mut *db)
                .await?;
            }
            None => {
                sqlx::query(
                    "INSERT INTO platnosci_planowane (umowa_id, rodzaj, opis, kwota_gr, termin, kolejnosc) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                )
                .bind(umowa_id)
                .bind(&p.rodzaj)
                .bind(&opis)
                .bind(p.kwota_gr)
                .bind(&p.termin)
                .bind(i as i64)
                .execute(&mut *db)
                .await?;
            }
        }
    }
    for s in stare.iter().filter(|s| !uzyte.contains(&s.id)) {
        sqlx::query("DELETE FROM platnosci_planowane WHERE id = ?1")
            .bind(s.id)
            .execute(&mut *db)
            .await?;
    }
    specyfikacje::dziennik(db, "platnosci_planowane", umowa_id, "zmiana").await?;
    Ok(())
}

pub async fn zmien_status(pula: &SqlitePool, id: i64, z: ZmianaStatusuUmowy) -> Wynik<WynikStatusu> {
    if !STATUSY_UMOWY.contains(&z.status.as_str()) {
        return Err(BladApi::NieprawidloweDane(format!(
            "nieznany status „{}” (dozwolone: {})",
            z.status,
            STATUSY_UMOWY.join(", ")
        )));
    }
    let data = sprawdz_date("data", &z.data)?;
    limity::tekst("powód", z.powod.as_deref(), MAKS_DLUGI)?;
    let mut tx = crate::baza::transakcja_zapisu(pula).await?;
    let p = pelna_w(&mut tx, id).await?;
    let u = &p.umowa;
    if u.status == z.status {
        return Err(BladApi::NieprawidloweDane(format!(
            "umowa ma już status „{}”",
            nazwa_statusu(&z.status)
        )));
    }
    if !dozwolone_przejscie(&u.status, &z.status) {
        return Err(BladApi::NieprawidloweDane(format!(
            "nie można zmienić statusu z „{}” na „{}”",
            nazwa_statusu(&u.status),
            nazwa_statusu(&z.status)
        )));
    }
    let mut harmonogram = None;
    if u.status == "szkic" && z.status != "anulowana" {
        if !p.braki.is_empty() {
            return Err(BladApi::NieprawidloweDane(format!(
                "uzupełnij umowę przed wysłaniem/podpisaniem: {}",
                p.braki.join("; ")
            )));
        }
        let m = migawka_w(&mut tx, u).await?;
        sqlx::query(
            "UPDATE umowy SET migawka = ?1, wyslano = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = ?2",
        )
        .bind(serde_json::to_string(&m).unwrap_or_default())
        .bind(id)
        .execute(&mut *tx)
        .await?;
        zapisz_plan_w(&mut tx, id, &p.plan).await?;
    }
    match z.status.as_str() {
        "szkic" => {
            // wysłana → szkic: klient prosi o zmiany przed podpisaniem
            sqlx::query("UPDATE umowy SET migawka = NULL, wyslano = NULL WHERE id = ?1")
                .bind(id)
                .execute(&mut *tx)
                .await?;
            sqlx::query("DELETE FROM platnosci_planowane WHERE umowa_id = ?1")
                .bind(id)
                .execute(&mut *tx)
                .await?;
        }
        "podpisana" => {
            let podpis = data
                .or_else(|| data_opc(&u.dane.data_zawarcia))
                .unwrap_or_else(dzis);
            sqlx::query("UPDATE umowy SET data_podpisania = ?1, specyfikacja_pierwotna_id = COALESCE(specyfikacja_pierwotna_id, specyfikacja_id) WHERE id = ?2")
                .bind(podpis.format("%Y-%m-%d").to_string())
                .bind(id)
                .execute(&mut *tx)
                .await?;
            harmonogram = Some(po_podpisaniu_w(&mut tx, id, Some(podpis)).await?);
        }
        "odebrana" => {
            if protokol_w(&mut tx, id).await?.is_none() {
                let d = data.unwrap_or_else(dzis);
                sqlx::query("INSERT INTO protokoly_odbioru (umowa_id, data, utworzono) VALUES (?1, ?2, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))")
                    .bind(id)
                    .bind(d.format("%Y-%m-%d").to_string())
                    .execute(&mut *tx)
                    .await?;
            }
            ustaw_etap_zlecenia(&mut tx, id, "odbior").await?;
        }
        "zamknieta" => ustaw_etap_zlecenia(&mut tx, id, "zakonczone").await?,
        "anulowana" => {
            sqlx::query("UPDATE umowy SET anulowano = ?1, anulowanie_powod = ?2 WHERE id = ?3")
                .bind(dzis().format("%Y-%m-%d").to_string())
                .bind(oczysc_tekst(z.powod.clone()))
                .bind(id)
                .execute(&mut *tx)
                .await?;
            let zid: Option<i64> = sqlx::query_scalar("SELECT id FROM zlecenia WHERE umowa_id = ?1")
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?;
            if let Some(zid) = zid {
                // niewykonane zadania wyliczone znikają z kalendarza, wykonane zostają jako historia
                sqlx::query("DELETE FROM zadania_kalendarza WHERE zlecenie_id = ?1 AND wykonano IS NULL AND wyliczone = 1")
                    .bind(zid)
                    .execute(&mut *tx)
                    .await?;
                ustaw_etap_zlecenia(&mut tx, id, "zakonczone").await?;
            }
        }
        _ => {}
    }
    if z.status != "anulowana" {
        sqlx::query("UPDATE umowy SET status = ?1 WHERE id = ?2")
            .bind(&z.status)
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    sqlx::query("UPDATE umowy SET wersja = wersja + 1, zmieniono = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = ?1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    // odbiór zmienia termin pozostałości w wariancie B
    if z.status == "odebrana" {
        let u2 = pobierz_w(&mut tx, id).await?;
        let plan = plan_w(&mut tx, &u2).await?;
        if plan.zgodny {
            zapisz_plan_w(&mut tx, id, &plan).await?;
        }
        przelicz_zlecenie_umowy(&mut tx, id).await?;
    }
    specyfikacje::dziennik(&mut tx, "umowy", id, "zmiana").await?;
    let wynik = WynikStatusu {
        umowa: pelna_w(&mut tx, id).await?,
        harmonogram,
    };
    tx.commit().await?;
    Ok(wynik)
}

async fn ustaw_etap_zlecenia(db: &mut SqliteConnection, umowa_id: i64, etap: &str) -> Wynik<()> {
    let zid: Option<i64> = sqlx::query_scalar("SELECT id FROM zlecenia WHERE umowa_id = ?1")
        .bind(umowa_id)
        .fetch_optional(&mut *db)
        .await?;
    if let Some(zid) = zid {
        sqlx::query("UPDATE zlecenia SET etap = ?1 WHERE id = ?2")
            .bind(etap)
            .bind(zid)
            .execute(&mut *db)
            .await?;
        specyfikacje::dziennik(db, "zlecenia", zid, "zmiana").await?;
    }
    Ok(())
}

/// Przelicza harmonogram zlecenia umowy (jeśli istnieje), np. po zmianie płatności.
pub async fn przelicz_zlecenie_umowy(db: &mut SqliteConnection, umowa_id: i64) -> Wynik<()> {
    let zid: Option<i64> = sqlx::query_scalar("SELECT id FROM zlecenia WHERE umowa_id = ?1")
        .bind(umowa_id)
        .fetch_optional(&mut *db)
        .await?;
    if let Some(zid) = zid {
        let ma: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM zadania_kalendarza WHERE zlecenie_id = ?1 AND klucz IS NOT NULL",
        )
        .bind(zid)
        .fetch_one(&mut *db)
        .await?;
        // jawne przeliczenie – także w etapie „odbiór” (termin pozostałości wariantu B)
        if ma > 0 {
            let _ = crate::terminarz::generuj_w_transakcji(db, zid).await?;
        }
    }
    Ok(())
}

/// Po podpisaniu umowy (lub aneksu): zlecenie z terminem montażu i czasami produkcji,
/// zamówienia materiałów z kategorii wierszy specyfikacji, terminy potwierdzenia pozycji
/// „do potwierdzenia” i wygenerowanie harmonogramu – wszystko w transakcji wywołującego.
pub async fn po_podpisaniu_w(
    db: &mut SqliteConnection,
    umowa_id: i64,
    data_podpisania: Option<NaiveDate>,
) -> Wynik<WynikHarmonogramu> {
    let u = pobierz_w(db, umowa_id).await?;
    let nazwa = format!(
        "{} – {}",
        u.numer(),
        u.klient_nazwa.clone().unwrap_or_default()
    );
    let nazwa: String = nazwa.chars().take(MAKS_NAZWA).collect();
    let istniejace: Option<i64> = sqlx::query_scalar("SELECT id FROM zlecenia WHERE umowa_id = ?1")
        .bind(umowa_id)
        .fetch_optional(&mut *db)
        .await?;
    let zid = match istniejace {
        Some(zid) => {
            sqlx::query(
                "UPDATE zlecenia SET klient_id = ?1, nazwa = ?2, montaz_od = ?3, czas_warsztat_dni = ?4, \
                 czas_montazu_dni = ?5, etap = CASE WHEN etap = 'oferta' THEN 'umowa' ELSE etap END WHERE id = ?6",
            )
            .bind(u.dane.klient_id)
            .bind(&nazwa)
            .bind(&u.dane.montaz_do)
            .bind(u.dane.czas_warsztat_dni)
            .bind(u.dane.czas_montazu_dni)
            .bind(zid)
            .execute(&mut *db)
            .await?;
            specyfikacje::dziennik(db, "zlecenia", zid, "zmiana").await?;
            zid
        }
        None => {
            let zid = sqlx::query(
                "INSERT INTO zlecenia (klient_id, umowa_id, nazwa, etap, montaz_od, czas_warsztat_dni, czas_montazu_dni) \
                 VALUES (?1, ?2, ?3, 'umowa', ?4, ?5, ?6)",
            )
            .bind(u.dane.klient_id)
            .bind(umowa_id)
            .bind(&nazwa)
            .bind(&u.dane.montaz_do)
            .bind(u.dane.czas_warsztat_dni)
            .bind(u.dane.czas_montazu_dni)
            .execute(&mut *db)
            .await?
            .last_insert_rowid();
            specyfikacje::dziennik(db, "zlecenia", zid, "dodanie").await?;
            zid
        }
    };
    let materialy_dodane = synchronizuj_materialy_w(db, zid, u.specyfikacja_id).await?;
    // Pozycje „do potwierdzenia”: termin = data podpisania + liczba dni z umowy.
    if let (Some(d), Some(n)) = (data_podpisania, u.dane.dni_na_brakujace_informacje) {
        let termin = (d + chrono::Days::new(n.max(0) as u64))
            .format("%Y-%m-%d")
            .to_string();
        sqlx::query(
            "UPDATE wiersze_specyfikacji SET termin_potwierdzenia = ?1 WHERE status = 'do_potwierdzenia' \
             AND termin_potwierdzenia IS NULL AND czesc_id IN (SELECT c.id FROM czesci_specyfikacji c \
             JOIN sekcje_specyfikacji s ON s.id = c.sekcja_id WHERE s.specyfikacja_id = ?2)",
        )
        .bind(termin)
        .bind(u.specyfikacja_id)
        .execute(&mut *db)
        .await?;
    }
    let mut w = WynikHarmonogramu {
        zlecenie_id: zid,
        materialy_dodane,
        ..Default::default()
    };
    match crate::terminarz::generuj_w_transakcji(db, zid).await? {
        Ok(g) => {
            w.wygenerowano = true;
            w.dodane = g.dodane;
            w.zmienione = g.zmienione;
        }
        Err(powod) => w.powod = Some(powod),
    }
    Ok(w)
}

/// Zamówienia materiałów zlecenia z kategorii wierszy specyfikacji (bez pozycji po stronie
/// klienta i poza wyceną). Dodaje brakujące kategorie, a usuwa tylko niezamówione materiały
/// dodane wcześniej automatycznie, których kategorii już nie ma. Zwraca liczbę dodanych.
pub async fn synchronizuj_materialy_w(db: &mut SqliteConnection, zid: i64, spec_id: i64) -> Wynik<usize> {
    const ZNACZNIK: &str = "Ze specyfikacji umowy";
    let wiersze: Vec<(i64, Option<String>)> = sqlx::query_as(
        "SELECT w.kategoria_materialu_id, w.material FROM wiersze_specyfikacji w \
         JOIN czesci_specyfikacji c ON c.id = w.czesc_id JOIN sekcje_specyfikacji s ON s.id = c.sekcja_id \
         WHERE s.specyfikacja_id = ?1 AND w.kategoria_materialu_id IS NOT NULL \
           AND w.status NOT IN ('po_stronie_klienta', 'poza_wycena') ORDER BY s.kolejnosc, c.kolejnosc, w.kolejnosc",
    )
    .bind(spec_id)
    .fetch_all(&mut *db)
    .await?;
    let mut kategorie: BTreeMap<i64, Vec<String>> = BTreeMap::new();
    for (k, m) in wiersze {
        let v = kategorie.entry(k).or_default();
        if let Some(m) = m.map(|m| m.trim().to_string()).filter(|m| !m.is_empty()) {
            if !v.contains(&m) {
                v.push(m);
            }
        }
    }
    let istniejace: Vec<(i64, i64, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT id, kategoria_id, opis, data_zamowienia FROM zamowienia_materialow WHERE zlecenie_id = ?1",
    )
    .bind(zid)
    .fetch_all(&mut *db)
    .await?;
    let mut dodane = 0;
    for (k, materialy) in &kategorie {
        let opis: String = format!("{ZNACZNIK}: {}", materialy.join("; "))
            .chars()
            .take(500)
            .collect();
        match istniejace.iter().find(|m| m.1 == *k) {
            Some((mid, _, stary_opis, zam)) => {
                if zam.is_none()
                    && stary_opis.as_deref().is_some_and(|o| o.starts_with(ZNACZNIK))
                    && stary_opis.as_deref() != Some(opis.as_str())
                {
                    sqlx::query("UPDATE zamowienia_materialow SET opis = ?1 WHERE id = ?2")
                        .bind(&opis)
                        .bind(mid)
                        .execute(&mut *db)
                        .await?;
                }
            }
            None => {
                let mid = sqlx::query(
                    "INSERT INTO zamowienia_materialow (zlecenie_id, kategoria_id, opis, status) VALUES (?1, ?2, ?3, 'do_zamowienia')",
                )
                .bind(zid)
                .bind(k)
                .bind(&opis)
                .execute(&mut *db)
                .await?
                .last_insert_rowid();
                specyfikacje::dziennik(db, "zamowienia_materialow", mid, "dodanie").await?;
                dodane += 1;
            }
        }
    }
    for (mid, k, opis, zam) in &istniejace {
        if !kategorie.contains_key(k)
            && zam.is_none()
            && opis.as_deref().is_some_and(|o| o.starts_with(ZNACZNIK))
        {
            sqlx::query("DELETE FROM zamowienia_materialow WHERE id = ?1")
                .bind(mid)
                .execute(&mut *db)
                .await?;
            specyfikacje::dziennik(db, "zamowienia_materialow", *mid, "usuniecie").await?;
        }
    }
    Ok(dodane)
}

// ---------------------------------------------------------------------------
// Różnice, usuwanie, osoby kontaktowe
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct RozniceUmowy {
    /// Specyfikacja źródłowa (oferta albo umowa przed aneksem).
    pub zrodlo: Option<String>,
    pub roznice: Vec<Roznica>,
}

/// Różnice specyfikacji umowy względem oferty źródłowej (pierwotnej specyfikacji umowy).
pub async fn roznice(pula: &SqlitePool, id: i64) -> Wynik<RozniceUmowy> {
    let mut db = pula.acquire().await?;
    let u = pobierz_w(&mut db, id).await?;
    let spec_umowy = u.specyfikacja_pierwotna_id.unwrap_or(u.specyfikacja_id);
    let Some(wid) = u.wersja_oferty_id else {
        return Ok(RozniceUmowy {
            zrodlo: None,
            roznice: vec![],
        });
    };
    let w = crate::oferty::pobierz_wersje_w(&mut db, wid).await?;
    let a = specyfikacje::pobierz_w(&mut db, w.specyfikacja_id).await?;
    let b = specyfikacje::pobierz_w(&mut db, spec_umowy).await?;
    Ok(RozniceUmowy {
        zrodlo: Some(format!(
            "Oferta {} v{}",
            u.oferta_numer.clone().unwrap_or_default(),
            w.nr_wersji
        )),
        roznice: crate::roznice::porownaj_specyfikacje(&a, &b),
    })
}

/// Usunięcie umowy – tylko szkic bez wpłat i aneksów.
pub async fn usun(pula: &SqlitePool, id: i64) -> Wynik<()> {
    let mut tx = crate::baza::transakcja_zapisu(pula).await?;
    let u = pobierz_w(&mut tx, id).await?;
    if u.status != "szkic" {
        return Err(BladApi::NieprawidloweDane(
            "usunąć można tylko szkic umowy – podpisaną anuluj (status „anulowana”)".into(),
        ));
    }
    let wplaty: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM wplaty WHERE umowa_id = ?1")
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;
    if wplaty > 0 {
        return Err(BladApi::NieprawidloweDane(
            "umowa ma zarejestrowane wpłaty – nie można jej usunąć".into(),
        ));
    }
    sqlx::query("UPDATE zlecenia SET umowa_id = NULL WHERE umowa_id = ?1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM umowy WHERE id = ?1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    specyfikacje::usun_specyfikacje_w(&mut tx, u.specyfikacja_id).await?;
    specyfikacje::dziennik(&mut tx, "umowy", id, "usuniecie").await?;
    tx.commit().await?;
    Ok(())
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DaneOsob {
    pub osoby: Vec<OsobaKontaktowa>,
}

pub async fn osoby_kontaktowe(pula: &SqlitePool, klient_id: i64) -> Wynik<Vec<OsobaKontaktowa>> {
    let mut db = pula.acquire().await?;
    osoby_kontaktowe_w(&mut db, klient_id).await
}

/// Zastępuje listę osób kontaktowych klienta.
pub async fn zapisz_osoby_kontaktowe(pula: &SqlitePool, klient_id: i64, d: DaneOsob) -> Wynik<Vec<OsobaKontaktowa>> {
    if d.osoby.len() > 50 {
        return Err(BladApi::NieprawidloweDane("najwyżej 50 osób kontaktowych".into()));
    }
    for o in &d.osoby {
        if o.imie_nazwisko.trim().is_empty() {
            return Err(BladApi::NieprawidloweDane(
                "osoba kontaktowa: wpisz imię i nazwisko".into(),
            ));
        }
        limity::tekst("imię i nazwisko", Some(&o.imie_nazwisko), MAKS_NAZWA)?;
        limity::tekst("rola", o.rola.as_deref(), MAKS_NAZWA)?;
        limity::tekst("e-mail", o.email.as_deref(), MAKS_NAZWA)?;
        limity::tekst("telefon", o.telefon.as_deref(), 100)?;
    }
    let mut tx = crate::baza::transakcja_zapisu(pula).await?;
    let ok: Option<i64> = sqlx::query_scalar("SELECT id FROM klienci WHERE id = ?1")
        .bind(klient_id)
        .fetch_optional(&mut *tx)
        .await?;
    if ok.is_none() {
        return Err(BladApi::NieZnaleziono(format!("klient {klient_id}")));
    }
    sqlx::query("DELETE FROM osoby_kontaktowe WHERE klient_id = ?1")
        .bind(klient_id)
        .execute(&mut *tx)
        .await?;
    for o in d.osoby {
        sqlx::query("INSERT INTO osoby_kontaktowe (klient_id, imie_nazwisko, rola, email, telefon) VALUES (?1, ?2, ?3, ?4, ?5)")
            .bind(klient_id)
            .bind(o.imie_nazwisko.trim())
            .bind(oczysc_tekst(o.rola))
            .bind(oczysc_tekst(o.email))
            .bind(oczysc_tekst(o.telefon))
            .execute(&mut *tx)
            .await?;
    }
    specyfikacje::dziennik(&mut tx, "osoby_kontaktowe", klient_id, "zmiana").await?;
    let w = osoby_kontaktowe_w(&mut tx, klient_id).await?;
    tx.commit().await?;
    Ok(w)
}

#[cfg(test)]
mod testy {
    use super::*;

    #[test]
    fn numeracja_z_formatu() {
        assert_eq!(numer_z_formatu("{NN}/{RRRR}", 2026, 7), "07/2026");
        assert_eq!(numer_z_formatu("{NN}/{RRRR}", 2026, 123), "123/2026");
        assert_eq!(numer_z_formatu("UM/{RR}/{NNN}", 2027, 5), "UM/27/005");
        assert_eq!(numer_z_formatu("{N}-{RRRR}", 2027, 5), "5-2027");
        assert_eq!(parsuj_numer("{NN}/{RRRR}", "07/2026"), Some((2026, 7)));
        assert_eq!(parsuj_numer("{NN}/{RRRR}", "15/2026"), Some((2026, 15)));
        assert_eq!(parsuj_numer("{NN}/{RRRR}", "15/2026a"), None);
        assert_eq!(parsuj_numer("{NN}/{RRRR}", "A/2026"), None);
        assert_eq!(parsuj_numer("UM/{RR}/{NNN}", "UM/27/005"), Some((2027, 5)));
    }

    #[test]
    fn saldo_i_zaleglosci() {
        let d = |t: &str| NaiveDate::parse_from_str(t, "%Y-%m-%d").unwrap();
        let raty = vec![
            ("Zadatek".into(), Some("2026-11-23".into()), 1_000),
            ("Reszta".into(), Some("2027-02-03".into()), 4_000),
        ];
        let s = policz_saldo(5_000, &raty, 0, d("2026-12-01"));
        assert_eq!(s.zalegle_gr, 1_000);
        assert_eq!(s.nastepna.as_ref().unwrap().opis, "Zadatek");
        let s = policz_saldo(5_000, &raty, 1_500, d("2026-12-01"));
        assert_eq!(s.zalegle_gr, 0);
        assert_eq!(s.pozostalo_gr, 3_500);
        assert_eq!(s.nastepna.as_ref().unwrap().do_zaplaty_gr, 3_500);
        let s = policz_saldo(5_000, &raty, 5_000, d("2027-03-01"));
        assert!(s.nastepna.is_none());
        assert_eq!(s.zalegle_gr, 0);
    }

    #[test]
    fn vat_ostrzezenia() {
        let mut d = DaneUmowy {
            stawka_vat: Some(8),
            ..Default::default()
        };
        assert_eq!(ostrzezenia_vat(&d).len(), 2);
        d.vat_rodzaj_nieruchomosci = Some("lokal_mieszkalny".into());
        d.vat_powierzchnia_m2 = Some(151.0);
        let w = ostrzezenia_vat(&d);
        assert_eq!(w.len(), 1);
        assert!(w[0].contains("150"));
        d.vat_rodzaj_nieruchomosci = Some("dom_jednorodzinny".into());
        assert!(ostrzezenia_vat(&d).is_empty());
        d.vat_powierzchnia_m2 = Some(300.5);
        assert!(ostrzezenia_vat(&d)[0].contains("300"));
    }

    #[test]
    fn przejscia_statusow() {
        assert!(dozwolone_przejscie("szkic", "podpisana"));
        assert!(dozwolone_przejscie("wyslana", "szkic"));
        assert!(!dozwolone_przejscie("podpisana", "szkic"));
        assert!(!dozwolone_przejscie("zamknieta", "anulowana"));
        assert!(dozwolone_przejscie("podpisana", "anulowana"));
    }
}
