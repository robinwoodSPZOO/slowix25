// Układ zadań w siatce kalendarza (czysta logika, testy w uklad.test.ts).
//
// * Zadanie zajmuje dni od `od` do `do` włącznie; zamówienie materiału – okno od sweet spotu do daty
//   krytycznej z wyróżnionym dniem planowym.
// * W każdym tygodniu zadania są cięte do pon.–niedz. i układane w „tory” (wiersze) zachłannie:
//   najwcześniejszy początek, potem najdłuższe, potem typ (montaż → warsztat → zamówienia → …).
// * Tory powyżej limitu są ukryte; dla każdego dnia liczymy ukryte pozycje („+N więcej”).

import type { ZadanieKalendarza } from "../../api/typy";
import { dodajDni, roznicaDni } from "./daty";

export const KOLEJNOSC_TYPOW = ["montaz", "warsztat", "zamowienie", "rysunki", "pomiar", "klient", "platnosc", "inne"];

/** Zakres dni zadania na siatce (włącznie). */
export function zakresZadania(z: ZadanieKalendarza): { od: string; do: string } {
  if (z.typ === "zamowienie") {
    const od = z.data_sweet_spot ?? z.data_od;
    const doDnia = z.data_krytyczna ?? z.data_do ?? od;
    return doDnia < od ? { od, do: od } : { od, do: doDnia };
  }
  const od = z.data_od;
  const doDnia = z.data_do ?? z.data_od;
  return doDnia < od ? { od, do: od } : { od, do: doDnia };
}

export interface Segment {
  zadanie: ZadanieKalendarza;
  /** Kolumny 0–6 (pon.–niedz.), włącznie. */
  kolOd: number;
  kolDo: number;
  tor: number;
  /** Zadanie zaczyna się przed tym tygodniem / kończy po nim. */
  ciagnieSieZLewej: boolean;
  ciagnieSieWPrawo: boolean;
  /** Kolumna dnia planowego zamówienia w tym tygodniu (null – poza tygodniem / nie dotyczy). */
  kolPlanowa: number | null;
}

export interface UkladTygodnia {
  /** Widoczne segmenty (tor < limit). */
  segmenty: Segment[];
  /** Liczba torów użytych przez widoczne segmenty. */
  torow: number;
  /** Ukryte pozycje w każdym dniu (0–6). */
  ukryte: number[];
  /** Wszystkie zadania każdego dnia (do listy „+N więcej”), w kolejności torów. */
  dni: ZadanieKalendarza[][];
}

function porownaj(a: Segment, b: Segment): number {
  if (a.kolOd !== b.kolOd) return a.kolOd - b.kolOd;
  const dl = b.kolDo - b.kolOd - (a.kolDo - a.kolOd);
  if (dl !== 0) return dl;
  const ta = KOLEJNOSC_TYPOW.indexOf(a.zadanie.typ);
  const tb = KOLEJNOSC_TYPOW.indexOf(b.zadanie.typ);
  if (ta !== tb) return (ta < 0 ? 99 : ta) - (tb < 0 ? 99 : tb);
  return a.zadanie.id - b.zadanie.id;
}

/**
 * Układ jednego tygodnia zaczynającego się w poniedziałek `poczatek`.
 * `limitTorow` = Infinity – bez ukrywania (widok tygodnia).
 */
export function ukladTygodnia(zadania: ZadanieKalendarza[], poczatek: string, limitTorow = Infinity): UkladTygodnia {
  const koniec = dodajDni(poczatek, 6);
  const wszystkie: Segment[] = [];
  for (const z of zadania) {
    const { od, do: doDnia } = zakresZadania(z);
    if (doDnia < poczatek || od > koniec) continue;
    const kolOd = Math.max(0, roznicaDni(poczatek, od));
    const kolDo = Math.min(6, roznicaDni(poczatek, doDnia));
    let kolPlanowa: number | null = null;
    if (z.typ === "zamowienie" && z.data_planowa) {
      const k = roznicaDni(poczatek, z.data_planowa);
      if (k >= 0 && k <= 6) kolPlanowa = k;
    }
    wszystkie.push({
      zadanie: z,
      kolOd,
      kolDo,
      tor: -1,
      ciagnieSieZLewej: od < poczatek,
      ciagnieSieWPrawo: doDnia > koniec,
      kolPlanowa,
    });
  }
  wszystkie.sort(porownaj);
  // zajete[tor][kol] – czy komórka zajęta
  const zajete: boolean[][] = [];
  for (const s of wszystkie) {
    let tor = 0;
    for (;;) {
      const wiersz = (zajete[tor] ??= Array<boolean>(7).fill(false));
      let wolne = true;
      for (let k = s.kolOd; k <= s.kolDo; k++) if (wiersz[k]) wolne = false;
      if (wolne) {
        for (let k = s.kolOd; k <= s.kolDo; k++) wiersz[k] = true;
        s.tor = tor;
        break;
      }
      tor++;
    }
  }
  const ukryte = Array<number>(7).fill(0);
  const dni: ZadanieKalendarza[][] = Array.from({ length: 7 }, () => []);
  const widoczne: Segment[] = [];
  let torow = 0;
  for (const s of [...wszystkie].sort((a, b) => a.tor - b.tor || a.kolOd - b.kolOd)) {
    for (let k = s.kolOd; k <= s.kolDo; k++) dni[k]?.push(s.zadanie);
    if (s.tor < limitTorow) {
      widoczne.push(s);
      torow = Math.max(torow, s.tor + 1);
    } else {
      for (let k = s.kolOd; k <= s.kolDo; k++) ukryte[k] = (ukryte[k] ?? 0) + 1;
    }
  }
  // Gdy w dniu coś jest ukryte, ostatni widoczny tor ustępuje miejsca napisowi „+N więcej” –
  // tu tylko liczymy; komponent rezerwuje osobny wiersz na napis, więc tory się nie zmieniają.
  return { segmenty: widoczne, torow, ukryte, dni };
}

/** Pozycja znacznika daty planowej wewnątrz paska (procent szerokości, środek dnia). */
export function pozycjaPlanowej(s: Segment): number | null {
  if (s.kolPlanowa === null) return null;
  const szer = s.kolDo - s.kolOd + 1;
  return ((s.kolPlanowa - s.kolOd + 0.5) / szer) * 100;
}

/** Ile torów mieści się w wierszu tygodnia o danej wysokości (px). */
export function torowWWierszu(wysokosc: number, naglowek: number, wysTor: number, napisWiecej: number): number {
  return Math.max(1, Math.floor((wysokosc - naglowek - napisWiecej) / wysTor));
}
