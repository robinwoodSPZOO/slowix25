// Rejestr modułów: JEDNO miejsce, z którego powstaje menu boczne i trasy.
// Nowy moduł = nowy folder w src/moduly/<modul>/ + wpis tutaj.

import type { ComponentType } from "react";
import { Pulpit } from "./pulpit/Pulpit";
import { ListaKlientow } from "./klienci/ListaKlientow";
import { Oferty } from "./oferty/Oferty";
import { Umowy } from "./umowy/Umowy";
import { Kalendarz } from "./kalendarz/Kalendarz";
import { Zamowienia } from "./zamowienia/Zamowienia";
import { Korespondencja } from "./korespondencja/Korespondencja";
import { Slowniki } from "./slowniki/Slowniki";
import { Ustawienia } from "./ustawienia/Ustawienia";

export interface Modul {
  /** Ścieżka trasy (bez ukośnika na końcu). */
  sciezka: string;
  nazwa: string;
  komponent: ComponentType;
  /** Czy moduł wymaga połączenia z serwerem. */
  wymagaPolaczenia: boolean;
}

export const MODULY: Modul[] = [
  { sciezka: "/", nazwa: "Pulpit", komponent: Pulpit, wymagaPolaczenia: false },
  { sciezka: "/klienci", nazwa: "Klienci", komponent: ListaKlientow, wymagaPolaczenia: true },
  { sciezka: "/oferty", nazwa: "Oferty", komponent: Oferty, wymagaPolaczenia: true },
  { sciezka: "/umowy", nazwa: "Umowy", komponent: Umowy, wymagaPolaczenia: true },
  { sciezka: "/kalendarz", nazwa: "Kalendarz", komponent: Kalendarz, wymagaPolaczenia: true },
  { sciezka: "/zamowienia", nazwa: "Zamówienia", komponent: Zamowienia, wymagaPolaczenia: true },
  { sciezka: "/korespondencja", nazwa: "Korespondencja", komponent: Korespondencja, wymagaPolaczenia: true },
  { sciezka: "/slowniki", nazwa: "Słowniki", komponent: Slowniki, wymagaPolaczenia: true },
  { sciezka: "/ustawienia", nazwa: "Ustawienia", komponent: Ustawienia, wymagaPolaczenia: false },
];
