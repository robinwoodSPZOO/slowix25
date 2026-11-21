import { describe, expect, it } from "vitest";
import { filtrujZadania, licznikiStanow, maOstrzezenie, PUSTE_FILTRY, wgPilnosci } from "./filtry";
import { zadanie, zamowienie } from "./fabryka";

const stan = (s: "spokojnie" | "zolty" | "pomaranczowy" | "czerwony" | "wykonane") => ({ stan: s, opis: s, dni_do_terminu: 1, przesuniecie_montazu_dni: null, nowy_termin_montazu: null });

const zadania = [
  zadanie({ id: 1, typ: "pomiar", data_od: "2027-01-11", zlecenie_id: 1 }),
  zadanie({ id: 2, typ: "montaz", data_od: "2027-01-12", zlecenie_id: 2, stan: stan("czerwony") }),
  zadanie({ id: 3, typ: "rysunki", data_od: "2027-01-12", zlecenie_id: 2, wykonano: "2027-01-10", stan: stan("wykonane") }),
  zamowienie("2027-01-08", "2027-01-11", "2027-01-13", { id: 4, zlecenie_id: 3 }),
  zadanie({ id: 5, typ: "inne", data_od: "2027-01-13", zlecenie_id: null }),
];
const klienci = new Map([[1, 10], [2, 20], [3, 10]]);

describe("filtrujZadania", () => {
  it("bez filtrów – wszystko", () => {
    expect(filtrujZadania(zadania, PUSTE_FILTRY, klienci)).toHaveLength(5);
  });
  it("typ, zlecenie, klient", () => {
    const ids = (f: Partial<typeof PUSTE_FILTRY>) => filtrujZadania(zadania, { ...PUSTE_FILTRY, ...f }, klienci).map((z) => z.id);
    expect(ids({ typy: ["montaz", "zamowienie"] })).toEqual([2, 4]);
    expect(ids({ zlecenieId: 2 })).toEqual([2, 3]);
    expect(ids({ klientId: 10 })).toEqual([1, 4]);
    expect(ids({ tylkoNiewykonane: true })).toEqual([1, 2, 4, 5]);
    expect(ids({ tylkoZOstrzezeniem: true })).toEqual([2]);
    expect(ids({ klientId: 20, tylkoNiewykonane: true })).toEqual([2]);
  });
  it("ostrzeżenie: zagrożona dostawa, blokada, nieaktualny harmonogram", () => {
    const z = zamowienie("2027-01-08", "2027-01-11", "2027-01-13");
    expect(maOstrzezenie(z)).toBe(false);
    expect(maOstrzezenie({ ...z, harmonogram_nieaktualny: true })).toBe(true);
    expect(maOstrzezenie({ ...z, zamowienie: { ...z.zamowienie!, zablokowane: true } })).toBe(true);
    expect(maOstrzezenie({ ...z, wykonano: "2027-01-09", zamowienie: { ...z.zamowienie!, dostawa_zagrozona: true } })).toBe(true);
  });
});

describe("pilność i liczniki", () => {
  it("sortuje od najpilniejszych", () => {
    const l = [...zadania].sort(wgPilnosci).map((z) => z.id);
    expect(l[0]).toBe(2);
  });
  it("liczy tylko niewykonane", () => {
    expect(licznikiStanow([...zadania, zadanie({ data_od: "2027-01-01", stan: stan("zolty") }), zadanie({ data_od: "2027-01-01", stan: stan("pomaranczowy"), wykonano: "2027-01-01" })])).toEqual({ zolty: 1, pomaranczowy: 0, czerwony: 1 });
  });
});
