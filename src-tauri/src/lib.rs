//! Aplikacja okienkowa ROBINWOOD (Tauri 2).
//!
//! Odpowiada za: okna, zasobnik, autostart, powiadomienia, konfigurację stanowiska
//! i – w trybie „serwer” – uruchomienie API z crate'a `robinwood-serwer` w tym samym procesie.
//! Dane biznesowe NIE idą przez komendy Tauri: interfejs zawsze rozmawia z API HTTP,
//! tak samo na serwerze (127.0.0.1) jak na stanowisku-kliencie (adres w sieci).

mod konfiguracja;
mod powiadomienia;
mod widzet;
mod zasobnik;

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;

use konfiguracja::{Konfiguracja, Tryb};
use robinwood_serwer::mdns::ZnalezionySerwer;
use robinwood_serwer::{UruchomionySerwer, UstawieniaSerwera};

/// Argument dodawany przy autostarcie: uruchom bez pokazywania głównego okna.
pub const ARG_ZMINIMALIZOWANY: &str = "--zminimalizowany";

#[derive(Default)]
struct StanAplikacji {
    konfiguracja: Mutex<Konfiguracja>,
    serwer: Mutex<Option<UruchomionySerwer>>,
    blad_serwera: Mutex<Option<String>>,
}

/// To, czego interfejs potrzebuje, żeby połączyć się z API.
#[derive(Serialize)]
struct Polaczenie {
    konfiguracja: Konfiguracja,
    /// Adres bazowy API, z którym ma rozmawiać interfejs (None = nieskonfigurowane).
    adres_api: Option<String>,
    klucz_api: Option<String>,
    /// Czy lokalny serwer działa (tryb serwer).
    serwer_dziala: bool,
    blad_serwera: Option<String>,
    folder_danych: Option<String>,
}

#[tauri::command]
fn pobierz_polaczenie(app: AppHandle, stan: State<'_, StanAplikacji>) -> Polaczenie {
    let konf = stan.konfiguracja.lock().unwrap().clone();
    let serwer_dziala = stan.serwer.lock().unwrap().is_some();
    let blad_serwera = stan.blad_serwera.lock().unwrap().clone();
    let adres_api = match konf.tryb {
        Tryb::Serwer => Some(format!("http://127.0.0.1:{}", konf.port)),
        Tryb::Klient => konf.adres_serwera.clone(),
        Tryb::Nieskonfigurowany => None,
    };
    let folder_danych = match konf.tryb {
        Tryb::Serwer => konfiguracja::folder_danych(&app, &konf)
            .ok()
            .map(|p| p.display().to_string()),
        _ => None,
    };
    Polaczenie {
        klucz_api: konf.klucz_api.clone(),
        konfiguracja: konf,
        adres_api,
        serwer_dziala,
        blad_serwera,
        folder_danych,
    }
}

/// Zapisuje konfigurację i restartuje aplikację, żeby zmiana trybu weszła w życie.
#[tauri::command]
fn zapisz_konfiguracje(app: AppHandle, mut konfiguracja: Konfiguracja) -> Result<(), String> {
    if konfiguracja.tryb == Tryb::Serwer
        && konfiguracja.klucz_api.as_deref().unwrap_or("").is_empty()
    {
        konfiguracja.klucz_api = Some(robinwood_serwer::nowy_klucz_api());
    }
    if let Some(adres) = konfiguracja.adres_serwera.as_mut() {
        *adres = adres.trim().trim_end_matches('/').to_string();
        if !adres.is_empty() && !adres.starts_with("http") {
            *adres = format!("http://{adres}");
        }
    }
    konfiguracja::zapisz(&app, &konfiguracja).map_err(|e| e.to_string())?;
    app.restart();
}

#[tauri::command]
async fn wyszukaj_serwery() -> Result<Vec<ZnalezionySerwer>, String> {
    tokio::task::spawn_blocking(|| robinwood_serwer::mdns::wyszukaj(Duration::from_secs(3)))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn powiadomienie_testowe(app: AppHandle) -> Result<(), String> {
    powiadomienia::testowe(&app).map_err(|e| e.to_string())
}

#[tauri::command]
fn przelacz_widzet(app: AppHandle) {
    widzet::przelacz(&app);
}

#[tauri::command]
fn ukryj_widzet(app: AppHandle) {
    widzet::ustaw_widocznosc(&app, false);
}

/// Powiadomienie Windows z planera powiadomień (logika „co i kiedy” – interfejs, src/lib/powiadomienia.ts).
#[tauri::command]
fn pokaz_powiadomienie(app: AppHandle, tytul: String, tresc: String) -> Result<(), String> {
    powiadomienia::pokaz(&app, &tytul, &tresc).map_err(|e| e.to_string())
}

/// Widżet → okno główne: pokazuje je i przechodzi pod ścieżkę (np. `/kalendarz?zadanie=5`).
#[tauri::command]
fn otworz_sciezke(app: AppHandle, sciezka: String) {
    zasobnik::pokaz_okno(&app, zasobnik::OKNO_GLOWNE);
    if sciezka.starts_with('/') {
        let _ = app.emit_to(zasobnik::OKNO_GLOWNE, "robinwood://przejdz", sciezka);
    }
}

/// Interfejs zapisał szkice po zdarzeniu `robinwood://przed-zamknieciem` – chowamy okno.
#[tauri::command]
fn ukryj_glowne_okno(app: AppHandle) {
    if let Some(o) = app.get_webview_window(zasobnik::OKNO_GLOWNE) {
        let _ = o.hide();
    }
}

/// Interfejs zapisał szkice po zdarzeniu `robinwood://przed-zakonczeniem` – koniec programu.
#[tauri::command]
fn zakoncz_aplikacje(app: AppHandle) {
    app.exit(0);
}

/// Zamknięcie okna / „Zakończ”: najpierw interfejs zapisuje niezapisane wiersze (zdarzenie),
/// potem sam wywołuje `ukryj_glowne_okno` / `zakoncz_aplikacje`. Gdyby interfejs nie
/// odpowiedział (np. jeszcze się ładuje), po `zapas` wykonujemy akcję mimo to.
pub(crate) fn po_zapisie_szkicow(
    app: &AppHandle,
    zdarzenie: &str,
    zapas: Duration,
    akcja: fn(&AppHandle),
) {
    let wyslano = app.emit_to(zasobnik::OKNO_GLOWNE, zdarzenie, ()).is_ok();
    if !wyslano {
        akcja(app);
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(zapas);
        akcja(&app);
    });
}

#[tauri::command]
fn otworz_glowne_okno(app: AppHandle) {
    zasobnik::pokaz_okno(&app, zasobnik::OKNO_GLOWNE);
}

fn uruchom_serwer_jesli_trzeba(app: &AppHandle) {
    let stan = app.state::<StanAplikacji>();
    let konf = stan.konfiguracja.lock().unwrap().clone();
    if konf.tryb != Tryb::Serwer {
        return;
    }
    let wynik = (|| -> anyhow::Result<UruchomionySerwer> {
        let folder = konfiguracja::folder_danych(app, &konf)?;
        let ustawienia = UstawieniaSerwera {
            sciezka_bazy: folder.join("robinwood.db"),
            adres: SocketAddr::from((Ipv4Addr::UNSPECIFIED, konf.port)),
            klucz_api: konf
                .klucz_api
                .clone()
                .ok_or_else(|| anyhow::anyhow!("brak klucza API w konfiguracji"))?,
            nazwa: konf.nazwa_stanowiska.clone(),
            oglaszaj_mdns: true,
        };
        tauri::async_runtime::block_on(robinwood_serwer::uruchom(ustawienia))
    })();
    match wynik {
        Ok(s) => *stan.serwer.lock().unwrap() = Some(s),
        Err(e) => {
            tracing::error!("Nie udało się uruchomić serwera: {e}");
            *stan.blad_serwera.lock().unwrap() = Some(e.to_string());
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .try_init();

    tauri::Builder::default()
        // Musi być pierwsza: drugie uruchomienie tylko pokazuje istniejące okno.
        .plugin(tauri_plugin_single_instance::init(
            |app, _argumenty, _katalog| {
                zasobnik::pokaz_okno(app, zasobnik::OKNO_GLOWNE);
            },
        ))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec![ARG_ZMINIMALIZOWANY]),
        ))
        .manage(StanAplikacji::default())
        .invoke_handler(tauri::generate_handler![
            pobierz_polaczenie,
            zapisz_konfiguracje,
            wyszukaj_serwery,
            powiadomienie_testowe,
            przelacz_widzet,
            ukryj_widzet,
            pokaz_powiadomienie,
            otworz_sciezke,
            otworz_glowne_okno,
            ukryj_glowne_okno,
            zakoncz_aplikacje,
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            let konf = konfiguracja::wczytaj(&handle).unwrap_or_else(|e| {
                tracing::warn!("Nie udało się wczytać konfiguracji, używam domyślnej: {e}");
                Konfiguracja::default()
            });
            *app.state::<StanAplikacji>().konfiguracja.lock().unwrap() = konf;
            uruchom_serwer_jesli_trzeba(&handle);
            zasobnik::utworz(&handle)?;

            // Widżet: zapamiętana pozycja; widoczny przy każdym starcie, chyba że użytkownik go schował.
            // Autostart (--zminimalizowany): bez okna głównego – tylko zasobnik i widżet.
            let zminimalizowany = std::env::args().any(|a| a == ARG_ZMINIMALIZOWANY);
            widzet::przywroc(&handle);
            if !zminimalizowany {
                zasobnik::pokaz_okno(&handle, zasobnik::OKNO_GLOWNE);
            }
            Ok(())
        })
        .on_window_event(|okno, zdarzenie| {
            if let WindowEvent::Moved(poz) = zdarzenie {
                if okno.label() == zasobnik::OKNO_WIDZET {
                    widzet::po_przesunieciu(okno.app_handle(), *poz);
                }
            }
            // Zamknięcie okna chowa je do zasobnika; program kończy „Zakończ” w menu zasobnika.
            if let WindowEvent::CloseRequested { api, .. } = zdarzenie {
                api.prevent_close();
                if okno.label() == zasobnik::OKNO_GLOWNE {
                    po_zapisie_szkicow(
                        okno.app_handle(),
                        "robinwood://przed-zamknieciem",
                        Duration::from_secs(4),
                        |a| {
                            if let Some(o) = a.get_webview_window(zasobnik::OKNO_GLOWNE) {
                                let _ = o.hide();
                            }
                        },
                    );
                } else {
                    let _ = okno.hide();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("błąd podczas uruchamiania aplikacji ROBINWOOD");
}
