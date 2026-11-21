// @vitest-environment jsdom
import { beforeEach, describe, expect, it } from "vitest";
import { zadanie, zamowienie } from "../moduly/kalendarz/fabryka";
import { ukladWidzetu, wczytajPamiec, zapiszPamiec } from "./logika";

const DZIS = "2027-01-11";
const cz = { stan: "czerwony" as const, opis: "po terminie", dni_do_terminu: -2, przesuniecie_montazu_dni: null, nowy_termin_montazu: null };

describe("ukladWidzetu", () => {
  it("zaległe na górze, dziś zawsze, dalej tylko dni z zadaniami, zakres 7 dni", () => {
    const u = ukladWidzetu(
      [
        zadanie({ id: 1, data_od: "2027-01-09", stan: cz }),
        zadanie({ id: 2, data_od: "2027-01-13" }),
        zadanie({ id: 3, data_od: "2027-01-19" }),
        zadanie({ id: 4, data_od: "2027-01-12", wykonano: "2027-01-10" }),
        zadanie({ id: 5, typ: "warsztat", data_od: "2027-01-04", data_do: "2027-01-15" }),
      ],
      DZIS,
    );
    expect(u.zalegle.map((z) => z.id)).toEqual([1]);
    expect(u.dni.map((d) => [d.etykieta, d.zadania.map((z) => z.id)])).toEqual([
      ["Dziś", [5]],
      ["środa 13.01", [2]],
    ]);
  });
  it("zamówienie pod datą planową; trwające okno – pod dziś; czerwone zamówienie – w zaległych", () => {
    const u = ukladWidzetu(
      [zamowienie("2027-01-12", "2027-01-14", "2027-01-15", { id: 7 }), zamowienie("2027-01-05", "2027-01-08", "2027-01-12", { id: 8 }), zamowienie("2027-01-01", "2027-01-05", "2027-01-08", { id: 9, stan: cz })],
      DZIS,
    );
    expect(u.dni.map((d) => [d.data, d.zadania.map((z) => z.id)])).toEqual([
      ["2027-01-11", [8]],
      ["2027-01-14", [7]],
    ]);
    expect(u.zalegle.map((z) => z.id)).toEqual([9]);
  });
});

describe("pamięć widżetu", () => {
  beforeEach(() => localStorage.clear());
  it("zapis i odczyt; uszkodzone dane → brak pamięci", () => {
    expect(wczytajPamiec()).toBeNull();
    zapiszPamiec({ czas: "2027-01-11T08:00:00Z", dzis: DZIS, zadania: [zadanie({ id: 1, data_od: DZIS })] });
    expect(wczytajPamiec()?.zadania).toHaveLength(1);
    localStorage.setItem("robinwood:widzet:pamiec", "{zepsute");
    expect(wczytajPamiec()).toBeNull();
  });
});

describe("pamięć o nieoczekiwanym kształcie (W10)", () => {
  beforeEach(() => localStorage.clear());
  it("odrzuca złe dane i pomija złe zadania", async () => {
    const { sprawdzPamiec } = await import("./logika");
    expect(sprawdzPamiec(null)).toBeNull();
    expect(sprawdzPamiec({ czas: "x", dzis: DZIS, zadania: [] })).toBeNull();
    expect(sprawdzPamiec({ czas: "2027-01-11T08:00:00Z", dzis: DZIS, zadania: "nie" })).toBeNull();
    const p = sprawdzPamiec({
      czas: "2027-01-11T08:00:00Z",
      dzis: DZIS,
      zadania: [zadanie({ id: 1, data_od: DZIS }), { id: 2 }, null, { ...zadanie({ id: 3, data_od: DZIS }), stan: null }, { ...zadanie({ id: 4, data_od: DZIS }), stan: { stan: "fioletowy", opis: "x" } }],
    });
    expect(p!.zadania.map((z) => z.id)).toEqual([1]);
  });
  it("wczytajPamiec usuwa uszkodzoną pamięć", () => {
    localStorage.setItem("robinwood:widzet:pamiec", JSON.stringify({ zadania: [{}], czas: 5 }));
    expect(wczytajPamiec()).toBeNull();
    expect(localStorage.getItem("robinwood:widzet:pamiec")).toBeNull();
    localStorage.setItem("robinwood:widzet:pamiec", "{nie json");
    expect(wczytajPamiec()).toBeNull();
  });
});
