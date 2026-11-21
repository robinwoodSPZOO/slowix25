// Filtry kalendarza (czysta logika, testy w filtry.test.ts).

import type { StanOstrzezenia, ZadanieKalendarza } from "../../api/typy";

export interface Filtry {
  /** Pokazywane typy zadań; pusta lista = wszystkie. */
  typy: string[];
  zlecenieId: number | null;
  klientId: number | null;
  tylkoNiewykonane: boolean;
  tylkoZOstrzezeniem: boolean;
}

export const PUSTE_FILTRY: Filtry = { typy: [], zlecenieId: null, klientId: null, tylkoNiewykonane: false, tylkoZOstrzezeniem: false };

export const STANY_OSTRZEGAWCZE: StanOstrzezenia[] = ["zolty", "pomaranczowy", "czerwony"];

/** Czy zadanie wymaga uwagi: żółty/pomarańczowy/czerwony, zagrożona dostawa, blokada albo nieaktualny harmonogram. */
export function maOstrzezenie(z: ZadanieKalendarza): boolean {
  if (z.wykonano && !z.zamowienie?.dostawa_zagrozona) return false;
  return (
    STANY_OSTRZEGAWCZE.includes(z.stan.stan) ||
    z.harmonogram_nieaktualny ||
    !!z.zamowienie?.dostawa_zagrozona ||
    (!!z.zamowienie?.zablokowane && !z.wykonano)
  );
}

/** `klientZlecenia` – id zlecenia → id klienta (do filtra po kliencie). */
export function filtrujZadania(zadania: ZadanieKalendarza[], f: Filtry, klientZlecenia: Map<number, number>): ZadanieKalendarza[] {
  return zadania.filter((z) => {
    if (f.typy.length > 0 && !f.typy.includes(z.typ)) return false;
    if (f.zlecenieId !== null && z.zlecenie_id !== f.zlecenieId) return false;
    if (f.klientId !== null && (z.zlecenie_id === null || klientZlecenia.get(z.zlecenie_id) !== f.klientId)) return false;
    if (f.tylkoNiewykonane && z.wykonano) return false;
    if (f.tylkoZOstrzezeniem && !maOstrzezenie(z)) return false;
    return true;
  });
}

export function liczbaAktywnych(f: Filtry): number {
  return (f.typy.length > 0 ? 1 : 0) + (f.zlecenieId !== null ? 1 : 0) + (f.klientId !== null ? 1 : 0) + (f.tylkoNiewykonane ? 1 : 0) + (f.tylkoZOstrzezeniem ? 1 : 0);
}

const PILNOSC: Record<StanOstrzezenia, number> = { wykonane: 0, spokojnie: 1, zolty: 2, pomaranczowy: 3, czerwony: 4 };

export function pilnosc(s: StanOstrzezenia): number {
  return PILNOSC[s] ?? 0;
}

/** Sortowanie: najpilniejsze najpierw, potem termin. */
export function wgPilnosci(a: ZadanieKalendarza, b: ZadanieKalendarza): number {
  return pilnosc(b.stan.stan) - pilnosc(a.stan.stan) || a.termin.localeCompare(b.termin) || a.id - b.id;
}

/** Liczniki stanów ostrzeżeń (tylko niewykonane). */
export function licznikiStanow(zadania: ZadanieKalendarza[]): { zolty: number; pomaranczowy: number; czerwony: number } {
  const l = { zolty: 0, pomaranczowy: 0, czerwony: 0 };
  for (const z of zadania) {
    if (z.wykonano) continue;
    if (z.stan.stan === "zolty" || z.stan.stan === "pomaranczowy" || z.stan.stan === "czerwony") l[z.stan.stan]++;
  }
  return l;
}
