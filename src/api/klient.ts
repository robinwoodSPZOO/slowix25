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
  BladPozycji,
  CzasDostawy,
  KonfliktPola,
  ZmianaCzesciowa,
} from "./typy";
import type * as K from "./typy";

/** Błąd API: kod HTTP, komunikat po polsku i – dla 409/422 – szczegóły z treści odpowiedzi. */
export class BladApi extends Error {
  readonly kod: number;
  /** 409: pola zmienione w międzyczasie na innym stanowisku. */
  readonly konflikty: KonfliktPola[];
  /** 409: aktualny rekord z serwera. */
  readonly aktualny: unknown;
  /** 422 (np. import): błędy poszczególnych pozycji. */
  readonly bledy: BladPozycji[];
  /** Kod rodzaju błędu z serwera (`konflikt`, `do_potwierdzenia`, `zajety`…). */
  readonly kodBledu: string | null;
  /** 409 `do_potwierdzenia`: id wierszy specyfikacji, na które czeka zamówienie. */
  readonly wierszeDoPotwierdzenia: number[];
  constructor(kod: number, komunikat: string, dane?: unknown) {
    super(komunikat);
    this.name = "BladApi";
    this.kod = kod;
    const d = (dane && typeof dane === "object" ? dane : {}) as Record<string, unknown>;
    this.konflikty = Array.isArray(d.konflikty) ? (d.konflikty as KonfliktPola[]) : [];
    this.aktualny = d.aktualny ?? null;
    this.bledy = Array.isArray(d.bledy) ? (d.bledy as BladPozycji[]) : [];
    this.kodBledu = typeof d.kod === "string" ? d.kod : null;
    this.wierszeDoPotwierdzenia = Array.isArray(d.wiersze_do_potwierdzenia) ? (d.wiersze_do_potwierdzenia as number[]) : [];
  }
}

function komunikatBledu(status: number, dane: unknown): string {
  return dane && typeof dane === "object" && "blad" in dane && typeof dane.blad === "string" ? dane.blad : `Błąd serwera (${status})`;
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
      let dane: unknown;
      try {
        dane = JSON.parse(tekst);
      } catch {
        /* treść nie jest JSON-em */
      }
      throw new BladApi(odp.status, komunikatBledu(odp.status, dane), dane);
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
        // Zapis ma dojść do serwera także wtedy, gdy karta/okno właśnie się zamyka.
        keepalive: metoda !== "GET" && cialo !== undefined && JSON.stringify(cialo).length < 60_000,
      });
    } catch {
      throw new BladApi(0, `Brak połączenia z serwerem (${this.adres})`);
    }
    if (odp.status === 204) return undefined as T;
    const tekst = await odp.text();
    let dane: unknown;
    try {
      dane = tekst ? JSON.parse(tekst) : undefined;
    } catch {
      if (odp.ok) throw new BladApi(odp.status, "Nieczytelna odpowiedź serwera");
    }
    if (!odp.ok) throw new BladApi(odp.status, komunikatBledu(odp.status, dane), dane);
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
    przywroc: (id: number) => this.zadanie<Klient>("POST", `/klienci/${id}/przywroc`),
  };

  slowniki = {
    lista: () => this.zadanie<Slownik[]>("GET", "/slowniki"),
    pozycje: (kod: string) =>
      this.zadanie<PozycjaSlownika[]>("GET", `/slowniki/${encodeURIComponent(kod)}`),
    kategorieMaterialow: () => this.zadanie<KategoriaMaterialu[]>("GET", "/kategorie-materialow"),
    /** Czasy dostaw par kategoria + dostawca (silnik terminów). */
    czasyDostaw: () => this.zadanie<CzasDostawy[]>("GET", "/czasy-dostaw"),
  };

  // --- Faza 2: specyfikacja (wspólna dla ofert i umów) ---

  specyfikacje = {
    pobierz: (id: number) => this.zadanie<Specyfikacja>("GET", `/specyfikacje/${id}`),
    dodajSekcje: (specId: number, d: DaneSekcji) => this.zadanie<WynikZmiany>("POST", `/specyfikacje/${specId}/sekcje`, d),
    sekcjaZSzablonu: (specId: number, d: { szablon_id: number; nazwa?: string; czesci?: string[] }) =>
      this.zadanie<WynikZmiany>("POST", `/specyfikacje/${specId}/z-szablonu`, d),
    kolejnoscSekcji: (specId: number, ids: number[]) =>
      this.zadanie<WynikZmiany>("POST", `/specyfikacje/${specId}/sekcje/kolejnosc`, { ids }),
    /** Zmiana częściowa: tylko zmienione pola (+ `poprzednie` – wykrywanie konfliktów, 409). */
    zmienSekcje: (id: number, d: ZmianaCzesciowa<DaneSekcji>) => this.zadanie<WynikZmiany>("PUT", `/sekcje/${id}`, d),
    usunSekcje: (id: number) => this.zadanie<WynikZmiany>("DELETE", `/sekcje/${id}`),
    kopiujSekcje: (id: number, celId?: number) => this.zadanie<WynikZmiany>("POST", `/sekcje/${id}/kopiuj`, { cel_id: celId ?? null }),
    dodajCzesc: (sekcjaId: number, d: DaneCzesci) => this.zadanie<WynikZmiany>("POST", `/sekcje/${sekcjaId}/czesci`, d),
    czescZSzablonu: (sekcjaId: number, d: { szablon_id: number; nazwa?: string; po_czesci_id?: number | null }) =>
      this.zadanie<WynikZmiany>("POST", `/sekcje/${sekcjaId}/z-szablonu`, d),
    kolejnoscCzesci: (sekcjaId: number, ids: number[]) =>
      this.zadanie<WynikZmiany>("POST", `/sekcje/${sekcjaId}/czesci/kolejnosc`, { ids }),
    zmienCzesc: (id: number, d: ZmianaCzesciowa<DaneCzesci>) => this.zadanie<WynikZmiany>("PUT", `/czesci/${id}`, d),
    usunCzesc: (id: number) => this.zadanie<WynikZmiany>("DELETE", `/czesci/${id}`),
    kopiujCzesc: (id: number, celId?: number) => this.zadanie<WynikZmiany>("POST", `/czesci/${id}/kopiuj`, { cel_id: celId ?? null }),
    dodajWiersz: (czescId: number, d: DaneWiersza) => this.zadanie<WynikZmiany>("POST", `/czesci/${czescId}/wiersze`, d),
    kolejnoscWierszy: (czescId: number, ids: number[]) =>
      this.zadanie<WynikZmiany>("POST", `/czesci/${czescId}/wiersze/kolejnosc`, { ids }),
    zmienWiersz: (id: number, d: ZmianaCzesciowa<DaneWiersza>) => this.zadanie<WynikZmiany>("PUT", `/wiersze/${id}`, d),
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

  /**
   * Plik (np. zdjęcie) jako Blob – pobierany z kluczem w nagłówku. Do `<img>` użyj
   * `URL.createObjectURL(blob)` (hook `useObrazek`); klucz NIGDY nie trafia do adresu obrazka.
   */
  async pobierzPlik(plikId: number, podglad = true): Promise<Blob> {
    const odp = await this.surowe("GET", `/pliki/${plikId}${podglad ? "?podglad=1" : ""}`);
    return odp.blob();
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
    lista: (f: { status?: StatusOferty | ""; klient_id?: number | null; szukaj?: string; limit?: number; od?: number } = {}) => {
      const q = new URLSearchParams();
      if (f.limit) q.set("limit", String(f.limit));
      if (f.od) q.set("od", String(f.od));
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
    /** Zmiana częściowa parametrów wersji (+ `poprzednie` – wykrywanie konfliktów, 409). */
    zmienWersje: (id: number, d: ZmianaCzesciowa<DaneWersji>) => this.zadanie<PelnaWersja>("PUT", `/wersje-oferty/${id}`, d),
    /** Pobiera plik eksportu jako Blob wraz z nazwą z nagłówka Content-Disposition. */
    eksport: async (wersjaId: number, format: "pdf" | "docx", zdjecia = true) => {
      const odp = await this.surowe("GET", `/wersje-oferty/${wersjaId}/eksport/${format}?zdjecia=${zdjecia ? 1 : 0}`);
      const naglowek = odp.headers.get("Content-Disposition") ?? "";
      const nazwa = /filename="([^"]+)"/.exec(naglowek)?.[1] ?? `oferta.${format}`;
      return { nazwa, plik: await odp.blob() };
    },
  };

  // --- Kalendarz (silnik terminów, Faza 4a/4b; kontrakt: docs/ARCHITEKTURA.md, rozdz. 10.4) ---

  private static zapytanie(p: Record<string, string | number | boolean | null | undefined>): string {
    const q = new URLSearchParams();
    for (const [k, v] of Object.entries(p)) if (v !== null && v !== undefined && v !== "" && v !== false) q.set(k, String(v));
    const t = q.toString();
    return t ? `?${t}` : "";
  }

  zlecenia = {
    lista: () => this.zadanie<K.Zlecenie[]>("GET", "/zlecenia"),
    pobierz: (id: number) => this.zadanie<K.Zlecenie>("GET", `/zlecenia/${id}`),
    dodaj: (d: K.DaneZlecenia) => this.zadanie<K.Zlecenie>("POST", "/zlecenia", d),
    /** Pełna zmiana zlecenia (pola pominięte = brak wartości). */
    zmien: (id: number, d: K.DaneZlecenia) => this.zadanie<K.Zlecenie>("PUT", `/zlecenia/${id}`, d),
    przelicz: (id: number) => this.zadanie<K.Zlecenie>("POST", `/zlecenia/${id}/przelicz`),
    materialy: (id: number) => this.zadanie<K.MaterialZlecenia[]>("GET", `/zlecenia/${id}/materialy`),
    podgladHarmonogramu: (id: number) => this.zadanie<K.Harmonogram>("GET", `/zlecenia/${id}/harmonogram`),
    generujHarmonogram: (id: number, dzis?: string) =>
      this.zadanie<K.WynikGenerowania>("POST", `/zlecenia/${id}/harmonogram${KlientApi.zapytanie({ dzis })}`),
  };

  kalendarz = {
    zadania: (f: { od?: string; do?: string; typ?: string; zlecenie_id?: number; stan?: string; tylko_niewykonane?: boolean; dzis?: string } = {}) =>
      this.zadanie<K.ZadanieKalendarza[]>("GET", `/kalendarz/zadania${KlientApi.zapytanie(f)}`),
    zadanieKalendarza: (id: number, dzis?: string) =>
      this.zadanie<K.ZadanieKalendarza>("GET", `/kalendarz/zadania/${id}${KlientApi.zapytanie({ dzis })}`),
    dodajZadanie: (d: K.DaneZadaniaRecznego) => this.zadanie<K.ZadanieKalendarza>("POST", "/kalendarz/zadania", d),
    zmienZadanie: (id: number, d: K.DaneZadaniaRecznego) => this.zadanie<K.ZadanieKalendarza>("PUT", `/kalendarz/zadania/${id}`, d),
    usunZadanie: (id: number) => this.zadanie<void>("DELETE", `/kalendarz/zadania/${id}`),
    /** Wykonanie / zamówienie (`wykonano: null` cofa). 409 `do_potwierdzenia` → ponów z `mimo_blokady: true`. */
    wykonanie: (id: number, w: K.Wykonanie, dzis?: string) =>
      this.zadanie<K.ZadanieKalendarza>("PUT", `/kalendarz/zadania/${id}/wykonanie${KlientApi.zapytanie({ dzis })}`, w),
    tydzien: (dzis?: string) => this.zadanie<K.Tydzien>("GET", `/kalendarz/tydzien${KlientApi.zapytanie({ dzis })}`),
    doZamowienia: (dzis?: string, horyzont?: string) =>
      this.zadanie<K.DoZamowienia>("GET", `/kalendarz/do-zamowienia${KlientApi.zapytanie({ dzis, horyzont })}`),
    kolizje: (od?: string, doDnia?: string) =>
      this.zadanie<K.KolizjaZlecen[]>("GET", `/kalendarz/kolizje${KlientApi.zapytanie({ od, do: doDnia })}`),
    statystykaDostaw: () => this.zadanie<K.StatystykaPary[]>("GET", "/kalendarz/statystyka-dostaw"),
    ustawienia: () => this.zadanie<K.UstawieniaTerminow>("GET", "/kalendarz/ustawienia"),
    zmienUstawienia: (z: Partial<K.UstawieniaTerminow>) =>
      this.zadanie<K.UstawieniaTerminow & K.WynikPrzeliczenia>("PUT", "/kalendarz/ustawienia", z),
    dniWolne: (od?: string, doDnia?: string) =>
      this.zadanie<K.DzienWolny[]>("GET", `/kalendarz/dni-wolne${KlientApi.zapytanie({ od, do: doDnia })}`),
    ustawDzienWolny: (data: string, d: { rodzaj: K.RodzajDniaWolnego; opis?: string | null }) =>
      this.zadanie<K.DzienWolny & K.WynikPrzeliczenia>("PUT", `/kalendarz/dni-wolne/${data}`, d),
    usunDzienWolny: (data: string) => this.zadanie<void>("DELETE", `/kalendarz/dni-wolne/${data}`),
    dodajZakresDniWolnych: (d: { od: string; do: string; rodzaj: K.RodzajDniaWolnego; opis?: string | null }) =>
      this.zadanie<{ dni: number } & K.WynikPrzeliczenia>("POST", "/kalendarz/dni-wolne/zakres", d),
    usunZakresDniWolnych: (od: string, doDnia: string) =>
      this.zadanie<{ dni: number } & K.WynikPrzeliczenia>("POST", "/kalendarz/dni-wolne/usun-zakres", { od, do: doDnia }),
    swieta: (rok: number) => this.zadanie<K.Swieto[]>("GET", `/kalendarz/swieta?rok=${rok}`),
    przeliczWszystko: () => this.zadanie<K.WynikPrzeliczenia>("POST", "/kalendarz/przelicz"),
    wierszeSpecyfikacji: (ids: number[]) =>
      this.zadanie<K.OpisWierszaSpecyfikacji[]>("GET", `/kalendarz/wiersze-specyfikacji?ids=${ids.join(",")}`),
  };

  materialy = {
    zmien: (id: number, z: Record<string, unknown>) => this.zadanie<K.MaterialZlecenia>("PUT", `/zamowienia-materialow/${id}`, z),
    cofnijZamowienie: (id: number) => this.zadanie<K.MaterialZlecenia>("POST", `/zamowienia-materialow/${id}/cofnij-zamowienie`),
  };

  dostawcy = {
    lista: () => this.zadanie<K.Dostawca[]>("GET", "/dostawcy"),
    dodaj: (d: K.DaneDostawcy) => this.zadanie<K.Dostawca>("POST", "/dostawcy", d),
    zmien: (id: number, d: K.DaneDostawcy) => this.zadanie<K.Dostawca>("PUT", `/dostawcy/${id}`, d),
  };

  czasyDostaw = {
    lista: () => this.zadanie<K.CzasDostawy[]>("GET", "/czasy-dostaw"),
    dodaj: (d: { kategoria_id: number; dostawca_id: number; dni_min: number; dni_max: number }) =>
      this.zadanie<K.CzasDostawy & K.WynikPrzeliczenia>("POST", "/czasy-dostaw", d),
    zmien: (id: number, d: { kategoria_id: number; dostawca_id: number; dni_min: number; dni_max: number }) =>
      this.zadanie<K.CzasDostawy & K.WynikPrzeliczenia>("PUT", `/czasy-dostaw/${id}`, d),
    usun: (id: number) => this.zadanie<void>("DELETE", `/czasy-dostaw/${id}`),
    /** Widełki kategorii (0–0 = „nie ustawiono”) i domyślny dostawca. */
    widelkiKategorii: (kategoriaId: number, d: { dni_dostawy_min: number; dni_dostawy_max: number; domyslny_dostawca_id?: number | null }) =>
      this.zadanie<void>("PUT", `/kategorie-materialow/${kategoriaId}/widelki`, d),
    importuj: (wiersze: K.WierszImportuKategorii[]) =>
      this.zadanie<K.RaportImportuKategorii>("POST", "/import/kategorie-materialow", { wiersze }),
  };

  /** Subskrypcja zdarzeń o zmianach (WebSocket); zwraca funkcję kończącą. Łączy się ponownie po zerwaniu. */
  subskrybujZdarzenia(obsluga: (z: Zdarzenie) => void, stanPolaczenia?: (polaczono: boolean) => void): () => void {
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
      gniazdo.onopen = () => stanPolaczenia?.(true);
      gniazdo.onclose = () => {
        if (!zamkniete) stanPolaczenia?.(false);
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
