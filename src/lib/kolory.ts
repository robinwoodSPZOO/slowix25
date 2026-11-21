// Typy zadań kalendarza i ich kolory. Kolory pochodzą WYŁĄCZNIE z tokenów `--zadanie-*`
// (src/ui/tokeny.css) – zarezerwowanych dla typów zadań. Nazwy kluczy odpowiadają kolumnie
// `zadania_kalendarza.typ` w bazie (silnik terminów: docs/ARCHITEKTURA.md, rozdz. 7).

export type TypZadania = "montaz" | "warsztat" | "zamowienie" | "rysunki" | "pomiar" | "platnosc" | "klient" | "inne";

export const TYPY_ZADAN: Record<TypZadania, { nazwa: string; kolor: string }> = {
  montaz: { nazwa: "Montaż", kolor: "var(--zadanie-montaz)" },
  warsztat: { nazwa: "Składanie na warsztacie", kolor: "var(--zadanie-warsztat)" },
  zamowienie: { nazwa: "Zamówienie materiałów", kolor: "var(--zadanie-zamowienie)" },
  rysunki: { nazwa: "Rysunki wykonawcze", kolor: "var(--zadanie-rysunki)" },
  pomiar: { nazwa: "Pomiar", kolor: "var(--zadanie-pomiar)" },
  platnosc: { nazwa: "Płatność", kolor: "var(--zadanie-platnosc)" },
  klient: { nazwa: "Termin po stronie klienta", kolor: "var(--zadanie-klient)" },
  inne: { nazwa: "Inne", kolor: "var(--zadanie-inne)" },
};
