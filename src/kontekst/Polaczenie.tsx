// Kontekst połączenia z serwerem: konfiguracja stanowiska + gotowy KlientApi.

import { createContext, useCallback, useContext, useEffect, useMemo, useState, type ReactNode } from "react";
import { KlientApi } from "../api/klient";
import type { Zdarzenie } from "../api/typy";
import { pobierzPolaczenie, type Polaczenie } from "../lib/tauri";

interface WartoscKontekstu {
  polaczenie: Polaczenie | null;
  api: KlientApi | null;
  /** Ostatnie zdarzenie o zmianie danych (do odświeżania list). */
  ostatnieZdarzenie: Zdarzenie | null;
  odswiez: () => void;
}

const Kontekst = createContext<WartoscKontekstu>({
  polaczenie: null,
  api: null,
  ostatnieZdarzenie: null,
  odswiez: () => {},
});

export function DostawcaPolaczenia({ children }: { children: ReactNode }) {
  const [polaczenie, ustawPolaczenie] = useState<Polaczenie | null>(null);
  const [ostatnieZdarzenie, ustawZdarzenie] = useState<Zdarzenie | null>(null);

  const odswiez = useCallback(() => {
    pobierzPolaczenie().then(ustawPolaczenie, (e: unknown) => console.error(e));
  }, []);

  useEffect(odswiez, [odswiez]);

  const api = useMemo(() => {
    if (!polaczenie?.adres_api || !polaczenie.klucz_api) return null;
    return new KlientApi(polaczenie.adres_api, polaczenie.klucz_api);
  }, [polaczenie]);

  useEffect(() => {
    if (!api) return;
    return api.subskrybujZdarzenia(ustawZdarzenie);
  }, [api]);

  return (
    <Kontekst.Provider value={{ polaczenie, api, ostatnieZdarzenie, odswiez }}>
      {children}
    </Kontekst.Provider>
  );
}

export function usePolaczenie() {
  return useContext(Kontekst);
}
