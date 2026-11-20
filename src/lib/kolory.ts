// Kolory typów zadań kalendarza. Wartości tymczasowe – docelowe przyjdą z projektu w Figmie
// (tokeny projektowe). Nazwy kluczy odpowiadają kolumnie `zadania_kalendarza.typ` w bazie.

export type TypZadania =
  | "montaz"
  | "warsztat"
  | "zamowienie"
  | "rysunki"
  | "pomiar"
  | "platnosc"
  | "klient"
  | "inne";

export const TYPY_ZADAN: Record<TypZadania, { nazwa: string; kolor: string }> = {
  montaz: { nazwa: "Montaż", kolor: "#2e7d32" },
  warsztat: { nazwa: "Składanie na warsztacie", kolor: "#6d4c41" },
  zamowienie: { nazwa: "Zamówienie materiałów", kolor: "#1565c0" },
  rysunki: { nazwa: "Rysunki wykonawcze", kolor: "#6a1b9a" },
  pomiar: { nazwa: "Pomiar", kolor: "#ef6c00" },
  platnosc: { nazwa: "Płatność", kolor: "#00838f" },
  klient: { nazwa: "Termin po stronie klienta", kolor: "#c2185b" },
  inne: { nazwa: "Inne", kolor: "#757575" },
};
