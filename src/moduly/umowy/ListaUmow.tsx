// Lista umów z filtrami (status, klient, rok) i wyszukiwaniem; nowa umowa z oferty albo od zera.

import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { FilePlus2, Search, Settings, TriangleAlert } from "lucide-react";
import type { Klient, OfertaNaLiscie } from "../../api/typy";
import type { StatusUmowy, UmowaNaLiscie } from "../../api/typyUmow";
import { usePolaczenie } from "../../kontekst/Polaczenie";
import { formatujDate, formatujKwote } from "../../lib/formaty";
import { EtykietaStatusu, Okno, Pole, Przycisk, Segment, WierszTabeli } from "../../ui";
import { lataDoFiltra } from "./logika";
import { KOLEJNOSC_STATUSOW_UMOWY, STATUSY_UMOWY } from "./statusy";

export function ListaUmow() {
  const { api, ostatnieZdarzenie } = usePolaczenie();
  const nawiguj = useNavigate();
  const [status, ustawStatus] = useState<StatusUmowy | "">("");
  const [klientId, ustawKlienta] = useState<number | null>(null);
  const [rok, ustawRok] = useState<number | null>(null);
  const [szukaj, ustawSzukaj] = useState("");
  const [umowy, ustawUmowy] = useState<UmowaNaLiscie[]>([]);
  const [klienci, ustawKlientow] = useState<Klient[]>([]);
  const [blad, ustawBlad] = useState<string | null>(null);
  const [nowa, ustawNowa] = useState(false);

  useEffect(() => {
    api?.klienci.lista().then(ustawKlientow, () => {});
  }, [api]);

  useEffect(() => {
    if (!api) return;
    const t = setTimeout(() => {
      api.umowy.lista({ status, klient_id: klientId, rok, szukaj }).then(
        (u) => {
          ustawUmowy(u);
          ustawBlad(null);
        },
        (e: Error) => ustawBlad(e.message),
      );
    }, 150);
    return () => clearTimeout(t);
  }, [api, status, klientId, rok, szukaj, ostatnieZdarzenie]);

  const suma = umowy.filter((u) => u.status !== "anulowana").reduce((s, u) => s + u.saldo_gr, 0);

  return (
    <div className="lista-ofert">
      <div className="naglowek-modulu">
        <h1>Umowy</h1>
        <div className="naglowek-modulu__akcje">
          <Przycisk wariant="tekstowy" ikona={Settings} onClick={() => nawiguj("/umowy/ustawienia")}>
            Ustawienia i szablony
          </Przycisk>
          <Przycisk wariant="glowny" ikona={FilePlus2} onClick={() => ustawNowa(true)}>
            Nowa umowa
          </Przycisk>
        </div>
      </div>
      <div className="lista-ofert__filtry">
        <select className="wejscie" aria-label="Filtr statusu" value={status} onChange={(e) => ustawStatus(e.target.value as StatusUmowy | "")}>
          <option value="">Wszystkie statusy</option>
          {KOLEJNOSC_STATUSOW_UMOWY.map((s) => (
            <option key={s} value={s}>
              {STATUSY_UMOWY[s].etykieta}
            </option>
          ))}
        </select>
        <select className="wejscie" aria-label="Filtr klienta" value={klientId ?? ""} onChange={(e) => ustawKlienta(e.target.value ? Number(e.target.value) : null)}>
          <option value="">Wszyscy klienci</option>
          {klienci.map((k) => (
            <option key={k.id} value={k.id}>
              {k.nazwa}
            </option>
          ))}
        </select>
        <select className="wejscie" aria-label="Filtr roku" value={rok ?? ""} onChange={(e) => ustawRok(e.target.value ? Number(e.target.value) : null)}>
          <option value="">Wszystkie lata</option>
          {lataDoFiltra().map((r) => (
            <option key={r} value={r}>
              {r}
            </option>
          ))}
        </select>
        <div className="lista-ofert__szukaj">
          <Search size={14} aria-hidden />
          <input className="wejscie" placeholder="Szukaj: numer, klient, oferta" aria-label="Szukaj umów" value={szukaj} onChange={(e) => ustawSzukaj(e.target.value)} />
        </div>
      </div>
      {blad && <p className="blad">{blad}</p>}
      <div className="karta">
        <table className="tabela">
          <thead>
            <tr>
              <th>Numer</th>
              <th>Klient</th>
              <th>Data</th>
              <th className="kolumna-kwoty">Wartość</th>
              <th className="kolumna-kwoty">Wpłacono</th>
              <th className="kolumna-kwoty">Saldo</th>
              <th>Termin montażu</th>
              <th>Status</th>
            </tr>
          </thead>
          <tbody>
            {umowy.map((u) => {
              const st = STATUSY_UMOWY[u.status];
              return (
                <WierszTabeli key={u.id} onWybierz={() => nawiguj(`/umowy/${u.id}`)}>
                  <td>
                    <strong>{u.numer}</strong>
                    {u.oferta_numer && <div className="grupa-umowy__mikro">z oferty {u.oferta_numer}</div>}
                  </td>
                  <td>{u.klient_nazwa ?? "—"}</td>
                  <td>{formatujDate(u.data_zawarcia)}</td>
                  <td className="kolumna-kwoty">{formatujKwote(u.wartosc_brutto_gr)}</td>
                  <td className="kolumna-kwoty">{formatujKwote(u.wplacono_gr)}</td>
                  <td className="kolumna-kwoty">{u.status === "anulowana" ? "—" : formatujKwote(u.saldo_gr)}</td>
                  <td>{formatujDate(u.montaz_do)}</td>
                  <td>
                    <EtykietaStatusu stan={st.stan} ikona={st.ikona} tekst={st.etykieta} />
                  </td>
                </WierszTabeli>
              );
            })}
            {umowy.length === 0 && !blad && (
              <tr>
                <td colSpan={8} className="tabela-wierszy__pusto">
                  Brak umów{status || klientId || rok || szukaj ? " dla wybranych filtrów" : ""}.
                </td>
              </tr>
            )}
          </tbody>
          {umowy.length > 0 && (
            <tfoot>
              <tr>
                <th colSpan={5}>Razem do zapłaty (bez anulowanych)</th>
                <th className="kolumna-kwoty">{formatujKwote(suma)}</th>
                <th colSpan={2} />
              </tr>
            </tfoot>
          )}
        </table>
      </div>
      {nowa && <NowaUmowa klienci={klienci} onZamknij={() => ustawNowa(false)} onUtworzona={(id) => nawiguj(`/umowy/${id}`)} />}
    </div>
  );
}

type Zrodlo = "oferta" | "zero";

function NowaUmowa({ klienci, onZamknij, onUtworzona }: { klienci: Klient[]; onZamknij: () => void; onUtworzona: (id: number) => void }) {
  const { api } = usePolaczenie();
  const [zrodlo, ustawZrodlo] = useState<Zrodlo>("oferta");
  const [oferty, ustawOferty] = useState<OfertaNaLiscie[]>([]);
  const [ofertaWersja, ustawOferte] = useState<number | null>(null);
  const [klientId, ustawKlienta] = useState<number | null>(klienci[0]?.id ?? null);
  const [blad, ustawBlad] = useState<string | null>(null);

  useEffect(() => {
    if (!api) return;
    Promise.all([api.oferty.lista({ status: "zaakceptowana" }), api.oferty.lista({ status: "wyslana" })]).then(
      ([a, b]) => {
        const lista = [...a, ...b];
        ustawOferty(lista);
        ustawOferte((o) => o ?? lista[0]?.wersja_id ?? null);
      },
      (e: Error) => ustawBlad(e.message),
    );
  }, [api]);

  const utworz = async () => {
    if (!api) return;
    try {
      const p = zrodlo === "oferta" && ofertaWersja ? await api.umowy.dodaj({ wersja_oferty_id: ofertaWersja }) : await api.umowy.dodaj({ klient_id: klientId ?? undefined });
      onUtworzona(p.umowa.id);
    } catch (e) {
      ustawBlad((e as Error).message);
    }
  };

  return (
    <Okno
      tytul="Nowa umowa"
      onZamknij={onZamknij}
      stopka={
        <>
          <Przycisk onClick={onZamknij}>Anuluj</Przycisk>
          <Przycisk wariant="glowny" disabled={zrodlo === "oferta" ? !ofertaWersja : !klientId} onClick={() => void utworz()}>
            Utwórz umowę
          </Przycisk>
        </>
      }
    >
      <Segment<Zrodlo>
        etykieta="Źródło umowy"
        wartosc={zrodlo}
        onZmiana={ustawZrodlo}
        opcje={[
          { wartosc: "oferta", etykieta: "Z oferty" },
          { wartosc: "zero", etykieta: "Od zera" },
        ]}
      />
      {zrodlo === "oferta" ? (
        oferty.length === 0 ? (
          <p className="komunikat-stanu status--zolty">
            <TriangleAlert size={14} aria-hidden /> Brak ofert wysłanych lub zaakceptowanych. Zmień status oferty albo utwórz umowę od zera.
          </p>
        ) : (
          <Pole etykieta="Oferta (wysłana lub zaakceptowana)" id="nu-oferta" pomoc="Specyfikacja zostanie skopiowana, kwota – z wyceny oferty">
            <select id="nu-oferta" value={ofertaWersja ?? ""} onChange={(e) => ustawOferte(Number(e.target.value))}>
              {oferty.map((o) => (
                <option key={o.wersja_id} value={o.wersja_id}>
                  {o.numer} v{o.nr_wersji} – {o.klient_nazwa} – {formatujKwote(o.suma_brutto_gr)}
                </option>
              ))}
            </select>
          </Pole>
        )
      ) : (
        <Pole etykieta="Zamawiający (klient z bazy)" id="nu-klient" pomoc="Nowego klienta dodasz w formularzu umowy">
          <select id="nu-klient" value={klientId ?? ""} onChange={(e) => ustawKlienta(Number(e.target.value))}>
            {klienci.map((k) => (
              <option key={k.id} value={k.id}>
                {k.nazwa}
              </option>
            ))}
          </select>
        </Pole>
      )}
      {blad && <p className="blad">{blad}</p>}
    </Okno>
  );
}
