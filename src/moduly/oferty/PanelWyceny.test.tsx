// @vitest-environment jsdom
// Testy komponentu PanelWyceny: walidacja rabatu, błędy wyceny, VAT sekcji AGD, zmiany częściowe.

import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { PelnaWersja } from "../../api/typy";

const fakeApi = {
  oferty: { zmienWersje: vi.fn(async () => dane()) },
  specyfikacje: { zmienSekcje: vi.fn(async () => ({ id: 2, specyfikacja: dane().specyfikacja })), zmienCzesc: vi.fn() },
};
vi.mock("../../kontekst/Polaczenie", () => ({ usePolaczenie: () => ({ api: fakeApi }) }));

const { PanelWyceny } = await import("./PanelWyceny");

function dane(z: Partial<PelnaWersja["wycena"]> = {}): PelnaWersja {
  const kv = { stawka: 8, netto_gr: 0, vat_gr: 0, brutto_gr: 10000 };
  return {
    oferta: { id: 1, numer: "OF/1", klient_id: 1, klient_nazwa: "Klient Testowy", inwestycja_id: null, status: "szkic", data_akceptacji: null, utworzono: "", zmieniono: "", klient_usuniety: false },
    wersje: [],
    wersja: {
      id: 5, oferta_id: 1, nr_wersji: 1, specyfikacja_id: 1, data_oferty: null, termin_realizacji: null, data_waznosci_cen: null, adres_inwestycji: null,
      poziom_wyceny: "sekcja", cena_calosc_gr: null, rabat_termin_gr: 0, rabat_calosc_bp: 0, stawka_vat: 8, tryb_cen: "brutto", koszt_wlasny_gr: null,
      klauzule: [], uwagi: null, utworzono: "", zmieniono: null, najnowsza: true, wersja: 1, wyslano: null, migawka: null, tylko_do_odczytu: false, powod_blokady: null,
    },
    wycena: {
      poziom: "sekcja", tryb_cen: "brutto",
      sekcje: [{ sekcja_id: 2, numer: null, nazwa: "Sprzęt AGD", agd: true, suma_gr: 10000, stawka_vat: 23, czesci: [], brak_ceny: false }],
      suma_meble_gr: 0, rabat_termin_gr: 0, rabat_calosc_bp: 0, rabat_calosc_gr: 0, meble_po_rabatach_gr: 0, meble: kv, suma_agd_gr: 10000, agd: [{ ...kv, stawka: 23 }],
      suma_netto_gr: 0, suma_vat_gr: 0, suma_brutto_gr: 10000, wewnetrzne: { koszt_wlasny_gr: null, marza_gr: null, marza_bp: null }, ostrzezenia: [], bledy: [], ...z,
    },
    specyfikacja: {
      id: 1, zrodlo_id: null, zdjecia: [], wlasciciel: null, tylko_do_odczytu: false,
      sekcje: [{ id: 2, specyfikacja_id: 1, ident: null, numer: null, nazwa: "Sprzęt AGD", typ: "agd", wariant: null, cena_gr: null, stawka_vat: 23, kolejnosc: 0, wersja: 1, zmieniono: null, czesci: [] }],
    },
  };
}

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("PanelWyceny", () => {
  it("rabat 12,345% – komunikat przy polu, bez zaokrąglania i bez zapisu", async () => {
    const onBlad = vi.fn();
    render(<PanelWyceny dane={dane()} onDane={vi.fn()} onBlad={onBlad} onSpecyfikacja={vi.fn()} />);
    const pole = screen.getByLabelText("Rabat za całość") as HTMLInputElement;
    fireEvent.change(pole, { target: { value: "12,345" } });
    fireEvent.blur(pole);
    expect(screen.getByRole("alert").textContent).toContain("2 miejsca po przecinku");
    expect(pole.value).toBe("12,345");
    expect(fakeApi.oferty.zmienWersje).not.toHaveBeenCalled();
    fireEvent.change(pole, { target: { value: "12,35" } });
    await act(async () => {
      fireEvent.blur(pole);
    });
    expect(fakeApi.oferty.zmienWersje).toHaveBeenCalledWith(5, { rabat_calosc_bp: 1235, poprzednie: { rabat_calosc_bp: 0 } });
  });

  it("pokazuje błędy blokujące dokument (rabaty większe niż suma)", () => {
    render(<PanelWyceny dane={dane({ bledy: ["Rabaty (500,00 zł) są większe niż suma za meble (100,00 zł)"] })} onDane={vi.fn()} onBlad={vi.fn()} onSpecyfikacja={vi.fn()} />);
    expect(screen.getByLabelText("Błędy wyceny").textContent).toContain("większe niż suma");
  });

  it("zmiana stawki VAT sekcji AGD na 8% wysyła zmianę częściową sekcji", async () => {
    const onSpec = vi.fn();
    render(<PanelWyceny dane={dane()} onDane={vi.fn()} onBlad={vi.fn()} onSpecyfikacja={onSpec} />);
    const grupa = screen.getByRole("radiogroup", { name: "Stawka VAT – Sprzęt AGD" });
    await act(async () => {
      fireEvent.click(grupa.querySelector('[role="radio"]')!);
    });
    expect(fakeApi.specyfikacje.zmienSekcje).toHaveBeenCalledWith(2, { stawka_vat: 8, poprzednie: { stawka_vat: 23 } });
    expect(onSpec).toHaveBeenCalled();
  });

  it("kwota ponad limit w polu ceny – komunikat zamiast zapisu", () => {
    render(<PanelWyceny dane={dane()} onDane={vi.fn()} onBlad={vi.fn()} onSpecyfikacja={vi.fn()} />);
    const pole = screen.getByLabelText("Rabat za termin") as HTMLInputElement;
    fireEvent.change(pole, { target: { value: "2 000 000 000" } });
    fireEvent.blur(pole);
    expect(screen.getByRole("alert").textContent).toContain("1 000 000 000,00");
    expect(fakeApi.oferty.zmienWersje).not.toHaveBeenCalled();
  });
});
