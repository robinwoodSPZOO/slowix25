// @vitest-environment jsdom
// W10: widżet z uszkodzoną pamięcią lokalną nie zostaje pustym oknem; granica błędów z „Odśwież”.

import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { GranicaBledow } from "../ui";

vi.mock("../kontekst/Polaczenie", () => ({ usePolaczenie: () => ({ api: null, naZywo: null, ostatnieZdarzenie: null, wznowienia: 0 }) }));
const { Widzet } = await import("./Widzet");
const { wyczyscPamiec } = await import("./logika");

beforeEach(() => localStorage.clear());
afterEach(cleanup);

describe("Widżet", () => {
  it("uszkodzona pamięć (zadania bez pól) – widżet się renderuje i pokazuje brak danych", () => {
    localStorage.setItem("robinwood:widzet:pamiec", JSON.stringify({ czas: "2027-01-11T08:00:00Z", dzis: "2027-01-11", zadania: [{ id: 1 }, { tytul: 5 }] }));
    render(<Widzet />);
    expect(screen.getByText("ROBINWOOD")).toBeTruthy();
    expect(screen.getByText(/Program nie jest połączony z serwerem/)).toBeTruthy();
  });
  it("pamięć innego typu (tablica, liczba)", () => {
    localStorage.setItem("robinwood:widzet:pamiec", "[1,2,3]");
    render(<Widzet />);
    expect(screen.getByText(/Brak zapisanych danych/)).toBeTruthy();
  });
  it("granica błędów: błąd renderowania → komunikat i „Odśwież” (czyści pamięć)", () => {
    let rzuc = true;
    function Zepsuty() {
      if (rzuc) throw new Error("test");
      return <p>działa</p>;
    }
    vi.spyOn(console, "error").mockImplementation(() => {});
    localStorage.setItem("robinwood:widzet:pamiec", "x");
    render(
      <GranicaBledow nazwa="Widżet" kompaktowo onOdswiez={wyczyscPamiec}>
        <Zepsuty />
      </GranicaBledow>,
    );
    expect(screen.getByRole("alert").textContent).toContain("coś poszło nie tak");
    rzuc = false;
    fireEvent.click(screen.getByRole("button", { name: /Odśwież/ }));
    expect(screen.getByText("działa")).toBeTruthy();
    expect(localStorage.getItem("robinwood:widzet:pamiec")).toBeNull();
  });
});
