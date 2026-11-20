//! Otwieranie bazy SQLite i uruchamianie migracji.

use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::SqlitePool;

/// Migracje z katalogu `migrations/` wkompilowane w program.
pub static MIGRACJE: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

/// Otwiera (lub tworzy) plik bazy i stosuje brakujące migracje.
pub async fn otworz(sciezka: &Path) -> anyhow::Result<SqlitePool> {
    if let Some(katalog) = sciezka.parent() {
        std::fs::create_dir_all(katalog)?;
    }
    let opcje = SqliteConnectOptions::new()
        .filename(sciezka)
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .busy_timeout(Duration::from_secs(5));
    let pula = SqlitePoolOptions::new()
        .max_connections(8)
        .connect_with(opcje)
        .await?;
    MIGRACJE.run(&pula).await?;
    Ok(pula)
}

/// Baza w pamięci (testy). Jedno połączenie, żeby wszystkie zapytania widziały te same dane.
pub async fn otworz_w_pamieci() -> anyhow::Result<SqlitePool> {
    let opcje = SqliteConnectOptions::from_str("sqlite::memory:")?.foreign_keys(true);
    let pula = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opcje)
        .await?;
    MIGRACJE.run(&pula).await?;
    Ok(pula)
}
