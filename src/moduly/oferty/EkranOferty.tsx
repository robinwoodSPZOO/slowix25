// Ekran oferty: nagłówek z wersjami, statusem i eksportem; pod spodem trzy kolumny –
// drzewo specyfikacji, tabela wierszy, panel (wycena / nagłówek / zdjęcia).

import { useCallback, useEffect, useState } from "react";
import { useNavigate, useParams, useSearchParams } from "react-router-dom";
import { ArrowLeft, FileDown, FileSignature, FileText, GitCompareArrows, Mail, CopyPlus, CircleAlert } from "lucide-react";
import type { BladApi } from "../../api/klient";
import type { KonfliktPola, Klient, PelnaWersja, Specyfikacja, StatusOferty } from "../../api/typy";
import { usePolaczenie } from "../../kontekst/Polaczenie";
import { formatujDate } from "../../lib/formaty";
import { EtykietaStatusu, Przycisk, Segment } from "../../ui";
import { EdytorSpecyfikacji, OknoKonfliktu, PanelZdjec, nazwaPolaWiersza } from "../specyfikacja";
import { PanelNaglowka } from "./PanelNaglowka";
import { PanelWyceny } from "./PanelWyceny";
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
  const [konflikt, ustawKonflikt] = useState<{ konflikty: KonfliktPola[]; ponow: (a: Record<string, unknown>) => Promise<void> } | null>(null);
  const obsluzKonflikt = useCallback((e: BladApi, ponow: (a: Record<string, unknown>) => Promise<void>) => {
    ustawKonflikt({ konflikty: e.konflikty, ponow });
  }, []);

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
    if (oferta.status === "szkic" && s !== "szkic" && !confirm("Po zmianie statusu ta wersja zostanie zamrożona (także dane inwestora i firmy w dokumencie). Dalsze zmiany – przez nową wersję. Kontynuować?")) return;
    try {
      await api.oferty.zmienStatus(oferta.id, s);
      ustawDane(await pobierzDane());
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

  const utworzUmowe = async () => {
    try {
      const u = await api.umowy.dodaj({ wersja_oferty_id: wersja.id });
      nawiguj(`/umowy/${u.umowa.id}`);
    } catch (e) {
      ustawBlad((e as Error).message);
    }
  };

  const pobierz = async (format: "pdf" | "docx") => {
    if (dane.wycena.bledy.length > 0) {
      ustawBlad(`Nie można utworzyć dokumentu: ${dane.wycena.bledy.join("; ")}`);
      return;
    }
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
                <select className="wejscie wejscie--maly" value={oferta.klient_id} aria-label="Inwestor" disabled={wersja.tylko_do_odczytu} onChange={(e) => void zmienKlienta(Number(e.target.value))}>
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
                <option key={s} value={s} disabled={s === "szkic" && oferta.status !== "szkic"}>
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
          <Przycisk ikona={CopyPlus} disabled={oferta.klient_usuniety} onClick={() => void nowaWersja()} title={`Nowa wersja jako kopia v${wersja.nr_wersji}`}>
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
          <Przycisk
            wariant="glowny"
            ikona={FileSignature}
            disabled={!wersja.wyslano || oferta.status === "odrzucona" || oferta.klient_usuniety}
            title={wersja.wyslano ? "Umowa z tej wersji: kopia specyfikacji, kwota z wyceny, klient i adres inwestycji" : "Umowę tworzy się z wersji wysłanej lub zaakceptowanej – zmień status oferty"}
            onClick={() => void utworzUmowe()}
          >
            Utwórz umowę
          </Przycisk>
        </div>
      </header>
      {oferta.klient_usuniety && (
        <div className="komunikat-stanu status--pomaranczowy ekran-oferty__blad">
          <CircleAlert size={14} aria-hidden /> Klient „{oferta.klient_nazwa}” jest usunięty – ofertę można tylko przeglądać.
          <Przycisk
            maly
            onClick={async () => {
              try {
                await api.klienci.przywroc(oferta.klient_id);
                setTimeout(() => void pobierzDane().then(ustawDane), 0);
              } catch (e) {
                ustawBlad((e as Error).message);
              }
            }}
          >
            Przywróć klienta
          </Przycisk>
        </div>
      )}
      {blad && (
        <div className="komunikat-stanu status--czerwony ekran-oferty__blad" role="alert">
          <CircleAlert size={14} aria-hidden /> {blad}
        </div>
      )}
      <div className="ekran-oferty__tresc">
        <EdytorSpecyfikacji
          specyfikacja={dane.specyfikacja}
          onZmiana={poZmianieSpecyfikacji}
          akcjaBlokady={
            wersja.tylko_do_odczytu && !oferta.klient_usuniety ? (
              <Przycisk maly wariant="glowny" ikona={CopyPlus} onClick={() => void nowaWersja()}>
                Nowa wersja
              </Przycisk>
            ) : undefined
          }
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
              {zakladka === "wycena" && <PanelWyceny dane={dane} onDane={ustawDane} onBlad={ustawBlad} onSpecyfikacja={poZmianieSpecyfikacji} onKonflikt={obsluzKonflikt} />}
              {zakladka === "naglowek" && <PanelNaglowka dane={dane} onDane={ustawDane} onBlad={ustawBlad} onKonflikt={obsluzKonflikt} />}
              {zakladka === "zdjecia" && <PanelZdjec specyfikacja={dane.specyfikacja} kontekst={k} onSpecyfikacja={poZmianieSpecyfikacji} onBlad={ustawBlad} />}
            </div>
          )}
        />
      </div>
      {konflikt && (
        <OknoKonfliktu
          tytul="Konflikt zmian oferty"
          konflikty={konflikt.konflikty}
          nazwaPola={nazwaPolaWiersza}
          onMoja={() => {
            const k = konflikt;
            ustawKonflikt(null);
            const akt = Object.fromEntries(k.konflikty.map((x) => [x.pole, x.aktualna]));
            k.ponow(akt).catch((e: Error) => ustawBlad(e.message));
          }}
          onCudza={() => {
            ustawKonflikt(null);
            void pobierzDane().then(ustawDane, (e: Error) => ustawBlad(e.message));
          }}
        />
      )}
      {porownanie && <PorownanieWersji ofertaId={oferta.id} wersje={wersje} biezaca={wersja.id} onZamknij={() => ustawPorownanie(false)} />}
    </div>
  );
}
