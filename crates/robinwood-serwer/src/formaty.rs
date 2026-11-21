//! Formatowanie wartości do dokumentów i opisów różnic (polskie konwencje, jak `src/lib/formaty.ts`).

/// 123456 → "1 234,56 zł" (separator tysięcy: twarda spacja).
pub fn kwota(grosze: i64) -> String {
    let znak = if grosze < 0 { "-" } else { "" };
    let g = grosze.unsigned_abs();
    let zl = (g / 100).to_string();
    let mut z_separatorami = String::new();
    for (i, c) in zl.chars().enumerate() {
        if i > 0 && (zl.len() - i).is_multiple_of(3) {
            z_separatorami.push('\u{a0}');
        }
        z_separatorami.push(c);
    }
    format!("{znak}{z_separatorami},{:02}\u{a0}zł", g % 100)
}

pub fn kwota_opc(grosze: Option<i64>) -> String {
    grosze.map(kwota).unwrap_or_else(|| "—".into())
}

/// 1250 (setne procenta) → "12,5%".
pub fn procent_bp(bp: i64) -> String {
    let calosc = bp / 100;
    let reszta = (bp % 100).abs();
    if reszta == 0 {
        format!("{calosc}%")
    } else if reszta % 10 == 0 {
        format!("{calosc},{}%", reszta / 10)
    } else {
        format!("{calosc},{reszta:02}%")
    }
}

/// "2026-11-23" → "23.11.2026".
pub fn data(iso: &str) -> String {
    let c: Vec<&str> = iso.get(..10).unwrap_or(iso).split('-').collect();
    if c.len() == 3 {
        format!("{}.{}.{}", c[2], c[1], c[0])
    } else {
        iso.to_string()
    }
}

/// Ilość bez zbędnych zer: 2.0 → "2", 2.5 → "2,5".
pub fn ilosc(i: f64) -> String {
    let s = format!("{:.3}", i);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    s.replace('.', ",")
}

#[cfg(test)]
mod testy {
    use super::*;

    #[test]
    fn formaty() {
        assert_eq!(kwota(123_456), "1\u{a0}234,56\u{a0}zł");
        assert_eq!(kwota(5), "0,05\u{a0}zł");
        assert_eq!(kwota(-100_000_000), "-1\u{a0}000\u{a0}000,00\u{a0}zł");
        assert_eq!(procent_bp(500), "5%");
        assert_eq!(procent_bp(1250), "12,5%");
        assert_eq!(procent_bp(1205), "12,05%");
        assert_eq!(data("2026-11-23"), "23.11.2026");
        assert_eq!(ilosc(2.0), "2");
        assert_eq!(ilosc(2.5), "2,5");
        assert_eq!(ilosc(0.125), "0,125");
    }
}
