// Zapis pól formularza umowy/aneksu: zmiany częściowe (tylko zmienione pole + `poprzednie`),
// zapis przy opuszczeniu pola, szkice opróżniane przy zamykaniu okna (src/lib/szkice.ts),
// konflikt 409 → okno „moja / cudza”.

import { useCallback, useEffect, useRef, useState } from "react";
import { BladApi } from "../../api/klient";
import type { KonfliktPola } from "../../api/typy";
import { zarejestrujOproznianie } from "../../lib/szkice";

export interface Konflikt {
  konflikty: KonfliktPola[];
  ponow: (aktualne: Record<string, unknown>) => Promise<void>;
}

/**
 * `baza` – aktualne wartości z serwera; `wyslij` – wysyła `{pole: wartość, poprzednie}`.
 * Zwraca wartość pola (szkic albo baza), `ustaw` (bez zapisu), `zapisz` (zapis szkicu pola
 * albo podanej wartości) i stan konfliktu.
 */
export function useFormularz<T extends object>(
  baza: T,
  wyslij: (zmiana: Partial<T> & { poprzednie?: Partial<T> }) => Promise<void>,
  onBlad: (b: string | null) => void,
) {
  const [szkic, ustawSzkic] = useState<Partial<T>>({});
  const [konflikt, ustawKonflikt] = useState<Konflikt | null>(null);
  const [zapisywanie, ustawZapisywanie] = useState(false);
  const ref = useRef({ szkic, baza, wyslij });
  useEffect(() => {
    ref.current = { szkic, baza, wyslij };
  });

  const wartosc = <K extends keyof T>(k: K): T[K] => (k in szkic ? (szkic[k] as T[K]) : baza[k]);
  const ustaw = <K extends keyof T>(k: K, v: T[K]) => ustawSzkic((s) => ({ ...s, [k]: v }));

  const wyslijZmiane = useCallback(
    async (zmiana: Partial<T>) => {
      const { baza: b, wyslij: w } = ref.current;
      const klucze = Object.keys(zmiana) as (keyof T)[];
      const realne = klucze.filter((k) => JSON.stringify(zmiana[k] ?? null) !== JSON.stringify(b[k] ?? null));
      const wyczysc = () =>
        ustawSzkic((s) => {
          const n = { ...s };
          for (const k of klucze) delete n[k];
          return n;
        });
      if (realne.length === 0) {
        wyczysc();
        return;
      }
      const z = Object.fromEntries(realne.map((k) => [k, zmiana[k] ?? null])) as Partial<T>;
      const poprzednie = Object.fromEntries(realne.map((k) => [k, b[k] ?? null])) as Partial<T>;
      ustawZapisywanie(true);
      try {
        await w({ ...z, poprzednie });
        wyczysc();
        onBlad(null);
      } catch (e) {
        if (e instanceof BladApi && e.kod === 409) {
          ustawKonflikt({
            konflikty: e.konflikty,
            ponow: async (akt) => {
              await w({ ...z, poprzednie: akt as Partial<T> });
              wyczysc();
            },
          });
        } else {
          onBlad((e as Error).message);
        }
      } finally {
        ustawZapisywanie(false);
      }
    },
    [onBlad],
  );

  /** Zapisuje szkic pola (np. przy opuszczeniu) albo podaną wartość (select, przełącznik). */
  const zapisz = <K extends keyof T>(k: K, v?: T[K]) => {
    const wart = v !== undefined ? v : k in ref.current.szkic ? ref.current.szkic[k] : undefined;
    if (wart === undefined) return;
    if (v !== undefined) ustaw(k, v);
    void wyslijZmiane({ [k]: wart } as unknown as Partial<T>);
  };

  // Niezapisane pola trafiają na serwer przy ukryciu/zamknięciu okna.
  useEffect(
    () =>
      zarejestrujOproznianie(async () => {
        const s = ref.current.szkic;
        if (Object.keys(s).length > 0) await wyslijZmiane(s);
      }),
    [wyslijZmiane],
  );

  return { wartosc, ustaw, zapisz, szkic, konflikt, zamknijKonflikt: () => ustawKonflikt(null), zapisywanie, odrzucSzkic: () => ustawSzkic({}) };
}
