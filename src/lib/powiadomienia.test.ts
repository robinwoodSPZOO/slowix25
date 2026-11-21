import { describe, expect, it } from "vitest";
import type { StanOstrzezenia } from "../api/typy";
import { zadanie } from "../moduly/kalendarz/fabryka";
import { DOMYSLNE_USTAWIENIA_POWIADOMIEN as D, MAKS_NARAZ, normalizujHistorie, normalizujUstawienia, PROG_GRUPOWANIA, wyznaczPowiadomienia, type HistoriaPowiadomien } from "./powiadomienia";

const stan = (s: StanOstrzezenia, nowy: string | null = null) => ({ stan: s, opis: `Opis ${s}`, dni_do_terminu: 0, przesuniecie_montazu_dni: nowy ? 5 : null, nowy_termin_montazu: nowy });
const RANO = { data: "2027-01-11", godzina: "08:00" };
const NOC = { data: "2027-01-11", godzina: "05:00" };
const DZIEN = { data: "2027-01-11", godzina: "10:00" };
const BEZ_POD = { ...D, podsumowanie: false };

/** Kolejne „uruchomienia” z tą samą historią (jak po restarcie programu). */
function przebieg(zadania: Parameters<typeof wyznaczPowiadomienia>[0], chwile: { data: string; godzina: string }[], u = D) {
  let h: HistoriaPowiadomien = {};
  return chwile.map((t) => {
    const w = wyznaczPowiadomienia(zadania, u, t, h);
    h = JSON.parse(JSON.stringify(w.historia)) as HistoriaPowiadomien; // zapis i odczyt z pamięci
    return w.powiadomienia;
  });
}

describe("poranne podsumowanie", () => {
  const z = [zadanie({ id: 1, typ: "pomiar", data_od: "2027-01-11", termin: "2027-01-11" })];
  it("raz dziennie, od ustawionej godziny", () => {
    const [noc, rano, poludnie] = przebieg(z, [NOC, RANO, { data: "2027-01-11", godzina: "12:00" }]);
    expect(noc!.filter((p) => p.rodzaj === "podsumowanie")).toHaveLength(0);
    expect(rano!.filter((p) => p.rodzaj === "podsumowanie")).toHaveLength(1);
    expect(rano![0]!.tytul).toContain("1 zadanie");
    expect(poludnie).toHaveLength(0);
  });
  it("następnego dnia – znowu", () => {
    const [, drugi] = przebieg(z, [RANO, { data: "2027-01-12", godzina: "09:00" }]);
    expect(drugi!.some((p) => p.rodzaj === "podsumowanie")).toBe(false); // brak zadań dziś → nic
    const z2 = [...z, zadanie({ id: 2, data_od: "2027-01-12", termin: "2027-01-12" })];
    const [, d2] = przebieg(z2, [RANO, { data: "2027-01-12", godzina: "09:00" }]);
    expect(d2!.filter((p) => p.rodzaj === "podsumowanie")).toHaveLength(1);
  });
  it("własna godzina podsumowania", () => {
    const [w] = przebieg(z, [{ data: "2027-01-11", godzina: "08:00" }], { ...D, godzinaPodsumowania: "09:15" });
    expect(w).toHaveLength(0);
  });
});

describe("przypomnienia N dni przed terminem", () => {
  const z = [zadanie({ id: 7, typ: "montaz", data_od: "2027-01-18", termin: "2027-01-18" })];
  it("7, 3 i 1 dzień przed – każde raz", () => {
    const dni = ["2027-01-10", "2027-01-11", "2027-01-12", "2027-01-15", "2027-01-16", "2027-01-17", "2027-01-17"];
    const wyniki = przebieg(z, dni.map((d) => ({ data: d, godzina: "10:00" })), { ...D, podsumowanie: false });
    expect(wyniki.map((w) => w.length)).toEqual([0, 1, 0, 1, 0, 1, 0]);
    expect(wyniki[1]![0]!.tytul).toContain("za 7 dni");
    expect(wyniki[5]![0]!.tytul).toContain("jutro");
  });
  it("program wyłączony w dniu progu: przypomnienie przy kolejnym progu, bez zaległych wcześniejszych", () => {
    const wyniki = przebieg(z, [{ data: "2027-01-16", godzina: "10:00" }, { data: "2027-01-16", godzina: "11:00" }], { ...D, podsumowanie: false });
    expect(wyniki[0]).toHaveLength(1);
    expect(wyniki[0]![0]!.klucz).toBe("przed:7:3");
    expect(wyniki[1]).toHaveLength(0);
  });
  it("W9d: przesunięcie terminu o dzień przez przeliczenie NIE powtarza przypomnienia", () => {
    let h: HistoriaPowiadomien = {};
    h = wyznaczPowiadomienia(z, BEZ_POD, { data: "2027-01-15", godzina: "10:00" }, h).historia; // „3 dni”
    const plus1 = [{ ...z[0]!, termin: "2027-01-19", data_od: "2027-01-19" }];
    const w1 = wyznaczPowiadomienia(plus1, BEZ_POD, { data: "2027-01-16", godzina: "10:00" }, h);
    expect(w1.powiadomienia).toHaveLength(0);
    const plus2 = [{ ...z[0]!, termin: "2027-01-20", data_od: "2027-01-20" }];
    expect(wyznaczPowiadomienia(plus2, BEZ_POD, { data: "2027-01-17", godzina: "10:00" }, w1.historia).powiadomienia).toHaveLength(0);
  });
  it("przesunięcie o kilka tygodni – przypomnienia od nowa", () => {
    let h: HistoriaPowiadomien = {};
    h = wyznaczPowiadomienia(z, BEZ_POD, { data: "2027-01-17", godzina: "10:00" }, h).historia;
    const przesuniete = [{ ...z[0]!, termin: "2027-02-08", data_od: "2027-02-08" }];
    const w = wyznaczPowiadomienia(przesuniete, BEZ_POD, { data: "2027-02-05", godzina: "10:00" }, h);
    expect(w.powiadomienia.map((p) => p.klucz)).toEqual(["przed:7:3"]);
  });
  it("przed godziną poranną – czekają; własne progi", () => {
    expect(przebieg(z, [{ data: "2027-01-17", godzina: "06:00" }], { ...D, podsumowanie: false })[0]).toHaveLength(0);
    const w = przebieg(z, [{ data: "2027-01-13", godzina: "10:00" }], { ...D, podsumowanie: false, dniPrzed: [5] })[0]!;
    expect(w.map((p) => p.klucz)).toEqual(["przed:7:5"]);
  });
});

describe("alerty pomarańczowy i czerwony", () => {
  it("osobny alert przy wejściu w pomarańczowy, potem w czerwony – każdy raz (w godzinach pracy)", () => {
    const po = [zadanie({ id: 3, typ: "zamowienie", data_od: "2027-01-05", termin: "2027-01-08", stan: stan("pomaranczowy") })];
    const cz = [{ ...po[0]!, stan: stan("czerwony", "2027-02-15") }];
    let h: HistoriaPowiadomien = {};
    const a = wyznaczPowiadomienia(po, BEZ_POD, DZIEN, h);
    expect(a.powiadomienia.map((p) => p.rodzaj)).toEqual(["pomaranczowy"]);
    expect(a.powiadomienia[0]!.tytul).toContain("Zbliżamy się do krytycznego momentu");
    h = a.historia;
    expect(wyznaczPowiadomienia(po, BEZ_POD, DZIEN, h).powiadomienia).toHaveLength(0);
    const b = wyznaczPowiadomienia(cz, BEZ_POD, DZIEN, h);
    expect(b.powiadomienia.map((p) => p.rodzaj)).toEqual(["czerwony"]);
    expect(b.powiadomienia[0]!.tresc).toContain("15.02");
    expect(wyznaczPowiadomienia(cz, BEZ_POD, DZIEN, b.historia).powiadomienia).toHaveLength(0);
  });
  it("od razu czerwony – bez spóźnionego pomarańczowego", () => {
    const cz = [zadanie({ id: 4, data_od: "2027-01-05", stan: stan("czerwony") })];
    const a = wyznaczPowiadomienia(cz, BEZ_POD, DZIEN, {});
    expect(a.powiadomienia.map((p) => p.rodzaj)).toEqual(["czerwony"]);
  });
  it("zadania pilne nie dostają dodatkowo przypomnienia „N dni przed”", () => {
    const po = [zadanie({ id: 5, data_od: "2027-01-12", termin: "2027-01-12", stan: stan("pomaranczowy") })];
    const w = wyznaczPowiadomienia(po, { ...D, podsumowanie: false }, RANO, {});
    expect(w.powiadomienia.map((p) => p.rodzaj)).toEqual(["pomaranczowy"]);
  });
  it("wiele naraz → jedno zbiorcze; klucze wszystkich w historii", () => {
    const cz = Array.from({ length: PROG_GRUPOWANIA + 2 }, (_, i) => zadanie({ id: 50 + i, data_od: "2027-01-05", stan: stan("czerwony") }));
    const a = wyznaczPowiadomienia(cz, BEZ_POD, DZIEN, {});
    expect(a.powiadomienia).toHaveLength(1);
    expect(a.powiadomienia[0]!.tytul).toContain(`${PROG_GRUPOWANIA + 2} zadań po terminie`);
    expect(Object.keys(a.historia).filter((k) => k.startsWith("stan:5")).length).toBeGreaterThanOrEqual(PROG_GRUPOWANIA + 2);
    expect(wyznaczPowiadomienia(cz, BEZ_POD, DZIEN, a.historia).powiadomienia).toHaveLength(0);
  });
  it("wyłączone alerty i wyciszone typy", () => {
    const cz = [zadanie({ id: 8, typ: "platnosc", data_od: "2027-01-05", stan: stan("czerwony") })];
    expect(wyznaczPowiadomienia(cz, { ...BEZ_POD, alertCzerwony: false }, DZIEN, {}).powiadomienia).toHaveLength(0);
    expect(wyznaczPowiadomienia(cz, { ...BEZ_POD, wyciszoneTypy: ["platnosc"] }, DZIEN, {}).powiadomienia).toHaveLength(0);
    expect(wyznaczPowiadomienia(cz, { ...D, wlaczone: false }, DZIEN, {}).powiadomienia).toHaveLength(0);
  });
  it("wykonane zadania są pomijane", () => {
    const cz = [zadanie({ id: 9, data_od: "2027-01-05", stan: stan("czerwony"), wykonano: "2027-01-06" })];
    expect(wyznaczPowiadomienia(cz, D, RANO, {}).powiadomienia).toHaveLength(0);
  });
});

describe("historia i ustawienia", () => {
  it("stare wpisy (ponad 90 dni) są usuwane", () => {
    const w = wyznaczPowiadomienia([], D, RANO, { "przed:1:1": "2026-09-01", "przed:2:1": "2027-01-01|2027-01-03" });
    expect(Object.keys(w.historia)).toEqual(["przed:2:1", "podsumowanie:2027-01-11"]);
  });
  it("normalizacja ustawień z pamięci", () => {
    const u = normalizujUstawienia({ dniPrzed: [1, 7, 7, -2, 3.5, 3], godzinaPodsumowania: "25:00" } as never);
    expect(u.dniPrzed).toEqual([7, 3, 1]);
    expect(u.godzinaPodsumowania).toBe(D.godzinaPodsumowania);
    expect(normalizujUstawienia(null)).toEqual(D);
    expect(normalizujUstawienia("śmieci")).toEqual(D);
    expect(normalizujUstawienia({ koniecPracy: "06:00" }).koniecPracy).toBe("23:59");
  });
  it("W9a: uszkodzona historia (wartości nie-tekstowe, tablica) nie powoduje wyjątku", () => {
    expect(normalizujHistorie([1, 2])).toEqual({});
    expect(normalizujHistorie(null)).toEqual({});
    expect(normalizujHistorie({ a: 5, b: { x: 1 }, c: "2027-01-01", d: "zła", e: "2027-01-01|xx" })).toEqual({ c: "2027-01-01" });
    const z = [zadanie({ id: 1, data_od: "2027-01-05", stan: stan("czerwony") })];
    expect(() => wyznaczPowiadomienia(z, D, DZIEN, { "stan:1:czerwony": 42 } as unknown as HistoriaPowiadomien)).not.toThrow();
  });
});

describe("limit powiadomień naraz (W3)", () => {
  it("ponad 5 – reszta w jednym zbiorczym; w historii tylko pokazane (pojedynczo lub w zbiorczym)", () => {
    const cz = [1, 2, 3].map((i) => zadanie({ id: i, data_od: "2027-01-05", tytul: `Czerwone ${i}`, stan: stan("czerwony") }));
    const po = [4, 5, 6].map((i) => zadanie({ id: i, data_od: "2027-01-13", tytul: `Pomarańczowe ${i}`, stan: stan("pomaranczowy") }));
    const prz = [7, 8, 9].map((i) => zadanie({ id: i, data_od: "2027-01-14", termin: "2027-01-14", tytul: `Pomiar ${i}` }));
    const w = wyznaczPowiadomienia([...cz, ...po, ...prz], BEZ_POD, DZIEN, {});
    expect(w.powiadomienia).toHaveLength(MAKS_NARAZ);
    const ost = w.powiadomienia.at(-1)!;
    expect(ost.rodzaj).toBe("zbiorcze");
    expect(ost.tytul).toMatch(/I jeszcze \d+ zada/);
    const objete = new Set(w.powiadomienia.flatMap((p) => p.klucze));
    for (const k of Object.keys(w.historia)) if (k.startsWith("stan:") && !k.endsWith(":pomaranczowy")) expect(objete.has(k)).toBe(true);
    for (const id of [7, 8, 9]) expect(objete.has(`przed:${id}:3`)).toBe(true);
    // nic nie przepada: drugi krok nie pokazuje tego samego jeszcze raz
    expect(wyznaczPowiadomienia([...cz, ...po, ...prz], BEZ_POD, DZIEN, w.historia).powiadomienia).toHaveLength(0);
  });
});

describe("zadania wielodniowe i zamówienia (W4)", () => {
  it("warsztat: przypomnienie przed POCZĄTKIEM, nie przed końcem", () => {
    const w = [zadanie({ id: 20, typ: "warsztat", data_od: "2027-01-14", data_do: "2027-01-27", termin: "2027-01-27", tytul: "Składanie: Kuchnia testowa" })];
    const r = wyznaczPowiadomienia(w, BEZ_POD, DZIEN, {});
    expect(r.powiadomienia.map((p) => p.tytul)).toEqual(["Początek za 3 dni: Składanie: Kuchnia testowa"]);
    expect(r.powiadomienia[0]!.tresc).toContain("14.01");
  });
  it("montaż wielodniowy: 1 dzień przed startem", () => {
    const m = [zadanie({ id: 21, typ: "montaz", data_od: "2027-01-12", data_do: "2027-01-14", termin: "2027-01-14" })];
    expect(wyznaczPowiadomienia(m, BEZ_POD, DZIEN, {}).powiadomienia[0]!.tytul).toContain("jutro");
  });
  it("zamówienie: względem daty planowej", () => {
    const z = [zadanie({ id: 22, typ: "zamowienie", data_od: "2027-01-05", data_do: "2027-01-30", data_planowa: "2027-01-18", termin: "2027-01-18" })];
    const r = wyznaczPowiadomienia(z, BEZ_POD, DZIEN, {});
    expect(r.powiadomienia.map((p) => p.klucz)).toEqual(["przed:22:7"]);
    expect(r.powiadomienia[0]!.tytul).toContain("Zamówienie planowo za 7 dni");
  });
});

describe("teksty i dublowanie (W9 b, c, e, f)", () => {
  it("tytuł podsumowania przy samych zaległych mówi o zaległych", () => {
    const zal = Array.from({ length: 21 }, (_, i) => zadanie({ id: 100 + i, data_od: "2026-10-05", stan: stan("czerwony") }));
    const w = wyznaczPowiadomienia(zal, D, RANO, {});
    const p = w.powiadomienia.find((x) => x.rodzaj === "podsumowanie")!;
    expect(p.tytul).toContain("21 zadań po terminie");
    expect(p.tytul).not.toContain("0 zadań");
    expect(w.powiadomienia.filter((x) => x.rodzaj === "czerwony")).toHaveLength(0); // są w podsumowaniu
  });
  it("odmiana: 2 zadania, 1 zadanie", () => {
    const zal = [1, 2].map((i) => zadanie({ id: 200 + i, data_od: "2026-10-05", stan: stan("czerwony") }));
    expect(wyznaczPowiadomienia(zal, D, RANO, {}).powiadomienia[0]!.tytul).toContain("2 zadania po terminie");
  });
  it("w dniu terminu: tylko „Dziś: X” w podsumowaniu, bez osobnego „Termin dziś”", () => {
    const z = [zadanie({ id: 30, typ: "pomiar", data_od: "2027-01-11", termin: "2027-01-11", tytul: "Pomiar: Kuchnia testowa" })];
    const w = wyznaczPowiadomienia(z, D, RANO, {});
    expect(w.powiadomienia).toHaveLength(1);
    expect(w.powiadomienia[0]!.tresc).toContain("Dziś: Pomiar: Kuchnia testowa");
  });
  it("po zdjęciu wyciszenia typu nie przychodzą zaległe alerty sprzed wyciszenia", () => {
    const z = [zadanie({ id: 31, typ: "platnosc", data_od: "2027-01-05", stan: stan("czerwony") })];
    const wyc = { ...BEZ_POD, wyciszoneTypy: ["platnosc"] };
    const a = wyznaczPowiadomienia(z, wyc, DZIEN, {});
    expect(a.powiadomienia).toHaveLength(0);
    expect(wyznaczPowiadomienia(z, BEZ_POD, { data: "2027-01-11", godzina: "11:00" }, a.historia).powiadomienia).toHaveLength(0);
  });
});

describe("godziny pracy (P6)", () => {
  const cz = [zadanie({ id: 40, data_od: "2027-01-05", tytul: "Zamówić: Płyta testowa", stan: stan("czerwony") })];
  it("poza godzinami pracy nic nie wyskakuje; rano alert jest w podsumowaniu", () => {
    const noc = wyznaczPowiadomienia(cz, D, NOC, {});
    expect(noc.powiadomienia).toHaveLength(0);
    const wieczor = wyznaczPowiadomienia(cz, BEZ_POD, { data: "2027-01-11", godzina: "19:30" }, {});
    expect(wieczor.powiadomienia).toHaveLength(0);
    const rano = wyznaczPowiadomienia(cz, D, { data: "2027-01-12", godzina: "07:45" }, noc.historia);
    expect(rano.powiadomienia.map((p) => p.rodzaj)).toEqual(["podsumowanie"]);
    expect(rano.powiadomienia[0]!.tresc).toContain("Zamówić: Płyta testowa");
    expect(wyznaczPowiadomienia(cz, D, { data: "2027-01-12", godzina: "09:00" }, rano.historia).powiadomienia).toHaveLength(0);
  });
  it("własny koniec pracy", () => {
    expect(wyznaczPowiadomienia(cz, { ...BEZ_POD, koniecPracy: "20:00" }, { data: "2027-01-11", godzina: "19:30" }, {}).powiadomienia).toHaveLength(1);
  });
});
