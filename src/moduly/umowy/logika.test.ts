import { describe, expect, it } from "vitest";
import type { Umowa } from "../../api/typyUmow";
import { daneUmowy, lataDoFiltra, numerZFormatu, ostrzezeniePowierzchni, parsujCalkowita, parsujDziesietna, poleZablokowane } from "./logika";
import { dozwolonePrzejscia, etykietaPrzejscia } from "./statusy";

describe("formularz umowy", () => {
  it("parsuje liczby dni i powierzchnię", () => {
    expect(parsujCalkowita("")).toBeNull();
    expect(parsujCalkowita("14")).toBe(14);
    expect(parsujCalkowita("-1")).toBeUndefined();
    expect(parsujCalkowita("0", 1)).toBeUndefined();
    expect(parsujCalkowita("400")).toBeUndefined();
    expect(parsujDziesietna("160,5")).toBe(160.5);
    expect(parsujDziesietna("12.25")).toBe(12.25);
    expect(parsujDziesietna("abc")).toBeUndefined();
    expect(parsujDziesietna(" ")).toBeNull();
  });

  it("ostrzega o limitach powierzchni dla 8% VAT", () => {
    expect(ostrzezeniePowierzchni("lokal_mieszkalny", 150)).toBeNull();
    expect(ostrzezeniePowierzchni("lokal_mieszkalny", 150.5)).toContain("150");
    expect(ostrzezeniePowierzchni("dom_jednorodzinny", 299)).toBeNull();
    expect(ostrzezeniePowierzchni("dom_jednorodzinny", 301)).toContain("300");
    expect(ostrzezeniePowierzchni("inny", 1000)).toBeNull();
    expect(ostrzezeniePowierzchni("lokal_mieszkalny", null)).toBeNull();
  });

  it("numer z formatu jak na serwerze", () => {
    expect(numerZFormatu("{NN}/{RRRR}", 2026, 7)).toBe("07/2026");
    expect(numerZFormatu("UM/{RR}/{NNN}", 2027, 5)).toBe("UM/27/005");
    expect(numerZFormatu("{N}-{RRRR}", 2027, 12)).toBe("12-2027");
  });

  it("blokuje pola po wyjściu ze szkicu poza wewnętrznymi", () => {
    const u = { tylko_do_odczytu: true, numer: "01/2026", czas_montazu_dni: 2 } as unknown as Umowa;
    expect(poleZablokowane(u, "wartosc_brutto_gr")).toBe(true);
    expect(poleZablokowane(u, "czas_montazu_dni")).toBe(false);
    expect(poleZablokowane(u, "uwagi")).toBe(false);
    expect(daneUmowy(u).numer).toBe("01/2026");
    expect(daneUmowy(u).montaz_ulica).toBeNull();
  });

  it("przejścia statusów jak na serwerze", () => {
    expect(dozwolonePrzejscia("szkic")).toEqual(["wyslana", "podpisana", "anulowana"]);
    expect(dozwolonePrzejscia("wyslana")).toContain("szkic");
    expect(dozwolonePrzejscia("podpisana")).not.toContain("szkic");
    expect(dozwolonePrzejscia("zamknieta")).toEqual([]);
    expect(dozwolonePrzejscia("anulowana")).toEqual([]);
    expect(etykietaPrzejscia("szkic", "wyslana")).toBe("Wyślij do klienta");
  });

  it("lata do filtra", () => {
    expect(lataDoFiltra(new Date(2026, 5, 1), 3)).toEqual([2027, 2026, 2025]);
  });
});
