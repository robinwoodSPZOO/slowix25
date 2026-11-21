// Statusy oferty: etykieta + ikona + stan kolorystyczny (zawsze razem, nie sam kolor).

import { CircleCheck, CircleX, FilePen, Hourglass, Send, type LucideIcon } from "lucide-react";
import type { StatusOferty } from "../../api/typy";
import type { Stan } from "../../ui";

export const STATUSY_OFERTY: Record<StatusOferty, { etykieta: string; ikona: LucideIcon; stan: Stan }> = {
  szkic: { etykieta: "Szkic", ikona: FilePen, stan: "neutralny" },
  wyslana: { etykieta: "Wysłana", ikona: Send, stan: "akcent" },
  zaakceptowana: { etykieta: "Zaakceptowana", ikona: CircleCheck, stan: "spokojnie" },
  odrzucona: { etykieta: "Odrzucona", ikona: CircleX, stan: "czerwony" },
  wygasla: { etykieta: "Wygasła", ikona: Hourglass, stan: "pomaranczowy" },
};

export const KOLEJNOSC_STATUSOW: StatusOferty[] = ["szkic", "wyslana", "zaakceptowana", "odrzucona", "wygasla"];
