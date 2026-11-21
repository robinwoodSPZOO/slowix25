// @vitest-environment jsdom
// Testy komponentu SiatkaKalendarza: paski z ikoną i opisem, „+N więcej”, klawiatura (strzałki, Enter), święta.

import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { zadanie, zamowienie } from "./fabryka";
import { SiatkaKalendarza, type PropsSiatki } from "./SiatkaKalendarza";

beforeAll(() => {
  globalThis.ResizeObserver ??= class {
    observe() {}
    unobserve() {}
    disconnect() {}
  } as unknown as typeof ResizeObserver;
});
afterEach(cleanup);

function props(p: Partial<PropsSiatki> = {}): PropsSiatki {
  return {
    tygodnie: ["2027-01-11"],
    tryb: "tydzien",
    miesiac: null,
    zadania: [],
    dzis: "2027-01-12",
    swieta: new Map(),
    dniWolne: new Map(),
    wybraneId: null,
    fokus: "2027-01-12",
    onFokus: vi.fn(),
    onOtworzDzien: vi.fn(),
    onWybierz: vi.fn(),
    etykieta: "Kalendarz testowy",
    ...p,
  };
}

describe("SiatkaKalendarza", () => {
  it("pasek zamówienia ma opis okna, stan i znacznik daty planowej; kliknięcie wybiera zadanie", () => {
    const z = zamowienie("2027-01-11", "2027-01-13", "2027-01-15", { id: 9, tytul: "Zamówić: Płyta testowa", stan: { stan: "pomaranczowy", opis: "Zamów natychmiast", dni_do_terminu: 3, przesuniecie_montazu_dni: null, nowy_termin_montazu: null } });
    const p = props({ zadania: [z] });
    render(<SiatkaKalendarza {...p} />);
    const pasek = screen.getByRole("button", { name: /Zamówienie materiałów: Zamówić: Płyta testowa\. Pomarańczowy – pilne, najlepiej zamówić od 2027-01-11, planowo 2027-01-13/ });
    expect(pasek.querySelector(".pasek__planowa")).not.toBeNull();
    expect(pasek.className).toContain("pasek--stan-pomaranczowy");
    fireEvent.click(pasek);
    expect(p.onWybierz).toHaveBeenCalledWith(z);
  });

  it("w widoku miesiąca nadmiar zadań chowa się pod „+N więcej”", () => {
    const zadania = Array.from({ length: 5 }, (_, i) => zadanie({ id: 100 + i, data_od: "2027-01-13" }));
    const p = props({ tryb: "miesiac", miesiac: "2027-01-01", zadania });
    render(<SiatkaKalendarza {...p} />);
    // jsdom nie ma wysokości – mieści się 1 tor
    const wiecej = screen.getByRole("button", { name: "+4 więcej" });
    fireEvent.click(wiecej);
    expect(p.onOtworzDzien).toHaveBeenCalledWith("2027-01-13");
  });

  it("strzałki zmieniają dzień, Enter otwiera listę dnia; dziś i święto w opisie komórki", () => {
    const p = props({ swieta: new Map([["2027-01-13", "Święto testowe"]]) });
    render(<SiatkaKalendarza {...p} />);
    const dzis = screen.getByRole("gridcell", { name: /wtorek, 12 stycznia 2027, dziś/ });
    expect(dzis.getAttribute("tabindex")).toBe("0");
    fireEvent.keyDown(dzis, { key: "ArrowRight" });
    expect(p.onFokus).toHaveBeenCalledWith("2027-01-13");
    fireEvent.keyDown(dzis, { key: "ArrowDown" });
    expect(p.onFokus).toHaveBeenCalledWith("2027-01-19");
    fireEvent.keyDown(dzis, { key: "Enter" });
    expect(p.onOtworzDzien).toHaveBeenCalledWith("2027-01-12");
    expect(screen.getByRole("gridcell", { name: /święto: Święto testowe/ })).toBeTruthy();
  });
});
