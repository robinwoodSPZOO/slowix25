import type { ReactNode } from "react";

/** Wspólna zaślepka modułu, który powstanie w kolejnej fazie. */
export function Zaslepka({ tytul, faza, children }: { tytul: string; faza: string; children: ReactNode }) {
  return (
    <div className="komunikat">
      <h1>{tytul}</h1>
      <p>
        <strong>Moduł w przygotowaniu ({faza}).</strong>
      </p>
      {children}
    </div>
  );
}
