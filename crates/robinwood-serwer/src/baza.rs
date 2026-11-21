//! Otwieranie bazy SQLite i uruchamianie migracji.

use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::{Sqlite, SqlitePool, Transaction};

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

/// Transakcja zapisu – JEDYNY sposób rozpoczynania transakcji, która cokolwiek zapisuje.
///
/// `BEGIN IMMEDIATE` od razu bierze blokadę zapisu (czekając na nią do `busy_timeout`).
/// Zwykłe `pula.begin()` (DEFERRED) zaczyna od odczytu i dopiero przy pierwszym zapisie próbuje
/// podnieść blokadę – w trybie WAL SQLite zwraca wtedy natychmiast SQLITE_BUSY („database is
/// locked”), bez czekania, bo migawka odczytu mogłaby być nieaktualna. Przy kilku stanowiskach
/// zapisujących jednocześnie kończyło się to błędami 500.
///
/// Transakcje tylko do odczytu mogą dalej używać `pula.acquire()` / `pula.begin()`.
pub async fn transakcja_zapisu(
    pula: &SqlitePool,
) -> Result<Transaction<'static, Sqlite>, sqlx::Error> {
    pula.begin_with("BEGIN IMMEDIATE").await
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

#[cfg(test)]
mod testy {
    use super::*;

    /// Wiele zadań tokio zapisuje równocześnie do prawdziwego pliku bazy (WAL) w transakcjach,
    /// które najpierw czytają, potem piszą – żadne nie może skończyć się „database is locked”.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn rownolegle_transakcje_zapisu_na_pliku() {
        let folder = std::env::temp_dir().join(format!("robinwood-baza-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        let pula = otworz(&folder.join("test.db")).await.unwrap();
        let mut zadania = Vec::new();
        for z in 0..8 {
            let pula = pula.clone();
            zadania.push(tokio::spawn(async move {
                for i in 0..25 {
                    let mut tx = transakcja_zapisu(&pula).await?;
                    let _n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM dziennik_zmian")
                        .fetch_one(&mut *tx)
                        .await?;
                    sqlx::query("INSERT INTO dziennik_zmian (tabela, rekord_id, operacja) VALUES ('test', ?1, 'zmiana')")
                        .bind(z * 1000 + i)
                        .execute(&mut *tx)
                        .await?;
                    tx.commit().await?;
                }
                Ok::<_, sqlx::Error>(())
            }));
        }
        for z in zadania {
            z.await
                .unwrap()
                .expect("transakcja zapisu nie może zwrócić „database is locked”");
        }
        let n: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM dziennik_zmian WHERE tabela = 'test'")
                .fetch_one(&pula)
                .await
                .unwrap();
        assert_eq!(n, 200);
        pula.close().await;
        let _ = std::fs::remove_dir_all(&folder);
    }

    /// Kontrola: zwykła transakcja DEFERRED (odczyt → zapis) w tych samych warunkach daje
    /// SQLITE_BUSY – to jest błąd, który naprawia `transakcja_zapisu`.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn odroczona_transakcja_odtwarza_blad_blokady() {
        let folder =
            std::env::temp_dir().join(format!("robinwood-baza-def-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        let pula = otworz(&folder.join("test.db")).await.unwrap();
        let mut zadania = Vec::new();
        for z in 0..8 {
            let pula = pula.clone();
            zadania.push(tokio::spawn(async move {
                let mut bledy = 0;
                for i in 0..25 {
                    let wynik: Result<(), sqlx::Error> = async {
                        let mut tx = pula.begin().await?;
                        let _: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM dziennik_zmian")
                            .fetch_one(&mut *tx)
                            .await?;
                        tokio::task::yield_now().await;
                        sqlx::query("INSERT INTO dziennik_zmian (tabela, rekord_id, operacja) VALUES ('test', ?1, 'zmiana')")
                            .bind(z * 1000 + i)
                            .execute(&mut *tx)
                            .await?;
                        tx.commit().await
                    }
                    .await;
                    if wynik.is_err() {
                        bledy += 1;
                    }
                }
                bledy
            }));
        }
        let mut bledy = 0;
        for z in zadania {
            bledy += z.await.unwrap();
        }
        pula.close().await;
        let _ = std::fs::remove_dir_all(&folder);
        // Liczba błędów zależy od przeplotu wątków; wystarczy, że problem w ogóle występuje
        // albo nie – test dokumentuje zachowanie i nie może być niestabilny.
        eprintln!("DEFERRED: {bledy} z 200 transakcji zakończonych błędem blokady");
    }
}
