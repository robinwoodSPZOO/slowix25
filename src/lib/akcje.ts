// Jeden wspólny mechanizm blokady akcji na czas żądania (podwójne kliknięcie nie wysyła akcji
// dwa razy). Blokada jest SYNCHRONICZNA (ref), więc drugie kliknięcie przed ponownym
// renderowaniem też jest odrzucane.
//
// * `useAkcja()` – dla formularzy i hooków: `const { trwa, uruchom } = useAkcja();`
//   `onSubmit={() => uruchom(() => api.cos())}` – kolejne wywołania w trakcie zwracają `undefined`.
// * `<Przycisk onClick={() => api.cos()}>` – przycisk sam blokuje się (disabled + aria-busy), gdy
//   `onClick` zwróci obietnicę (Promise). Nie pisz `void` przed wywołaniem – wtedy blokady nie ma.

import { useCallback, useRef, useState } from "react";

export interface Akcja {
  /** Czy akcja jest w toku (do `disabled` i napisów „Zapisywanie…”). */
  trwa: boolean;
  /** Uruchamia `f`, jeśli nic nie trwa; w przeciwnym razie zwraca `undefined` bez wywołania. */
  uruchom: <T>(f: () => Promise<T> | T) => Promise<T | undefined>;
  /**
   * Wywołuje `f` od razu (synchronicznie), jeśli nic nie trwa. Blokuje tylko wtedy, gdy `f`
   * zwróci obietnicę – do jej zakończenia. Używa tego `Przycisk`.
   */
  uruchomJesliObietnica: (f: () => unknown) => void;
}

export function useAkcja(): Akcja {
  const blokada = useRef(false);
  const [trwa, ustawTrwa] = useState(false);
  const uruchom = useCallback(async <T,>(f: () => Promise<T> | T): Promise<T | undefined> => {
    if (blokada.current) return undefined;
    blokada.current = true;
    ustawTrwa(true);
    try {
      return await f();
    } finally {
      blokada.current = false;
      ustawTrwa(false);
    }
  }, []);
  const uruchomJesliObietnica = useCallback((f: () => unknown) => {
    if (blokada.current) return;
    const w = f();
    if (!czyObietnica(w)) return;
    blokada.current = true;
    ustawTrwa(true);
    const koniec = () => {
      blokada.current = false;
      ustawTrwa(false);
    };
    w.then(koniec, koniec);
  }, []);
  return { trwa, uruchom, uruchomJesliObietnica };
}

export function czyObietnica(x: unknown): x is PromiseLike<unknown> {
  return !!x && typeof (x as { then?: unknown }).then === "function";
}
