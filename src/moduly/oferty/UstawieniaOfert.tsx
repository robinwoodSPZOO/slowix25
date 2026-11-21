// Dane wykonawcy do nagłówka ofert (z ustawień serwera, nie z kodu) i domyślne wartości nowej oferty.

import { useEffect, useState, type FormEvent } from "react";
import { useNavigate } from "react-router-dom";
import { ArrowLeft, CircleCheck } from "lucide-react";
import type { DaneFirmy } from "../../api/typy";
import { usePolaczenie } from "../../kontekst/Polaczenie";
import { EtykietaStatusu, Pole, Przycisk } from "../../ui";

export function UstawieniaOfert() {
  const { api } = usePolaczenie();
  const nawiguj = useNavigate();
  const [d, ustawD] = useState<DaneFirmy | null>(null);
  const [blad, ustawBlad] = useState<string | null>(null);
  const [zapisano, ustawZapisano] = useState(false);

  useEffect(() => {
    api?.firma.pobierz().then(ustawD, (e: Error) => ustawBlad(e.message));
  }, [api]);

  if (!api || !d) return <p>{blad ?? "Wczytywanie…"}</p>;

  const pole = (k: keyof DaneFirmy) => ({
    id: `f-${k}`,
    value: String(d[k]),
    onChange: (e: { target: { value: string } }) => {
      ustawZapisano(false);
      ustawD({ ...d, [k]: k === "oferta_waznosc_dni" ? Number(e.target.value) : e.target.value });
    },
  });

  const zapisz = async (e: FormEvent) => {
    e.preventDefault();
    try {
      ustawD(await api.firma.zapisz(d));
      ustawZapisano(true);
      ustawBlad(null);
    } catch (er) {
      ustawBlad((er as Error).message);
    }
  };

  return (
    <div className="ustawienia-ofert">
      <Przycisk wariant="tekstowy" ikona={ArrowLeft} onClick={() => nawiguj("/oferty")}>
        Oferty
      </Przycisk>
      <h1>Dane firmy w ofertach</h1>
      <p className="pole__pomoc">Te dane trafiają do nagłówka „Wykonawca” w PDF i DOCX. Są zapisane w bazie na serwerze.</p>
      <form className="karta ustawienia-ofert__formularz" onSubmit={zapisz}>
        <Pole etykieta="Nazwa firmy" id="f-firma_nazwa">
          <input {...pole("firma_nazwa")} />
        </Pole>
        <Pole etykieta="Adres (każda linia osobno)" id="f-firma_adres">
          <textarea rows={3} {...pole("firma_adres")} />
        </Pole>
        <div className="ustawienia-ofert__rzad">
          <Pole etykieta="NIP" id="f-firma_nip">
            <input {...pole("firma_nip")} />
          </Pole>
          <Pole etykieta="Telefon" id="f-firma_telefon">
            <input {...pole("firma_telefon")} />
          </Pole>
          <Pole etykieta="E-mail" id="f-firma_email">
            <input type="email" {...pole("firma_email")} />
          </Pole>
        </div>
        <div className="ustawienia-ofert__rzad">
          <Pole etykieta="Ważność cen nowej oferty (dni)" id="f-oferta_waznosc_dni">
            <input type="number" min={0} max={365} {...pole("oferta_waznosc_dni")} />
          </Pole>
          <Pole etykieta="Domyślny termin realizacji" id="f-oferta_termin_realizacji">
            <input {...pole("oferta_termin_realizacji")} />
          </Pole>
        </div>
        {blad && <p className="blad">{blad}</p>}
        <div className="ustawienia-ofert__przyciski">
          <Przycisk type="submit" wariant="glowny">
            Zapisz
          </Przycisk>
          {zapisano && <EtykietaStatusu stan="spokojnie" ikona={CircleCheck} tekst="Zapisano" />}
        </div>
      </form>
    </div>
  );
}
