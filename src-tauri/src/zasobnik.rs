//! Ikona w zasobniku systemowym (tray) i okno-widżet.

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Runtime};

pub const OKNO_GLOWNE: &str = "main";
pub const OKNO_WIDZET: &str = "widzet";

pub fn utworz(app: &AppHandle) -> tauri::Result<()> {
    let otworz = MenuItem::with_id(app, "otworz", "Otwórz ROBINWOOD", true, None::<&str>)?;
    let widzet = MenuItem::with_id(app, "widzet", "Pokaż / ukryj widżet", true, None::<&str>)?;
    let test = MenuItem::with_id(
        app,
        "powiadomienie",
        "Powiadomienie testowe",
        true,
        None::<&str>,
    )?;
    let separator = PredefinedMenuItem::separator(app)?;
    let zakoncz = MenuItem::with_id(app, "zakoncz", "Zakończ", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&otworz, &widzet, &test, &separator, &zakoncz])?;

    let mut budowniczy = TrayIconBuilder::with_id("robinwood")
        .tooltip("ROBINWOOD")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, zdarzenie| match zdarzenie.id().as_ref() {
            "otworz" => pokaz_okno(app, OKNO_GLOWNE),
            "widzet" => przelacz_okno(app, OKNO_WIDZET),
            "powiadomienie" => {
                if let Err(e) = crate::powiadomienia::testowe(app) {
                    tracing::warn!("Powiadomienie testowe nie powiodło się: {e}");
                }
            }
            // Najpierw interfejs zapisuje niezapisane wiersze, potem wychodzi (z zapasem czasu).
            "zakoncz" => crate::po_zapisie_szkicow(
                app,
                "robinwood://przed-zakonczeniem",
                std::time::Duration::from_secs(5),
                |a| a.exit(0),
            ),
            _ => {}
        })
        .on_tray_icon_event(|ikona, zdarzenie| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = zdarzenie
            {
                pokaz_okno(ikona.app_handle(), OKNO_GLOWNE);
            }
        });
    if let Some(ikona) = app.default_window_icon() {
        budowniczy = budowniczy.icon(ikona.clone());
    }
    budowniczy.build(app)?;
    Ok(())
}

pub fn pokaz_okno<R: Runtime>(app: &AppHandle<R>, etykieta: &str) {
    if let Some(okno) = app.get_webview_window(etykieta) {
        let _ = okno.unminimize();
        let _ = okno.show();
        let _ = okno.set_focus();
    }
}

pub fn przelacz_okno<R: Runtime>(app: &AppHandle<R>, etykieta: &str) {
    if let Some(okno) = app.get_webview_window(etykieta) {
        if okno.is_visible().unwrap_or(false) {
            let _ = okno.hide();
        } else {
            let _ = okno.show();
        }
    }
}
