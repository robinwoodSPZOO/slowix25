import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { Building2, FilePlus2, Search, TriangleAlert } from "lucide-react";
import type { Klient, OfertaNaLiscie, StatusOferty } from "../../api/typy";
import { usePolaczenie } from "../../kontekst/Polaczenie";
import { formatujDate, formatujKwote } from "../../lib/formaty";
import { EtykietaStatusu, Okno, Pole, Przycisk, Segment, WierszTabeli } from "../../ui";
import { KOLEJNOSC_STATUSOW, STATUSY_OFERTY } from "./statusy";

export function ListaOfert() {
  const { api, ostatnieZdarzenie } = usePolaczenie();
  const nawiguj = useNavigate();
  const [status, ustawStatus] = useState<StatusOferty | "">("");
  const [klientId, ustawKlienta] = useState<number | null>(null);
  const [szukaj, ustawSzukaj] = useState("");
  const [oferty, ustawOferty] = useState<OfertaNaLiscie[]>([]);
  const [klienci, ustawKlientow] = useState<Klient[]>([]);
  const [blad, ustawBlad] = useState<string | null>(null);
  const [nowa, ustawNowa] = useState(false);

  useEffect(() => {
    if (!api) return;
    api.klienci.lista().then(ustawKlientow, () => {});
  }, [api]);

  useEffect(() => {
    if (!api) return;
    const t = setTimeout(() => {
      api.oferty.lista({ status, klient_id: klientId, szukaj }).then(
        (o) => {
          ustawOferty(o);
          ustawBlad(null);
        },
        (e: Error) => ustawBlad(e.message),
      );
    }, 150);
    return () => clearTimeout(t);
  }, [api, status, klientId, szukaj, ostatnieZdarzenie]);

  return (
    <div className="lista-ofert">
      <div className="lista-ofert__naglowek">
        <h1>Oferty</h1>
        <div className="lista-ofert__akcje">
          <Przycisk wariant="tekstowy" ikona={Building2} onClick={() => nawiguj("/oferty/ustawienia")}>
            Dane firmy w ofertach
          </Przycisk>
          <Przycisk wariant="glowny" ikona={FilePlus2} onClick={() => ustawNowa(true)}>
            Nowa oferta
          </Przycisk>
        </div>
      </div>
      <div className="lista-ofert__filtry">
        <Segment<StatusOferty | "">
          etykieta="Filtr statusu"
          wartosc={status}
          onZmiana={ustawStatus}
          opcje={[{ wartosc: "", etykieta: "Wszystkie" }, ...KOLEJNOSC_STATUSOW.map((s) => ({ wartosc: s, etykieta: STATUSY_OFERTY[s].etykieta, ikona: STATUSY_OFERTY[s].ikona }))]}
        />
        <select className="wejscie" aria-label="Filtr klienta" value={klientId ?? ""} onChange={(e) => ustawKlienta(e.target.value ? Number(e.target.value) : null)}>
          <option value="">Wszyscy klienci</option>
          {klienci.map((k) => (
            <option key={k.id} value={k.id}>
              {k.nazwa}
            </option>
          ))}
        </select>
        <div className="lista-ofert__szukaj">
          <Search size={14} aria-hidden />
          <input className="wejscie" placeholder="Szukaj: numer, klient, adres" aria-label="Szukaj ofert" value={szukaj} onChange={(e) => ustawSzukaj(e.target.value)} />
        </div>
      </div>
      {blad && <p className="blad">{blad}</p>}
      <div className="karta">
        <table className="tabela">
          <thead>
            <tr>
              <th>Numer</th>
              <th>Inwestor</th>
              <th>Inwestycja</th>
              <th>Wersja</th>
              <th>Data</th>
              <th>Ważna do</th>
              <th>Status</th>
              <th className="kolumna-kwoty">Suma brutto</th>
            </tr>
          </thead>
          <tbody>
            {oferty.map((o) => {
              const st = STATUSY_OFERTY[o.status];
              return (
                <WierszTabeli key={o.id} onWybierz={() => nawiguj(`/oferty/${o.id}`)}>
                  <td>
                    <strong>{o.numer}</strong>
                  </td>
                  <td>{o.klient_nazwa}</td>
                  <td>{o.adres_inwestycji ?? "—"}</td>
                  <td>v{o.nr_wersji}</td>
                  <td>{formatujDate(o.data_oferty)}</td>
                  <td>
                    {formatujDate(o.data_waznosci_cen)}
                    {o.po_terminie_waznosci && (
                      <>
                        {" "}
                        <EtykietaStatusu stan="pomaranczowy" ikona={TriangleAlert} tekst="po terminie" />
                      </>
                    )}
                  </td>
                  <td>
                    <EtykietaStatusu stan={st.stan} ikona={st.ikona} tekst={st.etykieta} />
                  </td>
                  <td className="kolumna-kwoty">{formatujKwote(o.suma_brutto_gr)}</td>
                </WierszTabeli>
              );
            })}
            {oferty.length === 0 && !blad && (
              <tr>
                <td colSpan={8} className="tabela-wierszy__pusto">
                  Brak ofert{status || klientId || szukaj ? " dla wybranych filtrów" : ""}.
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
      {nowa && <NowaOferta klienci={klienci} onZamknij={() => ustawNowa(false)} onUtworzona={(id) => nawiguj(`/oferty/${id}`)} />}
    </div>
  );
}

function NowaOferta({ klienci, onZamknij, onUtworzona }: { klienci: Klient[]; onZamknij: () => void; onUtworzona: (id: number) => void }) {
  const { api } = usePolaczenie();
  const nawiguj = useNavigate();
  const [klientId, ustawKlienta] = useState<number | null>(klienci[0]?.id ?? null);
  const [adres, ustawAdres] = useState("");
  const [vat, ustawVat] = useState<8 | 23>(8);
  const [blad, ustawBlad] = useState<string | null>(null);

  const utworz = async () => {
    if (!api || !klientId) return;
    try {
      const p = await api.oferty.dodaj({ klient_id: klientId, adres_inwestycji: adres || null, stawka_vat: vat });
      onUtworzona(p.oferta.id);
    } catch (e) {
      ustawBlad((e as Error).message);
    }
  };

  return (
    <Okno
      tytul="Nowa oferta"
      onZamknij={onZamknij}
      stopka={
        <>
          <Przycisk onClick={onZamknij}>Anuluj</Przycisk>
          <Przycisk wariant="glowny" disabled={!klientId} onClick={() => void utworz()}>
            Utwórz ofertę
          </Przycisk>
        </>
      }
    >
      {klienci.length === 0 ? (
        <p>
          Najpierw dodaj klienta (inwestora) w bazie klientów.{" "}
          <Przycisk wariant="tekstowy" onClick={() => nawiguj("/klienci/nowy")}>
            Dodaj klienta
          </Przycisk>
        </p>
      ) : (
        <Pole etykieta="Inwestor (klient z bazy)" id="nowa-klient">
          <select id="nowa-klient" value={klientId ?? ""} onChange={(e) => ustawKlienta(Number(e.target.value))}>
            {klienci.map((k) => (
              <option key={k.id} value={k.id}>
                {k.nazwa}
                {k.adres_miasto ? ` – ${k.adres_miasto}` : ""}
              </option>
            ))}
          </select>
        </Pole>
      )}
      <Pole etykieta="Adres inwestycji" id="nowa-adres" pomoc="Miejsce montażu – można uzupełnić później">
        <input id="nowa-adres" value={adres} onChange={(e) => ustawAdres(e.target.value)} />
      </Pole>
      <div className="pole">
        <span className="pole__etykieta">Stawka VAT mebli</span>
        <Segment<8 | 23> etykieta="Stawka VAT" wartosc={vat} onZmiana={ustawVat} opcje={[{ wartosc: 8, etykieta: "8%" }, { wartosc: 23, etykieta: "23%" }]} />
      </div>
      {blad && <p className="blad">{blad}</p>}
    </Okno>
  );
}
