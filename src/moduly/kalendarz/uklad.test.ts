import { describe, expect, it } from "vitest";
import { pozycjaPlanowej, torowWWierszu, ukladTygodnia, zakresZadania } from "./uklad";
import { zadanie, zamowienie } from "./fabryka";

const PON = "2027-01-11"; // poniedziałek

describe("zakresZadania", () => {
  it("zamówienie zajmuje okno od sweet spotu do daty krytycznej", () => {
    expect(zakresZadania(zamowienie("2027-01-08", "2027-01-11", "2027-01-13"))).toEqual({ od: "2027-01-08", do: "2027-01-13" });
  });
  it("zadanie jednodniowe i z odwróconymi datami", () => {
    expect(zakresZadania(zadanie({ data_od: "2027-01-12" }))).toEqual({ od: "2027-01-12", do: "2027-01-12" });
    expect(zakresZadania(zadanie({ data_od: "2027-01-12", data_do: "2027-01-10" }))).toEqual({ od: "2027-01-12", do: "2027-01-12" });
  });
});

describe("ukladTygodnia", () => {
  it("tnie pasek zamówienia do tygodnia i zaznacza ciągłość oraz dzień planowy", () => {
    const u = ukladTygodnia([zamowienie("2027-01-08", "2027-01-11", "2027-01-13")], PON);
    expect(u.segmenty).toHaveLength(1);
    const s = u.segmenty[0]!;
    expect([s.kolOd, s.kolDo, s.ciagnieSieZLewej, s.ciagnieSieWPrawo, s.kolPlanowa]).toEqual([0, 2, true, false, 0]);
    expect(pozycjaPlanowej(s)).toBeCloseTo(100 / 6);
  });

  it("dzień planowy poza tygodniem nie daje znacznika", () => {
    const u = ukladTygodnia([zamowienie("2027-01-14", "2027-01-19", "2027-01-21")], PON);
    expect(u.segmenty[0]!.kolPlanowa).toBeNull();
    expect(u.segmenty[0]!.ciagnieSieWPrawo).toBe(true);
  });

  it("nakładające się zadania trafiają na kolejne tory, rozłączne – na ten sam", () => {
    const a = zadanie({ typ: "warsztat", data_od: "2027-01-11", data_do: "2027-01-15" });
    const b = zadanie({ typ: "pomiar", data_od: "2027-01-12" });
    const c = zadanie({ typ: "rysunki", data_od: "2027-01-16" });
    const u = ukladTygodnia([b, c, a], PON);
    const tor = (id: number) => u.segmenty.find((s) => s.zadanie.id === id)!.tor;
    expect(tor(a.id)).toBe(0); // najdłuższe i najwcześniejsze
    expect(tor(b.id)).toBe(1);
    expect(tor(c.id)).toBe(0);
    expect(u.torow).toBe(2);
  });

  it("ponad limit torów: ukrywa i liczy ukryte w każdym dniu („+N więcej”)", () => {
    const zadania = Array.from({ length: 6 }, (_, i) => zadanie({ data_od: "2027-01-13", id: 100 + i }));
    zadania.push(zadanie({ data_od: "2027-01-12", data_do: "2027-01-14", typ: "montaz", id: 200 }));
    const u = ukladTygodnia(zadania, PON, 3);
    expect(u.segmenty).toHaveLength(3);
    expect(u.ukryte).toEqual([0, 0, 4, 0, 0, 0, 0]);
    expect(u.dni[2]).toHaveLength(7);
    expect(u.dni[1]!.map((z) => z.id)).toEqual([200]);
  });

  it("pomija zadania spoza tygodnia", () => {
    const u = ukladTygodnia([zadanie({ data_od: "2027-01-10" }), zadanie({ data_od: "2027-01-18" })], PON);
    expect(u.segmenty).toHaveLength(0);
  });

  it("500 zadań w tygodniu układa się szybko", () => {
    const zadania = Array.from({ length: 500 }, (_, i) =>
      zadanie({ id: 1000 + i, data_od: `2027-01-${String(11 + (i % 7)).padStart(2, "0")}`, data_do: `2027-01-${String(11 + Math.min(6, (i % 7) + (i % 3))).padStart(2, "0")}` }),
    );
    const t0 = performance.now();
    const u = ukladTygodnia(zadania, PON, 4);
    expect(performance.now() - t0).toBeLessThan(200);
    expect(u.segmenty.every((s) => s.tor < 4)).toBe(true);
  });
});

describe("torowWWierszu", () => {
  it("co najmniej jeden tor", () => {
    expect(torowWWierszu(100, 24, 22, 16)).toBe(2);
    expect(torowWWierszu(10, 24, 22, 16)).toBe(1);
  });
});
