// Logika widżetu (czysta, testy w logika.test.ts): grupowanie zadań na „zaległe”, dziś i 7 dni,
// pamięć ostatnio pobranych danych (tryb tylko do odczytu, gdy serwer jest niedostępny).

import type { ZadanieKalendarza } from "../api/typy";
import { dodajDni, DNI_TYGODNIA, roznicaDni } from "../moduly/kalendarz/daty";
import { wgPilnosci } from "../moduly/kalendarz/filtry";
import { zakresZadania } from "../moduly/kalendarz/uklad";

export interface DzienWidzetu {
  data: string;
  etykieta: string;
  zadania: ZadanieKalendarza[];
}

export interface UkladWidzetu {
  zalegle: ZadanieKalendarza[];
  dni: DzienWidzetu[];
}

function etykietaDnia(d: string, dzis: string): string {
  const n = roznicaDni(dzis, d);
  if (n === 0) return "Dziś";
  if (n === 1) return "Jutro";
  return `${DNI_TYGODNIA[(new Date(`${d}T00:00:00Z`).getUTCDay() + 6) % 7]} ${d.slice(8, 10)}.${d.slice(5, 7)}`;
}

/**
 * Zaległe (czerwone po terminie, także zamówienia po dacie krytycznej) na górze, potem dziś i 7 kolejnych dni.
 * Zadanie wielodniowe (warsztat, montaż) – pod dniem rozpoczęcia, a jeśli już trwa – pod „dziś”.
 * Zamówienie – pod datą planową (albo dziś, jeśli okno już trwa).
 */
export function ukladWidzetu(zadania: ZadanieKalendarza[], dzis: string, dni = 7): UkladWidzetu {
  const koniec = dodajDni(dzis, dni);
  const zalegle: ZadanieKalendarza[] = [];
  const mapa = new Map<string, ZadanieKalendarza[]>();
  const widziane = new Set<number>();
  for (const z of zadania) {
    if (z.wykonano || widziane.has(z.id)) continue;
    widziane.add(z.id);
    if (z.stan.stan === "czerwony" && (z.termin < dzis || z.typ === "zamowienie")) {
      zalegle.push(z);
      continue;
    }
    const r = zakresZadania(z);
    let dzien = z.typ === "zamowienie" ? (z.data_planowa ?? r.od) : r.od;
    if (dzien < dzis && r.do >= dzis) dzien = dzis;
    if (z.typ === "zamowienie" && r.od <= dzis && dzien < dzis) dzien = dzis;
    if (dzien < dzis || dzien > koniec) continue;
    const l = mapa.get(dzien) ?? [];
    l.push(z);
    mapa.set(dzien, l);
  }
  const wynik: DzienWidzetu[] = [];
  for (let i = 0; i <= dni; i++) {
    const d = dodajDni(dzis, i);
    const l = mapa.get(d);
    if (l?.length || i === 0) wynik.push({ data: d, etykieta: etykietaDnia(d, dzis), zadania: (l ?? []).sort(wgPilnosci) });
  }
  return { zalegle: zalegle.sort((a, b) => a.termin.localeCompare(b.termin)), dni: wynik };
}

export interface PamiecWidzetu {
  /** Czas pobrania (ISO). */
  czas: string;
  dzis: string;
  zadania: ZadanieKalendarza[];
}

const KLUCZ = "robinwood:widzet:pamiec";

export function zapiszPamiec(p: PamiecWidzetu): void {
  try {
    localStorage.setItem(KLUCZ, JSON.stringify(p));
  } catch {
    /* brak miejsca / tryb prywatny – widżet działa bez pamięci */
  }
}

export function wczytajPamiec(): PamiecWidzetu | null {
  try {
    const t = localStorage.getItem(KLUCZ);
    if (!t) return null;
    const p = JSON.parse(t) as PamiecWidzetu;
    return p && Array.isArray(p.zadania) && typeof p.czas === "string" ? p : null;
  } catch {
    return null;
  }
}
