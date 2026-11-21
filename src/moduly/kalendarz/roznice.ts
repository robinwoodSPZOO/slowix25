// Co się przesunęło, a co jest zamrożone po przeliczeniu harmonogramu (czysta logika, testy obok).

import type { ZadanieKalendarza } from "../../api/typy";
import { roznicaDni } from "./daty";
import { zakresZadania } from "./uklad";

export interface Przesuniecie {
  id: number;
  tytul: string;
  typ: string;
  z: { od: string; do: string };
  na: { od: string; do: string };
  /** O ile dni kalendarzowych przesunął się początek (+ później, − wcześniej). */
  dni: number;
}

export interface Zamrozone {
  id: number;
  tytul: string;
  typ: string;
  powod: string;
}

export interface RoznicaHarmonogramu {
  przesuniete: Przesuniecie[];
  zamrozone: Zamrozone[];
  dodane: ZadanieKalendarza[];
  usuniete: ZadanieKalendarza[];
  bezZmian: number;
}

export function powodZamrozenia(z: ZadanieKalendarza): string | null {
  if (z.wykonano) return z.typ === "zamowienie" ? "zamówione – terminy okna zamówienia zostają" : "wykonane";
  if (!z.wyliczone || !z.klucz) return "dodane ręcznie";
  return null;
}

export function roznicaHarmonogramu(przed: ZadanieKalendarza[], po: ZadanieKalendarza[]): RoznicaHarmonogramu {
  const stare = new Map(przed.map((z) => [z.id, z]));
  const nowe = new Map(po.map((z) => [z.id, z]));
  const wynik: RoznicaHarmonogramu = { przesuniete: [], zamrozone: [], dodane: [], usuniete: [], bezZmian: 0 };
  for (const z of po) {
    const s = stare.get(z.id);
    if (!s) {
      wynik.dodane.push(z);
      continue;
    }
    const a = zakresZadania(s);
    const b = zakresZadania(z);
    const powod = powodZamrozenia(z);
    if (a.od !== b.od || a.do !== b.do) {
      wynik.przesuniete.push({ id: z.id, tytul: z.tytul, typ: z.typ, z: a, na: b, dni: roznicaDni(a.od, b.od) });
    } else if (powod) {
      wynik.zamrozone.push({ id: z.id, tytul: z.tytul, typ: z.typ, powod });
    } else {
      wynik.bezZmian++;
    }
  }
  for (const s of przed) if (!nowe.has(s.id)) wynik.usuniete.push(s);
  wynik.przesuniete.sort((x, y) => x.na.od.localeCompare(y.na.od));
  return wynik;
}
