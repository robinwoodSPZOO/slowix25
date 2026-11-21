//! Kwoty słownie po polsku (umowy, aneksy, protokoły): „dwadzieścia dwa złote 50/100”.
//!
//! Odmiana: 1 → złoty / tysiąc / milion, końcówka 2–4 (poza 12–14) → złote / tysiące / miliony,
//! pozostałe → złotych / tysięcy / milionów. Tysiąc bez „jeden” („tysiąc złotych”), milion
//! i miliard z „jeden” („jeden milion złotych”) – tak jak w dotychczasowych umowach.
//! Grosze zapisujemy cyframi jako ułamek „xx/100”.

const JEDNOSCI: [&str; 10] = [
    "",
    "jeden",
    "dwa",
    "trzy",
    "cztery",
    "pięć",
    "sześć",
    "siedem",
    "osiem",
    "dziewięć",
];
const NASTKI: [&str; 10] = [
    "dziesięć",
    "jedenaście",
    "dwanaście",
    "trzynaście",
    "czternaście",
    "piętnaście",
    "szesnaście",
    "siedemnaście",
    "osiemnaście",
    "dziewiętnaście",
];
const DZIESIATKI: [&str; 10] = [
    "",
    "",
    "dwadzieścia",
    "trzydzieści",
    "czterdzieści",
    "pięćdziesiąt",
    "sześćdziesiąt",
    "siedemdziesiąt",
    "osiemdziesiąt",
    "dziewięćdziesiąt",
];
const SETKI: [&str; 10] = [
    "",
    "sto",
    "dwieście",
    "trzysta",
    "czterysta",
    "pięćset",
    "sześćset",
    "siedemset",
    "osiemset",
    "dziewięćset",
];

/// Forma rzeczownika po liczebniku: `[1, 2–4, 5+]`.
pub fn forma(n: u64, formy: [&str; 3]) -> &str {
    if n == 1 {
        return formy[0];
    }
    let d = n % 10;
    let s = n % 100;
    if (2..=4).contains(&d) && !(12..=14).contains(&s) {
        formy[1]
    } else {
        formy[2]
    }
}

/// 1–999 słownie (0 → pusty napis).
fn do_tysiaca(n: u64) -> String {
    let mut czesci: Vec<&str> = Vec::new();
    let s = (n / 100) as usize;
    let r = n % 100;
    if s > 0 {
        czesci.push(SETKI[s]);
    }
    if (10..20).contains(&r) {
        czesci.push(NASTKI[(r - 10) as usize]);
    } else {
        let d = (r / 10) as usize;
        let j = (r % 10) as usize;
        if d > 0 {
            czesci.push(DZIESIATKI[d]);
        }
        if j > 0 {
            czesci.push(JEDNOSCI[j]);
        }
    }
    czesci.join(" ")
}

/// Liczba całkowita słownie: 0 → „zero”, 1000 → „tysiąc”, 2 000 000 → „dwa miliony”.
pub fn liczba(n: u64) -> String {
    if n == 0 {
        return "zero".into();
    }
    const GRUPY: [[&str; 3]; 6] = [
        ["", "", ""],
        ["tysiąc", "tysiące", "tysięcy"],
        ["milion", "miliony", "milionów"],
        ["miliard", "miliardy", "miliardów"],
        ["bilion", "biliony", "bilionów"],
        ["biliard", "biliardy", "biliardów"],
    ];
    let mut grupy = Vec::new();
    let mut x = n;
    while x > 0 {
        grupy.push(x % 1000);
        x /= 1000;
    }
    let mut wynik: Vec<String> = Vec::new();
    for (i, &g) in grupy.iter().enumerate().rev() {
        if g == 0 {
            continue;
        }
        if i == 0 {
            wynik.push(do_tysiaca(g));
        } else if i == 1 && g == 1 {
            wynik.push("tysiąc".into());
        } else {
            let nazwa = GRUPY.get(i).map(|f| forma(g, *f)).unwrap_or("");
            wynik.push(format!("{} {}", do_tysiaca(g), nazwa));
        }
    }
    wynik.join(" ")
}

/// Kwota w groszach słownie: 2250 → „dwadzieścia dwa złote 50/100”.
/// Kwota ujemna – z przedrostkiem „minus”.
pub fn kwota(grosze: i64) -> String {
    let znak = if grosze < 0 { "minus " } else { "" };
    let g = grosze.unsigned_abs();
    let zl = g / 100;
    let gr = g % 100;
    format!(
        "{znak}{} {} {:02}/100",
        liczba(zl),
        forma(zl, ["złoty", "złote", "złotych"]),
        gr
    )
}

#[cfg(test)]
mod testy {
    use super::*;

    fn zl(n: i64) -> String {
        kwota(n * 100)
    }

    #[test]
    fn male_liczby() {
        assert_eq!(zl(0), "zero złotych 00/100");
        assert_eq!(zl(1), "jeden złoty 00/100");
        assert_eq!(zl(2), "dwa złote 00/100");
        assert_eq!(zl(4), "cztery złote 00/100");
        assert_eq!(zl(5), "pięć złotych 00/100");
        assert_eq!(zl(10), "dziesięć złotych 00/100");
        assert_eq!(zl(11), "jedenaście złotych 00/100");
        assert_eq!(zl(12), "dwanaście złotych 00/100");
        assert_eq!(zl(14), "czternaście złotych 00/100");
        assert_eq!(zl(19), "dziewiętnaście złotych 00/100");
        assert_eq!(zl(20), "dwadzieścia złotych 00/100");
        assert_eq!(zl(21), "dwadzieścia jeden złotych 00/100");
        assert_eq!(zl(22), "dwadzieścia dwa złote 00/100");
        assert_eq!(zl(24), "dwadzieścia cztery złote 00/100");
        assert_eq!(zl(25), "dwadzieścia pięć złotych 00/100");
        assert_eq!(zl(99), "dziewięćdziesiąt dziewięć złotych 00/100");
    }

    #[test]
    fn setki() {
        assert_eq!(zl(100), "sto złotych 00/100");
        assert_eq!(zl(101), "sto jeden złotych 00/100");
        assert_eq!(zl(102), "sto dwa złote 00/100");
        assert_eq!(zl(112), "sto dwanaście złotych 00/100");
        assert_eq!(zl(113), "sto trzynaście złotych 00/100");
        assert_eq!(zl(122), "sto dwadzieścia dwa złote 00/100");
        assert_eq!(zl(200), "dwieście złotych 00/100");
        assert_eq!(zl(345), "trzysta czterdzieści pięć złotych 00/100");
        assert_eq!(
            zl(999),
            "dziewięćset dziewięćdziesiąt dziewięć złotych 00/100"
        );
    }

    #[test]
    fn tysiace() {
        assert_eq!(zl(1000), "tysiąc złotych 00/100");
        assert_eq!(zl(1001), "tysiąc jeden złotych 00/100");
        assert_eq!(zl(1002), "tysiąc dwa złote 00/100");
        assert_eq!(zl(2000), "dwa tysiące złotych 00/100");
        assert_eq!(zl(4000), "cztery tysiące złotych 00/100");
        assert_eq!(zl(5000), "pięć tysięcy złotych 00/100");
        assert_eq!(zl(12_000), "dwanaście tysięcy złotych 00/100");
        assert_eq!(zl(21_000), "dwadzieścia jeden tysięcy złotych 00/100");
        assert_eq!(zl(22_000), "dwadzieścia dwa tysiące złotych 00/100");
        assert_eq!(zl(25_000), "dwadzieścia pięć tysięcy złotych 00/100");
        assert_eq!(zl(112_000), "sto dwanaście tysięcy złotych 00/100");
        assert_eq!(zl(122_000), "sto dwadzieścia dwa tysiące złotych 00/100");
        assert_eq!(zl(125_000), "sto dwadzieścia pięć tysięcy złotych 00/100");
        assert_eq!(
            zl(48_352),
            "czterdzieści osiem tysięcy trzysta pięćdziesiąt dwa złote 00/100"
        );
    }

    #[test]
    fn miliony() {
        assert_eq!(zl(1_000_000), "jeden milion złotych 00/100");
        assert_eq!(zl(2_000_000), "dwa miliony złotych 00/100");
        assert_eq!(zl(5_000_000), "pięć milionów złotych 00/100");
        assert_eq!(zl(12_000_000), "dwanaście milionów złotych 00/100");
        assert_eq!(zl(22_000_000), "dwadzieścia dwa miliony złotych 00/100");
        assert_eq!(zl(1_001_000), "jeden milion tysiąc złotych 00/100");
        assert_eq!(
            zl(1_234_567),
            "jeden milion dwieście trzydzieści cztery tysiące pięćset sześćdziesiąt siedem złotych 00/100"
        );
        assert_eq!(zl(1_000_000_000), "jeden miliard złotych 00/100");
        assert_eq!(zl(3_000_000_000), "trzy miliardy złotych 00/100");
    }

    #[test]
    fn grosze() {
        assert_eq!(kwota(1), "zero złotych 01/100");
        assert_eq!(kwota(99), "zero złotych 99/100");
        assert_eq!(kwota(150), "jeden złoty 50/100");
        assert_eq!(kwota(2_250), "dwadzieścia dwa złote 50/100");
        assert_eq!(
            kwota(1_234_505),
            "dwanaście tysięcy trzysta czterdzieści pięć złotych 05/100"
        );
        assert_eq!(kwota(-500), "minus pięć złotych 00/100");
    }

    #[test]
    fn formy() {
        assert_eq!(forma(1, ["a", "b", "c"]), "a");
        assert_eq!(forma(3, ["a", "b", "c"]), "b");
        assert_eq!(forma(13, ["a", "b", "c"]), "c");
        assert_eq!(forma(23, ["a", "b", "c"]), "b");
        assert_eq!(forma(111, ["a", "b", "c"]), "c");
        assert_eq!(forma(0, ["a", "b", "c"]), "c");
    }
}
