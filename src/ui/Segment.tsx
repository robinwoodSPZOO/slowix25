import type { KeyboardEvent } from "react";
import type { LucideIcon } from "lucide-react";

export interface OpcjaSegmentu<T extends string | number> {
  wartosc: T;
  etykieta: string;
  ikona?: LucideIcon;
}

export interface SegmentProps<T extends string | number> {
  opcje: OpcjaSegmentu<T>[];
  wartosc: T;
  onZmiana: (w: T) => void;
  /** Opis grupy dla czytników ekranu. */
  etykieta: string;
  wylaczony?: boolean;
}

/** Segment: wybór jednej z kilku opcji (radiogroup, strzałki ←/→). */
export function Segment<T extends string | number>({ opcje, wartosc, onZmiana, etykieta, wylaczony }: SegmentProps<T>) {
  const klawisz = (e: KeyboardEvent<HTMLDivElement>) => {
    const i = opcje.findIndex((o) => o.wartosc === wartosc);
    const krok = e.key === "ArrowRight" ? 1 : e.key === "ArrowLeft" ? -1 : 0;
    if (!krok || wylaczony) return;
    e.preventDefault();
    const nastepna = opcje[(i + krok + opcje.length) % opcje.length];
    if (nastepna) onZmiana(nastepna.wartosc);
  };
  return (
    <div className="segment" role="radiogroup" aria-label={etykieta} onKeyDown={klawisz}>
      {opcje.map((o) => {
        const Ikona = o.ikona;
        const wybrana = o.wartosc === wartosc;
        return (
          <button
            key={String(o.wartosc)}
            type="button"
            role="radio"
            aria-checked={wybrana}
            tabIndex={wybrana ? 0 : -1}
            className="segment__opcja"
            disabled={wylaczony}
            onClick={() => onZmiana(o.wartosc)}
          >
            {Ikona && <Ikona size={13} aria-hidden />}
            {o.etykieta}
          </button>
        );
      })}
    </div>
  );
}
