// Pulpit: alerty krytyczne (czerwone i pomarańczowe) z akcją, zaległe, do zrobienia w tym tygodniu,
// kolizje warsztatu i montażu, nieaktualne harmonogramy, tablica etapów zleceń.

import { useCallback, useEffect, useMemo, useState, type CSSProperties } from "react";
import { Link } from "react-router-dom";
import { Check, CircleAlert, Columns3, OctagonAlert, ShoppingCart, TriangleAlert, Wrench } from "lucide-react";
import type { KolizjaZlecen, StatusSerwera, Tydzien, ZadanieKalendarza, Zlecenie, Klient, DoZamowienia } from "../../api/typy";
import { usePolaczenie } from "../../kontekst/Polaczenie";
import { formatujDate } from "../../lib/formaty";
import { Przycisk } from "../../ui";
import { komunikatBledu, useOdswiezaniePoZdarzeniu } from "../kalendarz/dane";
import { dodajDni, DNI_TYGODNIA, dzisIso, krotkaData, nazwaTygodnia, roznicaDni, wzgledny } from "../kalendarz/daty";
import { licznikiStanow, wgPilnosci } from "../kalendarz/filtry";
import { useWykonanie } from "../kalendarz/PanelZadania";
import { EtykietaStanu, IkonaTypu, kolorTypu, NAZWY_ETAPOW, nazwaTypu, STANY } from "../kalendarz/wyglad";
import "../kalendarz/kalendarz.css";
import "./pulpit.css";
import { DZIEN, liczba } from "../../lib/odmiana";

const ETAPY_TABLICY = ["umowa", "pomiar", "rysunki", "zamowienia", "warsztat", "montaz", "odbior"] as const;

function linkZadania(z: ZadanieKalendarza) {
  return `/kalendarz?zadanie=${z.id}&data=${z.termin}`;
}

function PozycjaZadania({ z, akcja }: { z: ZadanieKalendarza; akcja?: React.ReactNode }) {
  return (
    <li className="pozycja-pulpitu" style={{ "--kolor": kolorTypu(z.typ) } as CSSProperties}>
      <span className="pozycja-pulpitu__ikona" title={nazwaTypu(z.typ)}>
        <IkonaTypu typ={z.typ} rozmiar={13} />
      </span>
      <span className="pozycja-pulpitu__tekst">
        <Link to={linkZadania(z)}>{z.tytul}</Link>
        <span>{z.stan.opis}</span>
      </span>
      <span className="pozycja-pulpitu__stan">
        <EtykietaStanu stan={z.stan.stan} krotko />
        <small>{wzgledny(z.stan.dni_do_terminu)}</small>
      </span>
      {akcja}
    </li>
  );
}

function PulpitKalendarza() {
  const { api } = usePolaczenie();
  const dzis = dzisIso();
  const [tydzien, ustawTydzien] = useState<Tydzien | null>(null);
  const [doZam, ustawDoZam] = useState<DoZamowienia | null>(null);
  const [kolizje, ustawKolizje] = useState<KolizjaZlecen[]>([]);
  const [zlecenia, ustawZlecenia] = useState<Zlecenie[]>([]);
  const [klienci, ustawKlientow] = useState<Klient[]>([]);
  const [blad, ustawBlad] = useState<string | null>(null);
  const [wczytano, ustawWczytano] = useState(false);

  const pobierz = useCallback(() => {
    if (!api) return;
    Promise.all([api.kalendarz.tydzien(), api.kalendarz.doZamowienia(), api.kalendarz.kolizje(dzis, dodajDni(dzis, 120)), api.zlecenia.lista(), api.klienci.lista()]).then(
      ([t, d, k, z, kl]) => {
        ustawTydzien(t);
        ustawDoZam(d);
        ustawKolizje(k);
        ustawZlecenia(z);
        ustawKlientow(kl);
        ustawBlad(null);
        ustawWczytano(true);
      },
      (e: unknown) => {
        ustawBlad(komunikatBledu(e));
        ustawWczytano(true);
      },
    );
  }, [api, dzis]);
  useEffect(pobierz, [pobierz]);
  useOdswiezaniePoZdarzeniu(pobierz);
  const { wykonaj, oknoBlokady, zapisywanie, blad: bladAkcji } = useWykonanie(() => pobierz());

  const wszystkie = useMemo(() => {
    const m = new Map<number, ZadanieKalendarza>();
    for (const z of tydzien?.zadania ?? []) m.set(z.id, z);
    for (const g of doZam?.grupy ?? []) for (const z of g.pozycje) m.set(z.id, z);
    return [...m.values()].filter((z) => !z.wykonano);
  }, [tydzien, doZam]);
  const alerty = useMemo(() => wszystkie.filter((z) => (z.stan.stan === "czerwony" || z.stan.stan === "pomaranczowy") && (z.typ === "zamowienie" || z.termin >= dzis)).sort(wgPilnosci), [wszystkie, dzis]);
  const zalegle = useMemo(() => wszystkie.filter((z) => z.typ !== "zamowienie" && z.termin < dzis).sort((a, b) => a.termin.localeCompare(b.termin)), [wszystkie, dzis]);
  const liczniki = licznikiStanow(wszystkie);
  const tydzienDni = useMemo(() => {
    if (!tydzien) return [];
    return Array.from({ length: 7 }, (_, i) => {
      const d = dodajDni(tydzien.od, i);
      const lista = tydzien.zadania.filter((z) => !z.wykonano && z.termin === d && !(z.stan.stan === "czerwony" && z.termin < dzis));
      return { d, lista };
    });
  }, [tydzien, dzis]);
  const nazwaKlienta = useMemo(() => new Map(klienci.map((k) => [k.id, k.nazwa])), [klienci]);

  const akcja = (z: ZadanieKalendarza) =>
    z.typ === "zamowienie" ? (
      <Przycisk maly ikona={ShoppingCart} disabled={zapisywanie} onClick={() => void wykonaj(z, { wykonano: dzis, data_zamowienia: dzis })}>
        Zamówione dziś
      </Przycisk>
    ) : (
      <Przycisk maly ikona={Check} disabled={zapisywanie} onClick={() => void wykonaj(z, { wykonano: dzis })}>
        Wykonane
      </Przycisk>
    );

  if (!wczytano) return <p>Wczytywanie…</p>;
  return (
    <>
      {blad && (
        <div className="komunikat-stanu stan-tlo--czerwony" role="alert">
          <CircleAlert size={16} aria-hidden />
          <span>
            {blad}{" "}
            <button type="button" className="link" onClick={pobierz}>
              Spróbuj ponownie
            </button>
          </span>
        </div>
      )}
      {bladAkcji && (
        <div className="komunikat-stanu stan-tlo--czerwony" role="alert">
          <CircleAlert size={16} aria-hidden />
          <span>{bladAkcji}</span>
        </div>
      )}
      <div className="liczniki-pulpitu" aria-label="Podsumowanie ostrzeżeń">
        {(["czerwony", "pomaranczowy", "zolty"] as const).map((s) => {
          const I = STANY[s].ikona;
          return (
            <Link key={s} to={`/kalendarz?widok=tydzien`} className={`licznik-pulpitu stan-tlo--${s}`}>
              <I size={20} aria-hidden />
              <strong>{liczniki[s]}</strong>
              <span>{STANY[s].tekst}</span>
            </Link>
          );
        })}
        <Link to="/zamowienia" className="licznik-pulpitu stan-tlo--neutralny">
          <ShoppingCart size={20} aria-hidden />
          <strong>{doZam?.grupy.reduce((s, g) => s + g.pozycje.length, 0) ?? 0}</strong>
          <span>do zamówienia</span>
        </Link>
      </div>

      {tydzien && tydzien.nieaktualne_harmonogramy.length > 0 && (
        <section className="karta sekcja-pulpitu sekcja-pulpitu--uwaga">
          <h2>
            <TriangleAlert size={16} aria-hidden /> Zlecenia z nieaktualnym harmonogramem
          </h2>
          <ul className="lista-prosta">
            {tydzien.nieaktualne_harmonogramy.map((n) => (
              <li key={n.zlecenie_id}>
                <Link to={`/kalendarz/zlecenie/${n.zlecenie_id}`}>{n.zlecenie_nazwa}</Link> – {n.powod}
              </li>
            ))}
          </ul>
        </section>
      )}

      <div className="siatka-pulpitu">
        <section className="karta sekcja-pulpitu">
          <h2>
            <OctagonAlert size={16} aria-hidden /> Alerty – czerwone i pomarańczowe ({alerty.length})
          </h2>
          {alerty.length === 0 ? (
            <p className="pusto">Brak pilnych terminów. </p>
          ) : (
            <ul className="lista-pulpitu">
              {alerty.map((z) => (
                <PozycjaZadania key={z.id} z={z} akcja={akcja(z)} />
              ))}
            </ul>
          )}
        </section>

        <section className="karta sekcja-pulpitu">
          <h2>
            <CircleAlert size={16} aria-hidden /> Zaległe ({zalegle.length})
          </h2>
          {zalegle.length === 0 ? (
            <p className="pusto">Nic nie jest po terminie.</p>
          ) : (
            <ul className="lista-pulpitu">
              {zalegle.map((z) => (
                <PozycjaZadania key={z.id} z={z} akcja={akcja(z)} />
              ))}
            </ul>
          )}
        </section>

        <section className="karta sekcja-pulpitu sekcja-pulpitu--szeroka">
          <h2>Do zrobienia w tym tygodniu {tydzien && <span className="tekst-drugorzedny">({nazwaTygodnia(tydzien.od)})</span>}</h2>
          <div className="tydzien-pulpitu">
            {tydzienDni.map(({ d, lista }, i) => (
              <div key={d} className={`dzien-pulpitu${d === dzis ? " dzien-pulpitu--dzis" : ""}${d < dzis ? " dzien-pulpitu--miniony" : ""}`}>
                <h3>
                  {DNI_TYGODNIA[i]} <span>{krotkaData(d)}</span>
                </h3>
                {lista.length === 0 ? (
                  <p className="pusto">—</p>
                ) : (
                  <ul>
                    {lista.map((z) => (
                      <li key={z.id} style={{ "--kolor": kolorTypu(z.typ) } as CSSProperties}>
                        <Link to={linkZadania(z)} title={`${nazwaTypu(z.typ)}: ${z.tytul} – ${z.stan.opis}`}>
                          <IkonaTypu typ={z.typ} rozmiar={12} />
                          <span>{z.tytul}</span>
                        </Link>
                      </li>
                    ))}
                  </ul>
                )}
              </div>
            ))}
          </div>
        </section>

        <section className="karta sekcja-pulpitu">
          <h2>
            <Wrench size={16} aria-hidden /> Kolizje warsztatu i montażu ({kolizje.length})
          </h2>
          {kolizje.length === 0 ? (
            <p className="pusto">Brak nakładających się terminów w najbliższych 4 miesiącach.</p>
          ) : (
            <ul className="lista-prosta">
              {kolizje.map((k, i) => (
                <li key={i} style={{ "--kolor": kolorTypu(k.typ) } as CSSProperties} className="kolizja">
                  <IkonaTypu typ={k.typ} rozmiar={13} />
                  <span>
                    <strong>{k.typ === "montaz" ? "Montaż" : "Warsztat"}</strong> {formatujDate(k.od)}–{formatujDate(k.do)} ({liczba(roznicaDni(k.od, k.do) + 1, DZIEN)}):{" "}
                    <Link to={`/kalendarz/zlecenie/${k.zlecenie_a}`}>{k.zlecenie_a_nazwa}</Link> i <Link to={`/kalendarz/zlecenie/${k.zlecenie_b}`}>{k.zlecenie_b_nazwa}</Link>
                    {k.harmonogram_nieaktualny && " (harmonogram nieaktualny)"}
                  </span>
                </li>
              ))}
            </ul>
          )}
        </section>

        <section className="karta sekcja-pulpitu">
          <h2>
            <Columns3 size={16} aria-hidden /> Etapy zleceń
          </h2>
          {zlecenia.length === 0 ? (
            <p className="pusto">Brak zleceń. Zlecenia powstają z podpisanych umów.</p>
          ) : (
            <div className="tablica-etapow">
              {ETAPY_TABLICY.map((e) => {
                const lista = zlecenia.filter((z) => z.etap === e);
                return (
                  <div key={e} className="kolumna-etapu">
                    <h3>
                      {NAZWY_ETAPOW[e]} <span>{lista.length}</span>
                    </h3>
                    <ul>
                      {lista.map((z) => (
                        <li key={z.id}>
                          <Link to={`/kalendarz/zlecenie/${z.id}`}>{z.nazwa}</Link>
                          <small>
                            {nazwaKlienta.get(z.klient_id) ?? ""}
                            {z.montaz_od ? ` · montaż ${krotkaData(z.montaz_od)}` : ""}
                          </small>
                          {z.harmonogram_nieaktualny && (
                            <small className="tekst-ostrzezenia">
                              <TriangleAlert size={10} aria-hidden /> harmonogram nieaktualny
                            </small>
                          )}
                        </li>
                      ))}
                    </ul>
                  </div>
                );
              })}
            </div>
          )}
        </section>
      </div>
      {oknoBlokady}
    </>
  );
}

export function Pulpit() {
  const { api, polaczenie } = usePolaczenie();
  const [status, ustawStatus] = useState<StatusSerwera | null>(null);
  const [blad, ustawBlad] = useState<string | null>(null);

  useEffect(() => {
    if (!api) return;
    api.status().then(ustawStatus, (e: Error) => ustawBlad(e.message));
  }, [api]);

  return (
    <div className="pulpit">
      <h1>Pulpit</h1>
      {polaczenie?.konfiguracja.tryb === "nieskonfigurowany" && <p>Pierwsze uruchomienie: wybierz tryb pracy tego komputera w module Ustawienia.</p>}
      {api ? <PulpitKalendarza /> : <p>Brak połączenia z serwerem – lista zadań pojawi się po połączeniu (Ustawienia).</p>}
      <p className="stopka-pulpitu">
        {status && `Połączono z serwerem „${status.nazwa}” (wersja ${status.wersja}, API v${status.wersja_api}).`}
        {blad && <span className="blad">{blad}</span>}
      </p>
    </div>
  );
}
