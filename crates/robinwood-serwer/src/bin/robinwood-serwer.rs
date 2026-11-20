//! Samodzielny serwer ROBINWOOD (bez okna), np. na komputer lub mini PC pełniący rolę serwera.
//!
//! Zmienne środowiskowe:
//! * `ROBINWOOD_BAZA`  – ścieżka pliku bazy (domyślnie `./dane/robinwood.db`)
//! * `ROBINWOOD_ADRES` – adres nasłuchu (domyślnie `0.0.0.0:47800`)
//! * `ROBINWOOD_KLUCZ` – klucz API (gdy brak, losowany i wypisywany na ekran)
//! * `ROBINWOOD_NAZWA` – nazwa w sieci (domyślnie `ROBINWOOD serwer`)
//! * `ROBINWOOD_MDNS`  – `0`, aby wyłączyć ogłaszanie mDNS
//!
//! `robinwood-serwer --szukaj` – diagnostyka: wypisuje serwery znalezione w sieci przez mDNS.

use std::net::SocketAddr;
use std::path::PathBuf;

use robinwood_serwer::{nowy_klucz_api, uruchom, UstawieniaSerwera, DOMYSLNY_PORT};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    if std::env::args().any(|a| a == "--szukaj") {
        let znalezione = robinwood_serwer::mdns::wyszukaj(std::time::Duration::from_secs(3))?;
        if znalezione.is_empty() {
            println!("Nie znaleziono serwera ROBINWOOD w sieci.");
        }
        for s in znalezione {
            println!(
                "{} → {} (adresy: {})",
                s.nazwa,
                s.adres_api,
                s.adresy.join(", ")
            );
        }
        return Ok(());
    }

    let sciezka_bazy = std::env::var("ROBINWOOD_BAZA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("dane/robinwood.db"));
    let adres: SocketAddr = std::env::var("ROBINWOOD_ADRES")
        .unwrap_or_else(|_| format!("0.0.0.0:{DOMYSLNY_PORT}"))
        .parse()?;
    let klucz_api = match std::env::var("ROBINWOOD_KLUCZ") {
        Ok(k) if !k.is_empty() => k,
        _ => {
            let k = nowy_klucz_api();
            println!("Wygenerowano klucz API (podaj go na stanowiskach): {k}");
            k
        }
    };
    let nazwa = std::env::var("ROBINWOOD_NAZWA").unwrap_or_else(|_| "ROBINWOOD serwer".into());
    let oglaszaj_mdns = std::env::var("ROBINWOOD_MDNS")
        .map(|v| v != "0")
        .unwrap_or(true);

    let serwer = uruchom(UstawieniaSerwera {
        sciezka_bazy,
        adres,
        klucz_api,
        nazwa,
        oglaszaj_mdns,
    })
    .await?;
    println!("Serwer działa na {} (Ctrl+C kończy)", serwer.adres);
    tokio::signal::ctrl_c().await?;
    serwer.zatrzymaj();
    Ok(())
}
