// @vitest-environment jsdom
// Testy komponentu TabelaWierszy: autozapis, kopia szkiców, konflikt, błędne wartości,
// usunięcie z „Cofnij”, zapis przy zamykaniu.

import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { BladApi } from "../../api/klient";
import type { Czesc, Wiersz, WynikZmiany } from "../../api/typy";
import { oproznijWszystkieSzkice } from "../../lib/szkice";
import { TabelaWierszy, type ApiWierszy } from "./TabelaWierszy";

function wiersz(id: number, rodzaj: string, zmiany: Partial<Wiersz> = {}): Wiersz {
  return {
    id, czesc_id: 1, ident: null, podgrupa: "glowna", rodzaj, material: "Płyta A", opis: null, ilosc: null, jednostka: null,
    status: "ustalone", termin_potwierdzenia: null, kategoria_materialu_id: null, pozycja_slownika_id: null, cena_gr: null,
    material_z_korpusu: false, kolejnosc: id, wersja: 1, zmieniono: null, ...zmiany,
  };
}

const czesc = (): Czesc => ({ id: 1, sekcja_id: 1, ident: null, nazwa: "Szafki", cena_gr: null, kolejnosc: 0, wersja: 1, zmieniono: null, wiersze: [wiersz(10, "Korpus"), wiersz(11, "Fronty")] });
const wynik = { id: 1, specyfikacja: {} } as unknown as WynikZmiany;

type Atrapa = { [K in keyof ApiWierszy]: ReturnType<typeof vi.fn> & ApiWierszy[K] };

function api(): Atrapa {
  return {
    dodajWiersz: vi.fn(async () => wynik),
    zmienWiersz: vi.fn(async () => wynik),
    usunWiersz: vi.fn(async () => wynik),
    kopiujMaterialWDol: vi.fn(async () => wynik),
    kolejnoscWierszy: vi.fn(async () => wynik),
  } as Atrapa;
}

/** Tak jak edytor: wykonuje obietnicę, przekazuje błąd do `przechwyc`. */
const wykonaj = async (z: Promise<WynikZmiany>, przechwyc?: (e: BladApi) => boolean) => {
  try {
    return await z;
  } catch (e) {
    przechwyc?.(e as BladApi);
    return null;
  }
};

function pokaz(a: ApiWierszy, c = czesc()) {
  return render(<TabelaWierszy czesc={c} agd={false} tylkoDoOdczytu={false} podpowiedzi={{}} wykonaj={wykonaj} api={a} />);
}

const opis = (rodzaj: string) => screen.getAllByLabelText("Opis")[rodzaj === "Korpus" ? 0 : 1] as HTMLInputElement;

beforeEach(() => {
  vi.useFakeTimers();
  localStorage.clear();
});
afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

describe("TabelaWierszy", () => {
  it("zapisuje automatycznie ok. 1 s po pisaniu – tylko zmienione pole z wartością poprzednią", async () => {
    const a = api();
    pokaz(a);
    fireEvent.change(opis("Korpus"), { target: { value: "Biały mat" } });
    expect(screen.getByRole("status").textContent).toContain("Niezapisane");
    await act(async () => {
      await vi.advanceTimersByTimeAsync(500);
    });
    expect(a.zmienWiersz).not.toHaveBeenCalled();
    await act(async () => {
      await vi.advanceTimersByTimeAsync(600);
    });
    expect(a.zmienWiersz).toHaveBeenCalledWith(10, { opis: "Biały mat", poprzednie: { opis: null } });
  });

  it("trzyma kopię szkicu w localStorage i odtwarza ją po ponownym otwarciu", async () => {
    const a = api();
    const r = pokaz(a);
    fireEvent.change(opis("Fronty"), { target: { value: "Szkic przed awarią" } });
    expect(localStorage.getItem("robinwood:szkic:wiersze:1")).toContain("Szkic przed awarią");
    // „awaria” – odmontowanie bez zapisu (serwer nie odpowiada)
    a.zmienWiersz.mockImplementation(() => new Promise(() => {}));
    r.unmount();
    const b = api();
    pokaz(b);
    expect(opis("Fronty").value).toBe("Szkic przed awarią");
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1100);
    });
    expect(b.zmienWiersz).toHaveBeenCalledWith(11, { opis: "Szkic przed awarią", poprzednie: { opis: null } });
  });

  it("zapisuje szkice od razu przy zamykaniu okna/karty", async () => {
    const a = api();
    pokaz(a);
    fireEvent.change(opis("Korpus"), { target: { value: "przed zamknięciem" } });
    await act(async () => {
      await oproznijWszystkieSzkice();
    });
    expect(a.zmienWiersz).toHaveBeenCalledTimes(1);
  });

  it("konflikt 409: okno „moja / cudza”, „Zachowaj moją” wysyła ponownie z cudzą wartością jako poprzednią", async () => {
    const a = api();
    a.zmienWiersz.mockRejectedValueOnce(
      new BladApi(409, "Ktoś inny zmienił w międzyczasie: Materiał", { konflikty: [{ pole: "material", poprzednia: "Płyta A", twoja: "Płyta B", aktualna: "Płyta C" }] }),
    );
    pokaz(a);
    fireEvent.change(screen.getAllByLabelText("Materiał")[0]!, { target: { value: "Płyta B" } });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1100);
    });
    expect(screen.getByRole("dialog").textContent).toContain("Płyta C");
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Zachowaj moją" }));
      await vi.advanceTimersByTimeAsync(10);
    });
    expect(a.zmienWiersz).toHaveBeenLastCalledWith(10, { material: "Płyta B", poprzednie: { material: "Płyta C" } });
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("błędna ilość: czytelny komunikat, wartość nie znika i nie jest wysyłana", async () => {
    const a = api();
    pokaz(a);
    const il = screen.getAllByLabelText("Ilość")[0] as HTMLInputElement;
    fireEvent.change(il, { target: { value: "2,5x" } });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1100);
    });
    expect(a.zmienWiersz).not.toHaveBeenCalled();
    expect(il.value).toBe("2,5x");
    expect(il.getAttribute("aria-invalid")).toBe("true");
    expect(screen.getByRole("alert").textContent).toContain("Wpisz liczbę");
    fireEvent.change(il, { target: { value: "2,5" } });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1100);
    });
    expect(a.zmienWiersz).toHaveBeenCalledWith(10, { ilosc: 2.5, poprzednie: { ilosc: null } });
  });

  it("Ctrl+Delete usuwa wiersz z paskiem „Cofnij”, który przywraca wiersz", async () => {
    const a = api();
    pokaz(a);
    await act(async () => {
      fireEvent.keyDown(screen.getAllByLabelText("Rodzaj")[1]!, { key: "Delete", ctrlKey: true });
    });
    expect(a.usunWiersz).toHaveBeenCalledWith(11);
    expect(screen.getByText(/Usunięto wiersz „Fronty”/)).toBeTruthy();
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: /Cofnij/ }));
    });
    expect(a.dodajWiersz).toHaveBeenCalledWith(1, expect.objectContaining({ rodzaj: "Fronty", material: "Płyta A", po_wierszu_id: 10 }));
  });

  it("Alt+↓ przesuwa wiersz w dół", async () => {
    const a = api();
    pokaz(a);
    await act(async () => {
      fireEvent.keyDown(screen.getAllByLabelText("Rodzaj")[0]!, { key: "ArrowDown", altKey: true });
    });
    expect(a.kolejnoscWierszy).toHaveBeenCalledWith(1, [11, 10]);
  });
});
