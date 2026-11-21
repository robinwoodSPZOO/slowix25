// Oś czasu zlecenia: pomiar → rysunki → zamówienia (okno sweet spot / planowa / krytyczna) → dostawa
// wymagana → warsztat → montaż → odbiór, z linią „dziś”. Edycja terminu montażu, czasu warsztatu
// i montażu z przeliczeniem i listą: co się przesunęło, co jest zamrożone.

import { useCallback, useEffect, useMemo, useState, type CSSProperties, type ReactNode } from "react";
import { Link, useParams } from "react-router-dom";
import { ArrowLeft, CalendarDays, Check, CircleAlert, Diamond, Flag, History, Lock, PackageCheck, RefreshCw, Snowflake, TriangleAlert } from "lucide-react";
import { BladApi } from "../../api/klient";
import type { Harmonogram, Klient, ZadanieKalendarza, Zlecenie } from "../../api/typy";
import { usePolaczenie } from "../../kontekst/Polaczenie";
import { formatujDate } from "../../lib/formaty";
import { Pole, Przycisk, PoleDaty } from "../../ui";
import { komunikatBledu, useOdswiezaniePoZdarzeniu } from "./dane";
import { dodajDni, dzisIso, krotkaData, NAZWY_MIESIECY, roznicaDni } from "./daty";
import { PanelZadania } from "./PanelZadania";
import { roznicaHarmonogramu, type RoznicaHarmonogramu } from "./roznice";
import { useSzkic } from "./szkic";
import { zakresZadania } from "./uklad";
import { IkonaStanu, IkonaTypu, kolorTypu, NAZWY_ETAPOW, nazwaTypu } from "./wyglad";
import { DZIEN, liczba, ZADANIE } from "../../lib/odmiana";

interface FormularzTerminow {
  montaz_od: string;
  czas_warsztat_dni: string;
  czas_montazu_dni: string;
}

const KODY_USTAWIEN = new Set(["brak_widelek", "nieprawidlowe_widelki"]);

function liczbaLubNull(t: string): number | null {
  return t.trim() === "" ? null : Number(t);
}

/** Pozycja daty na osi (procent). */
function skala(od: string, doDnia: string) {
  const dni = Math.max(1, roznicaDni(od, doDnia) + 1);
  return {
    x: (d: string) => (roznicaDni(od, d) / dni) * 100,
    szer: (a: string, b: string) => ((roznicaDni(a, b) + 1) / dni) * 100,
    dni,
  };
}

function WierszOsi({ etykieta, typ, children, podpis }: { etykieta: ReactNode; typ: string; children: ReactNode; podpis?: ReactNode }) {
  return (
    <div className="os__wiersz" style={{ "--kolor": kolorTypu(typ) } as CSSProperties} role="row">
      <div className="os__etykieta" role="rowheader">
        <span className="os__ikona">
          <IkonaTypu typ={typ} rozmiar={13} />
        </span>
        <span className="os__nazwa">
          {etykieta}
          {podpis && <small>{podpis}</small>}
        </span>
      </div>
      <div className="os__tor" role="cell">
        {children}
      </div>
    </div>
  );
}

export function OsCzasu() {
  const { id } = useParams();
  const zlecenieId = Number(id);
  const { api } = usePolaczenie();
  const dzis = dzisIso();
  const [zlecenie, ustawZlecenie] = useState<Zlecenie | null>(null);
  const [klient, ustawKlienta] = useState<Klient | null>(null);
  const [zadania, ustawZadania] = useState<ZadanieKalendarza[]>([]);
  const [harmonogram, ustawHarmonogram] = useState<Harmonogram | null>(null);
  const [bladHarmonogramu, ustawBladHarmonogramu] = useState<string | null>(null);
  const [blad, ustawBlad] = useState<string | null>(null);
  const [wczytano, ustawWczytano] = useState(false);
  const [wybrane, ustawWybrane] = useState<number | null>(null);
  const [roznica, ustawRoznice] = useState<RoznicaHarmonogramu | null>(null);
  const [zapisywanie, ustawZapisywanie] = useState(false);
  const [bladZapisu, ustawBladZapisu] = useState<string | null>(null);

  const pobierz = useCallback(async () => {
    if (!api || !zlecenieId) return;
    try {
      const [z, t] = await Promise.all([api.zlecenia.pobierz(zlecenieId), api.kalendarz.zadania({ zlecenie_id: zlecenieId })]);
      ustawZlecenie(z);
      ustawZadania(t);
      ustawBlad(null);
      api.klienci.pobierz(z.klient_id).then(ustawKlienta, () => ustawKlienta(null));
      if (z.montaz_od) {
        try {
          ustawHarmonogram(await api.zlecenia.podgladHarmonogramu(zlecenieId));
          ustawBladHarmonogramu(null);
        } catch (e) {
          ustawHarmonogram(null);
          ustawBladHarmonogramu(komunikatBledu(e));
        }
      } else {
        ustawHarmonogram(null);
        ustawBladHarmonogramu(null);
      }
      return t;
    } catch (e) {
      ustawBlad(komunikatBledu(e));
    } finally {
      ustawWczytano(true);
    }
  }, [api, zlecenieId]);

  useEffect(() => {
    void Promise.resolve().then(pobierz);
  }, [pobierz]);
  useOdswiezaniePoZdarzeniu(() => void pobierz());

  const poczatkowe = useMemo<FormularzTerminow | null>(
    () =>
      zlecenie && {
        montaz_od: zlecenie.montaz_od ?? "",
        czas_warsztat_dni: zlecenie.czas_warsztat_dni?.toString() ?? "",
        czas_montazu_dni: zlecenie.czas_montazu_dni?.toString() ?? "",
      },
    [zlecenie],
  );
  const szkic = useSzkic<FormularzTerminow>(`zlecenie:${zlecenieId}:terminy`, poczatkowe);
  const f = szkic.wartosc;

  const bledy = f && {
    montaz_od: f.montaz_od === "" ? "Wpisz termin rozpoczęcia montażu" : null,
    czas_warsztat_dni: f.czas_warsztat_dni !== "" && !(Number.isInteger(Number(f.czas_warsztat_dni)) && Number(f.czas_warsztat_dni) >= 0 && Number(f.czas_warsztat_dni) <= 365) ? "Liczba dni 0–365" : null,
    czas_montazu_dni: !(Number.isInteger(Number(f.czas_montazu_dni)) && Number(f.czas_montazu_dni) >= 1 && Number(f.czas_montazu_dni) <= 365) ? "Liczba dni 1–365" : null,
  };
  const poprawny = !!bledy && !bledy.montaz_od && !bledy.czas_warsztat_dni && !bledy.czas_montazu_dni;

  const zapisz = async () => {
    if (!api || !zlecenie || !f || !poprawny) return;
    ustawZapisywanie(true);
    ustawBladZapisu(null);
    const przed = zadania;
    try {
      await api.zlecenia.zmien(zlecenie.id, {
        klient_id: zlecenie.klient_id,
        umowa_id: zlecenie.umowa_id,
        nazwa: zlecenie.nazwa,
        etap: zlecenie.etap,
        montaz_od: f.montaz_od,
        czas_warsztat_dni: liczbaLubNull(f.czas_warsztat_dni),
        czas_montazu_dni: liczbaLubNull(f.czas_montazu_dni),
        odstep_materialy_dni: zlecenie.odstep_materialy_dni,
        odstep_rysunki_dni: zlecenie.odstep_rysunki_dni,
        odstep_pomiar_dni: zlecenie.odstep_pomiar_dni,
      });
      szkic.wyczysc();
      if (przed.length === 0) await api.zlecenia.generujHarmonogram(zlecenie.id);
      const po = await pobierz();
      if (po) ustawRoznice(roznicaHarmonogramu(przed, po));
    } catch (e) {
      ustawBladZapisu(komunikatBledu(e));
    } finally {
      ustawZapisywanie(false);
    }
  };

  const generuj = async () => {
    if (!api || !zlecenie) return;
    ustawZapisywanie(true);
    try {
      await api.zlecenia.generujHarmonogram(zlecenie.id);
      await pobierz();
    } catch (e) {
      ustawBladZapisu(e instanceof BladApi ? komunikatBledu(e) : String(e));
    } finally {
      ustawZapisywanie(false);
    }
  };

  const zadanieWybrane = zadania.find((z) => z.id === wybrane) ?? null;

  // --- Zakres osi ---
  const os = useMemo(() => {
    const daty: string[] = [dzis];
    for (const z of zadania) {
      const r = zakresZadania(z);
      daty.push(r.od, r.do);
    }
    if (harmonogram) daty.push(harmonogram.pomiar_do, harmonogram.montaz_do);
    daty.sort();
    const od = dodajDni(daty[0]!, -4);
    const doDnia = dodajDni(daty[daty.length - 1]!, 6);
    return { od, doDnia, s: skala(od, doDnia) };
  }, [zadania, harmonogram, dzis]);

  const podzialki = useMemo(() => {
    const p: { data: string; miesiac: boolean }[] = [];
    for (let d = os.od; d <= os.doDnia; d = dodajDni(d, 1)) {
      if (d.endsWith("-01")) p.push({ data: d, miesiac: true });
      else if (os.s.dni <= 150 && new Date(d).getUTCDay() === 1) p.push({ data: d, miesiac: false });
    }
    return p;
  }, [os]);

  if (!wczytano) return <p>Wczytywanie zlecenia…</p>;
  if (blad || !zlecenie)
    return (
      <div className="komunikat-stanu stan-tlo--czerwony" role="alert">
        <CircleAlert size={16} aria-hidden />
        <span>
          {blad ?? "Nie znaleziono zlecenia."} <Link to="/kalendarz">Wróć do kalendarza</Link>
        </span>
      </div>
    );

  const poKluczu = (k: string) => zadania.find((z) => z.klucz === k);
  const zamowienia = zadania.filter((z) => z.typ === "zamowienie");
  const pozostale = zadania.filter((z) => !["pomiar", "rysunki", "zamowienie", "warsztat", "montaz"].includes(z.typ));
  const { x, szer } = os.s;

  const element = (z: ZadanieKalendarza, etykieta?: string) => {
    const r = zakresZadania(z);
    const okno = z.typ === "zamowienie";
    const jednodniowe = r.od === r.do && !okno;
    const planowa = okno && z.data_planowa ? ((roznicaDni(r.od, z.data_planowa) + 0.5) / (roznicaDni(r.od, r.do) + 1)) * 100 : null;
    return (
      <button
        key={z.id}
        type="button"
        className={`os__element${jednodniowe ? " os__element--punkt" : ""}${okno ? " os__element--okno" : ""} os__element--${z.stan.stan}${z.wykonano ? " os__element--wykonane" : ""}${wybrane === z.id ? " os__element--wybrany" : ""}`}
        style={{ left: `${x(r.od)}%`, width: jednodniowe ? undefined : `${szer(r.od, r.do)}%`, ...(planowa !== null ? { "--planowa": `${planowa}%` } : {}) } as CSSProperties}
        title={`${z.tytul}\n${z.stan.opis}`}
        aria-label={`${etykieta ?? z.tytul}: ${krotkaData(r.od)}${r.do !== r.od ? `–${krotkaData(r.do)}` : ""}. ${z.stan.opis}`}
        onClick={() => ustawWybrane(z.id)}
      >
        {planowa !== null && (
          <span className="os__planowa" style={{ left: `${planowa}%` }}>
            <Diamond size={10} aria-hidden />
          </span>
        )}
        {z.wykonano ? <Check size={11} aria-hidden /> : z.stan.stan !== "spokojnie" ? <IkonaStanu stan={z.stan.stan} rozmiar={11} /> : null}
        {!jednodniowe && <span className="os__element-tekst">{krotkaData(r.od)} – {krotkaData(r.do)}</span>}
      </button>
    );
  };

  const montaz = poKluczu("montaz");
  const warsztat = poKluczu("warsztat");
  const pomiar = poKluczu("pomiar");
  const rysunki = poKluczu("rysunki");

  return (
    <div className="os-czasu">
      <header className="naglowek-modulu">
        <div>
          <Link to="/kalendarz" className="link-wstecz">
            <ArrowLeft size={14} aria-hidden /> Kalendarz
          </Link>
          <h1>{zlecenie.nazwa}</h1>
          <p className="tekst-drugorzedny">
            {klient?.nazwa ?? `Klient ${zlecenie.klient_id}`} · etap: {NAZWY_ETAPOW[zlecenie.etap] ?? zlecenie.etap}
            {harmonogram && ` · dni liczone: ${harmonogram.liczenie_dni === "robocze" ? "robocze" : "kalendarzowe"}`}
          </p>
        </div>
        <div className="naglowek-modulu__akcje">
          <Link className="przycisk przycisk--drugorzedny" to={`/kalendarz?zadanie=${montaz?.id ?? ""}&data=${zlecenie.montaz_od ?? dzis}`}>
            <CalendarDays size={16} aria-hidden /> Pokaż w kalendarzu
          </Link>
        </div>
      </header>

      {zlecenie.harmonogram_nieaktualny && (
        <div className="komunikat-stanu stan-tlo--pomaranczowy" role="alert">
          <TriangleAlert size={16} aria-hidden />
          <span>
            <strong>Harmonogram nieaktualny</strong> – terminy poniżej mogą być błędne. Powód: {zlecenie.harmonogram_nieaktualny_powod ?? "nieznany"}. Popraw dane i
            kliknij „Przelicz i zapisz”.
          </span>
        </div>
      )}
      {bladHarmonogramu && !zlecenie.harmonogram_nieaktualny && (
        <div className="komunikat-stanu stan-tlo--pomaranczowy" role="note">
          <TriangleAlert size={16} aria-hidden />
          <span>Nie można policzyć harmonogramu: {bladHarmonogramu}</span>
        </div>
      )}
      {harmonogram && harmonogram.ostrzezenia.length > 0 && (
        <ul className="ostrzezenia-silnika">
          {harmonogram.ostrzezenia.map((o, i) => (
            <li key={i} className="komunikat-stanu stan-tlo--zolty">
              <TriangleAlert size={14} aria-hidden />
              <span>
                {o.komunikat}
                {KODY_USTAWIEN.has(o.kod) && (
                  <>
                    {" "}
                    <Link to="/kalendarz/ustawienia#czasy-dostaw">Ustaw czasy dostaw</Link>
                  </>
                )}
              </span>
            </li>
          ))}
        </ul>
      )}

      <div className="os-czasu__uklad">
        <div className="os-czasu__lewa">
          <form
            className="karta os-czasu__formularz"
            onSubmit={(e) => {
              e.preventDefault();
              void zapisz();
            }}
          >
            <h2>Terminy zlecenia</h2>
            {szkic.przywrocono && (
              <p className="komunikat-stanu stan-tlo--zolty">
                <History size={14} aria-hidden />
                Przywrócono niezapisane zmiany.{" "}
                <button type="button" className="link" onClick={() => szkic.wyczysc()}>
                  Odrzuć
                </button>
              </p>
            )}
            {f && bledy && (
              <div className="os-czasu__pola">
                <Pole etykieta="Początek montażu" id="os-montaz" blad={bledy.montaz_od}>
                  <PoleDaty id="os-montaz" wartosc={f.montaz_od} onZmiana={(v) => szkic.zmien({ ...f, montaz_od: v })} />
                </Pole>
                <Pole etykieta="Składanie na warsztacie (dni)" id="os-warsztat" blad={bledy.czas_warsztat_dni}>
                  <input id="os-warsztat" type="number" min={0} max={365} value={f.czas_warsztat_dni} onChange={(e) => szkic.zmien({ ...f, czas_warsztat_dni: e.target.value })} />
                </Pole>
                <Pole etykieta="Montaż (dni)" id="os-czas-montazu" blad={bledy.czas_montazu_dni}>
                  <input id="os-czas-montazu" type="number" min={1} max={365} value={f.czas_montazu_dni} onChange={(e) => szkic.zmien({ ...f, czas_montazu_dni: e.target.value })} />
                </Pole>
              </div>
            )}
            <div className="os-czasu__przyciski">
              <Przycisk wariant="glowny" type="submit" ikona={RefreshCw} disabled={zapisywanie || !poprawny}>
                {zapisywanie ? "Przeliczanie…" : "Przelicz i zapisz"}
              </Przycisk>
              {szkic.zmieniono && (
                <Przycisk wariant="tekstowy" onClick={() => szkic.wyczysc()}>
                  Anuluj zmiany
                </Przycisk>
              )}
            </div>
            <p className="pole__pomoc">
              Pozostałe terminy liczą się wstecz od montażu. Zadania wykonane, zamówione materiały i zadania dodane ręcznie nie są przesuwane.
            </p>
            {bladZapisu && (
              <p className="komunikat-stanu stan-tlo--czerwony" role="alert">
                <CircleAlert size={14} aria-hidden />
                {bladZapisu}
              </p>
            )}
            {zadania.length === 0 && zlecenie.montaz_od && (
              <Przycisk ikona={CalendarDays} onClick={() => void generuj()} disabled={zapisywanie}>
                Wygeneruj harmonogram
              </Przycisk>
            )}
          </form>

          {roznica && (
            <section className="karta os-czasu__roznica" aria-live="polite">
              <h2>Co się zmieniło</h2>
              {roznica.przesuniete.length === 0 && roznica.dodane.length === 0 && roznica.usuniete.length === 0 && <p>Żaden termin się nie zmienił.</p>}
              {roznica.przesuniete.length > 0 && (
                <ul className="lista-zmian">
                  {roznica.przesuniete.map((p) => (
                    <li key={p.id} style={{ "--kolor": kolorTypu(p.typ) } as CSSProperties}>
                      <IkonaTypu typ={p.typ} rozmiar={12} />
                      <span>
                        {p.tytul}: {krotkaData(p.z.od)} → <strong>{krotkaData(p.na.od)}</strong> ({p.dni > 0 ? "+" : ""}{liczba(p.dni, DZIEN)})
                      </span>
                    </li>
                  ))}
                </ul>
              )}
              {roznica.zamrozone.length > 0 && (
                <>
                  <h3>
                    <Snowflake size={13} aria-hidden /> Zamrożone (bez zmian)
                  </h3>
                  <ul className="lista-zmian">
                    {roznica.zamrozone.map((p) => (
                      <li key={p.id}>
                        <IkonaTypu typ={p.typ} rozmiar={12} />
                        <span>
                          {p.tytul} – {p.powod}
                        </span>
                      </li>
                    ))}
                  </ul>
                </>
              )}
              {roznica.dodane.length > 0 && <p>Nowe zadania: {roznica.dodane.map((z) => z.tytul).join("; ")}</p>}
              {roznica.usuniete.length > 0 && <p>Usunięte zadania: {roznica.usuniete.map((z) => z.tytul).join("; ")}</p>}
            </section>
          )}
        </div>

        <section className="karta os" aria-label="Oś czasu zlecenia">
          {zadania.length === 0 ? (
            <p className="os__pusto">
              To zlecenie nie ma jeszcze harmonogramu. Wpisz termin rozpoczęcia montażu i czas składania na warsztacie, a następnie kliknij „Przelicz i zapisz” –
              program policzy wstecz pomiar, rysunki i zamówienia materiałów.
            </p>
          ) : (
            <div className="os__siatka" role="table" aria-label={`Oś czasu: ${zlecenie.nazwa}`}>
              <div className="os__wiersz os__wiersz--podzialka" role="row">
                <div className="os__etykieta" role="columnheader">
                  Etap
                </div>
                <div className="os__tor" role="columnheader">
                  {podzialki.map((p) => (
                    <span key={p.data} className={`os__podzialka${p.miesiac ? " os__podzialka--miesiac" : ""}`} style={{ left: `${x(p.data)}%` }}>
                      {p.miesiac ? NAZWY_MIESIECY[Number(p.data.slice(5, 7)) - 1] : krotkaData(p.data)}
                    </span>
                  ))}
                </div>
              </div>
              <div className="os__tresc">
                <div className="os__dzis" style={{ left: `calc(var(--szer-etykiety) + (100% - var(--szer-etykiety)) * ${x(dzis) / 100})` }}>
                  <span>dziś</span>
                </div>
                {pomiar && (
                  <WierszOsi etykieta="Pomiar końcowy" typ="pomiar">
                    {element(pomiar)}
                  </WierszOsi>
                )}
                {rysunki && (
                  <WierszOsi etykieta="Rysunki wykonawcze" typ="rysunki">
                    {element(rysunki)}
                  </WierszOsi>
                )}
                {zamowienia.map((z) => (
                  <WierszOsi
                    key={z.id}
                    typ="zamowienie"
                    etykieta={z.zamowienie?.kategoria_nazwa ?? z.tytul}
                    podpis={
                      <>
                        {z.zamowienie?.zablokowane && !z.wykonano && <Lock size={10} aria-label="czeka na potwierdzenie klienta" />}
                        {z.zamowienie?.dostawca_nazwa ?? "bez dostawcy"}
                        {z.zamowienie?.dni_min !== null && z.zamowienie ? ` · ${z.zamowienie.dni_min}–${z.zamowienie.dni_max} dni` : ""}
                        {z.wykonano ? ` · zamówiono ${krotkaData(z.zamowienie?.data_zamowienia)}` : ""}
                      </>
                    }
                  >
                    {element(z, `Zamówienie: ${z.zamowienie?.kategoria_nazwa ?? ""}`)}
                    {z.zamowienie?.data_zamowienia && (
                      <span className="os__znacznik" style={{ left: `${x(z.zamowienie.data_zamowienia)}%` }} title={`Zamówiono ${formatujDate(z.zamowienie.data_zamowienia)}`}>
                        <Check size={11} aria-hidden />
                      </span>
                    )}
                    {(z.zamowienie?.data_dostawy ?? z.zamowienie?.oczekiwana_dostawa) && (
                      <span
                        className={`os__znacznik os__znacznik--dostawa${z.zamowienie?.dostawa_zagrozona ? " os__znacznik--zagrozona" : ""}`}
                        style={{ left: `${x((z.zamowienie?.data_dostawy ?? z.zamowienie?.oczekiwana_dostawa)!)}%` }}
                        title={z.zamowienie?.data_dostawy ? `Dostarczono ${formatujDate(z.zamowienie.data_dostawy)}` : `Oczekiwana dostawa ${formatujDate(z.zamowienie?.oczekiwana_dostawa)}${z.zamowienie?.dostawa_zagrozona ? " – zagrożona!" : ""}`}
                      >
                        <PackageCheck size={12} aria-hidden />
                      </span>
                    )}
                  </WierszOsi>
                ))}
                {harmonogram && (
                  <WierszOsi etykieta="Materiały na warsztacie" typ="zamowienie" podpis="dostawa wymagana najpóźniej">
                    <span className="os__flaga" style={{ left: `${x(harmonogram.data_wymaganej_dostawy)}%` }} title={`Dostawa wymagana do ${formatujDate(harmonogram.data_wymaganej_dostawy)}`}>
                      <Flag size={12} aria-hidden /> {krotkaData(harmonogram.data_wymaganej_dostawy)}
                    </span>
                  </WierszOsi>
                )}
                {warsztat && (
                  <WierszOsi etykieta="Składanie na warsztacie" typ="warsztat" podpis={zlecenie.czas_warsztat_dni ? liczba(zlecenie.czas_warsztat_dni, DZIEN) : undefined}>
                    {element(warsztat)}
                  </WierszOsi>
                )}
                {montaz && (
                  <WierszOsi etykieta="Montaż" typ="montaz" podpis={zlecenie.czas_montazu_dni ? liczba(zlecenie.czas_montazu_dni, DZIEN) : undefined}>
                    {element(montaz)}
                  </WierszOsi>
                )}
                {montaz && (
                  <WierszOsi etykieta="Odbiór" typ="klient" podpis="po zakończeniu montażu">
                    <span className="os__flaga" style={{ left: `${x(zakresZadania(montaz).do)}%` }}>
                      <Flag size={12} aria-hidden /> {krotkaData(zakresZadania(montaz).do)}
                    </span>
                  </WierszOsi>
                )}
                {pozostale.length > 0 && (
                  <WierszOsi etykieta="Klient, płatności, inne" typ="klient" podpis={liczba(pozostale.length, ZADANIE)}>
                    {pozostale.map((z) => (
                      element(z, `${nazwaTypu(z.typ)}: ${z.tytul}`)
                    ))}
                  </WierszOsi>
                )}
              </div>
            </div>
          )}
        </section>
        {zadanieWybrane && (
          <PanelZadania
            zadanie={zadanieWybrane}
            dzis={dzis}
            onZamknij={() => ustawWybrane(null)}
            onZmiana={(z) => {
              if (!z) ustawWybrane(null);
              void pobierz();
            }}
          />
        )}
      </div>
    </div>
  );
}
