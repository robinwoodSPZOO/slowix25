//! Konfiguracja stanowiska (plik `konfiguracja.json` w folderze konfiguracji aplikacji).
//!
//! Plik jest lokalny dla komputera i NIE trafia do repozytorium ani do bazy.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Tryb {
    /// Pierwsze uruchomienie – użytkownik wybiera tryb w Ustawieniach.
    #[default]
    Nieskonfigurowany,
    /// Ten komputer trzyma bazę i udostępnia API w sieci lokalnej.
    Serwer,
    /// Ten komputer łączy się z serwerem w sieci lokalnej.
    Klient,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Konfiguracja {
    pub tryb: Tryb,
    pub nazwa_stanowiska: String,
    /// Port API (tryb serwer).
    pub port: u16,
    /// Folder z bazą i plikami (tryb serwer); brak = folder danych aplikacji.
    pub folder_danych: Option<PathBuf>,
    /// Adres serwera, np. `http://192.168.1.10:47800` (tryb klient).
    pub adres_serwera: Option<String>,
    /// Klucz API: w trybie serwer generowany, w trybie klient przepisany z serwera.
    pub klucz_api: Option<String>,
}

impl Default for Konfiguracja {
    fn default() -> Self {
        Self {
            tryb: Tryb::Nieskonfigurowany,
            nazwa_stanowiska: domyslna_nazwa_stanowiska(),
            port: robinwood_serwer::DOMYSLNY_PORT,
            folder_danych: None,
            adres_serwera: None,
            klucz_api: None,
        }
    }
}

fn domyslna_nazwa_stanowiska() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "Stanowisko".into())
}

fn sciezka_pliku(app: &AppHandle) -> anyhow::Result<PathBuf> {
    Ok(app.path().app_config_dir()?.join("konfiguracja.json"))
}

pub fn wczytaj(app: &AppHandle) -> anyhow::Result<Konfiguracja> {
    let sciezka = sciezka_pliku(app)?;
    if !sciezka.exists() {
        return Ok(Konfiguracja::default());
    }
    let tekst = std::fs::read_to_string(&sciezka)?;
    Ok(serde_json::from_str(&tekst)?)
}

pub fn zapisz(app: &AppHandle, konf: &Konfiguracja) -> anyhow::Result<()> {
    let sciezka = sciezka_pliku(app)?;
    if let Some(k) = sciezka.parent() {
        std::fs::create_dir_all(k)?;
    }
    std::fs::write(&sciezka, serde_json::to_string_pretty(konf)?)?;
    Ok(())
}

/// Folder danych serwera (baza + pliki).
pub fn folder_danych(app: &AppHandle, konf: &Konfiguracja) -> anyhow::Result<PathBuf> {
    match &konf.folder_danych {
        Some(f) => Ok(f.clone()),
        None => Ok(app.path().app_data_dir()?.join("dane")),
    }
}
