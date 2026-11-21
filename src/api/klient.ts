// Klient HTTP do API serwera ROBINWOOD. Jedyny punkt, przez który interfejs czyta i zapisuje dane.

import type {
  DaneCzesci,
  DaneFirmy,
  DaneNowejOferty,
  DaneSekcji,
  DaneWersji,
  DaneWiersza,
  OfertaNaLiscie,
  PelnaWersja,
  Podpowiedzi,
  Porownanie,
  RaportImportu,
  Specyfikacja,
  StatusOferty,
  SzablonMebli,
  SzczegolyOferty,
  WynikZmiany,
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

  private async surowe(metoda: string, sciezka: string, cialo?: BodyInit, typ?: string): Promise<Response> {
    let odp: Response;
    try {
      odp = await this.fetchFn(`${this.adres}/api/v1${sciezka}`, {
        method: metoda,
        headers: {
          Authorization: `Bearer ${this.klucz}`,
          ...(typ ? { "Content-Type": typ } : {}),
        },
        body: cialo,
      });
    } catch {
      throw new BladApi(0, `Brak połączenia z serwerem (${this.adres})`);
    }
    if (!odp.ok) {
      const tekst = await odp.text();
      let komunikat = `Błąd serwera (${odp.status})`;
      try {
        const d: unknown = JSON.parse(tekst);
        if (d && typeof d === "object" && "blad" in d && typeof d.blad === "string") komunikat = d.blad;
      } catch {
        /* treść nie jest JSON-em */
      }
      throw new BladApi(odp.status, komunikat);
    }
    return odp;
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

  // --- Faza 2: specyfikacja (wspólna dla ofert i umów) ---

  specyfikacje = {
    pobierz: (id: number) => this.zadanie<Specyfikacja>("GET", `/specyfikacje/${id}`),
    dodajSekcje: (specId: number, d: DaneSekcji) => this.zadanie<WynikZmiany>("POST", `/specyfikacje/${specId}/sekcje`, d),
    sekcjaZSzablonu: (specId: number, d: { szablon_id: number; nazwa?: string; czesci?: string[] }) =>
      this.zadanie<WynikZmiany>("POST", `/specyfikacje/${specId}/z-szablonu`, d),
    kolejnoscSekcji: (specId: number, ids: number[]) =>
      this.zadanie<WynikZmiany>("POST", `/specyfikacje/${specId}/sekcje/kolejnosc`, { ids }),
    zmienSekcje: (id: number, d: DaneSekcji) => this.zadanie<WynikZmiany>("PUT", `/sekcje/${id}`, d),
    usunSekcje: (id: number) => this.zadanie<WynikZmiany>("DELETE", `/sekcje/${id}`),
    kopiujSekcje: (id: number, celId?: number) => this.zadanie<WynikZmiany>("POST", `/sekcje/${id}/kopiuj`, { cel_id: celId ?? null }),
    dodajCzesc: (sekcjaId: number, d: DaneCzesci) => this.zadanie<WynikZmiany>("POST", `/sekcje/${sekcjaId}/czesci`, d),
    czescZSzablonu: (sekcjaId: number, d: { szablon_id: number; nazwa?: string; po_czesci_id?: number | null }) =>
      this.zadanie<WynikZmiany>("POST", `/sekcje/${sekcjaId}/z-szablonu`, d),
    kolejnoscCzesci: (sekcjaId: number, ids: number[]) =>
      this.zadanie<WynikZmiany>("POST", `/sekcje/${sekcjaId}/czesci/kolejnosc`, { ids }),
    zmienCzesc: (id: number, d: DaneCzesci) => this.zadanie<WynikZmiany>("PUT", `/czesci/${id}`, d),
    usunCzesc: (id: number) => this.zadanie<WynikZmiany>("DELETE", `/czesci/${id}`),
    kopiujCzesc: (id: number, celId?: number) => this.zadanie<WynikZmiany>("POST", `/czesci/${id}/kopiuj`, { cel_id: celId ?? null }),
    dodajWiersz: (czescId: number, d: DaneWiersza) => this.zadanie<WynikZmiany>("POST", `/czesci/${czescId}/wiersze`, d),
    kolejnoscWierszy: (czescId: number, ids: number[]) =>
      this.zadanie<WynikZmiany>("POST", `/czesci/${czescId}/wiersze/kolejnosc`, { ids }),
    zmienWiersz: (id: number, d: DaneWiersza) => this.zadanie<WynikZmiany>("PUT", `/wiersze/${id}`, d),
    usunWiersz: (id: number) => this.zadanie<WynikZmiany>("DELETE", `/wiersze/${id}`),
    kopiujMaterialWDol: (id: number) => this.zadanie<WynikZmiany>("POST", `/wiersze/${id}/kopiuj-material`),
    dodajZdjecie: async (specId: number, plik: Blob, cel: { sekcja_id?: number | null; czesc_id?: number | null; nazwa?: string; podpis?: string }) => {
      const q = new URLSearchParams();
      if (cel.sekcja_id) q.set("sekcja_id", String(cel.sekcja_id));
      if (cel.czesc_id) q.set("czesc_id", String(cel.czesc_id));
      if (cel.nazwa) q.set("nazwa", cel.nazwa);
      if (cel.podpis) q.set("podpis", cel.podpis);
      const odp = await this.surowe("POST", `/specyfikacje/${specId}/zdjecia?${q}`, plik, plik.type || "application/octet-stream");
      return (await odp.json()) as WynikZmiany;
    },
    zmienZdjecie: (id: number, podpis: string | null) => this.zadanie<WynikZmiany>("PUT", `/zdjecia/${id}`, { podpis }),
    usunZdjecie: (id: number) => this.zadanie<WynikZmiany>("DELETE", `/zdjecia/${id}`),
  };

  /** Adres pliku do użycia w <img src> (klucz w parametrze, bo obraz nie wysyła nagłówków). */
  adresPliku(plikId: number): string {
    return `${this.adres}/api/v1/pliki/${plikId}?klucz=${encodeURIComponent(this.klucz)}`;
  }

  podpowiedzi = () => this.zadanie<Podpowiedzi>("GET", "/slowniki-podpowiedzi");

  szablony = {
    lista: () => this.zadanie<SzablonMebli[]>("GET", "/szablony-mebli"),
  };

  importy = {
    slowniki: (json: unknown) => this.zadanie<RaportImportu>("POST", "/import/slowniki", json),
    szablony: (json: unknown) => this.zadanie<RaportImportu>("POST", "/import/szablony-mebli", json),
  };

  firma = {
    pobierz: () => this.zadanie<DaneFirmy>("GET", "/ustawienia/firma"),
    zapisz: (d: DaneFirmy) => this.zadanie<DaneFirmy>("PUT", "/ustawienia/firma", d),
  };

  oferty = {
    lista: (f: { status?: StatusOferty | ""; klient_id?: number | null; szukaj?: string } = {}) => {
      const q = new URLSearchParams();
      if (f.status) q.set("status", f.status);
      if (f.klient_id) q.set("klient_id", String(f.klient_id));
      if (f.szukaj?.trim()) q.set("szukaj", f.szukaj.trim());
      const s = q.toString();
      return this.zadanie<OfertaNaLiscie[]>("GET", `/oferty${s ? `?${s}` : ""}`);
    },
    dodaj: (d: DaneNowejOferty) => this.zadanie<PelnaWersja>("POST", "/oferty", d),
    szczegoly: (id: number) => this.zadanie<SzczegolyOferty>("GET", `/oferty/${id}`),
    zmienKlienta: (id: number, klientId: number) => this.zadanie<SzczegolyOferty>("PUT", `/oferty/${id}`, { klient_id: klientId }),
    zmienStatus: (id: number, status: StatusOferty) => this.zadanie<SzczegolyOferty>("PUT", `/oferty/${id}/status`, { status }),
    usun: (id: number) => this.zadanie<void>("DELETE", `/oferty/${id}`),
    nowaWersja: (id: number, zWersjiId?: number) =>
      this.zadanie<PelnaWersja>("POST", `/oferty/${id}/wersje`, { z_wersji_id: zWersjiId ?? null }),
    porownaj: (id: number, z: number, doWersji: number) =>
      this.zadanie<Porownanie>("GET", `/oferty/${id}/porownanie?z=${z}&do=${doWersji}`),
    wersja: (id: number) => this.zadanie<PelnaWersja>("GET", `/wersje-oferty/${id}`),
    zmienWersje: (id: number, d: DaneWersji) => this.zadanie<PelnaWersja>("PUT", `/wersje-oferty/${id}`, d),
    /** Pobiera plik eksportu jako Blob wraz z nazwą z nagłówka Content-Disposition. */
    eksport: async (wersjaId: number, format: "pdf" | "docx", zdjecia = true) => {
      const odp = await this.surowe("GET", `/wersje-oferty/${wersjaId}/eksport/${format}?zdjecia=${zdjecia ? 1 : 0}`);
      const naglowek = odp.headers.get("Content-Disposition") ?? "";
      const nazwa = /filename="([^"]+)"/.exec(naglowek)?.[1] ?? `oferta.${format}`;
      return { nazwa, plik: await odp.blob() };
    },
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
