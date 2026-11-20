import type { CSSProperties } from "react";
import { TYPY_ZADAN } from "../lib/kolory";
import { usePolaczenie } from "../kontekst/Polaczenie";
import { otworzGlowneOkno } from "../lib/tauri";

/** Okienko na pulpicie: dziś i najbliższe 7 dni (dane – faza Kalendarz). */
export function Widzet() {
  const { api } = usePolaczenie();
  const dzis = new Date().toLocaleDateString("pl-PL", { weekday: "long", day: "numeric", month: "long" });
  return (
    <div className="widzet">
      <strong>ROBINWOOD</strong>
      <p>{dzis}</p>
      <p>{api ? "Najbliższe 7 dni: brak zadań (kalendarz w przygotowaniu)." : "Brak połączenia z serwerem."}</p>
      <div className="legenda">
        {(["montaz", "warsztat", "zamowienie", "rysunki", "pomiar"] as const).map((k) => (
          <span key={k} style={{ "--kolor": TYPY_ZADAN[k].kolor } as CSSProperties}>
            {TYPY_ZADAN[k].nazwa}
          </span>
        ))}
      </div>
      <p>
        <button onClick={() => otworzGlowneOkno()}>Otwórz program</button>
      </p>
    </div>
  );
}
