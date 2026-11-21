// Szkic formularza: kopia w localStorage przy każdej zmianie (src/lib/szkice.ts), odtwarzana po
// ponownym otwarciu – zamknięcie okna w trakcie edycji nie gubi wpisanych wartości.

import { useState } from "react";
import { wczytajKopie, zapiszKopie } from "../../lib/szkice";

/**
 * `poczatkowa` – wartość z serwera. Gdy istnieje szkic, startujemy od niego (`przywrocono = true`).
 * `wyczysc()` po udanym zapisie albo „Odrzuć zmiany”.
 */
export function useSzkic<T extends object>(klucz: string, poczatkowa: T | null) {
  const [szkic, ustawSzkic] = useState<T | null>(() => wczytajKopie<T>(klucz));
  const [przywrocono, ustawPrzywrocono] = useState(() => wczytajKopie<T>(klucz) !== null);

  const zmien = (w: T) => {
    zapiszKopie(klucz, w);
    ustawSzkic(w);
  };
  const wyczysc = () => {
    zapiszKopie(klucz, null);
    ustawPrzywrocono(false);
    ustawSzkic(null);
  };
  // Bez szkicu pokazujemy aktualne dane z serwera (także po zmianie na innym stanowisku).
  return { wartosc: szkic ?? poczatkowa, zmien, wyczysc, przywrocono, zmieniono: szkic !== null };
}
