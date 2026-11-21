import { useEffect, type ReactNode } from "react";
import { X } from "lucide-react";
import { Przycisk } from "./Przycisk";

/** Proste okno dialogowe (Esc zamyka). */
export function Okno({ tytul, onZamknij, stopka, szerokie, children }: { tytul: string; onZamknij: () => void; stopka?: ReactNode; szerokie?: boolean; children: ReactNode }) {
  useEffect(() => {
    const k = (e: globalThis.KeyboardEvent) => {
      if (e.key === "Escape") onZamknij();
    };
    window.addEventListener("keydown", k);
    return () => window.removeEventListener("keydown", k);
  }, [onZamknij]);
  return (
    <div className="okno-tlo" onMouseDown={(e) => e.target === e.currentTarget && onZamknij()}>
      <div className={`okno${szerokie ? " okno--szerokie" : ""}`} role="dialog" aria-modal="true" aria-label={tytul}>
        <div className="okno__naglowek">
          <h2>{tytul}</h2>
          <Przycisk wariant="tekstowy" tylkoIkona ikona={X} etykieta="Zamknij" onClick={onZamknij} />
        </div>
        <div className="okno__tresc">{children}</div>
        {stopka && <div className="okno__stopka">{stopka}</div>}
      </div>
    </div>
  );
}
