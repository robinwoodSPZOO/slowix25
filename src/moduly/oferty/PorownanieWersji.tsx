// Porównanie dwóch wersji oferty: lista zmian (nagłówek, wycena, sekcje, części, wiersze).

import { useEffect, useState } from "react";
import { CircleMinus, CirclePlus, PencilLine } from "lucide-react";
import type { Porownanie, WersjaKrotko } from "../../api/typy";
import { usePolaczenie } from "../../kontekst/Polaczenie";
import { EtykietaStatusu, Okno, Przycisk } from "../../ui";

const RODZAJ = {
  dodano: { ikona: CirclePlus, tekst: "Dodano", stan: "spokojnie" as const },
  usunieto: { ikona: CircleMinus, tekst: "Usunięto", stan: "czerwony" as const },
  zmieniono: { ikona: PencilLine, tekst: "Zmieniono", stan: "zolty" as const },
};

const POZIOM = { naglowek: "Nagłówek", wycena: "Wycena", sekcja: "Sekcja", czesc: "Część", wiersz: "Wiersz" };

export function PorownanieWersji({ ofertaId, wersje, biezaca, onZamknij }: { ofertaId: number; wersje: WersjaKrotko[]; biezaca: number; onZamknij: () => void }) {
  const { api } = usePolaczenie();
  const i = wersje.findIndex((w) => w.id === biezaca);
  const [z, ustawZ] = useState(wersje[Math.max(0, i - 1)]?.id ?? biezaca);
  const [doW, ustawDo] = useState(i > 0 ? biezaca : (wersje[1]?.id ?? biezaca));
  const [wynik, ustawWynik] = useState<Porownanie | null>(null);
  const [blad, ustawBlad] = useState<string | null>(null);

  useEffect(() => {
    if (!api || z === doW) return;
    api.oferty.porownaj(ofertaId, z, doW).then(
      (p) => {
        ustawWynik(p);
        ustawBlad(null);
      },
      (e: Error) => ustawBlad(e.message),
    );
  }, [api, ofertaId, z, doW]);

  const wybor = (wartosc: number, ustaw: (v: number) => void, etykieta: string) => (
    <label className="porownanie__wybor">
      <span className="pole__etykieta">{etykieta}</span>
      <select className="wejscie" value={wartosc} onChange={(e) => ustaw(Number(e.target.value))}>
        {wersje.map((w) => (
          <option key={w.id} value={w.id}>
            v{w.nr_wersji}
          </option>
        ))}
      </select>
    </label>
  );

  return (
    <Okno tytul="Porównanie wersji" szerokie onZamknij={onZamknij} stopka={<Przycisk onClick={onZamknij}>Zamknij</Przycisk>}>
      <div className="porownanie__wybory">
        {wybor(z, ustawZ, "Z wersji")}
        {wybor(doW, ustawDo, "Do wersji")}
      </div>
      {z === doW && <p className="pole__pomoc">Wybierz dwie różne wersje.</p>}
      {blad && <p className="blad">{blad}</p>}
      {wynik && z !== doW && (
        <>
          {wynik.roznice.length === 0 && <p>Wersje v{wynik.z_wersji} i v{wynik.do_wersji} są takie same.</p>}
          <ul className="porownanie__lista">
            {wynik.roznice.map((r, k) => {
              const rz = RODZAJ[r.zmiana];
              return (
                <li key={k} className="porownanie__pozycja">
                  <div className="porownanie__naglowek">
                    <EtykietaStatusu stan={rz.stan} ikona={rz.ikona} tekst={rz.tekst} />
                    <span className="porownanie__poziom">{POZIOM[r.poziom]}</span>
                    <strong>{r.sciezka}</strong>
                  </div>
                  {r.pola.length > 0 && (
                    <table className="tabela porownanie__pola">
                      <tbody>
                        {r.pola.map((p) => (
                          <tr key={p.pole}>
                            <th scope="row">{p.pole}</th>
                            <td className="porownanie__przed">{p.przed ?? "—"}</td>
                            <td className="porownanie__po">{p.po ?? "—"}</td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  )}
                </li>
              );
            })}
          </ul>
        </>
      )}
    </Okno>
  );
}
