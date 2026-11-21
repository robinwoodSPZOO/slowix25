// Statusy umowy i aneksu: etykieta + ikona + stan kolorystyczny (zawsze razem, nie sam kolor).

import { Ban, CircleCheck, FilePen, FileSignature, Hammer, PackageCheck, Send, type LucideIcon } from "lucide-react";
import type { StatusAneksu, StatusUmowy } from "../../api/typyUmow";
import type { Stan } from "../../ui";

export const STATUSY_UMOWY: Record<StatusUmowy, { etykieta: string; ikona: LucideIcon; stan: Stan }> = {
  szkic: { etykieta: "Szkic", ikona: FilePen, stan: "neutralny" },
  wyslana: { etykieta: "Wysłana do klienta", ikona: Send, stan: "akcent" },
  podpisana: { etykieta: "Podpisana", ikona: FileSignature, stan: "spokojnie" },
  w_realizacji: { etykieta: "W realizacji", ikona: Hammer, stan: "zolty" },
  odebrana: { etykieta: "Odebrana", ikona: PackageCheck, stan: "spokojnie" },
  zamknieta: { etykieta: "Zamknięta", ikona: CircleCheck, stan: "neutralny" },
  anulowana: { etykieta: "Anulowana", ikona: Ban, stan: "czerwony" },
};

export const KOLEJNOSC_STATUSOW_UMOWY: StatusUmowy[] = ["szkic", "wyslana", "podpisana", "w_realizacji", "odebrana", "zamknieta", "anulowana"];

export const STATUSY_ANEKSU: Record<StatusAneksu, { etykieta: string; ikona: LucideIcon; stan: Stan }> = {
  szkic: { etykieta: "Szkic", ikona: FilePen, stan: "neutralny" },
  wyslany: { etykieta: "Wysłany", ikona: Send, stan: "akcent" },
  podpisany: { etykieta: "Podpisany", ikona: FileSignature, stan: "spokojnie" },
};

/** Przejścia dozwolone przez serwer (`umowy::dozwolone_przejscie`). */
export function dozwolonePrzejscia(s: StatusUmowy): StatusUmowy[] {
  const mapa: Record<StatusUmowy, StatusUmowy[]> = {
    szkic: ["wyslana", "podpisana"],
    wyslana: ["podpisana", "szkic"],
    podpisana: ["w_realizacji", "odebrana"],
    w_realizacji: ["odebrana"],
    odebrana: ["zamknieta", "w_realizacji"],
    zamknieta: [],
    anulowana: [],
  };
  const w = [...mapa[s]];
  if (s !== "zamknieta" && s !== "anulowana") w.push("anulowana");
  return w;
}

/** Etykieta akcji przejścia (przycisk / pozycja menu). */
export function etykietaPrzejscia(z: StatusUmowy, na: StatusUmowy): string {
  if (na === "szkic") return "Cofnij do szkicu";
  if (na === "wyslana") return "Wyślij do klienta";
  if (na === "podpisana") return "Podpisana";
  if (na === "w_realizacji") return z === "odebrana" ? "Wróć do realizacji" : "Rozpocznij realizację";
  if (na === "odebrana") return "Odebrana (protokół)";
  if (na === "zamknieta") return "Zamknij umowę";
  return "Anuluj umowę";
}

export const ZALACZNIKI: { nr: string; nazwa: string; staly: boolean }[] = [
  { nr: "1", nazwa: "Specyfikacja zamówienia", staly: true },
  { nr: "2", nazwa: "Projekt / harmonogram etapów", staly: false },
  { nr: "3", nazwa: "Cennik montażu AGD", staly: false },
  { nr: "4", nazwa: "Standard wykonania i odbioru", staly: true },
  { nr: "5", nazwa: "Protokół odbioru", staly: true },
  { nr: "6", nazwa: "Zgoda na zdjęcia", staly: false },
];

export const RODZAJE_NIERUCHOMOSCI = [
  { wartosc: "lokal_mieszkalny", etykieta: "Lokal mieszkalny", limit: 150 },
  { wartosc: "dom_jednorodzinny", etykieta: "Budynek mieszkalny jednorodzinny", limit: 300 },
  { wartosc: "inny", etykieta: "Inny", limit: null },
] as const;

export const SPOSOBY_WPLATY = [
  { wartosc: "przelew", etykieta: "Przelew" },
  { wartosc: "gotowka", etykieta: "Gotówka" },
  { wartosc: "karta", etykieta: "Karta" },
  { wartosc: "inny", etykieta: "Inny" },
] as const;
