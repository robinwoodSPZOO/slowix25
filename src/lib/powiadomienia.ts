// Logika „co i kiedy powiadomić” – CZYSTA funkcja (bez zegara, bez sieci, bez localStorage).
// Wejście: zadania niewykonane (z API), ustawienia stanowiska, „teraz” (data + godzina), historia
// pokazanych powiadomień. Wyjście: lista powiadomień do pokazania i nowa historia.
//
// Zasady:
// * poranne podsumowanie – raz dziennie, od ustawionej godziny,
// * przypomnienia N dni przed terminem (domyślnie 7, 3, 1) – od godziny podsumowania; jeśli program
//   nie działał w dniu progu, przypomnienie przychodzi przy najbliższym progu, który nadal obowiązuje
//   (np. 2 dni przed terminem → próg „3 dni”), a wcześniejsze progi są oznaczane jako pokazane,
// * alert przy wejściu zadania w stan pomarańczowy i czerwony – od razu, o każdej porze,
// * bez duplikatów: klucz każdego powiadomienia trafia do historii (zapisywanej przez wywołującego,
//   także między uruchomieniami programu); klucz zawiera termin, więc po przesunięciu terminu
//   przypomnienia działają od nowa,
// * typy wyciszone są pomijane; więcej niż `PROG_GRUPOWANIA` powiadomień jednego rodzaju naraz →
//   jedno zbiorcze.

import type { ZadanieKalendarza } from "../api/typy";

export interface UstawieniaPowiadomien {
  wlaczone: boolean;
  podsumowanie: boolean;
  /** „HH:MM” */
  godzinaPodsumowania: string;
  przypomnienia: boolean;
  dniPrzed: number[];
  alertPomaranczowy: boolean;
  alertCzerwony: boolean;
  wyciszoneTypy: string[];
}

export const DOMYSLNE_USTAWIENIA_POWIADOMIEN: UstawieniaPowiadomien = {
  wlaczone: true,
  podsumowanie: true,
  godzinaPodsumowania: "07:30",
  przypomnienia: true,
  dniPrzed: [7, 3, 1],
  alertPomaranczowy: true,
  alertCzerwony: true,
  wyciszoneTypy: [],
};

/** Klucz powiadomienia → data (ISO) pokazania. */
export type HistoriaPowiadomien = Record<string, string>;

export type RodzajPowiadomienia = "podsumowanie" | "przypomnienie" | "pomaranczowy" | "czerwony";

export interface Powiadomienie {
  klucz: string;
  rodzaj: RodzajPowiadomienia;
  tytul: string;
  tresc: string;
  /** Zadanie, które otworzyć po kliknięciu (null – zbiorcze / podsumowanie). */
  zadanieId: number | null;
}

export interface Teraz {
  /** „RRRR-MM-DD” */
  data: string;
  /** „HH:MM” */
  godzina: string;
}

export const PROG_GRUPOWANIA = 3;
export const DNI_HISTORII = 90;

function dni(a: string, b: string): number {
  const t = (s: string) => Date.UTC(Number(s.slice(0, 4)), Number(s.slice(5, 7)) - 1, Number(s.slice(8, 10)));
  return Math.round((t(b) - t(a)) / 86_400_000);
}

function dm(iso: string | null | undefined): string {
  return iso ? `${iso.slice(8, 10)}.${iso.slice(5, 7)}` : "—";
}

function nazwa(z: ZadanieKalendarza): string {
  return z.tytul;
}

function kiedy(n: number): string {
  if (n === 0) return "dziś";
  if (n === 1) return "jutro";
  return `za ${n} dni`;
}

/** Poprawna godzina „HH:MM” (00:00–23:59). */
export function poprawnaGodzina(g: string): boolean {
  return /^([01]\d|2[0-3]):[0-5]\d$/.test(g);
}

/** Uzupełnia brakujące / błędne pola ustawień wartościami domyślnymi (dane z localStorage). */
export function normalizujUstawienia(u: Partial<UstawieniaPowiadomien> | null | undefined): UstawieniaPowiadomien {
  const d = DOMYSLNE_USTAWIENIA_POWIADOMIEN;
  const x = u ?? {};
  const dniPrzed = Array.isArray(x.dniPrzed)
    ? [...new Set(x.dniPrzed.filter((n) => Number.isInteger(n) && n >= 0 && n <= 60))].sort((a, b) => b - a)
    : d.dniPrzed;
  return {
    wlaczone: typeof x.wlaczone === "boolean" ? x.wlaczone : d.wlaczone,
    podsumowanie: typeof x.podsumowanie === "boolean" ? x.podsumowanie : d.podsumowanie,
    godzinaPodsumowania: typeof x.godzinaPodsumowania === "string" && poprawnaGodzina(x.godzinaPodsumowania) ? x.godzinaPodsumowania : d.godzinaPodsumowania,
    przypomnienia: typeof x.przypomnienia === "boolean" ? x.przypomnienia : d.przypomnienia,
    dniPrzed,
    alertPomaranczowy: typeof x.alertPomaranczowy === "boolean" ? x.alertPomaranczowy : d.alertPomaranczowy,
    alertCzerwony: typeof x.alertCzerwony === "boolean" ? x.alertCzerwony : d.alertCzerwony,
    wyciszoneTypy: Array.isArray(x.wyciszoneTypy) ? x.wyciszoneTypy.filter((t) => typeof t === "string") : d.wyciszoneTypy,
  };
}

function trwaDzis(z: ZadanieKalendarza, dzis: string): boolean {
  if (z.termin === dzis) return true;
  const od = z.data_od;
  const doDnia = z.data_do ?? z.data_od;
  return z.typ !== "zamowienie" && od <= dzis && dzis <= doDnia;
}

function grupuj(lista: Powiadomienie[], rodzaj: RodzajPowiadomienia, tytulZbiorczy: (n: number) => string): Powiadomienie[] {
  if (lista.length <= PROG_GRUPOWANIA) return lista;
  const nazwy = lista.slice(0, 4).map((p) => p.tytul.replace(/^[^:]*:\s*/, ""));
  const reszta = lista.length - nazwy.length;
  return [
    {
      klucz: lista.map((p) => p.klucz).join("|"),
      rodzaj,
      tytul: tytulZbiorczy(lista.length),
      tresc: nazwy.join("; ") + (reszta > 0 ? ` i ${reszta} innych` : ""),
      zadanieId: null,
    },
  ];
}

export function wyznaczPowiadomienia(
  zadania: ZadanieKalendarza[],
  ustawieniaWejscie: Partial<UstawieniaPowiadomien>,
  teraz: Teraz,
  historia: HistoriaPowiadomien,
): { powiadomienia: Powiadomienie[]; historia: HistoriaPowiadomien } {
  const u = normalizujUstawienia(ustawieniaWejscie);
  const nowa: HistoriaPowiadomien = {};
  for (const [k, d] of Object.entries(historia)) if (dni(d, teraz.data) <= DNI_HISTORII) nowa[k] = d;
  if (!u.wlaczone) return { powiadomienia: [], historia: nowa };

  const aktywne = zadania.filter((z) => !z.wykonano && !u.wyciszoneTypy.includes(z.typ));
  const poRanku = teraz.godzina >= u.godzinaPodsumowania;
  const wynik: Powiadomienie[] = [];
  const oznacz = (k: string) => {
    nowa[k] = teraz.data;
  };

  // 1. Poranne podsumowanie
  const kluczDnia = `podsumowanie:${teraz.data}`;
  if (u.podsumowanie && poRanku && !(kluczDnia in nowa)) {
    oznacz(kluczDnia);
    const dzis = aktywne.filter((z) => trwaDzis(z, teraz.data));
    const zalegle = aktywne.filter((z) => z.stan.stan === "czerwony");
    const pilne = aktywne.filter((z) => z.stan.stan === "pomaranczowy");
    if (dzis.length + zalegle.length + pilne.length > 0) {
      const czesci: string[] = [];
      if (dzis.length) czesci.push(`Dziś: ${dzis.slice(0, 3).map(nazwa).join("; ")}${dzis.length > 3 ? ` i ${dzis.length - 3} innych` : ""}`);
      if (zalegle.length) czesci.push(`Krytyczne / zaległe: ${zalegle.length}`);
      if (pilne.length) czesci.push(`Pilne (pomarańczowe): ${pilne.length}`);
      wynik.push({
        klucz: kluczDnia,
        rodzaj: "podsumowanie",
        tytul: `Plan na dziś (${dm(teraz.data)}): ${dzis.length} ${dzis.length === 1 ? "zadanie" : "zadań"}`,
        tresc: czesci.join(". "),
        zadanieId: null,
      });
    }
  }

  // 2. Alerty stanów (od razu)
  const czerwone: Powiadomienie[] = [];
  const pomaranczowe: Powiadomienie[] = [];
  for (const z of aktywne) {
    const s = z.stan.stan;
    const kCz = `stan:${z.id}:czerwony:${z.termin}`;
    const kPo = `stan:${z.id}:pomaranczowy:${z.termin}`;
    if (s === "czerwony") {
      const juzCz = kCz in nowa;
      oznacz(kPo); // pomarańczowy już nieaktualny – nie pokazujemy go po fakcie
      if (u.alertCzerwony && !juzCz) {
        const nowy = z.stan.nowy_termin_montazu ? ` Możliwy nowy termin montażu: ${dm(z.stan.nowy_termin_montazu)}.` : "";
        czerwone.push({ klucz: kCz, rodzaj: "czerwony", tytul: `Krytyczne: ${nazwa(z)}`, tresc: `${z.stan.opis}.${nowy}`.replace("..", "."), zadanieId: z.id });
      }
      oznacz(kCz);
    } else if (s === "pomaranczowy") {
      if (u.alertPomaranczowy && !(kPo in nowa)) {
        pomaranczowe.push({ klucz: kPo, rodzaj: "pomaranczowy", tytul: `Zbliżamy się do krytycznego momentu: ${nazwa(z)}`, tresc: z.stan.opis, zadanieId: z.id });
      }
      oznacz(kPo);
    }
  }
  wynik.push(...grupuj(czerwone, "czerwony", (n) => `Krytyczne: ${n} zadań po terminie`));
  wynik.push(...grupuj(pomaranczowe, "pomaranczowy", (n) => `Zbliżamy się do krytycznego momentu: ${n} zadań`));

  // 3. Przypomnienia N dni przed terminem (od godziny porannej)
  const progi = [...u.dniPrzed].sort((a, b) => a - b);
  const przypomnienia: Powiadomienie[] = [];
  if (u.przypomnienia && poRanku && progi.length > 0) {
    for (const z of aktywne) {
      if (z.stan.stan === "czerwony" || z.stan.stan === "pomaranczowy") continue; // ma własny alert
      const doTerminu = dni(teraz.data, z.termin);
      if (doTerminu < 0) continue;
      const prog = progi.find((n) => doTerminu <= n);
      if (prog === undefined) continue;
      const klucz = `przed:${z.id}:${z.termin}:${prog}`;
      const juzPokazane = klucz in nowa;
      // progi większe od obecnego uznajemy za pokazane (brak „7 dni” po „3 dniach”)
      for (const n of progi) if (n >= prog) oznacz(`przed:${z.id}:${z.termin}:${n}`);
      if (juzPokazane) continue;
      przypomnienia.push({ klucz, rodzaj: "przypomnienie", tytul: `Termin ${kiedy(doTerminu)}: ${nazwa(z)}`, tresc: `${dm(z.termin)} – ${z.stan.opis}`, zadanieId: z.id });
    }
  }
  wynik.push(...grupuj(przypomnienia, "przypomnienie", (n) => `Zbliżające się terminy: ${n}`));

  return { powiadomienia: wynik, historia: nowa };
}
