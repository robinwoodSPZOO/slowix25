//! Porównywanie nazw (kategorie materiałów, dostawcy) niezależnie od wielkości liter – także
//! polskich. `COLLATE NOCASE` w SQLite zmienia wielkość liter tylko w ASCII („PŁYTA” ≠ „płyta”),
//! dlatego nazwy porównujemy w Rust po [`klucz`]: przycięcie, zwinięcie wielokrotnych odstępów
//! i małe litery Unicode. Zapisujemy postać [`oczysc`] (bez zmiany wielkości liter).

use sqlx::SqliteConnection;

use crate::bledy::Wynik;

/// Największa długość nazwy dostawcy i kategorii (znaki).
pub const MAKS_NAZWY: usize = 200;

/// Nazwa do zapisu: przycięta, wielokrotne odstępy (także tabulatory, twarde spacje) → jedna spacja.
pub fn oczysc(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Klucz porównania: [`oczysc`] + małe litery Unicode.
pub fn klucz(s: &str) -> String {
    oczysc(s).to_lowercase()
}

/// Id rekordu tabeli `dostawcy` albo `kategorie_materialow` o nazwie równej (po [`klucz`]) podanej;
/// `pomin_id` – rekord pomijany (zmiana nazwy samego siebie).
pub async fn znajdz(
    c: &mut SqliteConnection,
    tabela: Tabela,
    nazwa: &str,
    pomin_id: Option<i64>,
) -> Wynik<Option<i64>> {
    let k = klucz(nazwa);
    let sql = match tabela {
        Tabela::Dostawcy => "SELECT id, nazwa FROM dostawcy ORDER BY id",
        Tabela::Kategorie => "SELECT id, nazwa FROM kategorie_materialow ORDER BY id",
    };
    let wiersze: Vec<(i64, String)> = sqlx::query_as(sql).fetch_all(&mut *c).await?;
    Ok(wiersze
        .into_iter()
        .find(|(id, n)| Some(*id) != pomin_id && klucz(n) == k)
        .map(|(id, _)| id))
}

#[derive(Debug, Clone, Copy)]
pub enum Tabela {
    Dostawcy,
    Kategorie,
}

#[cfg(test)]
mod testy {
    use super::*;

    #[test]
    fn polskie_litery_i_odstepy() {
        assert_eq!(klucz("PŁYTA MEBLOWA"), klucz("Płyta meblowa"));
        assert_eq!(klucz("  Płyta \t  meblowa\u{a0} "), "płyta meblowa");
        assert_eq!(klucz("ŻÓŁĆ ĘŚĄŹŃ"), "żółć ęśąźń");
        assert_ne!(klucz("Płyta"), klucz("Plyta"));
        assert_eq!(oczysc("  Blum   Polska "), "Blum Polska");
    }
}
