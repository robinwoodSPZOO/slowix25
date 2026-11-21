// Planer powiadomień Windows (okno główne – działa także, gdy okno jest schowane do zasobnika).
// Co minutę sprawdza, co pokazać (czysta funkcja `wyznaczPowiadomienia`); dane z serwera pobiera
// co 5 minut i po zdarzeniach WebSocket. Ustawienia i historia pokazanych powiadomień są
// zapisywane lokalnie na tym stanowisku (localStorage) – każde stanowisko ma własne.

import { useEffect, useRef } from "react";
import type { ZadanieKalendarza } from "../api/typy";
import type { KlientApi } from "../api/klient";
import { usePolaczenie } from "../kontekst/Polaczenie";
import { dodajDni, dzisIso } from "../moduly/kalendarz/daty";
import { normalizujUstawienia, wyznaczPowiadomienia, type HistoriaPowiadomien, type Powiadomienie, type UstawieniaPowiadomien } from "./powiadomienia";
import { pokazPowiadomienie } from "./tauri";

const KLUCZ_USTAWIEN = "robinwood:powiadomienia:ustawienia";
const KLUCZ_HISTORII = "robinwood:powiadomienia:historia";
const CO_ILE_POBIERAC_MS = 5 * 60 * 1000;
const MAKS_NARAZ = 5;

export function wczytajUstawieniaPowiadomien(): UstawieniaPowiadomien {
  try {
    return normalizujUstawienia(JSON.parse(localStorage.getItem(KLUCZ_USTAWIEN) ?? "null") as Partial<UstawieniaPowiadomien> | null);
  } catch {
    return normalizujUstawienia(null);
  }
}

export function zapiszUstawieniaPowiadomien(u: UstawieniaPowiadomien): void {
  try {
    localStorage.setItem(KLUCZ_USTAWIEN, JSON.stringify(u));
  } catch {
    /* brak pamięci lokalnej */
  }
}

function wczytajHistorie(): HistoriaPowiadomien {
  try {
    const h = JSON.parse(localStorage.getItem(KLUCZ_HISTORII) ?? "{}") as unknown;
    return h && typeof h === "object" ? (h as HistoriaPowiadomien) : {};
  } catch {
    return {};
  }
}

function zapiszHistorie(h: HistoriaPowiadomien): void {
  try {
    localStorage.setItem(KLUCZ_HISTORII, JSON.stringify(h));
  } catch {
    /* brak pamięci lokalnej */
  }
}

export function wyczyscHistoriePowiadomien(): void {
  zapiszHistorie({});
}

export function terazLokalnie(d = new Date()): { data: string; godzina: string } {
  return { data: dzisIso(d), godzina: `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}` };
}

/** Zadania potrzebne do powiadomień: najbliższe dni (do największego progu) + wszystkie pilne. */
export async function pobierzZadaniaDoPowiadomien(api: KlientApi, u: UstawieniaPowiadomien): Promise<ZadanieKalendarza[]> {
  const dzis = dzisIso();
  const horyzont = Math.max(1, ...u.dniPrzed) + 1;
  const [najblizsze, pilne] = await Promise.all([
    api.kalendarz.zadania({ od: dzis, do: dodajDni(dzis, horyzont), tylko_niewykonane: true }),
    api.kalendarz.zadania({ stan: "czerwony,pomaranczowy", tylko_niewykonane: true }),
  ]);
  const m = new Map<number, ZadanieKalendarza>();
  for (const z of [...pilne, ...najblizsze]) m.set(z.id, z);
  return [...m.values()];
}

/** Podgląd (bez zapisu historii): co zostałoby pokazane teraz. */
export async function podgladPowiadomien(api: KlientApi): Promise<Powiadomienie[]> {
  const u = wczytajUstawieniaPowiadomien();
  const z = await pobierzZadaniaDoPowiadomien(api, u);
  return wyznaczPowiadomienia(z, u, terazLokalnie(), wczytajHistorie()).powiadomienia;
}

/** Hook planera – montowany raz, w oknie głównym. */
export function usePlanerPowiadomien(): void {
  const { api, ostatnieZdarzenie } = usePolaczenie();
  const zadania = useRef<ZadanieKalendarza[] | null>(null);
  const pobrano = useRef(0);
  const nieaktualne = useRef(true);

  useEffect(() => {
    nieaktualne.current = true;
  }, [ostatnieZdarzenie]);

  useEffect(() => {
    if (!api) return;
    let aktywny = true;
    const krok = async () => {
      const u = wczytajUstawieniaPowiadomien();
      if (!u.wlaczone) return;
      try {
        if (nieaktualne.current || Date.now() - pobrano.current > CO_ILE_POBIERAC_MS || !zadania.current) {
          zadania.current = await pobierzZadaniaDoPowiadomien(api, u);
          pobrano.current = Date.now();
          nieaktualne.current = false;
        }
      } catch {
        return; // brak serwera – spróbujemy za minutę (historia bez zmian)
      }
      if (!aktywny || !zadania.current) return;
      const { powiadomienia, historia } = wyznaczPowiadomienia(zadania.current, u, terazLokalnie(), wczytajHistorie());
      zapiszHistorie(historia);
      for (const p of powiadomienia.slice(0, MAKS_NARAZ)) {
        try {
          await pokazPowiadomienie(p.tytul, p.tresc);
        } catch {
          /* powiadomienie się nie pokazało – nie ponawiamy, żeby nie dublować */
        }
      }
    };
    const start = setTimeout(() => void krok(), 5000);
    const t = setInterval(() => void krok(), 60_000);
    return () => {
      aktywny = false;
      clearTimeout(start);
      clearInterval(t);
    };
  }, [api]);
}
