import type { CSSProperties } from "react";
import { TYPY_ZADAN } from "../../lib/kolory";
import { Zaslepka } from "../Zaslepka";

export function Kalendarz() {
  return (
    <Zaslepka tytul="Kalendarz" faza="Kalendarz">
      <p>Terminy liczone wstecz od montażu: warsztat → materiały → zamówienia (sweet spot / planowa / krytyczna) → rysunki → pomiar.</p>
      <div className="legenda">
        {Object.entries(TYPY_ZADAN).map(([k, t]) => (
          <span key={k} style={{ "--kolor": t.kolor } as CSSProperties}>
            {t.nazwa}
          </span>
        ))}
      </div>
    </Zaslepka>
  );
}
