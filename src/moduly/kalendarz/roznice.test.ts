import { describe, expect, it } from "vitest";
import { zadanie, zamowienie } from "./fabryka";
import { roznicaHarmonogramu } from "./roznice";

describe("roznicaHarmonogramu", () => {
  it("przesunięte, zamrożone (wykonane / ręczne), dodane i usunięte", () => {
    const przed = [
      zadanie({ id: 1, typ: "montaz", klucz: "montaz", data_od: "2027-02-08", data_do: "2027-02-10" }),
      zadanie({ id: 2, typ: "pomiar", klucz: "pomiar", data_od: "2026-12-14", wykonano: "2026-12-14" }),
      zadanie({ id: 3, typ: "inne", klucz: null, wyliczone: false, data_od: "2027-01-05" }),
      zamowienie("2027-01-08", "2027-01-11", "2027-01-13", { id: 4, klucz: "zamowienie:1" }),
      zadanie({ id: 5, typ: "klient", klucz: "potwierdzenie:1", data_od: "2027-01-11" }),
    ];
    const po = [
      { ...przed[0]!, data_od: "2027-02-15", data_do: "2027-02-17" },
      przed[1]!,
      przed[2]!,
      { ...przed[3]!, data_sweet_spot: "2027-01-15", data_planowa: "2027-01-18", data_krytyczna: "2027-01-20" },
      zadanie({ id: 6, typ: "rysunki", klucz: "rysunki", data_od: "2026-12-28" }),
    ];
    const r = roznicaHarmonogramu(przed, po);
    expect(r.przesuniete.map((p) => [p.id, p.dni])).toEqual([
      [4, 7],
      [1, 7],
    ]);
    expect(r.zamrozone.map((z) => [z.id, z.powod])).toEqual([
      [2, "wykonane"],
      [3, "dodane ręcznie"],
    ]);
    expect(r.dodane.map((z) => z.id)).toEqual([6]);
    expect(r.usuniete.map((z) => z.id)).toEqual([5]);
    expect(r.bezZmian).toBe(0);
  });
});
