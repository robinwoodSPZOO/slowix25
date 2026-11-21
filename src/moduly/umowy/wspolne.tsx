// Wspólne elementy ekranów umowy i aneksu: lista różnic specyfikacji, okno dokumentu
// (podgląd pól przed generowaniem + DOCX/PDF), okno zmiany statusu.

import { useEffect, useState } from "react";
import { CircleAlert, CircleCheck, FileDown, FileText, Info, TriangleAlert } from "lucide-react";
import type { Roznica } from "../../api/typy";
import type { PodgladPol, StatusUmowy } from "../../api/typyUmow";
import { EtykietaStatusu, Okno, Pole, Przycisk } from "../../ui";
import { zapiszPlik } from "../oferty/pobieranie";
import { etykietaPrzejscia } from "./statusy";

const ZMIANY: Record<string, string> = { dodano: "Dodano", usunieto: "Usunięto", zmieniono: "Zmieniono", przeniesiono: "Przeniesiono" };

export function ListaRoznic({ roznice, zrodlo, pusta }: { roznice: Roznica[]; zrodlo?: string | null; pusta?: string }) {
  if (roznice.length === 0) return <p className="grupa-umowy__info">{pusta ?? "Brak różnic."}</p>;
  return (
    <div className="lista-roznic">
      {zrodlo && <p className="grupa-umowy__mikro">Względem: {zrodlo}</p>}
      <ul>
        {roznice.map((r, i) => (
          <li key={i} className={`lista-roznic__pozycja lista-roznic__pozycja--${r.zmiana}`}>
            <div>
              <strong>{ZMIANY[r.zmiana] ?? r.zmiana}</strong> <span>{r.sciezka}</span>
            </div>
            {r.pola.length > 0 && r.zmiana === "zmieniono" && (
              <dl>
                {r.pola.map((p) => (
                  <div key={p.pole}>
                    <dt>{p.pole}</dt>
                    <dd>
                      <span className="lista-roznic__przed">{p.przed ?? "—"}</span> → <span className="lista-roznic__po">{p.po ?? "—"}</span>
                    </dd>
                  </div>
                ))}
              </dl>
            )}
          </li>
        ))}
      </ul>
    </div>
  );
}

/** Podgląd pól szablonu z wartościami (puste i nieznane wyróżnione) + pobranie DOCX/PDF. */
export function OknoDokumentu({
  tytul,
  podglad,
  pobierz,
  onZamknij,
}: {
  tytul: string;
  podglad: () => Promise<PodgladPol>;
  pobierz: (format: "docx" | "pdf") => Promise<{ nazwa: string; plik: Blob }>;
  onZamknij: () => void;
}) {
  const [p, ustawP] = useState<PodgladPol | null>(null);
  const [blad, ustawBlad] = useState<string | null>(null);
  const [trwa, ustawTrwa] = useState<"docx" | "pdf" | null>(null);
  // Jedno sprawdzenie szablonu na otwarcie okna (funkcja z rodzica może zmieniać się co render).
  const [zadanie] = useState(() => podglad());
  useEffect(() => {
    zadanie.then(ustawP, (e: Error) => ustawBlad(e.message));
  }, [zadanie]);
  const pobierzPlik = async (f: "docx" | "pdf") => {
    ustawTrwa(f);
    try {
      const { nazwa, plik } = await pobierz(f);
      zapiszPlik(plik, nazwa);
      ustawBlad(null);
    } catch (e) {
      ustawBlad((e as Error).message);
    } finally {
      ustawTrwa(null);
    }
  };
  const puste = p?.pola.filter((x) => x.pusta) ?? [];
  return (
    <Okno
      tytul={tytul}
      szerokie
      onZamknij={onZamknij}
      stopka={
        <>
          <Przycisk onClick={onZamknij}>Zamknij</Przycisk>
          <Przycisk ikona={FileDown} disabled={trwa !== null || !p} onClick={() => void pobierzPlik("pdf")} title="PDF powstaje z dokumentu Word na komputerze-serwerze (Word albo LibreOffice)">
            {trwa === "pdf" ? "Tworzenie PDF…" : "PDF"}
          </Przycisk>
          <Przycisk wariant="glowny" ikona={FileText} disabled={trwa !== null || !p} onClick={() => void pobierzPlik("docx")}>
            {trwa === "docx" ? "Tworzenie…" : "Pobierz DOCX"}
          </Przycisk>
        </>
      }
    >
      {!p && !blad && <p>Sprawdzanie szablonu…</p>}
      {p && (
        <div className="okno-dokumentu">
          {p.zrodlo === "przykladowy" ? (
            <p className="komunikat-stanu status--zolty">
              <Info size={14} aria-hidden /> Brak własnego szablonu – dokument powstanie z wbudowanego szablonu przykładowego (z napisem „SZABLON PRZYKŁADOWY”). Wgraj swój szablon w Umowy → Ustawienia.
            </p>
          ) : (
            <p className="grupa-umowy__info">Szablon: {p.nazwa_szablonu ?? "własny"}</p>
          )}
          {puste.length > 0 ? (
            <p className="komunikat-stanu status--pomaranczowy">
              <TriangleAlert size={14} aria-hidden /> Pola bez wartości ({puste.length}) – w dokumencie pojawi się „………” do uzupełnienia ręcznie: {puste.map((x) => x.pole).join(", ")}.
            </p>
          ) : (
            <EtykietaStatusu stan="spokojnie" ikona={CircleCheck} tekst="Wszystkie pola szablonu mają wartości" />
          )}
          {p.raport.nieznane.length > 0 && (
            <p className="komunikat-stanu status--zolty">
              <CircleAlert size={14} aria-hidden /> Pola nieznane programowi (zostaną bez zmian): {p.raport.nieznane.map((n) => `[${n}]`).join(", ")}
            </p>
          )}
          {p.raport.do_uzupelnienia > 0 && <p className="grupa-umowy__mikro">Miejsca „[uzupełnij]” do wpisania ręcznie: {p.raport.do_uzupelnienia}.</p>}
          {p.raport.ostrzezenia.map((o) => (
            <p key={o} className="grupa-umowy__mikro">
              {o}
            </p>
          ))}
          <div className="okno-dokumentu__pola">
            <table className="tabela tabela--zwarta">
              <thead>
                <tr>
                  <th>Pole</th>
                  <th>Wartość w dokumencie</th>
                </tr>
              </thead>
              <tbody>
                {p.pola.map((x) => (
                  <tr key={x.pole} className={x.pusta ? "wiersz-pusty" : undefined}>
                    <td>[{x.pole}]</td>
                    <td>{x.pusta ? "……… (puste)" : x.wartosc}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}
      {blad && <p className="blad">{blad}</p>}
    </Okno>
  );
}

/** Zmiana statusu z datą (podpisanie, odbiór) albo powodem (anulowanie). */
export function OknoStatusu({
  z,
  na,
  dataDomyslna,
  onZamknij,
  onZatwierdz,
}: {
  z: StatusUmowy;
  na: StatusUmowy;
  dataDomyslna: string | null;
  onZamknij: () => void;
  onZatwierdz: (data: string | null, powod: string | null) => void;
}) {
  const [data, ustawData] = useState(dataDomyslna ?? new Date().toISOString().slice(0, 10));
  const [powod, ustawPowod] = useState("");
  const opis: Partial<Record<StatusUmowy, string>> = {
    wyslana: "Dane umowy, specyfikacja i dane stron zostaną zamrożone; plan płatności trafi do kalendarza. Zmiany przed podpisaniem – „Cofnij do szkicu”.",
    podpisana: "Powstanie zlecenie w kalendarzu (termin montażu, czas warsztatu i montażu), zamówienia materiałów z kategorii specyfikacji i harmonogram. Dalsze zmiany – tylko aneksem.",
    anulowana: "Umowa zostanie zablokowana, a niewykonane zadania jej zlecenia znikną z kalendarza (wykonane zostają jako historia).",
    szkic: "Umowa wróci do edycji (zamrożone dane stron zostaną odświeżone przy ponownym wysłaniu).",
    zamknieta: "Umowa zostanie zamknięta, zlecenie przejdzie do etapu „zakończone”.",
  };
  return (
    <Okno
      tytul={etykietaPrzejscia(z, na)}
      onZamknij={onZamknij}
      stopka={
        <>
          <Przycisk onClick={onZamknij}>Anuluj</Przycisk>
          <Przycisk wariant={na === "anulowana" ? "niebezpieczny" : "glowny"} onClick={() => onZatwierdz(na === "podpisana" || na === "odebrana" ? data : null, na === "anulowana" ? powod.trim() || null : null)}>
            {etykietaPrzejscia(z, na)}
          </Przycisk>
        </>
      }
    >
      {opis[na] && <p>{opis[na]}</p>}
      {(na === "podpisana" || na === "odebrana") && (
        <Pole etykieta={na === "podpisana" ? "Data podpisania" : "Data odbioru"} id="st-data">
          <input id="st-data" type="date" value={data} onChange={(e) => ustawData(e.target.value)} />
        </Pole>
      )}
      {na === "anulowana" && (
        <Pole etykieta="Powód anulowania" id="st-powod">
          <input id="st-powod" value={powod} onChange={(e) => ustawPowod(e.target.value)} />
        </Pole>
      )}
    </Okno>
  );
}
