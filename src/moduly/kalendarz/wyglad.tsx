// Wspólny wygląd zadań kalendarza: ikona i nazwa typu (kolor z tokenów --zadanie-*), etykieta stanu
// ostrzeżenia (ikona + tekst). Używane przez Kalendarz, Zamówienia, Pulpit i widżet.

import type { CSSProperties } from "react";
import {
  Banknote,
  CalendarDays,
  Check,
  CircleCheck,
  Clock3,
  Hammer,
  OctagonAlert,
  PencilRuler,
  Ruler,
  ShoppingCart,
  TriangleAlert,
  UserRound,
  Wrench,
  type LucideIcon,
} from "lucide-react";
import type { StanOstrzezenia } from "../../api/typy";
import { TYPY_ZADAN, type TypZadania } from "../../lib/kolory";
import { EtykietaStatusu, type Stan } from "../../ui";

export const IKONY_TYPOW: Record<TypZadania, LucideIcon> = {
  montaz: Wrench,
  warsztat: Hammer,
  zamowienie: ShoppingCart,
  rysunki: PencilRuler,
  pomiar: Ruler,
  platnosc: Banknote,
  klient: UserRound,
  inne: CalendarDays,
};

/** Kolejność typów w legendzie i filtrach (główne etapy, potem pomocnicze). */
export const TYPY_W_LEGENDZIE: TypZadania[] = ["pomiar", "rysunki", "zamowienie", "warsztat", "montaz", "platnosc", "klient", "inne"];

export function typZadania(typ: string): TypZadania {
  return (typ in TYPY_ZADAN ? typ : "inne") as TypZadania;
}

export function kolorTypu(typ: string): string {
  return TYPY_ZADAN[typZadania(typ)].kolor;
}

export function nazwaTypu(typ: string): string {
  return TYPY_ZADAN[typZadania(typ)].nazwa;
}

export function IkonaTypu({ typ, rozmiar = 14 }: { typ: string; rozmiar?: number }) {
  const I = IKONY_TYPOW[typZadania(typ)];
  return <I size={rozmiar} aria-hidden />;
}

/** Kolorowy znacznik z ikoną i nazwą typu (legenda, filtry). */
export function ZnacznikTypu({ typ, bezNazwy }: { typ: string; bezNazwy?: boolean }) {
  return (
    <span className="znacznik-typu" style={{ "--kolor": kolorTypu(typ) } as CSSProperties}>
      <span className="znacznik-typu__ikona">
        <IkonaTypu typ={typ} rozmiar={12} />
      </span>
      {bezNazwy ? <span className="sr-only">{nazwaTypu(typ)}</span> : nazwaTypu(typ)}
    </span>
  );
}

export const STANY: Record<StanOstrzezenia, { tekst: string; krotko: string; ikona: LucideIcon; ui: Stan; opis: string }> = {
  wykonane: { tekst: "Wykonane", krotko: "Wykonane", ikona: Check, ui: "neutralny", opis: "Zadanie oznaczone jako wykonane" },
  spokojnie: { tekst: "Spokojnie", krotko: "Spokojnie", ikona: CircleCheck, ui: "spokojnie", opis: "Termin jeszcze nie jest zagrożony" },
  zolty: { tekst: "Żółty – uwaga", krotko: "Uwaga", ikona: Clock3, ui: "zolty", opis: "Minęła data planowa albo termin się zbliża" },
  pomaranczowy: { tekst: "Pomarańczowy – pilne", krotko: "Pilne", ikona: TriangleAlert, ui: "pomaranczowy", opis: "Zbliżamy się do daty krytycznej" },
  czerwony: { tekst: "Czerwony – krytyczne", krotko: "Krytyczne", ikona: OctagonAlert, ui: "czerwony", opis: "Po terminie krytycznym – zagrożony termin montażu" },
};

export function EtykietaStanu({ stan, krotko, title }: { stan: StanOstrzezenia; krotko?: boolean; title?: string }) {
  const s = STANY[stan] ?? STANY.spokojnie;
  return <EtykietaStatusu stan={s.ui} ikona={s.ikona} tekst={krotko ? s.krotko : s.tekst} title={title ?? s.opis} />;
}

/** Sama ikona stanu (z opisem dla czytników) – do pasków w siatce. */
export function IkonaStanu({ stan, rozmiar = 12 }: { stan: StanOstrzezenia; rozmiar?: number }) {
  const s = STANY[stan] ?? STANY.spokojnie;
  const I = s.ikona;
  return (
    <span className={`ikona-stanu ikona-stanu--${stan}`} title={s.tekst}>
      <I size={rozmiar} aria-hidden />
      <span className="sr-only">{s.tekst}</span>
    </span>
  );
}

export const NAZWY_ETAPOW: Record<string, string> = {
  oferta: "Oferta",
  umowa: "Umowa",
  pomiar: "Pomiar",
  rysunki: "Rysunki",
  zamowienia: "Zamówienia",
  warsztat: "Warsztat",
  montaz: "Montaż",
  odbior: "Odbiór",
  gwarancja: "Gwarancja",
  zakonczone: "Zakończone",
};
