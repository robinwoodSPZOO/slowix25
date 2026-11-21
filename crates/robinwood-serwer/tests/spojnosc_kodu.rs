//! Testy spójności kodu serwera (sprawdzają źródła, nie działanie).

use std::path::Path;

fn pliki_rs(katalog: &Path, wynik: &mut Vec<std::path::PathBuf>) {
    for e in std::fs::read_dir(katalog).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            pliki_rs(&p, wynik);
        } else if p.extension().is_some_and(|x| x == "rs") {
            wynik.push(p);
        }
    }
}

/// Każda transakcja zapisu musi startować przez `baza::transakcja_zapisu` (BEGIN IMMEDIATE).
/// `pula.begin()` / `Connection::begin` (DEFERRED) w trybie WAL kończy się „database is locked”
/// przy równoczesnych zapisach z kilku stanowisk. Punkt zapisu (`SAVEPOINT`) wewnątrz
/// istniejącej transakcji jest dozwolony – oznacz go komentarzem `// savepoint`.
#[test]
fn brak_odroczonych_transakcji_zapisu() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut pliki = Vec::new();
    pliki_rs(&src, &mut pliki);
    let mut naruszenia = Vec::new();
    for p in pliki {
        if p.ends_with("baza.rs") {
            continue;
        }
        let tresc = std::fs::read_to_string(&p).unwrap();
        for (nr, linia) in tresc.lines().enumerate() {
            let l = linia.trim();
            if l.starts_with("//") || l.contains("// savepoint") {
                continue;
            }
            if l.contains(".begin()")
                || l.contains("Connection::begin(")
                || l.contains("begin_with(")
            {
                naruszenia.push(format!("{}:{}: {}", p.display(), nr + 1, l));
            }
        }
    }
    assert!(
        naruszenia.is_empty(),
        "transakcje bez baza::transakcja_zapisu:\n{}",
        naruszenia.join("\n")
    );
}
