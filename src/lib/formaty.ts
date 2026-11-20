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

/** "1 234,56" / "1234.56" / "1234" → 123456 groszy; null dla nieprawidłowego tekstu. */
export function parsujKwote(tekst: string): number | null {
  const oczyszczony = tekst.replace(/\s|zł/g, "").replace(",", "."); // \s obejmuje też twardą spację
  if (!/^-?\d+(\.\d{1,2})?$/.test(oczyszczony)) return null;
  const [calosc = "0", ulamek = ""] = oczyszczony.split(".");
  const znak = calosc.startsWith("-") ? -1 : 1;
  const zl = Math.abs(parseInt(calosc, 10));
  const gr = parseInt(ulamek.padEnd(2, "0") || "0", 10);
  return znak * (zl * 100 + gr);
}

/** "2026-11-23" → "23.11.2026" (jeden format daty w całym programie). */
export function formatujDate(iso: string | null | undefined): string {
  if (!iso) return "—";
  const m = /^(\d{4})-(\d{2})-(\d{2})/.exec(iso);
  if (!m) return iso;
  return `${m[3]}.${m[2]}.${m[1]}`;
}
