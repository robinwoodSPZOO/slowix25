import { useState } from "react";
import type { DaneWersji, WersjaOferty } from "../../api/typy";
import { formatujKwote, parsujKwote } from "../../lib/formaty";

/** Dane do PUT /wersje-oferty/{id} z bieżącej wersji. */
export function daneWersji(w: WersjaOferty): DaneWersji {
  return {
    data_oferty: w.data_oferty,
    termin_realizacji: w.termin_realizacji,
    data_waznosci_cen: w.data_waznosci_cen,
    adres_inwestycji: w.adres_inwestycji,
    poziom_wyceny: w.poziom_wyceny,
    cena_calosc_gr: w.cena_calosc_gr,
    rabat_termin_gr: w.rabat_termin_gr,
    rabat_calosc_bp: w.rabat_calosc_bp,
    stawka_vat: w.stawka_vat,
    tryb_cen: w.tryb_cen,
    koszt_wlasny_gr: w.koszt_wlasny_gr,
    klauzule: w.klauzule,
    uwagi: w.uwagi,
  };
}

const bezZl = (gr: number | null) => (gr === null ? "" : formatujKwote(gr).replace(/\s?zł$/, ""));

/** Pole kwoty (zł, przecinek); zapis przy opuszczeniu pola lub Enter. Pusty tekst = brak kwoty. */
export function PoleKwoty({
  id,
  wartosc,
  onZapisz,
  wylaczone,
  etykieta,
  wymagane,
}: {
  id?: string;
  wartosc: number | null;
  onZapisz: (gr: number | null) => void;
  wylaczone?: boolean;
  etykieta?: string;
  wymagane?: boolean;
}) {
  const [tekst, ustawTekst] = useState<string | null>(null);
  const pokazany = tekst ?? bezZl(wartosc);
  const zatwierdz = () => {
    if (tekst === null) return;
    const t = tekst.trim();
    const gr = t ? parsujKwote(t) : wymagane ? 0 : null;
    if (t && gr === null) return;
    ustawTekst(null);
    if (gr !== wartosc) onZapisz(gr);
  };
  const bledna = tekst !== null && tekst.trim() !== "" && parsujKwote(tekst) === null;
  return (
    <span className="pole-kwoty">
      <input
        id={id}
        className="wejscie wejscie--kwota"
        inputMode="decimal"
        aria-label={etykieta}
        aria-invalid={bledna}
        disabled={wylaczone}
        value={pokazany}
        placeholder="—"
        onChange={(e) => ustawTekst(e.target.value)}
        onBlur={zatwierdz}
        onKeyDown={(e) => {
          if (e.key === "Enter") zatwierdz();
          if (e.key === "Escape") ustawTekst(null);
        }}
      />
      <span className="pole-kwoty__waluta">zł</span>
    </span>
  );
}
