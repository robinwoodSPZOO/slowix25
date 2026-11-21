// Ekran oferty: nagłówek z wersjami, statusem i eksportem; pod spodem trzy kolumny –
// drzewo specyfikacji, tabela wierszy, panel (wycena / nagłówek / zdjęcia).

import { useCallback, useEffect, useState } from "react";
import { useNavigate, useParams, useSearchParams } from "react-router-dom";
import { ArrowLeft, FileDown, FileSignature, FileText, GitCompareArrows, Mail, CopyPlus, CircleAlert } from "lucide-react";
import type { Klient, PelnaWersja, Specyfikacja, StatusOferty } from "../../api/typy";
import { usePolaczenie } from "../../kontekst/Polaczenie";
import { formatujDate } from "../../lib/formaty";
import { EtykietaStatusu, Przycisk, Segment } from "../../ui";
import { EdytorSpecyfikacji } from "../specyfikacja";
import { PanelNaglowka } from "./PanelNaglowka";
import { PanelWyceny } from "./PanelWyceny";
import { PanelZdjec } from "./PanelZdjec";
import { PorownanieWersji } from "./PorownanieWersji";
import { zapiszPlik } from "./pobieranie";
import { KOLEJNOSC_STATUSOW, STATUSY_OFERTY } from "./statusy";

type Zakladka = "wycena" | "naglowek" | "zdjecia";

export function EkranOferty() {
  const { id } = useParams();
  const ofertaId = Number(id);
  const [parametry, ustawParametry] = useSearchParams();
  const wersjaZAdresu = parametry.get("wersja") ? Number(parametry.get("wersja")) : null;
  const { api, ostatnieZdarzenie } = usePolaczenie();
  const nawiguj = useNavigate();
  const [dane, ustawDane] = useState<PelnaWersja | null>(null);
  const [klienci, ustawKlientow] = useState<Klient[]>([]);
  const [blad, ustawBlad] = useState<string | null>(null);
  const [zakladka, ustawZakladke] = useState<Zakladka>("wycena");
  const [porownanie, ustawPorownanie] = useState(false);
  const [eksport, ustawEksport] = useState<"pdf" | "docx" | null>(null);

  /** Pobiera wskazaną (albo najnowszą) wersję oferty. */
  const pobierzDane = useCallback(async (): Promise<PelnaWersja> => {
    if (!api) throw new Error("Brak połączenia z serwerem");
    let wersjaId = wersjaZAdresu;
    if (!wersjaId) {
      const s = await api.oferty.szczegoly(ofertaId);
      wersjaId = s.wersje[s.wersje.length - 1]?.id ?? null;
    }
    if (!wersjaId) throw new Error("Oferta nie ma wersji");
    return api.oferty.wersja(wersjaId);
  }, [api, ofertaId, wersjaZAdresu]);

  // Pierwsze wczytanie, zmiana wersji oraz zmiana tej oferty na innym stanowisku.
  const zdarzenieOferty = ostatnieZdarzenie?.zasob === "oferty" && ostatnieZdarzenie.id === ofertaId ? ostatnieZdarzenie : null;
  useEffect(() => {
    pobierzDane().then(
      (d) => {
        ustawDane(d);
        ustawBlad(null);
      },
      (e: Error) => ustawBlad(e.message),
    );
  }, [pobierzDane, zdarzenieOferty]);

  useEffect(() => {
    api?.klienci.lista().then(ustawKlientow, () => {});
  }, [api]);

  const poZmianieSpecyfikacji = useCallback(
    (s: Specyfikacja) => {
      ustawDane((d) => (d ? { ...d, specyfikacja: s } : d));
      // Wycena zależy od cen w specyfikacji – pobieramy ją od nowa.
      if (api && dane) api.oferty.wersja(dane.wersja.id).then(ustawDane, (e: Error) => ustawBlad(e.message));
    },
    [api, dane],
  );

  if (!api) return null;
  if (!dane) return <div className="ekran-oferty__ladowanie">{blad ? <p className="blad">{blad}</p> : "Wczytywanie oferty…"}</div>;

  const { oferta, wersja, wersje } = dane;
  const st = STATUSY_OFERTY[oferta.status];

  const wybierzWersje = (wid: number) => {
    const najnowsza = wersje[wersje.length - 1]?.id;
    ustawParametry(wid === najnowsza ? {} : { wersja: String(wid) });
  };

  const nowaWersja = async () => {
    try {
      const p = await api.oferty.nowaWersja(oferta.id, wersja.id);
      ustawParametry({});
      ustawDane(p);
    } catch (e) {
      ustawBlad((e as Error).message);
    }
  };

  const zmienStatus = async (s: StatusOferty) => {
    try {
      const r = await api.oferty.zmienStatus(oferta.id, s);
      ustawDane((d) => (d ? { ...d, oferta: r.oferta } : d));
    } catch (e) {
      ustawBlad((e as Error).message);
    }
  };

  const zmienKlienta = async (k: number) => {
    try {
      const r = await api.oferty.zmienKlienta(oferta.id, k);
      ustawDane((d) => (d ? { ...d, oferta: r.oferta } : d));
    } catch (e) {
      ustawBlad((e as Error).message);
    }
  };

  const pobierz = async (format: "pdf" | "docx") => {
    ustawEksport(format);
    try {
      const { nazwa, plik } = await api.oferty.eksport(wersja.id, format);
      zapiszPlik(plik, nazwa);
    } catch (e) {
      ustawBlad((e as Error).message);
    } finally {
      ustawEksport(null);
    }
  };

  return (
    <div className="ekran-oferty">
      <header className="ekran-oferty__naglowek">
        <div className="ekran-oferty__tytul">
          <Przycisk wariant="tekstowy" tylkoIkona ikona={ArrowLeft} etykieta="Wróć do listy ofert" onClick={() => nawiguj("/oferty")} />
          <div>
            <div className="ekran-oferty__numer">
              <h1>{oferta.numer}</h1>
              <EtykietaStatusu stan={st.stan} ikona={st.ikona} tekst={st.etykieta} />
            </div>
            <div className="ekran-oferty__meta">
              <label>
                <span className="sr-only">Inwestor</span>
                <select className="wejscie wejscie--maly" value={oferta.klient_id} aria-label="Inwestor" onChange={(e) => void zmienKlienta(Number(e.target.value))}>
                  {klienci.length === 0 && <option value={oferta.klient_id}>{oferta.klient_nazwa}</option>}
                  {klienci.map((k) => (
                    <option key={k.id} value={k.id}>
                      {k.nazwa}
                    </option>
                  ))}
                </select>
              </label>
              <span>· {formatujDate(wersja.data_oferty)}</span>
              {wersja.adres_inwestycji && <span>· {wersja.adres_inwestycji}</span>}
            </div>
          </div>
        </div>
        <div className="ekran-oferty__narzedzia">
          <label className="ekran-oferty__pole">
            <span>Status</span>
            <select className="wejscie wejscie--maly" value={oferta.status} onChange={(e) => void zmienStatus(e.target.value as StatusOferty)}>
              {KOLEJNOSC_STATUSOW.map((s) => (
                <option key={s} value={s}>
                  {STATUSY_OFERTY[s].etykieta}
                </option>
              ))}
            </select>
          </label>
          <label className="ekran-oferty__pole">
            <span>Wersja</span>
            <select className="wejscie wejscie--maly" value={wersja.id} onChange={(e) => wybierzWersje(Number(e.target.value))}>
              {wersje.map((w) => (
                <option key={w.id} value={w.id}>
                  v{w.nr_wersji} – {formatujDate(w.data_oferty)}
                  {w.id === wersje[wersje.length - 1]?.id ? " (najnowsza)" : ""}
                </option>
              ))}
            </select>
          </label>
          <Przycisk ikona={CopyPlus} onClick={() => void nowaWersja()} title={`Nowa wersja jako kopia v${wersja.nr_wersji}`}>
            Nowa wersja
          </Przycisk>
          <Przycisk ikona={GitCompareArrows} disabled={wersje.length < 2} onClick={() => ustawPorownanie(true)}>
            Porównaj
          </Przycisk>
          <span className="ekran-oferty__separator" aria-hidden />
          <Przycisk ikona={FileDown} disabled={eksport !== null} onClick={() => void pobierz("pdf")}>
            {eksport === "pdf" ? "Tworzenie…" : "PDF"}
          </Przycisk>
          <Przycisk ikona={FileText} disabled={eksport !== null} onClick={() => void pobierz("docx")}>
            {eksport === "docx" ? "Tworzenie…" : "DOCX"}
          </Przycisk>
          <Przycisk ikona={Mail} disabled title="Wysyłka e-mail – w fazie Korespondencja">
            Wyślij e-mail
          </Przycisk>
          <Przycisk wariant="glowny" ikona={FileSignature} disabled title="Tworzenie umowy z oferty – w fazie Generator Umów">
            Utwórz umowę
          </Przycisk>
        </div>
      </header>
      {blad && (
        <div className="komunikat-stanu status--czerwony ekran-oferty__blad" role="alert">
          <CircleAlert size={14} aria-hidden /> {blad}
        </div>
      )}
      <div className="ekran-oferty__tresc">
        <EdytorSpecyfikacji
          specyfikacja={dane.specyfikacja}
          onZmiana={poZmianieSpecyfikacji}
          panel={(k) => (
            <div className="panel-oferty">
              <div className="panel-oferty__zakladki">
                <Segment<Zakladka>
                  etykieta="Panel oferty"
                  wartosc={zakladka}
                  onZmiana={ustawZakladke}
                  opcje={[
                    { wartosc: "wycena", etykieta: "Wycena" },
                    { wartosc: "naglowek", etykieta: "Nagłówek" },
                    { wartosc: "zdjecia", etykieta: `Zdjęcia (${dane.specyfikacja.zdjecia.length})` },
                  ]}
                />
              </div>
              {zakladka === "wycena" && <PanelWyceny dane={dane} onDane={ustawDane} onBlad={ustawBlad} onSpecyfikacja={poZmianieSpecyfikacji} />}
              {zakladka === "naglowek" && <PanelNaglowka dane={dane} onDane={ustawDane} onBlad={ustawBlad} />}
              {zakladka === "zdjecia" && <PanelZdjec specyfikacja={dane.specyfikacja} kontekst={k} onSpecyfikacja={poZmianieSpecyfikacji} onBlad={ustawBlad} />}
            </div>
          )}
        />
      </div>
      {porownanie && <PorownanieWersji ofertaId={oferta.id} wersje={wersje} biezaca={wersja.id} onZamknij={() => ustawPorownanie(false)} />}
    </div>
  );
}
