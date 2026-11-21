// @vitest-environment jsdom
// Testy panelu szczegółów: 409 „do potwierdzenia” → okno z wierszami → świadome „zamawiam mimo to”.

import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, describe, expect, it, vi } from "vitest";
import { BladApi } from "../../api/klient";
import { zadanie, zamowienie } from "./fabryka";

const fakeApi = {
  kalendarz: {
    wykonanie: vi.fn(),
    wierszeSpecyfikacji: vi.fn(async () => [{ id: 5, sekcja: "01 Kuchnia", czesc: "Szafki górne", rodzaj: "Fronty", material: "MDF – kolor do wyboru", opis: null, status: "do_potwierdzenia" }]),
    zadanieKalendarza: vi.fn(),
    usunZadanie: vi.fn(),
  },
  materialy: { cofnijZamowienie: vi.fn() },
};
vi.mock("../../kontekst/Polaczenie", () => ({ usePolaczenie: () => ({ api: fakeApi }) }));
const { PanelZadania } = await import("./PanelZadania");

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("PanelZadania", () => {
  it("zamówienie zablokowane: 409 → lista wierszy → „Zamawiam mimo to” wysyła mimo_blokady", async () => {
    const z = zamowienie("2027-01-08", "2027-01-11", "2027-01-13", { id: 4 });
    z.zamowienie = { ...z.zamowienie!, zablokowane: true, potwierdzic_do: "2027-01-11", wiersze_do_potwierdzenia: [5] };
    fakeApi.kalendarz.wykonanie
      .mockRejectedValueOnce(new BladApi(409, "materiał czeka na potwierdzenie", { kod: "do_potwierdzenia", wiersze_do_potwierdzenia: [5] }))
      .mockResolvedValueOnce({ ...z, wykonano: "2027-01-10" });
    const onZmiana = vi.fn();
    render(
      <MemoryRouter>
        <PanelZadania zadanie={z} dzis="2027-01-10" onZamknij={() => {}} onZmiana={onZmiana} />
      </MemoryRouter>,
    );
    expect(screen.getByText(/Czeka na potwierdzenie klienta do 11.01.2027/)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: /Zamówione/ }));
    await screen.findByRole("dialog", { name: /zamówić mimo to/ });
    await screen.findByText("MDF – kolor do wyboru");
    expect(onZmiana).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Zamawiam mimo to" }));
    await waitFor(() => expect(onZmiana).toHaveBeenCalled());
    expect(fakeApi.kalendarz.wykonanie).toHaveBeenLastCalledWith(4, { wykonano: "2027-01-10", data_zamowienia: "2027-01-10", mimo_blokady: true });
  });

  it("czerwone: nowy możliwy termin montażu; data z przyszłości blokuje zapis; błąd sieci po polsku", async () => {
    const z = zadanie({ id: 6, typ: "pomiar", data_od: "2027-01-05", stan: { stan: "czerwony", opis: "Termin minął", dni_do_terminu: -5, przesuniecie_montazu_dni: 4, nowy_termin_montazu: "2027-02-19" } });
    fakeApi.kalendarz.wykonanie.mockRejectedValueOnce(new BladApi(0, "Brak połączenia z serwerem (http://test)"));
    render(
      <MemoryRouter>
        <PanelZadania zadanie={z} dzis="2027-01-10" onZamknij={() => {}} onZmiana={() => {}} />
      </MemoryRouter>,
    );
    expect(screen.getByText(/Nowy możliwy termin montażu: 19.02.2027/)).toBeTruthy();
    const data = screen.getByLabelText("Data wykonania");
    fireEvent.change(data, { target: { value: "2027-01-20" } });
    expect(screen.getByText("Data nie może być z przyszłości")).toBeTruthy();
    expect((screen.getByRole("button", { name: /Oznacz jako wykonane/ }) as HTMLButtonElement).disabled).toBe(true);
    fireEvent.change(data, { target: { value: "2027-01-09" } });
    fireEvent.click(screen.getByRole("button", { name: /Oznacz jako wykonane/ }));
    expect(await screen.findByRole("alert")).toHaveProperty("textContent", expect.stringContaining("Brak połączenia z serwerem"));
  });
});
