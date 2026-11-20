import { describe, expect, it, vi } from "vitest";
import { BladApi, KlientApi } from "./klient";

function odpowiedz(status: number, cialo?: unknown): Response {
  return new Response(cialo === undefined ? null : JSON.stringify(cialo), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}

describe("KlientApi", () => {
  it("wysyła klucz i buduje adres", async () => {
    const f = vi.fn(async () => odpowiedz(200, []));
    const api = new KlientApi("http://192.168.1.10:47800/", "abc", f as unknown as typeof fetch);
    await api.klienci.lista("Klient Testowy");
    expect(f).toHaveBeenCalledTimes(1);
    const [url, opcje] = f.mock.calls[0] as unknown as [string, RequestInit];
    expect(url).toBe("http://192.168.1.10:47800/api/v1/klienci?szukaj=Klient%20Testowy");
    expect((opcje.headers as Record<string, string>).Authorization).toBe("Bearer abc");
  });

  it("zamienia błąd serwera na BladApi z komunikatem", async () => {
    const f = vi.fn(async () => odpowiedz(422, { blad: "Nieprawidłowe dane: nazwa klienta jest wymagana" }));
    const api = new KlientApi("http://x", "k", f as unknown as typeof fetch);
    await expect(api.klienci.dodaj({ nazwa: "" })).rejects.toMatchObject({
      kod: 422,
      message: "Nieprawidłowe dane: nazwa klienta jest wymagana",
    });
  });

  it("zgłasza brak połączenia", async () => {
    const f = vi.fn(async () => {
      throw new TypeError("Failed to fetch");
    });
    const api = new KlientApi("http://x", "k", f as unknown as typeof fetch);
    const blad = await api.status().catch((e: unknown) => e);
    expect(blad).toBeInstanceOf(BladApi);
    expect((blad as BladApi).kod).toBe(0);
  });

  it("obsługuje 204 bez treści", async () => {
    const f = vi.fn(async () => new Response(null, { status: 204 }));
    const api = new KlientApi("http://x", "k", f as unknown as typeof fetch);
    await expect(api.klienci.usun(1)).resolves.toBeUndefined();
  });
});
