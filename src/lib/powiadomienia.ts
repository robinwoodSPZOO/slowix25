// Logika „co i kiedy powiadomić” – CZYSTA funkcja (bez zegara, bez sieci, bez localStorage).
// Wejście: zadania niewykonane (z API), ustawienia stanowiska, „teraz” (data + godzina), historia
// pokazanych powiadomień. Wyjście: lista powiadomień do pokazania (najwyżej `MAKS_NARAZ`) i nowa
// historia – zawiera klucze TYLKO tego, co pokazano (pojedynczo albo w zbiorczym), oraz celowo
// pominiętych (np. pomarańczowy po fakcie, typ wyciszony).
//
// Zasady:
// * godziny pracy stanowiska: od godziny porannego podsumowania (domyślnie 7:30) do
//   `koniecPracy` (domyślnie 18:00); poza nimi nic nie wyskakuje – alerty czekają do rana
//   i wchodzą do podsumowania,
// * poranne podsumowanie – raz dziennie; tytuł mówi o tym, co najważniejsze (zaległe, dziś),
// * przypomnienia N dni przed (domyślnie 7, 3, 1): dla montażu i warsztatu – przed POCZĄTKIEM
//   (`data_od`), dla zamówienia – przed datą planową, dla pozostałych – przed terminem; jeśli program
//   nie działał w dniu progu – przy najbliższym progu, który nadal obowiązuje,
// * alert przy wejściu w pomarańczowy i w czerwony (czerwony bez spóźnionego pomarańczowego),
// * bez powtórek: klucz historii to id zadania + próg / stan (bez daty); w wartości zapisujemy
//   datę pokazania i termin, którego dotyczyło. Przypomnienie wraca dopiero, gdy termin przesunie
//   się o więcej niż `PRZESUNIECIE_OD_NOWA` dni (przeliczenie o 1–2 dni go nie powtarza),
// * w dniu terminu zadanie jest tylko w podsumowaniu („Dziś: …”), bez osobnego „Termin dziś”,
// * typy wyciszone: nic nie pokazujemy, ale klucze są zapisywane – po zdjęciu wyciszenia nie
//   przychodzą zaległe alerty sprzed wyciszenia,
// * > `PROG_GRUPOWANIA` jednego rodzaju → jedno zbiorcze; ponad `MAKS_NARAZ` łącznie → reszta w
//   jednym zbiorczym „i jeszcze N”.

import type { ZadanieKalendarza } from "../api/typy";
import { liczba, odmiana, ZADANIE, DZIEN, INNE } from "./odmiana";

export interface UstawieniaPowiadomien {
  wlaczone: boolean;
  podsumowanie: boolean;
  /** „HH:MM” – także początek godzin pracy */
  godzinaPodsumowania: string;
  /** „HH:MM” – koniec godzin pracy (alerty po tej godzinie czekają do rana) */
  koniecPracy: string;
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
  koniecPracy: "18:00",
  przypomnienia: true,
  dniPrzed: [7, 3, 1],
  alertPomaranczowy: true,
  alertCzerwony: true,
  wyciszoneTypy: [],
};

/** Klucz powiadomienia → „RRRR-MM-DD” (data pokazania) albo „RRRR-MM-DD|RRRR-MM-DD” (+ termin). */
export type HistoriaPowiadomien = Record<string, string>;

export type RodzajPowiadomienia = "podsumowanie" | "przypomnienie" | "pomaranczowy" | "czerwony" | "zbiorcze";

export interface Powiadomienie {
  klucz: string;
  rodzaj: RodzajPowiadomienia;
  tytul: string;
  tresc: string;
  /** Zadanie, które otworzyć po kliknięciu (null – zbiorcze / podsumowanie). */
  zadanieId: number | null;
  /** Klucze historii obejmowane przez to powiadomienie (zbiorcze – wiele). */
  klucze: string[];
}

export interface Teraz {
  /** „RRRR-MM-DD” */
  data: string;
  /** „HH:MM” */
  godzina: string;
}

export const PROG_GRUPOWANIA = 3;
export const MAKS_NARAZ = 5;
export const DNI_HISTORII = 90;
export const PRZESUNIECIE_OD_NOWA = 7;

const ISO = /^\d{4}-\d{2}-\d{2}$/;

function dni(a: string, b: string): number {
  const t = (s: string) => Date.UTC(Number(s.slice(0, 4)), Number(s.slice(5, 7)) - 1, Number(s.slice(8, 10)));
  return Math.round((t(b) - t(a)) / 86_400_000);
}

function dm(iso: string | null | undefined): string {
  return iso ? `${iso.slice(8, 10)}.${iso.slice(5, 7)}` : "—";
}

function kiedy(n: number): string {
  if (n === 0) return "dziś";
  if (n === 1) return "jutro";
  return `za ${liczba(n, DZIEN)}`;
}

/** Poprawna godzina „HH:MM” (00:00–23:59). */
export function poprawnaGodzina(g: string): boolean {
  return /^([01]\d|2[0-3]):[0-5]\d$/.test(g);
}

/** Uzupełnia brakujące / błędne pola ustawień wartościami domyślnymi (dane z localStorage). */
export function normalizujUstawienia(u: unknown): UstawieniaPowiadomien {
  const d = DOMYSLNE_USTAWIENIA_POWIADOMIEN;
  const x = (u && typeof u === "object" ? u : {}) as Partial<Record<keyof UstawieniaPowiadomien, unknown>>;
  const bool = (k: "wlaczone" | "podsumowanie" | "przypomnienia" | "alertPomaranczowy" | "alertCzerwony") => (typeof x[k] === "boolean" ? (x[k] as boolean) : d[k]);
  const godz = (k: "godzinaPodsumowania" | "koniecPracy") => (typeof x[k] === "string" && poprawnaGodzina(x[k] as string) ? (x[k] as string) : d[k]);
  const dniPrzed = Array.isArray(x.dniPrzed)
    ? [...new Set((x.dniPrzed as unknown[]).filter((n): n is number => typeof n === "number" && Number.isInteger(n) && n >= 0 && n <= 60))].sort((a, b) => b - a)
    : d.dniPrzed;
  const wynik: UstawieniaPowiadomien = {
    wlaczone: bool("wlaczone"),
    podsumowanie: bool("podsumowanie"),
    godzinaPodsumowania: godz("godzinaPodsumowania"),
    koniecPracy: godz("koniecPracy"),
    przypomnienia: bool("przypomnienia"),
    dniPrzed,
    alertPomaranczowy: bool("alertPomaranczowy"),
    alertCzerwony: bool("alertCzerwony"),
    wyciszoneTypy: Array.isArray(x.wyciszoneTypy) ? (x.wyciszoneTypy as unknown[]).filter((t): t is string => typeof t === "string") : d.wyciszoneTypy,
  };
  if (wynik.koniecPracy <= wynik.godzinaPodsumowania) wynik.koniecPracy = "23:59";
  return wynik;
}

/** Historia z pamięci o nieoczekiwanym kształcie – zostają tylko poprawne wpisy. */
export function normalizujHistorie(h: unknown): HistoriaPowiadomien {
  if (!h || typeof h !== "object" || Array.isArray(h)) return {};
  const wynik: HistoriaPowiadomien = {};
  for (const [k, v] of Object.entries(h as Record<string, unknown>)) {
    if (typeof v !== "string") continue;
    const [data, ref] = v.split("|");
    if (!data || !ISO.test(data) || (ref !== undefined && !ISO.test(ref))) continue;
    wynik[k] = v;
  }
  return wynik;
}

/** Data, względem której liczymy „N dni przed”. */
export function dataPrzypomnienia(z: ZadanieKalendarza): string {
  if (z.typ === "montaz" || z.typ === "warsztat") return z.data_od;
  if (z.typ === "zamowienie") return z.data_planowa ?? z.termin;
  return z.termin;
}

function trwaDzis(z: ZadanieKalendarza, dzis: string): boolean {
  if (z.typ === "zamowienie") return (z.data_planowa ?? z.termin) === dzis;
  if (z.termin === dzis) return true;
  const od = z.data_od;
  const doDnia = z.data_do ?? z.data_od;
  return od <= dzis && dzis <= doDnia;
}

function lista(nazwy: string[], max = 3): string {
  const r = nazwy.length - max;
  return nazwy.slice(0, max).join("; ") + (r > 0 ? ` i ${r} ${odmiana(r, INNE)}` : "");
}

interface Kandydat extends Powiadomienie {
  waga: number;
}

function grupuj(l: Kandydat[], rodzaj: RodzajPowiadomienia, waga: number, tytul: (n: number) => string): Kandydat[] {
  if (l.length <= PROG_GRUPOWANIA) return l;
  return [
    {
      klucz: l.map((p) => p.klucz).join("|"),
      klucze: l.flatMap((p) => p.klucze),
      rodzaj,
      tytul: tytul(l.length),
      tresc: lista(l.map((p) => p.tytul.replace(/^[^:]*:\s*/, "")), 4),
      zadanieId: null,
      waga,
    },
  ];
}

/** Czy wpis historii obejmuje ten termin (przesunięcie o więcej niż 7 dni = sprawa od nowa). */
function pokazane(h: HistoriaPowiadomien, klucz: string, termin: string): boolean {
  const v = h[klucz];
  if (!v) return false;
  const ref = v.split("|")[1];
  return !ref || Math.abs(dni(ref, termin)) <= PRZESUNIECIE_OD_NOWA;
}

export function wyznaczPowiadomienia(
  zadania: ZadanieKalendarza[],
  ustawieniaWejscie: Partial<UstawieniaPowiadomien>,
  teraz: Teraz,
  historiaWejscie: HistoriaPowiadomien,
): { powiadomienia: Powiadomienie[]; historia: HistoriaPowiadomien } {
  const u = normalizujUstawienia(ustawieniaWejscie);
  const historia = normalizujHistorie(historiaWejscie);
  const nowa: HistoriaPowiadomien = {};
  for (const [k, v] of Object.entries(historia)) if (dni(v.slice(0, 10), teraz.data) <= DNI_HISTORII) nowa[k] = v;
  if (!u.wlaczone) return { powiadomienia: [], historia: nowa };

  const wPracy = teraz.godzina >= u.godzinaPodsumowania && teraz.godzina < u.koniecPracy;
  const poRanku = teraz.godzina >= u.godzinaPodsumowania;
  const niewykonane = zadania.filter((z) => !z.wykonano);
  const wyciszone = (z: ZadanieKalendarza) => u.wyciszoneTypy.includes(z.typ);
  const oznacz = (k: string, termin?: string) => {
    nowa[k] = termin ? `${teraz.data}|${termin}` : teraz.data;
  };
  const kandydaci: Kandydat[] = [];

  // Zadania, które wyszły ze stanu pomarańczowego/czerwonego – po ponownym wejściu znów alert.
  for (const z of niewykonane) {
    if (z.stan.stan !== "czerwony") delete nowa[`stan:${z.id}:czerwony`];
    if (z.stan.stan !== "czerwony" && z.stan.stan !== "pomaranczowy") delete nowa[`stan:${z.id}:pomaranczowy`];
  }

  // 1. Poranne podsumowanie (raz dziennie, po godzinie porannej – także po 18:00, jeśli program
  //    wcześniej nie działał). Obejmuje alerty, które czekały poza godzinami pracy.
  const kluczDnia = `podsumowanie:${teraz.data}`;
  const podsumowanieTeraz = u.podsumowanie && poRanku && !(kluczDnia in nowa);
  if (podsumowanieTeraz) {
    const widoczne = niewykonane.filter((z) => !wyciszone(z));
    const dzis = widoczne.filter((z) => trwaDzis(z, teraz.data) && z.stan.stan !== "czerwony");
    const zalegle = widoczne.filter((z) => z.stan.stan === "czerwony");
    const pilne = widoczne.filter((z) => z.stan.stan === "pomaranczowy");
    const klucze = [kluczDnia];
    for (const z of [...zalegle, ...pilne]) {
      const k = `stan:${z.id}:${z.stan.stan}`;
      klucze.push(k);
      if (z.stan.stan === "czerwony") klucze.push(`stan:${z.id}:pomaranczowy`);
    }
    if (dzis.length + zalegle.length + pilne.length > 0) {
      const czesci: string[] = [];
      if (zalegle.length) czesci.push(`Zaległe i krytyczne: ${lista(zalegle.map((z) => z.tytul), 2)}`);
      if (pilne.length) czesci.push(`Pilne: ${lista(pilne.map((z) => z.tytul), 2)}`);
      if (dzis.length) czesci.push(`Dziś: ${lista(dzis.map((z) => z.tytul))}`);
      const tytulCzesci: string[] = [];
      if (zalegle.length) tytulCzesci.push(`${liczba(zalegle.length, ZADANIE)} po terminie`);
      if (pilne.length) tytulCzesci.push(`${pilne.length} pilne`);
      tytulCzesci.push(dzis.length ? `${liczba(dzis.length, ZADANIE)} na dziś` : "nic nowego na dziś");
      kandydaci.push({ klucz: kluczDnia, klucze, rodzaj: "podsumowanie", tytul: `Plan dnia ${dm(teraz.data)}: ${tytulCzesci.join(", ")}`, tresc: czesci.join(". "), zadanieId: null, waga: 2 });
    } else oznacz(kluczDnia);
    // Typy wyciszone: klucze bez pokazywania.
    for (const z of niewykonane.filter(wyciszone)) if (z.stan.stan === "czerwony" || z.stan.stan === "pomaranczowy") oznacz(`stan:${z.id}:${z.stan.stan}`, z.termin);
  }
  const wPodsumowaniu = new Set(podsumowanieTeraz ? (kandydaci[0]?.klucze ?? []) : []);

  // 2. Alerty stanów – tylko w godzinach pracy (poza nimi czekają do rana / do podsumowania).
  const czerwone: Kandydat[] = [];
  const pomaranczowe: Kandydat[] = [];
  if (wPracy) {
    for (const z of niewykonane) {
      const s = z.stan.stan;
      if (s !== "czerwony" && s !== "pomaranczowy") continue;
      const k = `stan:${z.id}:${s}`;
      if (wPodsumowaniu.has(k) || pokazane(nowa, k, z.termin)) continue;
      if (s === "czerwony") oznacz(`stan:${z.id}:pomaranczowy`, z.termin); // pomarańczowy po fakcie – nie pokazujemy
      const wlaczony = s === "czerwony" ? u.alertCzerwony : u.alertPomaranczowy;
      if (wyciszone(z) || !wlaczony) {
        oznacz(k, z.termin);
        continue;
      }
      if (s === "czerwony") {
        const nowy = z.stan.nowy_termin_montazu ? ` Możliwy nowy termin montażu: ${dm(z.stan.nowy_termin_montazu)}.` : "";
        czerwone.push({ klucz: k, klucze: [k], rodzaj: "czerwony", tytul: `Krytyczne: ${z.tytul}`, tresc: `${z.stan.opis}.${nowy}`.replace("..", "."), zadanieId: z.id, waga: 0 });
      } else {
        pomaranczowe.push({ klucz: k, klucze: [k], rodzaj: "pomaranczowy", tytul: `Zbliżamy się do krytycznego momentu: ${z.tytul}`, tresc: z.stan.opis, zadanieId: z.id, waga: 1 });
      }
    }
  }
  kandydaci.push(...grupuj(czerwone, "czerwony", 0, (n) => `Krytyczne: ${liczba(n, ZADANIE)} po terminie`));
  kandydaci.push(...grupuj(pomaranczowe, "pomaranczowy", 1, (n) => `Zbliżamy się do krytycznego momentu: ${liczba(n, ZADANIE)}`));

  // 3. Przypomnienia N dni przed (w godzinach pracy)
  const progi = [...u.dniPrzed].sort((a, b) => a - b);
  const przypomnienia: Kandydat[] = [];
  if (u.przypomnienia && wPracy && progi.length > 0) {
    for (const z of niewykonane) {
      if (z.stan.stan === "czerwony" || z.stan.stan === "pomaranczowy") continue; // ma własny alert
      const termin = dataPrzypomnienia(z);
      const doTerminu = dni(teraz.data, termin);
      if (doTerminu < 0) continue;
      const prog = progi.find((n) => doTerminu <= n);
      if (prog === undefined) continue;
      const klucz = `przed:${z.id}:${prog}`;
      const juz = pokazane(nowa, klucz, termin);
      // większe progi uznajemy za pokazane (brak „7 dni” po „3 dniach”)
      for (const n of progi) if (n > prog && !pokazane(nowa, `przed:${z.id}:${n}`, termin)) oznacz(`przed:${z.id}:${n}`, termin);
      if (juz) continue;
      // W dniu terminu zadanie jest w podsumowaniu („Dziś: …”) – bez osobnego „Termin dziś”.
      if (doTerminu === 0 && u.podsumowanie) {
        oznacz(klucz, termin);
        continue;
      }
      if (wyciszone(z)) {
        oznacz(klucz, termin);
        continue;
      }
      const co = z.typ === "montaz" || z.typ === "warsztat" ? "Początek" : z.typ === "zamowienie" ? "Zamówienie planowo" : "Termin";
      przypomnienia.push({ klucz, klucze: [klucz], rodzaj: "przypomnienie", tytul: `${co} ${kiedy(doTerminu)}: ${z.tytul}`, tresc: `${dm(termin)} – ${z.stan.opis}`, zadanieId: z.id, waga: 3 });
    }
  }
  kandydaci.push(...grupuj(przypomnienia, "przypomnienie", 3, (n) => `Zbliżające się terminy: ${liczba(n, ZADANIE)}`));

  // 4. Limit: najwyżej MAKS_NARAZ; nadmiar w jednym zbiorczym (nic nie przepada bez śladu).
  kandydaci.sort((a, b) => a.waga - b.waga);
  let wynik: Kandydat[] = kandydaci;
  if (kandydaci.length > MAKS_NARAZ) {
    const reszta = kandydaci.slice(MAKS_NARAZ - 1);
    const n = reszta.reduce((s, p) => s + Math.max(1, p.klucze.filter((k) => !k.startsWith("podsumowanie:")).length), 0);
    wynik = [
      ...kandydaci.slice(0, MAKS_NARAZ - 1),
      {
        klucz: `zbiorcze:${teraz.data}:${reszta.map((p) => p.klucz).join("|")}`,
        klucze: reszta.flatMap((p) => p.klucze),
        rodzaj: "zbiorcze",
        tytul: `I jeszcze ${liczba(n, ZADANIE)} wymaga uwagi`,
        tresc: `${lista(reszta.map((p) => p.tytul), 3)}. Szczegóły w kalendarzu (Agenda).`,
        zadanieId: null,
        waga: 9,
      },
    ];
  }
  const terminy = new Map(niewykonane.map((z) => [z.id, z] as const));
  for (const p of wynik) {
    for (const k of p.klucze) {
      const id = Number(k.split(":")[1]);
      const z = terminy.get(id);
      oznacz(k, k.startsWith("podsumowanie:") || !z ? undefined : k.startsWith("przed:") ? dataPrzypomnienia(z) : z.termin);
    }
  }
  return { powiadomienia: wynik.map((p): Powiadomienie => ({ klucz: p.klucz, rodzaj: p.rodzaj, tytul: p.tytul, tresc: p.tresc, zadanieId: p.zadanieId, klucze: p.klucze })), historia: nowa };
}
