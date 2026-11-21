// Pola formularza z zapisem przy opuszczeniu (kwoty i liczby z walidacją po stronie interfejsu).

import { useState, type ReactNode } from "react";
import { bladKwoty, formatujKwote, parsujKwote } from "../../lib/formaty";
import { Pole } from "../../ui";

interface Formularz<T> {
  wartosc: <K extends keyof T>(k: K) => T[K];
  ustaw: <K extends keyof T>(k: K, v: T[K]) => void;
  zapisz: <K extends keyof T>(k: K, v?: T[K]) => void;
}

export function PoleTekstu<T>({
  f,
  k,
  etykieta,
  typ = "text",
  pomoc,
  blokada,
  wiele,
  className,
}: {
  f: Formularz<T>;
  k: keyof T;
  etykieta: string;
  typ?: "text" | "date" | "email";
  pomoc?: string;
  blokada?: boolean;
  wiele?: boolean;
  className?: string;
}) {
  const id = `u-${String(k)}`;
  const v = (f.wartosc(k) as string | null) ?? "";
  const wspolne = {
    id,
    value: v,
    disabled: blokada,
    onChange: (e: { target: { value: string } }) => f.ustaw(k, (e.target.value === "" ? null : e.target.value) as T[keyof T]),
    onBlur: () => f.zapisz(k),
  };
  return (
    <Pole etykieta={etykieta} id={id} pomoc={pomoc} className={className}>
      {wiele ? <textarea rows={2} {...wspolne} /> : <input type={typ} {...wspolne} />}
    </Pole>
  );
}

/** Kwota w złotych (zapis w groszach). */
export function PoleKwoty<T>({
  f,
  k,
  etykieta,
  pomoc,
  blokada,
  podpowiedz,
}: {
  f: Formularz<T>;
  k: keyof T;
  etykieta: string;
  pomoc?: string;
  blokada?: boolean;
  podpowiedz?: ReactNode;
}) {
  const id = `u-${String(k)}`;
  const gr = f.wartosc(k) as number | null;
  const [tekst, ustawTekst] = useState<string | null>(null);
  const pokazywany = tekst ?? (gr === null || gr === undefined ? "" : formatujKwote(gr).replace(/\s?zł$/, ""));
  const blad = tekst !== null ? bladKwoty(tekst) : null;
  return (
    <Pole etykieta={etykieta} id={id} pomoc={podpowiedz ? undefined : pomoc} blad={blad}>
      <span className="pole-zl">
        <input
          id={id}
          inputMode="decimal"
          value={pokazywany}
          disabled={blokada}
          aria-invalid={blad ? true : undefined}
          onFocus={() => ustawTekst(pokazywany)}
          onChange={(e) => ustawTekst(e.target.value)}
          onBlur={() => {
            if (tekst === null) return;
            if (bladKwoty(tekst)) return;
            const n = tekst.trim() === "" ? null : parsujKwote(tekst);
            ustawTekst(null);
            if (n !== null && n < 0) return;
            f.zapisz(k, n as T[keyof T]);
          }}
        />
        <span aria-hidden>zł</span>
      </span>
      {podpowiedz}
    </Pole>
  );
}

/** Liczba (dni, m²) z własnym parserem; `null` = puste, `undefined` = niepoprawne. */
export function PoleLiczby<T>({
  f,
  k,
  etykieta,
  parsuj,
  jednostka,
  pomoc,
  blokada,
}: {
  f: Formularz<T>;
  k: keyof T;
  etykieta: string;
  parsuj: (t: string) => number | null | undefined;
  jednostka?: string;
  pomoc?: string;
  blokada?: boolean;
}) {
  const id = `u-${String(k)}`;
  const n = f.wartosc(k) as number | null;
  const [tekst, ustawTekst] = useState<string | null>(null);
  const pokazywany = tekst ?? (n === null || n === undefined ? "" : String(n).replace(".", ","));
  const blad = tekst !== null && parsuj(tekst) === undefined ? "Nieprawidłowa liczba" : null;
  return (
    <Pole etykieta={etykieta} id={id} pomoc={pomoc} blad={blad}>
      <span className="pole-zl">
        <input
          id={id}
          inputMode="decimal"
          value={pokazywany}
          disabled={blokada}
          onFocus={() => ustawTekst(pokazywany)}
          onChange={(e) => ustawTekst(e.target.value)}
          onBlur={() => {
            if (tekst === null) return;
            const v = parsuj(tekst);
            if (v === undefined) return;
            ustawTekst(null);
            f.zapisz(k, v as T[keyof T]);
          }}
        />
        {jednostka && <span aria-hidden>{jednostka}</span>}
      </span>
    </Pole>
  );
}
