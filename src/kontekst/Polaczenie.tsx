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
  /** Stan gniazda zdarzeń (WebSocket): true – połączono, false – zerwane, null – jeszcze nie wiadomo. */
  naZywo: boolean | null;
  /**
   * Licznik ponownych połączeń gniazda (rośnie przy każdym przejściu zerwane → połączone).
   * W czasie przerwy zdarzenia przepadają, więc każdy widok po zmianie licznika pobiera dane od nowa
   * (`useOdswiezaniePoZdarzeniu` robi to sam).
   */
  wznowienia: number;
}

const Kontekst = createContext<WartoscKontekstu>({
  polaczenie: null,
  api: null,
  ostatnieZdarzenie: null,
  odswiez: () => {},
  naZywo: null,
  wznowienia: 0,
});

/** Do testów i podglądów: bezpośredni dostęp do kontekstu. */
export const KontekstPolaczenia = Kontekst;
export type { WartoscKontekstu };

export function DostawcaPolaczenia({ children }: { children: ReactNode }) {
  const [polaczenie, ustawPolaczenie] = useState<Polaczenie | null>(null);
  const [ostatnieZdarzenie, ustawZdarzenie] = useState<Zdarzenie | null>(null);
  const [naZywo, ustawNaZywo] = useState<boolean | null>(null);
  const [wznowienia, ustawWznowienia] = useState(0);

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
    let bylaPrzerwa = false;
    return api.subskrybujZdarzenia(ustawZdarzenie, (polaczono) => {
      ustawNaZywo(polaczono);
      if (!polaczono) bylaPrzerwa = true;
      else if (bylaPrzerwa) {
        bylaPrzerwa = false;
        ustawWznowienia((n) => n + 1);
      }
    });
  }, [api]);

  return (
    <Kontekst.Provider value={{ polaczenie, api, ostatnieZdarzenie, odswiez, naZywo, wznowienia }}>
      {children}
    </Kontekst.Provider>
  );
}

export function usePolaczenie() {
  return useContext(Kontekst);
}
