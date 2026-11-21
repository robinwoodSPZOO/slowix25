// Odmiana liczebników po polsku – JEDNA funkcja dla wszystkich tekstów z liczbą („1 zadanie”,
// „2 zadania”, „5 zadań”, „22 zadania”, „112 zadań”). Używaj jej zamiast pisać „n zadań”.
//
//   liczba(3, ZADANIE)            → "3 zadania"
//   odmiana(5, ZADANIE)           → "zadań"   (sama forma, bez liczby)
//   liczba(1, ["dzień", "dni", "dni"]) → "1 dzień"
//
// Formy: [mianownik l. poj. (1), mianownik l. mn. (2–4, 22–24…), dopełniacz l. mn. (0, 5–21, 25…)].

export type Formy = readonly [jeden: string, kilka: string, wiele: string];

export function odmiana(n: number, [jeden, kilka, wiele]: Formy): string {
  const a = Math.abs(Math.trunc(n));
  if (a === 1 && Math.abs(n) === 1) return jeden;
  if (!Number.isInteger(n)) return kilka; // „2,5 dnia” – ułamki: forma dopełniacza l. poj. rzadko potrzebna
  const d = a % 10;
  const s = a % 100;
  if (d >= 2 && d <= 4 && !(s >= 12 && s <= 14)) return kilka;
  return wiele;
}

/** „3 zadania” – liczba + odmieniony rzeczownik. */
export function liczba(n: number, formy: Formy): string {
  return `${n} ${odmiana(n, formy)}`;
}

export const ZADANIE: Formy = ["zadanie", "zadania", "zadań"];
export const DZIEN: Formy = ["dzień", "dni", "dni"];
export const DNI_ROBOCZYCH: Formy = ["dzień roboczy", "dni robocze", "dni roboczych"];
export const WIERSZ: Formy = ["wiersz", "wiersze", "wierszy"];
export const POZYCJA: Formy = ["pozycja", "pozycje", "pozycji"];
export const ZLECENIE: Formy = ["zlecenie", "zlecenia", "zleceń"];
export const KATEGORIA: Formy = ["kategoria", "kategorie", "kategorii"];
export const DOSTAWCA: Formy = ["dostawca", "dostawcy", "dostawców"];
export const PARA: Formy = ["para", "pary", "par"];
export const ZAMOWIENIE: Formy = ["zamówienie", "zamówienia", "zamówień"];
export const MONTAZ: Formy = ["montaż", "montaże", "montaży"];
export const TERMIN: Formy = ["termin", "terminy", "terminów"];
export const CZAS_DOSTAWY: Formy = ["czas dostawy", "czasy dostaw", "czasów dostaw"];
export const ZMIANA: Formy = ["zmiana", "zmiany", "zmian"];
export const DOSTAWA: Formy = ["dostawa", "dostawy", "dostaw"];
export const INNE: Formy = ["inne", "inne", "innych"];
