// @vitest-environment jsdom
// W3/W9a: krok planera – historia tylko z pokazanych, uszkodzona pamięć nie zatrzymuje planera.

import { beforeEach, describe, expect, it, vi } from "vitest";
import { zadanie } from "../moduly/kalendarz/fabryka";
import { krokPlanera, wczytajHistorie } from "./planerPowiadomien";
import { DOMYSLNE_USTAWIENIA_POWIADOMIEN as D, MAKS_NARAZ } from "./powiadomienia";

vi.mock("./tauri", () => ({ pokazPowiadomienie: vi.fn(async () => {}) }));

const stan = (s: "czerwony" | "pomaranczowy") => ({ stan: s, opis: `Opis ${s}`, dni_do_terminu: -1, przesuniecie_montazu_dni: null, nowy_termin_montazu: null });
const TERAZ = { data: "2027-01-11", godzina: "10:00" };

beforeEach(() => localStorage.clear());

describe("krokPlanera", () => {
  it("uszkodzona historia w pamięci (wartości nie-tekstowe) – planer działa dalej", async () => {
    localStorage.setItem("robinwood:powiadomienia:historia", JSON.stringify({ "stan:1:czerwony": 7, x: [1] }));
    const pokaz = vi.fn(async () => {});
    const p = await krokPlanera([zadanie({ id: 1, data_od: "2027-01-05", stan: stan("czerwony") })], { ...D, podsumowanie: false }, pokaz, TERAZ);
    expect(p).toHaveLength(1);
    expect(pokaz).toHaveBeenCalledTimes(1);
    expect(Object.keys(wczytajHistorie())).toContain("stan:1:czerwony");
  });
  it("historia nie do odczytu (nie-JSON) i błąd wyświetlania – bez wyjątku", async () => {
    localStorage.setItem("robinwood:powiadomienia:historia", "{nie json");
    const pokaz = vi.fn(async () => {
      throw new Error("brak uprawnień");
    });
    await expect(krokPlanera([zadanie({ id: 2, data_od: "2027-01-05", stan: stan("czerwony") })], { ...D, podsumowanie: false }, pokaz, TERAZ)).resolves.toHaveLength(1);
  });
  it("najwyżej 5 naraz – każde zadanie jest w którymś pokazanym powiadomieniu", async () => {
    const z = [
      ...[1, 2, 3].map((i) => zadanie({ id: i, data_od: "2027-01-05", stan: stan("czerwony") })),
      ...[4, 5, 6].map((i) => zadanie({ id: i, data_od: "2027-01-13", stan: stan("pomaranczowy") })),
      ...[7, 8, 9].map((i) => zadanie({ id: i, data_od: "2027-01-14", termin: "2027-01-14" })),
    ];
    const pokaz = vi.fn(async () => {});
    const p = await krokPlanera(z, { ...D, podsumowanie: false }, pokaz, TERAZ);
    expect(pokaz).toHaveBeenCalledTimes(MAKS_NARAZ);
    expect(p.at(-1)!.rodzaj).toBe("zbiorcze");
    // drugi krok – nic nie dochodzi (nic nie przepadło, nic się nie powtarza)
    expect(await krokPlanera(z, { ...D, podsumowanie: false }, pokaz, TERAZ)).toHaveLength(0);
  });
});
