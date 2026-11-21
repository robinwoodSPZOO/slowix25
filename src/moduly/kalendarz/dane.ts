// Pobieranie danych kalendarza z API i odświeżanie po zdarzeniach WebSocket.

import { useCallback, useEffect, useRef, useState } from "react";
import { BladApi } from "../../api/klient";
import type { DzienWolny, Klient, Swieto, ZadanieKalendarza, Zlecenie } from "../../api/typy";
import { usePolaczenie } from "../../kontekst/Polaczenie";
import { lataZakresu } from "./daty";

/** Zasoby, po których zmianie kalendarz i widżet odświeżają widoczny zakres (rozdz. 10.4). */
export const ZASOBY_KALENDARZA = [
  "zadania_kalendarza",
  "zlecenia",
  "zamowienia_materialow",
  "ustawienia",
  "dni_wolne",
  "kategorie_materialow",
  "czasy_dostaw",
  "dostawcy",
  "klienci",
  "*",
];

/** Czytelny komunikat po polsku dla błędu API (brak sieci / 409 / 422 / 500 / 503). */
export function komunikatBledu(e: unknown): string {
  if (e instanceof BladApi) {
    if (e.kod === 0) return `Brak połączenia z serwerem. Sprawdź, czy komputer-serwer jest włączony i w tej samej sieci. (${e.message})`;
    if (e.kod === 503) return "Serwer jest chwilowo zajęty – spróbuj ponownie za chwilę.";
    if (e.kod === 500) return `Błąd serwera – nic nie zostało zapisane. ${e.message}`;
    if (e.kod === 401) return "Serwer odrzucił klucz dostępu – sprawdź klucz w Ustawieniach.";
    if (e.kod === 404) return `Nie znaleziono – rekord mógł zostać usunięty na innym stanowisku. ${e.message}`;
    return e.message.charAt(0).toUpperCase() + e.message.slice(1);
  }
  return e instanceof Error ? e.message : String(e);
}

/** Wywołuje `f` (z opóźnieniem 300 ms, scalając serie) po zdarzeniu dotyczącym podanych zasobów. */
export function useOdswiezaniePoZdarzeniu(f: () => void, zasoby: string[] = ZASOBY_KALENDARZA) {
  const { ostatnieZdarzenie } = usePolaczenie();
  const ref = useRef(f);
  useEffect(() => {
    ref.current = f;
  });
  const klucz = zasoby.join(",");
  useEffect(() => {
    if (!ostatnieZdarzenie || !klucz.split(",").includes(ostatnieZdarzenie.zasob)) return;
    const t = setTimeout(() => ref.current(), 300);
    return () => clearTimeout(t);
  }, [ostatnieZdarzenie, klucz]);
}

export interface DaneKalendarza {
  zadania: ZadanieKalendarza[];
  swieta: Map<string, string>;
  dniWolne: Map<string, DzienWolny>;
  ladowanie: boolean;
  blad: string | null;
  odswiez: () => void;
}

/** Zadania nachodzące na zakres [od, do] + święta i dni wolne w tym zakresie. */
export function useDaneKalendarza(od: string, doDnia: string): DaneKalendarza {
  const { api } = usePolaczenie();
  const [zadania, ustawZadania] = useState<ZadanieKalendarza[]>([]);
  const [swieta, ustawSwieta] = useState<Map<string, string>>(new Map());
  const [dniWolne, ustawDniWolne] = useState<Map<string, DzienWolny>>(new Map());
  // Wczytywanie = dane nie dotyczą jeszcze bieżącego zakresu (bez setState w efekcie).
  const [wczytanyZakres, ustawWczytanyZakres] = useState<string | null>(null);
  const ladowanie = wczytanyZakres !== `${od}|${doDnia}`;
  const [blad, ustawBlad] = useState<string | null>(null);
  const licznik = useRef(0);

  const pobierz = useCallback(() => {
    if (!api) return;
    const nr = ++licznik.current;
    Promise.all([
      api.kalendarz.zadania({ od, do: doDnia }),
      Promise.all(lataZakresu(od, doDnia).map((r) => api.kalendarz.swieta(r))),
      api.kalendarz.dniWolne(od, doDnia),
    ]).then(
      ([z, s, w]) => {
        if (nr !== licznik.current) return; // odpowiedź na starszy zakres
        ustawZadania(z);
        ustawSwieta(new Map(s.flat().map((x: Swieto) => [x.data, x.nazwa])));
        ustawDniWolne(new Map(w.map((x) => [x.data, x])));
        ustawBlad(null);
        ustawWczytanyZakres(`${od}|${doDnia}`);
      },
      (e: unknown) => {
        if (nr !== licznik.current) return;
        ustawBlad(komunikatBledu(e));
        ustawWczytanyZakres(`${od}|${doDnia}`);
      },
    );
  }, [api, od, doDnia]);

  useEffect(pobierz, [pobierz]);
  useOdswiezaniePoZdarzeniu(pobierz);
  return { zadania, swieta, dniWolne, ladowanie, blad, odswiez: pobierz };
}

/** Zlecenia i klienci (filtry, nazwy, tablica etapów). */
export function useZleceniaIKlienci() {
  const { api } = usePolaczenie();
  const [zlecenia, ustawZlecenia] = useState<Zlecenie[]>([]);
  const [klienci, ustawKlientow] = useState<Klient[]>([]);
  const [blad, ustawBlad] = useState<string | null>(null);
  const pobierz = useCallback(() => {
    if (!api) return;
    Promise.all([api.zlecenia.lista(), api.klienci.lista()]).then(
      ([z, k]) => {
        ustawZlecenia(z);
        ustawKlientow(k);
        ustawBlad(null);
      },
      (e: unknown) => ustawBlad(komunikatBledu(e)),
    );
  }, [api]);
  useEffect(pobierz, [pobierz]);
  useOdswiezaniePoZdarzeniu(pobierz, ["zlecenia", "klienci", "*"]);
  return { zlecenia, klienci, blad, odswiez: pobierz };
}
