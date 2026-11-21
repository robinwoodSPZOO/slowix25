// Daty kalendarza jako tekst ISO „RRRR-MM-DD” (jak w API). Arytmetyka na UTC, żeby zmiana czasu
// (marzec/październik) nie przesuwała dni. Czyste funkcje – bez sięgania do zegara (poza `dzisIso`).

import { DZIEN, liczba } from "../../lib/odmiana";

export const NAZWY_MIESIECY = [
  "styczeń", "luty", "marzec", "kwiecień", "maj", "czerwiec",
  "lipiec", "sierpień", "wrzesień", "październik", "listopad", "grudzień",
];
export const NAZWY_MIESIECY_DOPELNIACZ = [
  "stycznia", "lutego", "marca", "kwietnia", "maja", "czerwca",
  "lipca", "sierpnia", "września", "października", "listopada", "grudnia",
];
/** Poniedziałek = 0. */
export const DNI_TYGODNIA = ["poniedziałek", "wtorek", "środa", "czwartek", "piątek", "sobota", "niedziela"];
export const DNI_TYGODNIA_KROTKO = ["pon.", "wt.", "śr.", "czw.", "pt.", "sob.", "niedz."];

function naDate(iso: string): Date {
  const [r, m, d] = iso.split("-").map(Number);
  return new Date(Date.UTC(r ?? 1970, (m ?? 1) - 1, d ?? 1));
}

function naIso(d: Date): string {
  return d.toISOString().slice(0, 10);
}

/** Dzisiejsza data lokalna komputera jako ISO. */
export function dzisIso(teraz: Date = new Date()): string {
  const r = teraz.getFullYear();
  const m = String(teraz.getMonth() + 1).padStart(2, "0");
  const d = String(teraz.getDate()).padStart(2, "0");
  return `${r}-${m}-${d}`;
}

export function dodajDni(iso: string, n: number): string {
  const d = naDate(iso);
  d.setUTCDate(d.getUTCDate() + n);
  return naIso(d);
}

/** Liczba dni od `a` do `b` (b − a). */
export function roznicaDni(a: string, b: string): number {
  return Math.round((naDate(b).getTime() - naDate(a).getTime()) / 86_400_000);
}

/** Dzień tygodnia: poniedziałek = 0 … niedziela = 6. */
export function dzienTygodnia(iso: string): number {
  return (naDate(iso).getUTCDay() + 6) % 7;
}

export function poczatekTygodnia(iso: string): string {
  return dodajDni(iso, -dzienTygodnia(iso));
}

export function rokMiesiac(iso: string): { rok: number; miesiac: number } {
  const [r, m] = iso.split("-").map(Number);
  return { rok: r ?? 1970, miesiac: (m ?? 1) - 1 };
}

export function pierwszyDzienMiesiaca(iso: string): string {
  return `${iso.slice(0, 7)}-01`;
}

/** Przesunięcie o `n` miesięcy (pierwszy dzień miesiąca wynikowego). */
export function dodajMiesiace(iso: string, n: number): string {
  const { rok, miesiac } = rokMiesiac(iso);
  const d = new Date(Date.UTC(rok, miesiac + n, 1));
  return naIso(d);
}

/** Zakres siatki miesiąca: pełne tygodnie (pon.–niedz.) obejmujące cały miesiąc. */
export function siatkaMiesiaca(iso: string): { od: string; do: string; tygodnie: string[] } {
  const pierwszy = pierwszyDzienMiesiaca(iso);
  const ostatni = dodajDni(dodajMiesiace(pierwszy, 1), -1);
  const od = poczatekTygodnia(pierwszy);
  const doDnia = dodajDni(poczatekTygodnia(ostatni), 6);
  const tygodnie: string[] = [];
  for (let t = od; t <= doDnia; t = dodajDni(t, 7)) tygodnie.push(t);
  return { od, do: doDnia, tygodnie };
}

export function czyWeekend(iso: string): boolean {
  return dzienTygodnia(iso) >= 5;
}

/** „21.11” */
export function krotkaData(iso: string | null | undefined): string {
  if (!iso) return "—";
  return `${iso.slice(8, 10)}.${iso.slice(5, 7)}`;
}

/** „sobota, 21 listopada 2026” */
export function pelnaData(iso: string): string {
  const { rok, miesiac } = rokMiesiac(iso);
  return `${DNI_TYGODNIA[dzienTygodnia(iso)]}, ${Number(iso.slice(8, 10))} ${NAZWY_MIESIECY_DOPELNIACZ[miesiac]} ${rok}`;
}

/** „listopad 2026” */
export function nazwaMiesiaca(iso: string): string {
  const { rok, miesiac } = rokMiesiac(iso);
  return `${NAZWY_MIESIECY[miesiac]} ${rok}`;
}

/** „16–22 listopada 2026” / „30 listopada – 6 grudnia 2026” */
export function nazwaTygodnia(poczatek: string): string {
  const koniec = dodajDni(poczatek, 6);
  const a = rokMiesiac(poczatek);
  const b = rokMiesiac(koniec);
  const dA = Number(poczatek.slice(8, 10));
  const dB = Number(koniec.slice(8, 10));
  if (a.miesiac === b.miesiac) return `${dA}–${dB} ${NAZWY_MIESIECY_DOPELNIACZ[b.miesiac]} ${b.rok}`;
  const rokA = a.rok !== b.rok ? ` ${a.rok}` : "";
  return `${dA} ${NAZWY_MIESIECY_DOPELNIACZ[a.miesiac]}${rokA} – ${dB} ${NAZWY_MIESIECY_DOPELNIACZ[b.miesiac]} ${b.rok}`;
}

/** „za 3 dni”, „dziś”, „jutro”, „wczoraj”, „2 dni temu” – dni kalendarzowe. */
export function wzgledny(dni: number | null | undefined): string {
  if (dni === null || dni === undefined) return "";
  if (dni === 0) return "dziś";
  if (dni === 1) return "jutro";
  if (dni === -1) return "wczoraj";
  const n = Math.abs(dni);
  return dni > 0 ? `za ${liczba(n, DZIEN)}` : `${liczba(n, DZIEN)} po terminie`;
}

/** Lata obejmowane przez zakres (do pobrania świąt). */
export function lataZakresu(od: string, doDnia: string): number[] {
  const a = rokMiesiac(od).rok;
  const b = rokMiesiac(doDnia).rok;
  const lata: number[] = [];
  for (let r = a; r <= b; r++) lata.push(r);
  return lata;
}

export function poprawnaData(iso: string): boolean {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(iso)) return false;
  const d = naDate(iso);
  return naIso(d) === iso;
}
