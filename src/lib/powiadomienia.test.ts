import { describe, expect, it } from "vitest";
import type { StanOstrzezenia } from "../api/typy";
import { zadanie } from "../moduly/kalendarz/fabryka";
import { DOMYSLNE_USTAWIENIA_POWIADOMIEN as D, normalizujUstawienia, PROG_GRUPOWANIA, wyznaczPowiadomienia, type HistoriaPowiadomien } from "./powiadomienia";

const stan = (s: StanOstrzezenia, nowy: string | null = null) => ({ stan: s, opis: `Opis ${s}`, dni_do_terminu: 0, przesuniecie_montazu_dni: nowy ? 5 : null, nowy_termin_montazu: nowy });
const RANO = { data: "2027-01-11", godzina: "08:00" };
const NOC = { data: "2027-01-11", godzina: "05:00" };

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
    expect(wyniki[0]![0]!.klucz).toBe("przed:7:2027-01-18:3");
    expect(wyniki[1]).toHaveLength(0);
  });
  it("po przesunięciu terminu – od nowa", () => {
    let h: HistoriaPowiadomien = {};
    h = wyznaczPowiadomienia(z, { ...D, podsumowanie: false }, { data: "2027-01-17", godzina: "10:00" }, h).historia;
    const przesuniete = [{ ...z[0]!, termin: "2027-01-20", data_od: "2027-01-20" }];
    const w = wyznaczPowiadomienia(przesuniete, { ...D, podsumowanie: false }, { data: "2027-01-17", godzina: "10:00" }, h);
    expect(w.powiadomienia.map((p) => p.klucz)).toEqual(["przed:7:2027-01-20:3"]);
  });
  it("przed godziną poranną – czekają; własne progi", () => {
    expect(przebieg(z, [{ data: "2027-01-17", godzina: "06:00" }], { ...D, podsumowanie: false })[0]).toHaveLength(0);
    const w = przebieg(z, [{ data: "2027-01-13", godzina: "10:00" }], { ...D, podsumowanie: false, dniPrzed: [5] })[0]!;
    expect(w.map((p) => p.klucz)).toEqual(["przed:7:2027-01-18:5"]);
  });
});

describe("alerty pomarańczowy i czerwony", () => {
  it("osobny alert przy wejściu w pomarańczowy, potem w czerwony – każdy raz, także w nocy", () => {
    const po = [zadanie({ id: 3, typ: "zamowienie", data_od: "2027-01-05", termin: "2027-01-08", stan: stan("pomaranczowy") })];
    const cz = [{ ...po[0]!, stan: stan("czerwony", "2027-02-15") }];
    let h: HistoriaPowiadomien = {};
    const a = wyznaczPowiadomienia(po, D, NOC, h);
    expect(a.powiadomienia.map((p) => p.rodzaj)).toEqual(["pomaranczowy"]);
    expect(a.powiadomienia[0]!.tytul).toContain("Zbliżamy się do krytycznego momentu");
    h = a.historia;
    expect(wyznaczPowiadomienia(po, D, NOC, h).powiadomienia).toHaveLength(0);
    const b = wyznaczPowiadomienia(cz, D, NOC, h);
    expect(b.powiadomienia.map((p) => p.rodzaj)).toEqual(["czerwony"]);
    expect(b.powiadomienia[0]!.tresc).toContain("15.02");
    expect(wyznaczPowiadomienia(cz, D, NOC, b.historia).powiadomienia).toHaveLength(0);
  });
  it("od razu czerwony – bez spóźnionego pomarańczowego", () => {
    const cz = [zadanie({ id: 4, data_od: "2027-01-05", stan: stan("czerwony") })];
    const a = wyznaczPowiadomienia(cz, D, NOC, {});
    expect(a.powiadomienia.map((p) => p.rodzaj)).toEqual(["czerwony"]);
  });
  it("zadania pilne nie dostają dodatkowo przypomnienia „N dni przed”", () => {
    const po = [zadanie({ id: 5, data_od: "2027-01-12", termin: "2027-01-12", stan: stan("pomaranczowy") })];
    const w = wyznaczPowiadomienia(po, { ...D, podsumowanie: false }, RANO, {});
    expect(w.powiadomienia.map((p) => p.rodzaj)).toEqual(["pomaranczowy"]);
  });
  it("wiele naraz → jedno zbiorcze; klucze wszystkich w historii", () => {
    const cz = Array.from({ length: PROG_GRUPOWANIA + 2 }, (_, i) => zadanie({ id: 50 + i, data_od: "2027-01-05", stan: stan("czerwony") }));
    const a = wyznaczPowiadomienia(cz, D, NOC, {});
    expect(a.powiadomienia).toHaveLength(1);
    expect(a.powiadomienia[0]!.tytul).toContain(`${PROG_GRUPOWANIA + 2} zadań`);
    expect(Object.keys(a.historia).filter((k) => k.startsWith("stan:5")).length).toBeGreaterThanOrEqual(PROG_GRUPOWANIA + 2);
    expect(wyznaczPowiadomienia(cz, D, NOC, a.historia).powiadomienia).toHaveLength(0);
  });
  it("wyłączone alerty i wyciszone typy", () => {
    const cz = [zadanie({ id: 8, typ: "platnosc", data_od: "2027-01-05", stan: stan("czerwony") })];
    expect(wyznaczPowiadomienia(cz, { ...D, alertCzerwony: false }, NOC, {}).powiadomienia).toHaveLength(0);
    expect(wyznaczPowiadomienia(cz, { ...D, wyciszoneTypy: ["platnosc"] }, NOC, {}).powiadomienia).toHaveLength(0);
    expect(wyznaczPowiadomienia(cz, { ...D, wlaczone: false }, NOC, {}).powiadomienia).toHaveLength(0);
  });
  it("wykonane zadania są pomijane", () => {
    const cz = [zadanie({ id: 9, data_od: "2027-01-05", stan: stan("czerwony"), wykonano: "2027-01-06" })];
    expect(wyznaczPowiadomienia(cz, D, RANO, {}).powiadomienia).toHaveLength(0);
  });
});

describe("historia i ustawienia", () => {
  it("stare wpisy (ponad 90 dni) są usuwane", () => {
    const w = wyznaczPowiadomienia([], D, RANO, { "przed:1:2026-01-01:1": "2026-09-01", "przed:2:2027-01-01:1": "2027-01-01" });
    expect(Object.keys(w.historia)).toEqual(["przed:2:2027-01-01:1", "podsumowanie:2027-01-11"]);
  });
  it("normalizacja ustawień z pamięci", () => {
    const u = normalizujUstawienia({ dniPrzed: [1, 7, 7, -2, 3.5, 3], godzinaPodsumowania: "25:00" } as never);
    expect(u.dniPrzed).toEqual([7, 3, 1]);
    expect(u.godzinaPodsumowania).toBe(D.godzinaPodsumowania);
    expect(normalizujUstawienia(null)).toEqual(D);
  });
});
