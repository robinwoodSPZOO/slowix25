//! Ogłaszanie i wykrywanie serwera w sieci lokalnej (mDNS / DNS-SD).
//!
//! Serwer ogłasza usługę `_robinwood._tcp.local.`; stanowisko-klient przegląda sieć przez
//! kilka sekund i pokazuje listę znalezionych serwerów. Gdy mDNS jest zablokowany (np. przez
//! zaporę lub izolację klientów w routerze Wi-Fi), adres serwera wpisuje się ręcznie.

use std::collections::HashMap;
use std::net::IpAddr;
use std::time::{Duration, Instant};

use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use serde::Serialize;

use crate::USLUGA_MDNS;

/// Aktywne ogłoszenie; upuszczenie wyrejestrowuje usługę.
pub struct Ogloszenie {
    demon: ServiceDaemon,
    pelna_nazwa: String,
}

impl Drop for Ogloszenie {
    fn drop(&mut self) {
        let _ = self.demon.unregister(&self.pelna_nazwa);
        let _ = self.demon.shutdown();
    }
}

/// Ogłasza serwer w sieci lokalnej.
pub fn oglos(nazwa: &str, port: u16) -> anyhow::Result<Ogloszenie> {
    let demon = ServiceDaemon::new()?;
    let instancja = oczysc_nazwe(nazwa);
    let host_ascii: String = instancja
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let host = format!("{}.local.", host_ascii.trim_matches('-'));
    let mut wlasciwosci = HashMap::new();
    wlasciwosci.insert("wersja_api".to_string(), crate::WERSJA_API.to_string());
    wlasciwosci.insert("nazwa".to_string(), nazwa.to_string());
    let info =
        ServiceInfo::new(USLUGA_MDNS, &instancja, &host, "", port, wlasciwosci)?.enable_addr_auto();
    let pelna_nazwa = info.get_fullname().to_string();
    demon.register(info)?;
    Ok(Ogloszenie { demon, pelna_nazwa })
}

/// Serwer znaleziony w sieci.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ZnalezionySerwer {
    pub nazwa: String,
    pub adresy: Vec<String>,
    pub port: u16,
    /// Gotowy adres bazowy API, np. `http://192.168.1.10:47800`.
    pub adres_api: String,
}

/// Przegląda sieć przez `czas` i zwraca znalezione serwery (funkcja blokująca).
pub fn wyszukaj(czas: Duration) -> anyhow::Result<Vec<ZnalezionySerwer>> {
    let demon = ServiceDaemon::new()?;
    let odbiornik = demon.browse(USLUGA_MDNS)?;
    let koniec = Instant::now() + czas;
    let mut wynik: Vec<ZnalezionySerwer> = Vec::new();
    while let Some(pozostalo) = koniec.checked_duration_since(Instant::now()) {
        match odbiornik.recv_timeout(pozostalo) {
            Ok(ServiceEvent::ServiceResolved(info)) => {
                let mut adresy: Vec<IpAddr> = info.get_addresses().iter().copied().collect();
                // IPv4 najpierw – prościej wpisać i pewniej działa w sieciach domowych.
                adresy.sort_by_key(|a| (a.is_ipv6(), *a));
                let Some(pierwszy) = adresy.first() else {
                    continue;
                };
                let host = match pierwszy {
                    IpAddr::V4(a) => a.to_string(),
                    IpAddr::V6(a) => format!("[{a}]"),
                };
                let nazwa = info
                    .get_property_val_str("nazwa")
                    .map(str::to_owned)
                    .unwrap_or_else(|| info.get_fullname().to_string());
                let serwer = ZnalezionySerwer {
                    nazwa,
                    adresy: adresy.iter().map(ToString::to_string).collect(),
                    port: info.get_port(),
                    adres_api: format!("http://{host}:{}", info.get_port()),
                };
                if !wynik.iter().any(|s| s.adres_api == serwer.adres_api) {
                    wynik.push(serwer);
                }
            }
            Ok(_) => {}
            Err(_) => break,
        }
    }
    let _ = demon.shutdown();
    Ok(wynik)
}

/// Nazwa instancji DNS-SD: bez kropek i znaków sterujących, maks. 63 bajty.
fn oczysc_nazwe(nazwa: &str) -> String {
    let mut s: String = nazwa
        .chars()
        .map(|c| if c == '.' || c.is_control() { '-' } else { c })
        .collect();
    if s.trim().is_empty() {
        s = "ROBINWOOD".into();
    }
    while s.len() > 63 {
        s.pop();
    }
    s
}

#[cfg(test)]
mod testy {
    use super::oczysc_nazwe;

    #[test]
    fn nazwa_instancji() {
        assert_eq!(oczysc_nazwe("Biuro.serwer"), "Biuro-serwer");
        assert_eq!(oczysc_nazwe("  "), "ROBINWOOD");
        assert!(oczysc_nazwe(&"ż".repeat(60)).len() <= 63);
    }
}
