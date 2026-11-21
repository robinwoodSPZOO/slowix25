//! Okno-widżet: zapamiętana pozycja i widoczność (plik `widzet.json` w folderze konfiguracji
//! stanowiska), przywracanie przy starcie (tylko gdy pozycja leży na którymś z monitorów).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, PhysicalPosition, Runtime, WebviewWindow};

use crate::zasobnik::OKNO_WIDZET;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct StanWidzetu {
    pub x: Option<i32>,
    pub y: Option<i32>,
    /// Użytkownik schował widżet – nie pokazujemy go przy starcie.
    pub ukryty: bool,
}

fn sciezka<R: Runtime>(app: &AppHandle<R>) -> anyhow::Result<PathBuf> {
    Ok(app.path().app_config_dir()?.join("widzet.json"))
}

pub fn wczytaj<R: Runtime>(app: &AppHandle<R>) -> StanWidzetu {
    sciezka(app)
        .ok()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

pub fn zapisz<R: Runtime>(app: &AppHandle<R>, stan: &StanWidzetu) {
    let wynik = (|| -> anyhow::Result<()> {
        let p = sciezka(app)?;
        if let Some(k) = p.parent() {
            std::fs::create_dir_all(k)?;
        }
        std::fs::write(p, serde_json::to_string_pretty(stan)?)?;
        Ok(())
    })();
    if let Err(e) = wynik {
        tracing::warn!("Nie udało się zapisać stanu widżetu: {e}");
    }
}

/// Czy punkt (lewy górny róg + margines) leży na którymś z monitorów.
pub fn na_monitorze(monitory: &[(i32, i32, u32, u32)], x: i32, y: i32) -> bool {
    monitory.iter().any(|&(mx, my, w, h)| {
        let (px, py) = (x + 40, y + 20);
        px >= mx && py >= my && px < mx + w as i32 && py < my + h as i32
    })
}

fn monitory<R: Runtime>(okno: &WebviewWindow<R>) -> Vec<(i32, i32, u32, u32)> {
    okno.available_monitors()
        .unwrap_or_default()
        .iter()
        .map(|m| {
            (
                m.position().x,
                m.position().y,
                m.size().width,
                m.size().height,
            )
        })
        .collect()
}

/// Start programu: ustawia zapamiętaną pozycję i pokazuje widżet, jeśli nie był schowany.
pub fn przywroc<R: Runtime>(app: &AppHandle<R>) {
    let Some(okno) = app.get_webview_window(OKNO_WIDZET) else {
        return;
    };
    let stan = wczytaj(app);
    if let (Some(x), Some(y)) = (stan.x, stan.y) {
        if na_monitorze(&monitory(&okno), x, y) {
            let _ = okno.set_position(PhysicalPosition::new(x, y));
        }
    }
    if !stan.ukryty {
        let _ = okno.show();
    }
}

pub fn po_przesunieciu<R: Runtime>(app: &AppHandle<R>, poz: PhysicalPosition<i32>) {
    let mut stan = wczytaj(app);
    if stan.x == Some(poz.x) && stan.y == Some(poz.y) {
        return;
    }
    stan.x = Some(poz.x);
    stan.y = Some(poz.y);
    zapisz(app, &stan);
}

pub fn ustaw_widocznosc<R: Runtime>(app: &AppHandle<R>, widoczny: bool) {
    if let Some(okno) = app.get_webview_window(OKNO_WIDZET) {
        let _ = if widoczny { okno.show() } else { okno.hide() };
    }
    let mut stan = wczytaj(app);
    stan.ukryty = !widoczny;
    zapisz(app, &stan);
}

pub fn przelacz<R: Runtime>(app: &AppHandle<R>) {
    let widoczny = app
        .get_webview_window(OKNO_WIDZET)
        .and_then(|o| o.is_visible().ok())
        .unwrap_or(false);
    ustaw_widocznosc(app, !widoczny);
}

#[cfg(test)]
mod testy {
    use super::*;

    #[test]
    fn pozycja_na_monitorze() {
        let m = [(0, 0, 1920, 1080), (1920, 0, 1280, 1024)];
        assert!(na_monitorze(&m, 40, 40));
        assert!(na_monitorze(&m, 2500, 100));
        assert!(!na_monitorze(&m, -800, 100), "monitor odłączony");
        assert!(!na_monitorze(&m, 100, 1200));
    }

    #[test]
    fn stan_z_pliku_z_brakami() {
        let s: StanWidzetu = serde_json::from_str(r#"{"x": 10}"#).unwrap();
        assert_eq!(
            s,
            StanWidzetu {
                x: Some(10),
                y: None,
                ukryty: false
            }
        );
    }
}
