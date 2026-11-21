// Import pliku `kategorie-materialow.csv` (czysta logika, testy w importCsv.test.ts).
//
// Format: separator średnik, UTF-8 (z BOM albo bez), pierwszy wiersz = nagłówek z kolumnami
// kategoria; przykladowe_materialy; liczba_dokumentow; dostawca; czas_dostawy_min_dni; czas_dostawy_max_dni
// (kolejność dowolna, wielkość liter bez znaczenia; liczba_dokumentow jest pomijana – to tylko
// informacja, z ilu dokumentów pochodzi kategoria). Pola w cudzysłowie mogą zawierać średnik.
// Wiersze błędne trafiają do `pominiete` z powodem; serwer sprawdza je jeszcze raz.

import type { WierszImportuKategorii } from "../../api/typy";

export const KOLUMNY = ["kategoria", "przykladowe_materialy", "liczba_dokumentow", "dostawca", "czas_dostawy_min_dni", "czas_dostawy_max_dni"] as const;
const WYMAGANE = ["kategoria"] as const;

export interface WynikParsowania {
  wiersze: WierszImportuKategorii[];
  pominiete: { wiersz: number; powod: string }[];
  /** Błąd całego pliku (np. brak kolumny „kategoria”) – nic nie importujemy. */
  blad: string | null;
}

/** Dzieli tekst CSV na rekordy (z obsługą cudzysłowów, "" w środku i znaków nowej linii w polu). */
export function podzielCsv(tekst: string, separator = ";"): { pola: string[]; wiersz: number }[] {
  const wynik: { pola: string[]; wiersz: number }[] = [];
  let pola: string[] = [];
  let pole = "";
  let wCudz = false;
  let nrWiersza = 1;
  let poczatekRekordu = 1;
  const t = tekst.replace(/^﻿/, "");
  for (let i = 0; i < t.length; i++) {
    const c = t[i];
    if (wCudz) {
      if (c === '"') {
        if (t[i + 1] === '"') {
          pole += '"';
          i++;
        } else wCudz = false;
      } else {
        if (c === "\n") nrWiersza++;
        pole += c;
      }
      continue;
    }
    if (c === '"' && pole.trim() === "") {
      pole = "";
      wCudz = true;
    } else if (c === separator) {
      pola.push(pole);
      pole = "";
    } else if (c === "\n" || c === "\r") {
      if (c === "\r" && t[i + 1] === "\n") i++;
      pola.push(pole);
      wynik.push({ pola, wiersz: poczatekRekordu });
      pola = [];
      pole = "";
      nrWiersza++;
      poczatekRekordu = nrWiersza;
    } else pole += c;
  }
  if (pole !== "" || pola.length > 0) {
    pola.push(pole);
    wynik.push({ pola, wiersz: poczatekRekordu });
  }
  return wynik.filter((r) => r.pola.some((p) => p.trim() !== ""));
}

function liczbaDni(tekst: string | undefined): { wartosc: number | null; blad: string | null } {
  const t = (tekst ?? "").trim().replace(/\s/g, "");
  if (t === "") return { wartosc: null, blad: null };
  if (!/^\d+([.,]0+)?$/.test(t)) return { wartosc: null, blad: `„${tekst?.trim()}” nie jest liczbą dni` };
  return { wartosc: parseInt(t, 10), blad: null };
}

export function parsujKategorieCsv(tekst: string): WynikParsowania {
  const rekordy = podzielCsv(tekst);
  if (rekordy.length === 0) return { wiersze: [], pominiete: [], blad: "Plik jest pusty." };
  const [naglowek, ...dane] = rekordy;
  const nazwy = naglowek!.pola.map((p) => p.trim().toLowerCase().replace(/\s+/g, "_"));
  if (naglowek!.pola.length === 1 && naglowek!.pola[0]!.includes(",")) {
    return { wiersze: [], pominiete: [], blad: "Plik wygląda na rozdzielany przecinkami – oczekiwany separator to średnik (;)." };
  }
  for (const w of WYMAGANE) {
    if (!nazwy.includes(w)) return { wiersze: [], pominiete: [], blad: `Brak kolumny „${w}” w pierwszym wierszu pliku (nagłówku).` };
  }
  const idx = (k: string) => nazwy.indexOf(k);
  const pole = (r: string[], k: string) => {
    const i = idx(k);
    return i >= 0 ? (r[i] ?? "").trim() : "";
  };
  const wiersze: WierszImportuKategorii[] = [];
  const pominiete: { wiersz: number; powod: string }[] = [];
  for (const r of dane) {
    const kategoria = pole(r.pola, "kategoria");
    if (!kategoria) {
      pominiete.push({ wiersz: r.wiersz, powod: "brak nazwy kategorii" });
      continue;
    }
    const min = liczbaDni(pole(r.pola, "czas_dostawy_min_dni"));
    const max = liczbaDni(pole(r.pola, "czas_dostawy_max_dni"));
    const blad = min.blad ?? max.blad;
    if (blad) {
      pominiete.push({ wiersz: r.wiersz, powod: blad });
      continue;
    }
    if ((min.wartosc === null) !== (max.wartosc === null)) {
      pominiete.push({ wiersz: r.wiersz, powod: "podano tylko jeden z czasów dostawy (min albo max)" });
      continue;
    }
    if (min.wartosc !== null && max.wartosc !== null && min.wartosc > max.wartosc) {
      pominiete.push({ wiersz: r.wiersz, powod: `minimalny czas dostawy (${min.wartosc}) większy niż maksymalny (${max.wartosc})` });
      continue;
    }
    wiersze.push({
      wiersz: r.wiersz,
      kategoria,
      przykladowe_materialy: pole(r.pola, "przykladowe_materialy") || null,
      dostawca: pole(r.pola, "dostawca") || null,
      czas_dostawy_min_dni: min.wartosc,
      czas_dostawy_max_dni: max.wartosc,
    });
  }
  return { wiersze, pominiete, blad: null };
}
