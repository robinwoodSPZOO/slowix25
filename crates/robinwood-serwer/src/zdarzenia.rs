//! Zdarzenia o zmianach danych, rozsyłane do stanowisk przez WebSocket.

use serde::Serialize;
use tokio::sync::broadcast;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Operacja {
    Dodanie,
    Zmiana,
    Usuniecie,
}

/// Zdarzenie: „w tabeli X zmienił się rekord Y”. Klient po otrzymaniu odświeża dane.
#[derive(Debug, Clone, Serialize)]
pub struct Zdarzenie {
    pub zasob: &'static str,
    pub id: i64,
    pub operacja: Operacja,
}

#[derive(Clone)]
pub struct Rozglosnia {
    tx: broadcast::Sender<Zdarzenie>,
}

impl Default for Rozglosnia {
    fn default() -> Self {
        let (tx, _) = broadcast::channel(256);
        Self { tx }
    }
}

impl Rozglosnia {
    pub fn wyslij(&self, z: Zdarzenie) {
        // Brak słuchaczy to nie błąd.
        let _ = self.tx.send(z);
    }

    pub fn subskrybuj(&self) -> broadcast::Receiver<Zdarzenie> {
        self.tx.subscribe()
    }
}
