// Typy API Generatora Umów (Faza 3) – odpowiadają strukturom z crates/robinwood-serwer/src/umowy.rs,
// aneksy.rs, dokument_umowy.rs i szablon_docx.rs. Kwoty w groszach, daty ISO „RRRR-MM-DD”.

import type { Roznica, Specyfikacja } from "./typy";

export type StatusUmowy = "szkic" | "wyslana" | "podpisana" | "w_realizacji" | "odebrana" | "zamknieta" | "anulowana";
export type WariantPlatnosci = "A" | "B";
export type RodzajNieruchomosci = "lokal_mieszkalny" | "dom_jednorodzinny" | "inny";

/** Pola skróconego formularza (PUT /umowy/{id} przyjmuje dowolny ich podzbiór + `poprzednie`). */
export interface DaneUmowy {
  numer: string | null;
  miejscowosc: string | null;
  data_zawarcia: string | null;
  klient_id: number | null;
  wartosc_brutto_gr: number | null;
  stawka_vat: 8 | 23 | null;
  zadatek_gr: number | null;
  termin_zadatku: string | null;
  wariant_platnosci: WariantPlatnosci | null;
  kwota_przed_montazem_gr: number | null;
  montaz_do: string | null;
  dni_na_brakujace_informacje: number | null;
  vat_rodzaj_nieruchomosci: RodzajNieruchomosci | null;
  vat_powierzchnia_m2: number | null;
  vat_podstawa_powierzchni: string | null;
  zalaczniki: string[] | null;
  czas_warsztat_dni: number | null;
  czas_montazu_dni: number | null;
  uwagi: string | null;
  montaz_ulica: string | null;
  montaz_kod: string | null;
  montaz_miasto: string | null;
  montaz_pietro: string | null;
  montaz_kod_domofonu: string | null;
  montaz_parking: string | null;
  montaz_winda_wniesienie: string | null;
}

export interface OsobaKontaktowa {
  id?: number;
  imie_nazwisko: string;
  rola: string | null;
  email: string | null;
  telefon: string | null;
}

export interface Umowa extends DaneUmowy {
  id: number;
  rok: number;
  nr_kolejny: number;
  status: StatusUmowy;
  klient_nazwa: string | null;
  klient_usuniety: boolean;
  inwestycja_id: number | null;
  wersja_oferty_id: number | null;
  oferta_id: number | null;
  oferta_numer: string | null;
  nr_wersji_oferty: number | null;
  specyfikacja_id: number;
  specyfikacja_pierwotna_id: number | null;
  wersja: number;
  utworzono: string;
  zmieniono: string;
  wyslano: string | null;
  data_podpisania: string | null;
  anulowano: string | null;
  anulowanie_powod: string | null;
  migawka: unknown;
  tylko_do_odczytu: boolean;
  powod_blokady: string | null;
}

export interface PozycjaPlanu {
  rodzaj: "zadatek" | "przed_montazem" | "koncowa";
  opis: string;
  kwota_gr: number;
  termin: string | null;
  termin_opis: string;
  szacunkowy: boolean;
}

export interface PlanPlatnosci {
  wariant: WariantPlatnosci | null;
  pozycje: PozycjaPlanu[];
  suma_gr: number;
  zgodny: boolean;
  bledy: string[];
  ostrzezenia: string[];
}

export interface PlatnoscPlanowana {
  id: number;
  umowa_id: number;
  rodzaj: string;
  opis: string | null;
  kwota_gr: number;
  termin: string | null;
  kolejnosc: number;
}

export type SposobWplaty = "przelew" | "gotowka" | "karta" | "inny";

export interface Wplata {
  id: number;
  umowa_id: number;
  platnosc_planowana_id: number | null;
  data: string;
  kwota_gr: number;
  sposob: SposobWplaty | null;
  uwagi: string | null;
  utworzono: string;
}

export interface PozycjaSalda {
  opis: string;
  termin: string | null;
  kwota_gr: number;
  do_zaplaty_gr: number;
  zalegla: boolean;
}

export interface Saldo {
  wartosc_gr: number;
  wplacono_gr: number;
  pozostalo_gr: number;
  zalegle_gr: number;
  pozycje: PozycjaSalda[];
  nastepna: PozycjaSalda | null;
}

export interface Etap {
  id?: number;
  lp?: number;
  nazwa: string;
  termin: string | null;
  kwota_gr: number;
}

export interface KontrolaEtapow {
  suma_gr: number;
  roznica_gr: number;
  zgodna: boolean;
}

export interface AneksKrotko {
  id: number;
  nr: number;
  data: string | null;
  status: StatusAneksu;
  zmiana_ceny_gr: number;
  nowy_termin_montazu: string | null;
  opis_zmian: string;
}

export interface Protokol {
  id: number;
  umowa_id: number;
  data: string;
  uwagi: string | null;
  bez_zastrzezen: boolean;
}

export interface ZlecenieUmowy {
  id: number;
  etap: string;
  montaz_od: string | null;
  harmonogram_nieaktualny_powod: string | null;
  zadania: number;
  materialy: number;
}

export interface PelnaUmowa {
  umowa: Umowa;
  specyfikacja: Specyfikacja;
  plan: PlanPlatnosci;
  platnosci: PlatnoscPlanowana[];
  wplaty: Wplata[];
  saldo: Saldo;
  etapy: Etap[];
  kontrola_etapow: KontrolaEtapow;
  aneksy: AneksKrotko[];
  protokol: Protokol | null;
  zlecenie: ZlecenieUmowy | null;
  ostrzezenia: string[];
  braki: string[];
}

export interface WynikHarmonogramu {
  zlecenie_id: number;
  wygenerowano: boolean;
  powod: string | null;
  dodane: number;
  zmienione: number;
  materialy_dodane: number;
}

export interface WynikStatusu extends PelnaUmowa {
  harmonogram: WynikHarmonogramu | null;
}

export interface UmowaNaLiscie {
  id: number;
  numer: string;
  rok: number;
  klient_id: number;
  klient_nazwa: string | null;
  status: StatusUmowy;
  wartosc_brutto_gr: number | null;
  wplacono_gr: number;
  saldo_gr: number;
  montaz_do: string | null;
  data_zawarcia: string | null;
  oferta_numer: string | null;
  zmieniono: string;
}

export interface FiltrUmow {
  status?: StatusUmowy | "";
  klient_id?: number | null;
  rok?: number | null;
  szukaj?: string;
}

export interface RozniceUmowy {
  zrodlo: string | null;
  roznice: Roznica[];
}

export type StatusAneksu = "szkic" | "wyslany" | "podpisany";

export interface DaneAneksu {
  data: string | null;
  opis_zmian: string;
  zmiana_ceny_gr: number;
  nowy_termin_montazu: string | null;
}

export interface Aneks extends DaneAneksu {
  id: number;
  umowa_id: number;
  nr: number;
  status: StatusAneksu;
  specyfikacja_id: number | null;
  poprzednia_wartosc_gr: number | null;
  poprzedni_termin_montazu: string | null;
  data_podpisania: string | null;
  wersja: number;
}

export interface PelnyAneks {
  aneks: Aneks;
  umowa_numer: string;
  umowa_status: StatusUmowy;
  nowa_wartosc_gr: number | null;
  specyfikacja: Specyfikacja | null;
  roznice: Roznica[];
  lista_zmian: string[];
  tylko_do_odczytu: boolean;
}

export interface WynikStatusuAneksu extends PelnyAneks {
  harmonogram: WynikHarmonogramu | null;
}

export interface UstawieniaUmow {
  format_numeru: string;
  miejscowosc: string;
  dni_na_informacje: number;
  termin_zadatku_dni: number;
  zadatek_procent: number;
  wariant_a_dni_przed_montazem: number;
  wariant_b_dni_przed_montazem: number;
  wariant_b_dni_po_odbiorze: number;
  czas_warsztat_dni: number;
  czas_montazu_dni: number;
}

export interface RaportSzablonu {
  pola: string[];
  nieznane: string[];
  puste: string[];
  bloki_warunkowe: string[];
  do_uzupelnienia: number;
  kontrolki: number;
  ostrzezenia: string[];
}

export type RodzajSzablonu = "umowa" | "aneks";

export interface StanSzablonu {
  rodzaj: RodzajSzablonu;
  zrodlo: "wlasny" | "przykladowy";
  meta: { nazwa_pliku: string; wgrano: string; rozmiar: number } | null;
  raport: RaportSzablonu;
}

export interface OpisPola {
  nazwa: string;
  opis: string;
  rodzaj: "tekst" | "blok" | "warunek";
  dokument: "umowa" | "aneks" | "oba";
}

export interface PodgladPol {
  zrodlo: "wlasny" | "przykladowy";
  nazwa_szablonu: string | null;
  pola: { pole: string; wartosc: string; pusta: boolean }[];
  raport: RaportSzablonu;
}
