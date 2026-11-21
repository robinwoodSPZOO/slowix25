// Formatowanie wartości do wyświetlania (polskie konwencje).
// Kwoty w całym programie są przechowywane w groszach (liczby całkowite).

const formatKwoty = new Intl.NumberFormat("pl-PL", {
  minimumFractionDigits: 2,
  maximumFractionDigits: 2,
  useGrouping: true,
});

/** 123456 → "1 234,56 zł" (separator tysięcy: twarda spacja, jak w Intl pl-PL). */
export function formatujKwote(grosze: number | null | undefined): string {
  if (grosze === null || grosze === undefined) return "—";
  if (!Number.isInteger(grosze)) throw new Error("Kwota musi być liczbą groszy (całkowitą)");
  return `${formatKwoty.format(grosze / 100)} zł`;
}

/** Największa kwota przyjmowana przez serwer: 1 000 000 000,00 zł (jak `limity::MAKS_KWOTA_GR`). */
export const MAKS_KWOTA_GR = 100_000_000_000;
/** Największa ilość w wierszu specyfikacji (jak `limity::MAKS_ILOSC`). */
export const MAKS_ILOSC = 1_000_000;

/** Powód, dla którego tekst nie jest poprawną kwotą (null = poprawna). */
export function bladKwoty(tekst: string): string | null {
  const oczyszczony = tekst.replace(/\s|zł/g, "").replace(",", ".");
  if (!oczyszczony) return null;
  if (/^-?\d+\.\d{3,}$/.test(oczyszczony)) return "Najwyżej 2 miejsca po przecinku (grosze)";
  if (!/^-?\d+(\.\d{1,2})?$/.test(oczyszczony)) return "Wpisz kwotę, np. 1 234,50";
  const gr = Math.round(Math.abs(Number(oczyszczony)) * 100);
  if (gr > MAKS_KWOTA_GR) return "Najwyżej 1 000 000 000,00 zł";
  return null;
}

/** "1 234,56" / "1234.56" / "1234" → 123456 groszy; null dla nieprawidłowego tekstu lub kwoty ponad limit. */
export function parsujKwote(tekst: string): number | null {
  const oczyszczony = tekst.replace(/\s|zł/g, "").replace(",", "."); // \s obejmuje też twardą spację
  if (!/^-?\d+(\.\d{1,2})?$/.test(oczyszczony)) return null;
  const [calosc = "0", ulamek = ""] = oczyszczony.split(".");
  const znak = calosc.startsWith("-") ? -1 : 1;
  const zl = Math.abs(parseInt(calosc, 10));
  const gr = parseInt(ulamek.padEnd(2, "0") || "0", 10);
  const wynik = zl * 100 + gr;
  if (!Number.isSafeInteger(wynik) || wynik > MAKS_KWOTA_GR) return null;
  return znak * wynik;
}

/** "2026-11-23" → "23.11.2026" (jeden format daty w całym programie). */
export function formatujDate(iso: string | null | undefined): string {
  if (!iso) return "—";
  const m = /^(\d{4})-(\d{2})-(\d{2})/.exec(iso);
  if (!m) return iso;
  return `${m[3]}.${m[2]}.${m[1]}`;
}
