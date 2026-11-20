// Typy danych API. Muszą odpowiadać strukturom serde w crates/robinwood-serwer/src/*.rs.

export type TypKlienta = "indywidualny" | "firma" | "architekt" | "instytucja";

export const TYPY_KLIENTA: Record<TypKlienta, string> = {
  indywidualny: "Klient indywidualny",
  firma: "Firma",
  architekt: "Architekt / biuro projektowe",
  instytucja: "Instytucja",
};

export interface Klient {
  id: number;
  typ: TypKlienta;
  nazwa: string;
  nip: string | null;
  email: string | null;
  telefon: string | null;
  adres_ulica: string | null;
  adres_kod: string | null;
  adres_miasto: string | null;
  polecajacy_id: number | null;
  zgoda_marketing: boolean;
  zgoda_marketing_data: string | null;
  uwagi: string | null;
  utworzono: string;
  zmieniono: string;
}

export type DaneKlienta = Partial<Omit<Klient, "id" | "utworzono" | "zmieniono" | "zgoda_marketing_data">> & {
  nazwa: string;
};

export interface StatusSerwera {
  aplikacja: "robinwood";
  nazwa: string;
  wersja: string;
  wersja_api: number;
}

export interface Slownik {
  kod: string;
  nazwa: string;
}

export interface PozycjaSlownika {
  id: number;
  slownik_kod: string;
  wartosc: string;
  producent: string | null;
  dekor: string | null;
  kategoria_materialu_id: number | null;
  kolejnosc: number;
}

export interface KategoriaMaterialu {
  id: number;
  nazwa: string;
  opis: string | null;
  domyslny_dostawca_id: number | null;
  dni_dostawy_min: number;
  dni_dostawy_max: number;
}

export interface Zdarzenie {
  zasob: string;
  id: number;
  operacja: "dodanie" | "zmiana" | "usuniecie";
}
