import { describe, expect, it } from "vitest";
import { dodajDni, dzienTygodnia, nazwaTygodnia, pelnaData, poczatekTygodnia, poprawnaData, roznicaDni, siatkaMiesiaca, wzgledny } from "./daty";

describe("daty", () => {
  it("arytmetyka dni przez zmianę czasu", () => {
    expect(dodajDni("2027-03-27", 2)).toBe("2027-03-29");
    expect(roznicaDni("2026-10-24", "2026-10-26")).toBe(2);
    expect(dodajDni("2026-12-31", 1)).toBe("2027-01-01");
  });
  it("tydzień od poniedziałku", () => {
    expect(dzienTygodnia("2026-11-21")).toBe(5);
    expect(poczatekTygodnia("2026-11-22")).toBe("2026-11-16");
  });
  it("siatka miesiąca – pełne tygodnie", () => {
    const s = siatkaMiesiaca("2026-11-15");
    expect(s.od).toBe("2026-10-26");
    expect(s.do).toBe("2026-12-06");
    expect(s.tygodnie).toHaveLength(6);
    expect(siatkaMiesiaca("2027-02-10").tygodnie).toHaveLength(4);
  });
  it("opisy po polsku", () => {
    expect(pelnaData("2026-11-21")).toBe("sobota, 21 listopada 2026");
    expect(nazwaTygodnia("2026-11-30")).toBe("30 listopada – 6 grudnia 2026");
    expect(nazwaTygodnia("2026-12-28")).toBe("28 grudnia 2026 – 3 stycznia 2027");
    expect(wzgledny(-2)).toBe("2 dni po terminie");
    expect(wzgledny(1)).toBe("jutro");
  });
  it("poprawność daty", () => {
    expect(poprawnaData("2027-02-29")).toBe(false);
    expect(poprawnaData("2028-02-29")).toBe(true);
  });
});
