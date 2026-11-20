import { describe, expect, it } from "vitest";
import { formatujDate, formatujKwote, parsujKwote } from "./formaty";

describe("formatujKwote", () => {
  it("formatuje grosze po polsku", () => {
    expect(formatujKwote(123456).replace(/\s/g, " ")).toBe("1 234,56 zł");
    expect(formatujKwote(5)).toBe("0,05 zł");
    expect(formatujKwote(null)).toBe("—");
  });
  it("odrzuca ułamki groszy", () => {
    expect(() => formatujKwote(1.5)).toThrow();
  });
});

describe("parsujKwote", () => {
  it("czyta typowe zapisy", () => {
    expect(parsujKwote("1 234,56 zł")).toBe(123456);
    expect(parsujKwote("1234.5")).toBe(123450);
    expect(parsujKwote("1234")).toBe(123400);
    expect(parsujKwote("-10,01")).toBe(-1001);
  });
  it("zwraca null dla błędnych danych", () => {
    expect(parsujKwote("abc")).toBeNull();
    expect(parsujKwote("1,234")).toBeNull();
  });
});

describe("formatujDate", () => {
  it("zamienia ISO na DD.MM.RRRR", () => {
    expect(formatujDate("2026-11-23")).toBe("23.11.2026");
    expect(formatujDate("2026-11-23T10:00:00Z")).toBe("23.11.2026");
    expect(formatujDate(null)).toBe("—");
  });
});
