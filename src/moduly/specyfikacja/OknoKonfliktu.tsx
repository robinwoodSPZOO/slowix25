// Konflikt równoczesnej edycji (409): ktoś na innym stanowisku zmienił w międzyczasie te same
// pola. Użytkownik wybiera dla wszystkich pól: zachować swoją wartość czy przyjąć cudzą.

import { TriangleAlert } from "lucide-react";
import type { KonfliktPola } from "../../api/typy";
import { Okno, Przycisk } from "../../ui";

const pokaz = (v: unknown): string => {
  if (v === null || v === undefined || v === "") return "(puste)";
  if (typeof v === "number") return String(v).replace(".", ",");
  if (Array.isArray(v)) return v.join(" | ");
  return String(v);
};

export function OknoKonfliktu({
  tytul,
  konflikty,
  nazwaPola,
  formatuj,
  onMoja,
  onCudza,
}: {
  tytul: string;
  konflikty: KonfliktPola[];
  nazwaPola: (p: string) => string;
  /** Formatowanie wartości (np. kwoty w groszach). */
  formatuj?: (pole: string, v: unknown) => string;
  onMoja: () => void;
  onCudza: () => void;
}) {
  const f = (p: string, v: unknown) => (formatuj ? formatuj(p, v) : pokaz(v));
  return (
    <Okno
      tytul={tytul}
      onZamknij={onCudza}
      stopka={
        <>
          <Przycisk onClick={onCudza}>Przyjmij cudzą zmianę</Przycisk>
          <Przycisk wariant="glowny" onClick={onMoja}>
            Zachowaj moją
          </Przycisk>
        </>
      }
    >
      <p className="komunikat-stanu status--zolty">
        <TriangleAlert size={14} aria-hidden /> Ktoś na innym stanowisku zmienił w międzyczasie te same pola. Wybierz, którą wartość zachować.
      </p>
      <table className="tabela">
        <thead>
          <tr>
            <th>Pole</th>
            <th>Moja wartość</th>
            <th>Cudza (zapisana)</th>
          </tr>
        </thead>
        <tbody>
          {konflikty.map((k) => (
            <tr key={k.pole}>
              <th scope="row">{nazwaPola(k.pole)}</th>
              <td>{f(k.pole, k.twoja)}</td>
              <td>{f(k.pole, k.aktualna)}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </Okno>
  );
}
