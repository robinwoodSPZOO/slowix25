import { useEffect, useState } from "react";
import type { StatusSerwera } from "../../api/typy";
import { usePolaczenie } from "../../kontekst/Polaczenie";

export function Pulpit() {
  const { api, polaczenie } = usePolaczenie();
  const [status, ustawStatus] = useState<StatusSerwera | null>(null);
  const [blad, ustawBlad] = useState<string | null>(null);

  useEffect(() => {
    if (!api) return;
    api.status().then(ustawStatus, (e: Error) => ustawBlad(e.message));
  }, [api]);

  return (
    <>
      <h1>Pulpit</h1>
      {polaczenie?.konfiguracja.tryb === "nieskonfigurowany" && (
        <p>Pierwsze uruchomienie: wybierz tryb pracy tego komputera w module Ustawienia.</p>
      )}
      {status && (
        <p>
          Połączono z serwerem „{status.nazwa}” (wersja {status.wersja}, API v{status.wersja_api}).
        </p>
      )}
      {blad && <p className="blad">{blad}</p>}
      <p>
        <em>Tu pojawi się lista „do zrobienia w tym tygodniu” i zadania zagrożone (faza Kalendarz).</em>
      </p>
    </>
  );
}
