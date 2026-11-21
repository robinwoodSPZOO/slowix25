// @vitest-environment jsdom
// W1: licznik ponownych połączeń rośnie po każdym przejściu zerwane → połączone (z prawdziwym klientem).

import { act, cleanup, render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

const stanyGniazda: ((p: boolean) => void)[] = [];
vi.mock("../lib/tauri", () => ({
  pobierzPolaczenie: async () => ({ konfiguracja: { tryb: "klient" }, adres_api: "http://serwer.example.com:47800", klucz_api: "k", serwer_dziala: false, blad_serwera: null, folder_danych: null }),
}));
vi.mock("../api/klient", () => ({
  KlientApi: class {
    subskrybujZdarzenia(_o: unknown, s: (p: boolean) => void) {
      stanyGniazda.push(s);
      return () => {};
    }
  },
}));
const { DostawcaPolaczenia, usePolaczenie } = await import("./Polaczenie");

afterEach(cleanup);

describe("DostawcaPolaczenia", () => {
  it("wznowienia: 0 przy pierwszym połączeniu, +1 po każdej przerwie", async () => {
    const odczyty: { naZywo: boolean | null; wznowienia: number }[] = [];
    function Sonda() {
      const { naZywo, wznowienia } = usePolaczenie();
      odczyty.push({ naZywo, wznowienia });
      return null;
    }
    render(
      <DostawcaPolaczenia>
        <Sonda />
      </DostawcaPolaczenia>,
    );
    await act(async () => {});
    const s = stanyGniazda.at(-1)!;
    act(() => s(true));
    expect(odczyty.at(-1)).toEqual({ naZywo: true, wznowienia: 0 });
    act(() => s(false));
    expect(odczyty.at(-1)).toEqual({ naZywo: false, wznowienia: 0 });
    act(() => s(true));
    expect(odczyty.at(-1)).toEqual({ naZywo: true, wznowienia: 1 });
    act(() => s(false));
    act(() => s(true));
    expect(odczyty.at(-1)!.wznowienia).toBe(2);
  });
});
