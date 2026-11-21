// Czysta logika formularza umowy (bez Reacta) – testowana w logika.test.ts.

import type { DaneUmowy, Umowa } from "../../api/typyUmow";

/** Pola formularza (kolejność jak na serwerze – `umowy::POLA_UMOWY`). */
export const POLA_UMOWY: (keyof DaneUmowy)[] = [
  "numer",
  "miejscowosc",
  "data_zawarcia",
  "klient_id",
  "wartosc_brutto_gr",
  "stawka_vat",
  "zadatek_gr",
  "termin_zadatku",
  "wariant_platnosci",
  "kwota_przed_montazem_gr",
  "montaz_do",
  "dni_na_brakujace_informacje",
  "vat_rodzaj_nieruchomosci",
  "vat_powierzchnia_m2",
  "vat_podstawa_powierzchni",
  "zalaczniki",
  "czas_warsztat_dni",
  "czas_montazu_dni",
  "uwagi",
  "montaz_ulica",
  "montaz_kod",
  "montaz_miasto",
  "montaz_pietro",
  "montaz_kod_domofonu",
  "montaz_parking",
  "montaz_winda_wniesienie",
];

/** Pola wewnętrzne – można je zmieniać także po podpisaniu (nie trafiają do dokumentu). */
export const POLA_WEWNETRZNE: (keyof DaneUmowy)[] = ["czas_warsztat_dni", "czas_montazu_dni", "uwagi"];

export const NAZWY_POL: Record<string, string> = {
  numer: "numer umowy",
  miejscowosc: "miejscowość",
  data_zawarcia: "data umowy",
  klient_id: "klient",
  wartosc_brutto_gr: "wartość brutto",
  stawka_vat: "stawka VAT",
  zadatek_gr: "zadatek",
  termin_zadatku: "termin zadatku",
  wariant_platnosci: "wariant płatności",
  kwota_przed_montazem_gr: "kwota przed montażem",
  montaz_do: "termin rozpoczęcia montażu",
  dni_na_brakujace_informacje: "dni na brakujące informacje",
  vat_rodzaj_nieruchomosci: "rodzaj nieruchomości",
  vat_powierzchnia_m2: "powierzchnia użytkowa",
  vat_podstawa_powierzchni: "podstawa określenia powierzchni",
  zalaczniki: "załączniki",
  czas_warsztat_dni: "czas warsztatu",
  czas_montazu_dni: "czas montażu",
  uwagi: "uwagi",
  montaz_ulica: "adres montażu",
  montaz_kod: "kod pocztowy",
  montaz_miasto: "miejscowość montażu",
  montaz_pietro: "piętro",
  montaz_kod_domofonu: "kod domofonu",
  montaz_parking: "parking",
  montaz_winda_wniesienie: "winda / wniesienie",
  data: "data aneksu",
  opis_zmian: "opis zmian",
  zmiana_ceny_gr: "zmiana kwoty",
  nowy_termin_montazu: "nowy termin montażu",
};

export const nazwaPolaUmowy = (p: string) => NAZWY_POL[p] ?? p;

/** Dane formularza z umowy (tylko pola edytowalne). */
export function daneUmowy(u: Umowa): DaneUmowy {
  return Object.fromEntries(POLA_UMOWY.map((p) => [p, u[p] ?? null])) as unknown as DaneUmowy;
}

/** Czy pole jest zablokowane (umowa poza szkicem – poza polami wewnętrznymi). */
export function poleZablokowane(u: Umowa, pole: keyof DaneUmowy): boolean {
  return u.tylko_do_odczytu && !POLA_WEWNETRZNE.includes(pole);
}

/** Liczba całkowita z pola tekstowego (puste → null, niepoprawne → undefined). */
export function parsujCalkowita(t: string, min = 0, max = 365): number | null | undefined {
  const s = t.trim();
  if (!s) return null;
  if (!/^\d+$/.test(s)) return undefined;
  const n = Number(s);
  return n < min || n > max ? undefined : n;
}

/** Liczba dziesiętna (powierzchnia) z przecinkiem lub kropką. */
export function parsujDziesietna(t: string, max = 100_000): number | null | undefined {
  const s = t.trim().replace(",", ".");
  if (!s) return null;
  if (!/^\d+(\.\d{1,2})?$/.test(s)) return undefined;
  const n = Number(s);
  return n > max ? undefined : n;
}

/** Ostrzeżenie o limicie powierzchni dla stawki 8% (150 m² lokal, 300 m² dom). */
export function ostrzezeniePowierzchni(rodzaj: string | null, m2: number | null): string | null {
  if (m2 === null) return null;
  const limit = rodzaj === "lokal_mieszkalny" ? 150 : rodzaj === "dom_jednorodzinny" ? 300 : null;
  if (limit === null || m2 <= limit) return null;
  return `Powyżej ${limit} m² – stawka 8% tylko proporcjonalnie do limitu, nadwyżka 23%`;
}

/** Podgląd numeru wg formatu z ustawień (jak `umowy::numer_z_formatu`). */
export function numerZFormatu(format: string, rok: number, nr: number): string {
  const pad = (n: number, d: number) => String(n).padStart(d, "0");
  return format
    .replaceAll("{NNN}", pad(nr, 3))
    .replaceAll("{NN}", pad(nr, 2))
    .replaceAll("{N}", String(nr))
    .replaceAll("{RRRR}", pad(rok, 4))
    .replaceAll("{RR}", pad(rok % 100, 2));
}

/** Rok bieżący i kilka poprzednich – do filtra listy umów. */
export function lataDoFiltra(dzis = new Date(), ile = 5): number[] {
  const r = dzis.getFullYear();
  return Array.from({ length: ile }, (_, i) => r + 1 - i);
}
