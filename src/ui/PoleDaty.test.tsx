// @vitest-environment jsdom
// W15: wspólne pole daty dd.mm.rrrr – wpisywanie ręczne, klawiatura, wybór z kalendarza.

import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { formatPolski, parsujDatePolska, PoleDaty, poprawnaDataIso } from "./PoleDaty";

afterEach(cleanup);

function Pole({ start = "", onZ = () => {}, min, max }: { start?: string; onZ?: (v: string) => void; min?: string; max?: string }) {
  const [v, ustaw] = useState(start);
  return (
    <>
      <PoleDaty
        id="d"
        etykieta="Data testowa"
        wartosc={v}
        min={min}
        max={max}
        onZmiana={(x) => {
          ustaw(x);
          onZ(x);
        }}
      />
      <output data-testid="wartosc">{v}</output>
    </>
  );
}

describe("parsowanie i format", () => {
  it("polskie zapisy daty", () => {
    expect(parsujDatePolska("5.3.2027")).toBe("2027-03-05");
    expect(parsujDatePolska("05.03.27")).toBe("2027-03-05");
    expect(parsujDatePolska("05032027")).toBe("2027-03-05");
    expect(parsujDatePolska("5-3-2027")).toBe("2027-03-05");
    expect(parsujDatePolska("2027-03-05")).toBe("2027-03-05");
    expect(parsujDatePolska("31.02.2027")).toBeNull();
    expect(parsujDatePolska("03/05")).toBeNull();
    expect(formatPolski("2027-03-05")).toBe("05.03.2027");
    expect(poprawnaDataIso("2028-02-29")).toBe(true);
    expect(poprawnaDataIso("2027-02-29")).toBe(false);
  });
});

describe("PoleDaty", () => {
  it("pokazuje dd.mm.rrrr niezależnie od języka przeglądarki", () => {
    render(<Pole start="2026-11-21" />);
    expect((screen.getByLabelText("Data testowa") as HTMLInputElement).value).toBe("21.11.2026");
  });
  it("wpisanie ręczne → ISO; błędny wpis przekazywany jako tekst (rodzic pokazuje błąd)", () => {
    const onZ = vi.fn();
    render(<Pole onZ={onZ} />);
    const pole = screen.getByLabelText("Data testowa");
    fireEvent.focus(pole);
    fireEvent.change(pole, { target: { value: "7.1.2027" } });
    expect(screen.getByTestId("wartosc").textContent).toBe("2027-01-07");
    fireEvent.blur(pole);
    expect((pole as HTMLInputElement).value).toBe("07.01.2027");
    fireEvent.focus(pole);
    fireEvent.change(pole, { target: { value: "32.01.2027" } });
    expect(screen.getByTestId("wartosc").textContent).toBe("32.01.2027");
    fireEvent.change(pole, { target: { value: "" } });
    expect(onZ).toHaveBeenLastCalledWith("");
  });
  it("strzałki zmieniają dzień, z poszanowaniem max", () => {
    render(<Pole start="2027-01-10" max="2027-01-11" />);
    const pole = screen.getByLabelText("Data testowa");
    fireEvent.keyDown(pole, { key: "ArrowUp" });
    expect(screen.getByTestId("wartosc").textContent).toBe("2027-01-11");
    fireEvent.keyDown(pole, { key: "ArrowUp" });
    expect(screen.getByTestId("wartosc").textContent).toBe("2027-01-11");
    fireEvent.keyDown(pole, { key: "ArrowDown" });
    expect(screen.getByTestId("wartosc").textContent).toBe("2027-01-10");
  });
  it("Alt+↓ otwiera kalendarz; strzałki + Enter wybierają dzień; Esc zamyka i oddaje fokus", () => {
    render(<Pole start="2027-01-10" />);
    const pole = screen.getByLabelText("Data testowa");
    fireEvent.keyDown(pole, { key: "ArrowDown", altKey: true });
    const okno = screen.getByRole("dialog");
    expect(screen.getByText("styczeń 2027")).toBeTruthy();
    const dzien = document.activeElement as HTMLElement;
    expect(dzien.dataset.dzien).toBe("2027-01-10");
    fireEvent.keyDown(okno, { key: "ArrowRight" });
    expect((document.activeElement as HTMLElement).dataset.dzien).toBe("2027-01-11");
    fireEvent.keyDown(okno, { key: "PageDown" });
    expect((document.activeElement as HTMLElement).dataset.dzien).toBe("2027-02-11");
    fireEvent.click(document.activeElement as HTMLElement);
    expect(screen.getByTestId("wartosc").textContent).toBe("2027-02-11");
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(document.activeElement).toBe(pole);
    fireEvent.keyDown(pole, { key: "F4" });
    fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" });
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(document.activeElement).toBe(pole);
  });
  it("dni spoza min/max są wyłączone w kalendarzu", () => {
    render(<Pole start="2027-01-10" min="2027-01-05" max="2027-01-12" />);
    fireEvent.click(screen.getByRole("button", { name: /Wybierz datę/ }));
    expect((screen.getByRole("button", { name: /, 4 stycznia 2027/ }) as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole("button", { name: /, 13 stycznia 2027/ }) as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole("button", { name: /, 12 stycznia 2027/ }) as HTMLButtonElement).disabled).toBe(false);
  });
});
