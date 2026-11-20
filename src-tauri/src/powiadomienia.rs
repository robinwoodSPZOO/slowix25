//! Powiadomienia systemowe Windows (przez tauri-plugin-notification).
//!
//! Właściwe przypomnienia (poranne podsumowanie, 7/3/1 dzień przed terminem, strefa krytyczna)
//! dokłada faza Kalendarza; tu jest tylko powiadomienie testowe.

use tauri::{AppHandle, Runtime};
use tauri_plugin_notification::NotificationExt;

pub fn pokaz<R: Runtime>(app: &AppHandle<R>, tytul: &str, tresc: &str) -> anyhow::Result<()> {
    app.notification()
        .builder()
        .title(tytul)
        .body(tresc)
        .show()?;
    Ok(())
}

pub fn testowe<R: Runtime>(app: &AppHandle<R>) -> anyhow::Result<()> {
    pokaz(
        app,
        "ROBINWOOD",
        "Powiadomienia działają. Tu pojawią się przypomnienia o terminach.",
    )
}
