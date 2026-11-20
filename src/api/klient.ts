// Klient HTTP do API serwera ROBINWOOD. Jedyny punkt, przez który interfejs czyta i zapisuje dane.

import type {
  DaneKlienta,
  KategoriaMaterialu,
  Klient,
  PozycjaSlownika,
  Slownik,
  StatusSerwera,
  Zdarzenie,
} from "./typy";

export class BladApi extends Error {
  readonly kod: number;
  constructor(kod: number, komunikat: string) {
    super(komunikat);
    this.name = "BladApi";
    this.kod = kod;
  }
}

type Fetch = typeof fetch;

export class KlientApi {
  readonly adres: string;
  private readonly klucz: string;
  private readonly fetchFn: Fetch;

  constructor(adres: string, klucz: string, fetchFn: Fetch = (...a) => fetch(...a)) {
    this.adres = adres.replace(/\/+$/, "");
    this.klucz = klucz;
    this.fetchFn = fetchFn;
  }

  private async zadanie<T>(metoda: string, sciezka: string, cialo?: unknown): Promise<T> {
    let odp: Response;
    try {
      odp = await this.fetchFn(`${this.adres}/api/v1${sciezka}`, {
        method: metoda,
        headers: {
          Authorization: `Bearer ${this.klucz}`,
          ...(cialo !== undefined ? { "Content-Type": "application/json" } : {}),
        },
        body: cialo !== undefined ? JSON.stringify(cialo) : undefined,
      });
    } catch {
      throw new BladApi(0, `Brak połączenia z serwerem (${this.adres})`);
    }
    if (odp.status === 204) return undefined as T;
    const tekst = await odp.text();
    const dane: unknown = tekst ? JSON.parse(tekst) : undefined;
    if (!odp.ok) {
      const komunikat =
        dane && typeof dane === "object" && "blad" in dane && typeof dane.blad === "string"
          ? dane.blad
          : `Błąd serwera (${odp.status})`;
      throw new BladApi(odp.status, komunikat);
    }
    return dane as T;
  }

  status() {
    return this.zadanie<StatusSerwera>("GET", "/status");
  }

  klienci = {
    lista: (szukaj?: string) =>
      this.zadanie<Klient[]>(
        "GET",
        `/klienci${szukaj ? `?szukaj=${encodeURIComponent(szukaj)}` : ""}`,
      ),
    pobierz: (id: number) => this.zadanie<Klient>("GET", `/klienci/${id}`),
    dodaj: (dane: DaneKlienta) => this.zadanie<Klient>("POST", "/klienci", dane),
    zmien: (id: number, dane: DaneKlienta) => this.zadanie<Klient>("PUT", `/klienci/${id}`, dane),
    usun: (id: number) => this.zadanie<void>("DELETE", `/klienci/${id}`),
  };

  slowniki = {
    lista: () => this.zadanie<Slownik[]>("GET", "/slowniki"),
    pozycje: (kod: string) =>
      this.zadanie<PozycjaSlownika[]>("GET", `/slowniki/${encodeURIComponent(kod)}`),
    kategorieMaterialow: () => this.zadanie<KategoriaMaterialu[]>("GET", "/kategorie-materialow"),
  };

  /** Subskrypcja zdarzeń o zmianach (WebSocket); zwraca funkcję kończącą. Łączy się ponownie po zerwaniu. */
  subskrybujZdarzenia(obsluga: (z: Zdarzenie) => void): () => void {
    let zamkniete = false;
    let gniazdo: WebSocket | null = null;
    let ponowienie: ReturnType<typeof setTimeout> | undefined;
    const polacz = () => {
      const url = `${this.adres.replace(/^http/, "ws")}/api/v1/zdarzenia?klucz=${encodeURIComponent(this.klucz)}`;
      gniazdo = new WebSocket(url);
      gniazdo.onmessage = (e) => {
        try {
          obsluga(JSON.parse(String(e.data)) as Zdarzenie);
        } catch {
          /* pomijamy nieczytelne wiadomości */
        }
      };
      gniazdo.onclose = () => {
        if (!zamkniete) ponowienie = setTimeout(polacz, 3000);
      };
    };
    polacz();
    return () => {
      zamkniete = true;
      clearTimeout(ponowienie);
      gniazdo?.close();
    };
  }
}
