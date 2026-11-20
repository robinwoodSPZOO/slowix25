//! Serwer danych programu ROBINWOOD.
//!
//! Ten crate nie zależy od Tauri: trzyma bazę SQLite, migracje oraz API HTTP/WebSocket
//! wystawiane w sieci lokalnej. Używa go aplikacja okienkowa (tryb „serwer”) oraz
//! samodzielny program `robinwood-serwer` (np. na mini PC bez interfejsu).

pub mod api;
pub mod baza;
pub mod bledy;
pub mod klienci;
pub mod mdns;
pub mod slowniki;
pub mod zdarzenia;

use std::net::SocketAddr;
use std::path::PathBuf;

use tokio::sync::oneshot;

pub use api::StanApi;

/// Nazwa usługi ogłaszanej przez mDNS w sieci lokalnej.
pub const USLUGA_MDNS: &str = "_robinwood._tcp.local.";
/// Domyślny port API.
pub const DOMYSLNY_PORT: u16 = 47800;
/// Wersja protokołu API (zmieniamy przy niekompatybilnych zmianach).
pub const WERSJA_API: u32 = 1;

/// Ustawienia uruchomienia serwera.
#[derive(Debug, Clone)]
pub struct UstawieniaSerwera {
    /// Plik bazy SQLite.
    pub sciezka_bazy: PathBuf,
    /// Adres nasłuchu, domyślnie 0.0.0.0:47800 (cała sieć lokalna).
    pub adres: SocketAddr,
    /// Klucz dostępu, który muszą podać stanowiska-klienci.
    pub klucz_api: String,
    /// Nazwa stanowiska widoczna w wykrywaniu mDNS.
    pub nazwa: String,
    /// Czy ogłaszać serwer przez mDNS.
    pub oglaszaj_mdns: bool,
}

/// Uruchomiony serwer; upuszczenie uchwytu NIE zatrzymuje serwera, służy do tego [`UruchomionySerwer::zatrzymaj`].
pub struct UruchomionySerwer {
    pub adres: SocketAddr,
    zatrzymanie: Option<oneshot::Sender<()>>,
    _mdns: Option<mdns::Ogloszenie>,
}

impl UruchomionySerwer {
    pub fn zatrzymaj(mut self) {
        if let Some(tx) = self.zatrzymanie.take() {
            let _ = tx.send(());
        }
    }
}

/// Otwiera bazę (z migracjami), uruchamia API w tle i opcjonalnie ogłasza się przez mDNS.
pub async fn uruchom(ustawienia: UstawieniaSerwera) -> anyhow::Result<UruchomionySerwer> {
    let pula = baza::otworz(&ustawienia.sciezka_bazy).await?;
    let stan = StanApi::nowy(pula, ustawienia.klucz_api.clone(), ustawienia.nazwa.clone());
    let aplikacja = api::router(stan);

    let nasluch = tokio::net::TcpListener::bind(ustawienia.adres).await?;
    let adres = nasluch.local_addr()?;
    let (tx, rx) = oneshot::channel::<()>();

    tokio::spawn(async move {
        let wynik = axum::serve(nasluch, aplikacja)
            .with_graceful_shutdown(async {
                let _ = rx.await;
            })
            .await;
        if let Err(e) = wynik {
            tracing::error!("Serwer API zakończył się błędem: {e}");
        }
    });
    tracing::info!("API ROBINWOOD nasłuchuje na {adres}");

    let ogloszenie = if ustawienia.oglaszaj_mdns {
        match mdns::oglos(&ustawienia.nazwa, adres.port()) {
            Ok(o) => Some(o),
            Err(e) => {
                tracing::warn!("Nie udało się ogłosić serwera przez mDNS: {e}");
                None
            }
        }
    } else {
        None
    };

    Ok(UruchomionySerwer {
        adres,
        zatrzymanie: Some(tx),
        _mdns: ogloszenie,
    })
}

/// Losowy klucz API (32 znaki alfanumeryczne).
pub fn nowy_klucz_api() -> String {
    use rand::distributions::Alphanumeric;
    use rand::Rng;
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(32)
        .map(char::from)
        .collect()
}
