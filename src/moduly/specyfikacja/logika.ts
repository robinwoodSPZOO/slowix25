// Czysta logika edytora specyfikacji (bez Reacta) – testowana w logika.test.ts.

import type { Czesc, DaneWiersza, Podgrupa, Podpowiedzi, Sekcja, Specyfikacja, StatusWiersza, Wiersz } from "../../api/typy";

export const STATUSY_WIERSZA: { wartosc: StatusWiersza; etykieta: string }[] = [
  { wartosc: "ustalone", etykieta: "Ustalone" },
  { wartosc: "do_potwierdzenia", etykieta: "Do potwierdzenia" },
  { wartosc: "po_stronie_klienta", etykieta: "Po stronie klienta" },
  { wartosc: "poza_wycena", etykieta: "Poza wyceną" },
];

export const PODGRUPY: { wartosc: Podgrupa; etykieta: string }[] = [
  { wartosc: "glowna", etykieta: "Elementy" },
  { wartosc: "frezowanie", etykieta: "Frezowanie" },
  { wartosc: "wyposazenie", etykieta: "Wyposażenie" },
];

/** Kolumny edytowalne w kolejności przechodzenia klawiszem Tab. */
export type Kolumna = "rodzaj" | "material" | "opis" | "ilosc" | "jednostka" | "status" | "cena";
export const KOLUMNY: Kolumna[] = ["rodzaj", "material", "opis", "ilosc", "jednostka", "status"];
export const KOLUMNY_AGD: Kolumna[] = ["rodzaj", "material", "opis", "ilosc", "jednostka", "status", "cena"];

/** Dane do zapisu (PUT) z wiersza z serwera. */
export function daneZWiersza(w: Wiersz): DaneWiersza {
  return {
    podgrupa: w.podgrupa,
    rodzaj: w.rodzaj,
    material: w.material,
    opis: w.opis,
    ilosc: w.ilosc,
    jednostka: w.jednostka,
    status: w.status,
    termin_potwierdzenia: w.termin_potwierdzenia,
    kategoria_materialu_id: w.kategoria_materialu_id,
    pozycja_slownika_id: w.pozycja_slownika_id,
    cena_gr: w.cena_gr,
  };
}

const norm = (v: unknown) => (v === "" || v === undefined ? null : v);

/** Czy szkic różni się od wiersza z serwera (pomijamy zapis bez zmian). */
export function czyZmieniony(w: Wiersz, szkic: DaneWiersza): boolean {
  const baza = daneZWiersza(w);
  return (Object.keys(szkic) as (keyof DaneWiersza)[]).some(
    (k) => k in baza && norm(szkic[k]) !== norm(baza[k as keyof typeof baza]),
  );
}

/** "2,5" → 2.5; pusty tekst → null; nieprawidłowy → undefined. */
export function parsujIlosc(t: string): number | null | undefined {
  const s = t.trim().replace(/\s/g, "").replace(",", ".");
  if (!s) return null;
  if (!/^\d+(\.\d+)?$/.test(s)) return undefined;
  return Number(s);
}

export function formatujIlosc(i: number | null | undefined): string {
  if (i === null || i === undefined) return "";
  return String(Math.round(i * 1000) / 1000).replace(".", ",");
}

/** Wiersze części pogrupowane: elementy, frezowanie, wyposażenie (puste grupy pomijane, poza główną). */
export function wierszePogrupowane(c: Czesc): { podgrupa: Podgrupa; etykieta: string; wiersze: Wiersz[] }[] {
  return PODGRUPY.map((p) => ({
    podgrupa: p.wartosc,
    etykieta: p.etykieta,
    wiersze: c.wiersze.filter((w) => w.podgrupa === p.wartosc),
  })).filter((g) => g.podgrupa === "glowna" || g.wiersze.length > 0);
}

/** Wiersze w kolejności wyświetlania (jak w tabeli). */
export function wierszeWidoczne(c: Czesc): Wiersz[] {
  return wierszePogrupowane(c).flatMap((g) => g.wiersze);
}

function unikalne(listy: (string | null | undefined)[]): string[] {
  const wynik: string[] = [];
  const znane = new Set<string>();
  for (const w of listy) {
    if (!w) continue;
    const k = w.toLowerCase();
    if (znane.has(k)) continue;
    znane.add(k);
    wynik.push(w);
  }
  return wynik;
}

/** Podpowiedzi dla kolumny – zależne od podgrupy wiersza i typu sekcji. */
export function podpowiedziDla(kolumna: Kolumna, podgrupa: Podgrupa, agd: boolean, p: Podpowiedzi): string[] {
  const z = (...kody: string[]) => kody.flatMap((k) => (p[k] ?? []).map((x) => x.wartosc));
  switch (kolumna) {
    case "rodzaj":
      if (agd) return unikalne(z("agd"));
      return unikalne(
        (p.rodzaj_elementu ?? [])
          .filter((x) => (podgrupa === "wyposazenie" ? x.podgrupa === "wyposazenie" : x.podgrupa !== "wyposazenie"))
          .map((x) => x.wartosc),
      );
    case "material":
      if (agd) return unikalne(z("agd_marka"));
      return podgrupa === "wyposazenie" ? unikalne(z("okucie", "oswietlenie")) : unikalne(z("material", "plyta", "front", "blat"));
    case "opis":
      return unikalne(z("opis", "dekor"));
    case "jednostka":
      return unikalne(z("jednostka"));
    default:
      return [];
  }
}

/** Podgrupa podpowiadana przez słownik dla wpisanego rodzaju (np. „Uchwyty” → wyposażenie). */
export function podgrupaDlaRodzaju(rodzaj: string, p: Podpowiedzi): Podgrupa | null {
  const r = rodzaj.trim().toLowerCase();
  const poz = (p.rodzaj_elementu ?? []).find((x) => x.wartosc.toLowerCase() === r);
  return poz?.podgrupa ?? null;
}

/** Sekcja i część po identyfikatorze części. */
export function znajdzCzesc(s: Specyfikacja, czescId: number): { sekcja: Sekcja; czesc: Czesc } | null {
  for (const sekcja of s.sekcje) {
    const czesc = sekcja.czesci.find((c) => c.id === czescId);
    if (czesc) return { sekcja, czesc };
  }
  return null;
}

export type Wybor = { typ: "sekcja"; id: number } | { typ: "czesc"; id: number } | null;

/** Po zmianie drzewa: zachowaj wybór, jeśli element nadal istnieje, inaczej pierwsza część. */
export function poprawWybor(s: Specyfikacja, w: Wybor): Wybor {
  if (w?.typ === "czesc" && znajdzCzesc(s, w.id)) return w;
  if (w?.typ === "sekcja" && s.sekcje.some((x) => x.id === w.id)) return w;
  const pierwsza = s.sekcje.find((x) => x.czesci.length > 0)?.czesci[0];
  if (pierwsza) return { typ: "czesc", id: pierwsza.id };
  const sekcja = s.sekcje[0];
  return sekcja ? { typ: "sekcja", id: sekcja.id } : null;
}

/** Nowa kolejność po przesunięciu elementu o krok (−1 w górę, +1 w dół). */
export function przesun(ids: number[], id: number, krok: -1 | 1): number[] | null {
  const i = ids.indexOf(id);
  const j = i + krok;
  if (i < 0 || j < 0 || j >= ids.length) return null;
  const wynik = [...ids];
  [wynik[i], wynik[j]] = [wynik[j]!, wynik[i]!];
  return wynik;
}

/** Etykieta sekcji z numerem: „01 Zabudowa Kuchni”. */
export function etykietaSekcji(s: Sekcja): string {
  return s.numer ? `${s.numer} ${s.nazwa}` : s.nazwa;
}

/** Liczba wierszy wymagających decyzji klienta w części / sekcji. */
export function liczbaDoPotwierdzenia(wiersze: Wiersz[]): number {
  return wiersze.filter((w) => w.status === "do_potwierdzenia").length;
}

/** Nazwa pola wiersza do komunikatów (konflikty, błędy). */
export function nazwaPolaWiersza(p: string): string {
  const n: Record<string, string> = {
    rodzaj: "Rodzaj",
    material: "Materiał",
    opis: "Opis",
    ilosc: "Ilość",
    jednostka: "j.m.",
    status: "Status",
    cena_gr: "Cena jedn.",
    podgrupa: "Podgrupa",
    termin_potwierdzenia: "Termin potwierdzenia",
    nazwa: "Nazwa",
    typ: "Typ",
    stawka_vat: "Stawka VAT",
    wariant: "Wariant",
  };
  return n[p] ?? p;
}

/**
 * Nowa kolejność wszystkich wierszy części po przesunięciu wiersza o krok w obrębie jego
 * podgrupy (Alt+↑/↓). `null`, gdy nie ma dokąd przesunąć.
 */
export function przesunWiersz(c: Czesc, id: number, krok: -1 | 1): number[] | null {
  const w = c.wiersze.find((x) => x.id === id);
  if (!w) return null;
  const wGrupie = c.wiersze.filter((x) => x.podgrupa === w.podgrupa).map((x) => x.id);
  const i = wGrupie.indexOf(id);
  const sasiad = wGrupie[i + krok];
  if (sasiad === undefined) return null;
  const ids = c.wiersze.map((x) => x.id);
  const a = ids.indexOf(id);
  const b = ids.indexOf(sasiad);
  [ids[a], ids[b]] = [ids[b]!, ids[a]!];
  return ids;
}

/** Kolejność po przeciągnięciu wiersza `id` przed wiersz `przed` (ta sama część). */
export function wstawPrzed(c: Czesc, id: number, przed: number): number[] | null {
  if (id === przed) return null;
  const ids = c.wiersze.map((x) => x.id).filter((x) => x !== id);
  const j = ids.indexOf(przed);
  if (j < 0) return null;
  ids.splice(j, 0, id);
  return ids;
}

/** Zmienione pola szkicu względem wartości z serwera (pomija pola równe bazie). */
export function zmienionePola(w: Wiersz, pola: DaneWiersza): DaneWiersza {
  const baza = daneZWiersza(w);
  const wynik: Record<string, unknown> = {};
  for (const [k, v] of Object.entries(pola)) {
    if (norm(v) !== norm((baza as Record<string, unknown>)[k])) wynik[k] = v;
  }
  return wynik as DaneWiersza;
}
