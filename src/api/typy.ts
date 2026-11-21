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
  podgrupa?: "glowna" | "wyposazenie" | "frezowanie" | null;
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

// ---------------------------------------------------------------------------
// Faza 2: specyfikacja, oferty, szablony, import, dane firmy
// (odpowiadają crates/robinwood-serwer/src/{specyfikacje,oferty,wycena,szablony,firma,roznice}.rs)
// ---------------------------------------------------------------------------

export type Podgrupa = "glowna" | "wyposazenie" | "frezowanie";
export type StatusWiersza = "ustalone" | "do_potwierdzenia" | "po_stronie_klienta" | "poza_wycena";
export type TypSekcji = "meble" | "agd";
export type StatusOferty = "szkic" | "wyslana" | "zaakceptowana" | "odrzucona" | "wygasla";
export type PoziomWyceny = "czesc" | "sekcja" | "calosc";
export type TrybCen = "brutto" | "netto";

export interface Wiersz {
  id: number;
  czesc_id: number;
  ident: string | null;
  podgrupa: Podgrupa;
  rodzaj: string | null;
  material: string | null;
  opis: string | null;
  ilosc: number | null;
  jednostka: string | null;
  status: StatusWiersza;
  termin_potwierdzenia: string | null;
  kategoria_materialu_id: number | null;
  pozycja_slownika_id: number | null;
  cena_gr: number | null;
  material_z_korpusu: boolean;
  kolejnosc: number;
  /** Licznik zmian rekordu. */
  wersja: number;
  zmieniono: string | null;
}

export interface Czesc {
  id: number;
  sekcja_id: number;
  ident: string | null;
  nazwa: string;
  cena_gr: number | null;
  kolejnosc: number;
  wersja: number;
  zmieniono: string | null;
  wiersze: Wiersz[];
}

export interface Sekcja {
  id: number;
  specyfikacja_id: number;
  ident: string | null;
  numer: string | null;
  nazwa: string;
  typ: TypSekcji;
  wariant: string | null;
  cena_gr: number | null;
  stawka_vat: 8 | 23 | null;
  kolejnosc: number;
  wersja: number;
  zmieniono: string | null;
  czesci: Czesc[];
}

export interface Zdjecie {
  id: number;
  specyfikacja_id: number;
  sekcja_id: number | null;
  czesc_id: number | null;
  plik_id: number;
  podpis: string | null;
  kolejnosc: number;
  nazwa: string;
  typ_mime: string | null;
}

export interface Specyfikacja {
  id: number;
  zrodlo_id: number | null;
  sekcje: Sekcja[];
  zdjecia: Zdjecie[];
  wlasciciel: WlascicielSpecyfikacji | null;
  /** Powód w `wlasciciel.powod_blokady`. */
  tylko_do_odczytu: boolean;
}

/** Do czego należy specyfikacja (wersja oferty, umowa, aneks). */
export interface WlascicielSpecyfikacji {
  rodzaj: "wersja_oferty" | "umowa" | "aneks" | string;
  id: number;
  nadrzedny_id: number;
  opis: string;
  /** Zasób zdarzenia WebSocket odświeżanego po zmianie (oferty, umowy). */
  zasob: string;
  powod_blokady: string | null;
}

/**
 * Zmiana częściowa (PUT): tylko zmienione pola + wartości widziane przed edycją. Serwer zwraca
 * 409 z `konflikty`, jeśli ktoś inny zmienił w międzyczasie to samo pole.
 */
export type ZmianaCzesciowa<T> = Partial<T> & { poprzednie?: Partial<T> };

export interface KonfliktPola {
  pole: string;
  poprzednia: unknown;
  twoja: unknown;
  aktualna: unknown;
}

export interface BladPozycji {
  pozycja: string;
  pole: string;
  powod: string;
}

export interface WynikZmiany {
  id: number;
  specyfikacja: Specyfikacja;
}

export interface DaneSekcji {
  nazwa: string;
  typ?: TypSekcji;
  wariant?: string | null;
  cena_gr?: number | null;
  stawka_vat?: 8 | 23 | null;
  po_sekcji_id?: number | null;
}

export interface DaneCzesci {
  nazwa: string;
  cena_gr?: number | null;
  po_czesci_id?: number | null;
}

export type DaneWiersza = Partial<
  Pick<
    Wiersz,
    | "podgrupa"
    | "rodzaj"
    | "material"
    | "opis"
    | "ilosc"
    | "jednostka"
    | "status"
    | "termin_potwierdzenia"
    | "kategoria_materialu_id"
    | "pozycja_slownika_id"
    | "cena_gr"
  >
> & { material_z_korpusu?: boolean | null; po_wierszu_id?: number | null };

export interface KwotyVat {
  stawka: number;
  netto_gr: number;
  vat_gr: number;
  brutto_gr: number;
}

export interface WycenaSekcji {
  sekcja_id: number;
  numer: string | null;
  nazwa: string;
  agd: boolean;
  suma_gr: number | null;
  stawka_vat: number;
  czesci: { czesc_id: number; nazwa: string; cena_gr: number | null }[];
  brak_ceny: boolean;
}

export interface Wycena {
  poziom: PoziomWyceny;
  tryb_cen: TrybCen;
  sekcje: WycenaSekcji[];
  suma_meble_gr: number;
  rabat_termin_gr: number;
  rabat_calosc_bp: number;
  rabat_calosc_gr: number;
  meble_po_rabatach_gr: number;
  meble: KwotyVat;
  suma_agd_gr: number;
  agd: KwotyVat[];
  suma_netto_gr: number;
  suma_vat_gr: number;
  suma_brutto_gr: number;
  wewnetrzne: { koszt_wlasny_gr: number | null; marza_gr: number | null; marza_bp: number | null };
  ostrzezenia: string[];
  /** Błędy blokujące dokument dla klienta (np. rabaty większe niż suma, kwoty poza zakresem). */
  bledy: string[];
}

export interface Oferta {
  id: number;
  numer: string;
  klient_id: number;
  klient_nazwa: string;
  inwestycja_id: number | null;
  status: StatusOferty;
  data_akceptacji: string | null;
  utworzono: string;
  zmieniono: string;
  klient_usuniety: boolean;
}

export interface OfertaNaLiscie {
  id: number;
  numer: string;
  klient_id: number;
  klient_nazwa: string;
  status: StatusOferty;
  zmieniono: string;
  wersja_id: number;
  nr_wersji: number;
  data_oferty: string | null;
  data_waznosci_cen: string | null;
  adres_inwestycji: string | null;
  klient_usuniety: boolean;
  specyfikacja_id: number;
  /** null, gdy wyceny nie da się policzyć – powód w `blad_wyceny`. */
  suma_brutto_gr: number | null;
  blad_wyceny: string | null;
  po_terminie_waznosci: boolean;
}

export interface MigawkaStron {
  inwestor: { nazwa: string; adres_ulica: string | null; adres_kod: string | null; adres_miasto: string | null; nip: string | null; email: string | null; telefon: string | null };
  wykonawca: { nazwa: string; adres: string; nip: string; telefon: string; email: string };
}

export interface WersjaOferty {
  id: number;
  oferta_id: number;
  nr_wersji: number;
  specyfikacja_id: number;
  data_oferty: string | null;
  termin_realizacji: string | null;
  data_waznosci_cen: string | null;
  adres_inwestycji: string | null;
  poziom_wyceny: PoziomWyceny;
  cena_calosc_gr: number | null;
  rabat_termin_gr: number;
  rabat_calosc_bp: number;
  stawka_vat: 8 | 23;
  tryb_cen: TrybCen;
  koszt_wlasny_gr: number | null;
  klauzule: string[];
  uwagi: string | null;
  utworzono: string;
  zmieniono: string | null;
  najnowsza: boolean;
  wersja: number;
  wyslano: string | null;
  migawka: MigawkaStron | null;
  /** Starsza wersja albo oferta poza szkicem – zmiany tylko przez nową wersję. */
  tylko_do_odczytu: boolean;
  powod_blokady: string | null;
}

export type DaneWersji = Pick<
  WersjaOferty,
  | "data_oferty"
  | "termin_realizacji"
  | "data_waznosci_cen"
  | "adres_inwestycji"
  | "poziom_wyceny"
  | "cena_calosc_gr"
  | "rabat_termin_gr"
  | "rabat_calosc_bp"
  | "stawka_vat"
  | "tryb_cen"
  | "koszt_wlasny_gr"
  | "klauzule"
  | "uwagi"
>;

export interface WersjaKrotko {
  id: number;
  nr_wersji: number;
  data_oferty: string | null;
  utworzono: string;
  wyslano: string | null;
}

export interface PelnaWersja {
  oferta: Oferta;
  wersje: WersjaKrotko[];
  wersja: WersjaOferty;
  wycena: Wycena;
  specyfikacja: Specyfikacja;
}

export interface SzczegolyOferty {
  oferta: Oferta;
  wersje: WersjaKrotko[];
}

export interface DaneNowejOferty {
  klient_id: number;
  adres_inwestycji?: string | null;
  termin_realizacji?: string | null;
  stawka_vat?: 8 | 23;
  poziom_wyceny?: PoziomWyceny;
}

export interface ZmianaPola {
  pole: string;
  przed: string | null;
  po: string | null;
}

export interface Roznica {
  poziom: "naglowek" | "wycena" | "sekcja" | "czesc" | "wiersz" | "zdjecie";
  zmiana: "dodano" | "usunieto" | "zmieniono" | "przeniesiono";
  sciezka: string;
  pola: ZmianaPola[];
}

export interface Porownanie {
  z_wersji: number;
  do_wersji: number;
  roznice: Roznica[];
}

export interface WierszSzablonu {
  podgrupa: Podgrupa | null;
  rodzaj: string | null;
  material: string | null;
  opis: string | null;
  kategoria: string | null;
  udzial: number | null;
}

export interface SzablonMebli {
  id: number;
  kod: string;
  nazwa: string;
  tytul_sekcji: string;
  zrodlo: string | null;
  czesci: string[];
  wiersze: WierszSzablonu[];
}

export interface RaportImportu {
  dodane: Record<string, number>;
  zaktualizowane: number;
  pominiete: number;
  nowe_kategorie: string[];
  ostrzezenia: string[];
}

export interface DaneFirmy {
  firma_nazwa: string;
  firma_adres: string;
  firma_nip: string;
  firma_telefon: string;
  firma_email: string;
  oferta_waznosc_dni: number;
  oferta_termin_realizacji: string;
}

/** Podpowiedzi edytora: kod słownika → pozycje. */
export type Podpowiedzi = Record<string, PozycjaSlownika[]>;
