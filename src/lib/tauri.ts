// Most do warstwy natywnej (komendy Tauri). Poza oknem Tauri (np. `npm run dev` w przeglądarce)
// funkcje zwracają wartości zastępcze, a adres API i klucz biorą się ze zmiennych VITE_API_*.

import { invoke, isTauri } from "@tauri-apps/api/core";

export type Tryb = "nieskonfigurowany" | "serwer" | "klient";

export interface Konfiguracja {
  tryb: Tryb;
  nazwa_stanowiska: string;
  port: number;
  folder_danych: string | null;
  adres_serwera: string | null;
  klucz_api: string | null;
}

export interface Polaczenie {
  konfiguracja: Konfiguracja;
  adres_api: string | null;
  klucz_api: string | null;
  serwer_dziala: boolean;
  blad_serwera: string | null;
  folder_danych: string | null;
}

export interface ZnalezionySerwer {
  nazwa: string;
  adresy: string[];
  port: number;
  adres_api: string;
}

export const wTauri = (): boolean => isTauri();

export async function pobierzPolaczenie(): Promise<Polaczenie> {
  if (wTauri()) return invoke<Polaczenie>("pobierz_polaczenie");
  const adres = import.meta.env.VITE_API_URL as string | undefined;
  const klucz = import.meta.env.VITE_API_KLUCZ as string | undefined;
  return {
    konfiguracja: {
      tryb: adres ? "klient" : "nieskonfigurowany",
      nazwa_stanowiska: "Przeglądarka (tryb deweloperski)",
      port: 47800,
      folder_danych: null,
      adres_serwera: adres ?? null,
      klucz_api: klucz ?? null,
    },
    adres_api: adres ?? null,
    klucz_api: klucz ?? null,
    serwer_dziala: false,
    blad_serwera: null,
    folder_danych: null,
  };
}

export async function zapiszKonfiguracje(konfiguracja: Konfiguracja): Promise<void> {
  if (!wTauri()) throw new Error("Zmiana konfiguracji działa tylko w aplikacji okienkowej");
  await invoke("zapisz_konfiguracje", { konfiguracja });
}

export async function wyszukajSerwery(): Promise<ZnalezionySerwer[]> {
  if (!wTauri()) return [];
  return invoke<ZnalezionySerwer[]>("wyszukaj_serwery");
}

export async function powiadomienieTestowe(): Promise<void> {
  if (!wTauri()) {
    alert("Powiadomienie testowe (poza aplikacją okienkową)");
    return;
  }
  await invoke("powiadomienie_testowe");
}

export async function przelaczWidzet(): Promise<void> {
  if (wTauri()) await invoke("przelacz_widzet");
}

export async function otworzGlowneOkno(): Promise<void> {
  if (wTauri()) await invoke("otworz_glowne_okno");
}

/** Etykieta bieżącego okna Tauri ("main" lub "widzet"); poza Tauri – na podstawie adresu. */
export async function etykietaOkna(): Promise<string> {
  if (wTauri()) {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    return getCurrentWindow().label;
  }
  return window.location.hash.startsWith("#/widzet") ? "widzet" : "main";
}

export async function autostart(): Promise<{ wlaczony: boolean; ustaw: (w: boolean) => Promise<void> } | null> {
  if (!wTauri()) return null;
  const a = await import("@tauri-apps/plugin-autostart");
  return {
    wlaczony: await a.isEnabled(),
    ustaw: async (w) => (w ? a.enable() : a.disable()),
  };
}

/**
 * Tauri: przed schowaniem okna (krzyżyk) i przed „Zakończ” z zasobnika zapisujemy szkice
 * edytorów, a dopiero potem okno jest chowane / program kończy działanie.
 */
export async function obsluzZamykanieOkna(oproznij: () => Promise<void>): Promise<void> {
  if (!wTauri()) return;
  const { listen } = await import("@tauri-apps/api/event");
  await listen("robinwood://przed-zamknieciem", async () => {
    await oproznij();
    await invoke("ukryj_glowne_okno");
  });
  await listen("robinwood://przed-zakonczeniem", async () => {
    await oproznij();
    await invoke("zakoncz_aplikacje");
  });
}

// --- Kalendarz (Faza 4b): widżet, powiadomienia, przejście do zadania ---

/** Powiadomienie systemowe Windows (Tauri). Poza Tauri – powiadomienie przeglądarki, jeśli dozwolone. */
export async function pokazPowiadomienie(tytul: string, tresc: string): Promise<void> {
  if (wTauri()) {
    await invoke("pokaz_powiadomienie", { tytul, tresc });
    return;
  }
  if (typeof Notification !== "undefined" && Notification.permission === "granted") new Notification(tytul, { body: tresc });
}

/**
 * Pokazuje główne okno i przechodzi pod podaną ścieżkę (np. `/kalendarz?zadanie=5`).
 * Poza Tauri (widżet w przeglądarce) – otwiera ścieżkę w nowej karcie.
 */
export async function otworzWGlownymOknie(sciezka: string): Promise<void> {
  if (wTauri()) {
    await invoke("otworz_sciezke", { sciezka });
    return;
  }
  window.open(`${window.location.pathname}#${sciezka}`, "_blank");
}

/** Okno główne: nasłuch próśb o przejście pod ścieżkę (z widżetu). Zwraca funkcję kończącą. */
export async function nasluchujNawigacji(przejdz: (sciezka: string) => void): Promise<() => void> {
  if (!wTauri()) return () => {};
  const { listen } = await import("@tauri-apps/api/event");
  return listen<string>("robinwood://przejdz", (e) => przejdz(e.payload));
}

/** Chowa widżet (przywrócenie: menu zasobnika „Pokaż / ukryj widżet” albo Ustawienia). */
export async function ukryjWidzet(): Promise<void> {
  if (wTauri()) await invoke("ukryj_widzet");
}
