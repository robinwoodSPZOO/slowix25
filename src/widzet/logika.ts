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

const ISO = /^\d{4}-\d{2}-\d{2}$/;
const STANY_OK = new Set(["wykonane", "spokojnie", "zolty", "pomaranczowy", "czerwony"]);
const jestIso = (x: unknown): x is string => typeof x === "string" && ISO.test(x);
const isoLubNull = (x: unknown) => x === null || x === undefined || jestIso(x);

/**
 * Sprawdza kształt zadania z pamięci lokalnej (mogła zostać zapisana przez starszą wersję albo
 * uszkodzona). Zwraca zadanie z uzupełnionymi polami opcjonalnymi albo null, gdy nie da się go użyć.
 */
export function poprawneZadanie(x: unknown): ZadanieKalendarza | null {
  if (!x || typeof x !== "object") return null;
  const z = x as Record<string, unknown>;
  const st = z.stan as Record<string, unknown> | null | undefined;
  if (typeof z.id !== "number" || typeof z.typ !== "string" || typeof z.tytul !== "string") return null;
  if (!jestIso(z.data_od) || !jestIso(z.termin) || !isoLubNull(z.data_do)) return null;
  if (!isoLubNull(z.data_sweet_spot) || !isoLubNull(z.data_planowa) || !isoLubNull(z.data_krytyczna)) return null;
  if (!st || typeof st !== "object" || typeof st.stan !== "string" || !STANY_OK.has(st.stan) || typeof st.opis !== "string") return null;
  if (z.wykonano !== null && z.wykonano !== undefined && typeof z.wykonano !== "string") return null;
  const zam = z.zamowienie;
  if (zam !== null && zam !== undefined && typeof zam !== "object") return null;
  return {
    ...(z as unknown as ZadanieKalendarza),
    data_do: (z.data_do as string | null | undefined) ?? null,
    data_sweet_spot: (z.data_sweet_spot as string | null | undefined) ?? null,
    data_planowa: (z.data_planowa as string | null | undefined) ?? null,
    data_krytyczna: (z.data_krytyczna as string | null | undefined) ?? null,
    wykonano: (z.wykonano as string | null | undefined) ?? null,
    zlecenie_id: typeof z.zlecenie_id === "number" ? z.zlecenie_id : null,
    zlecenie_nazwa: typeof z.zlecenie_nazwa === "string" ? z.zlecenie_nazwa : null,
    zamowienie: (zam as ZadanieKalendarza["zamowienie"]) ?? null,
    harmonogram_nieaktualny: z.harmonogram_nieaktualny === true,
    stan: {
      stan: st.stan as ZadanieKalendarza["stan"]["stan"],
      opis: st.opis,
      dni_do_terminu: typeof st.dni_do_terminu === "number" ? st.dni_do_terminu : null,
      przesuniecie_montazu_dni: typeof st.przesuniecie_montazu_dni === "number" ? st.przesuniecie_montazu_dni : null,
      nowy_termin_montazu: jestIso(st.nowy_termin_montazu) ? st.nowy_termin_montazu : null,
    },
  };
}

/** Pamięć o nieoczekiwanym kształcie jest odrzucana; złe zadania – pomijane. */
export function sprawdzPamiec(x: unknown): PamiecWidzetu | null {
  if (!x || typeof x !== "object") return null;
  const p = x as Record<string, unknown>;
  if (typeof p.czas !== "string" || Number.isNaN(Date.parse(p.czas)) || !jestIso(p.dzis) || !Array.isArray(p.zadania)) return null;
  const zadania = p.zadania.map(poprawneZadanie).filter((z): z is ZadanieKalendarza => z !== null);
  return { czas: p.czas, dzis: p.dzis, zadania };
}

export function wyczyscPamiec(): void {
  try {
    localStorage.removeItem(KLUCZ);
  } catch {
    /* brak pamięci lokalnej */
  }
}

export function wczytajPamiec(): PamiecWidzetu | null {
  try {
    const t = localStorage.getItem(KLUCZ);
    if (!t) return null;
    const p = sprawdzPamiec(JSON.parse(t));
    if (!p) localStorage.removeItem(KLUCZ);
    return p;
  } catch {
    return null;
  }
}
