import { describe, expect, it } from "vitest";
import { DZIEN, liczba, odmiana, POZYCJA, WIERSZ, ZADANIE } from "./odmiana";

describe("odmiana liczebników", () => {
  it("podstawowe przypadki", () => {
    expect(liczba(0, ZADANIE)).toBe("0 zadań");
    expect(liczba(1, ZADANIE)).toBe("1 zadanie");
    expect(liczba(2, ZADANIE)).toBe("2 zadania");
    expect(liczba(4, ZADANIE)).toBe("4 zadania");
    expect(liczba(5, ZADANIE)).toBe("5 zadań");
    expect(liczba(12, ZADANIE)).toBe("12 zadań");
    expect(liczba(13, ZADANIE)).toBe("13 zadań");
    expect(liczba(21, ZADANIE)).toBe("21 zadań");
    expect(liczba(22, ZADANIE)).toBe("22 zadania");
    expect(liczba(112, ZADANIE)).toBe("112 zadań");
    expect(liczba(104, ZADANIE)).toBe("104 zadania");
  });
  it("teksty z recenzji", () => {
    expect(liczba(1, DZIEN)).toBe("1 dzień");
    expect(liczba(1, WIERSZ)).toBe("1 wiersz");
    expect(liczba(2, POZYCJA)).toBe("2 pozycje");
    expect(odmiana(-1, DZIEN)).toBe("dzień");
    expect(odmiana(-3, DZIEN)).toBe("dni");
  });
});
