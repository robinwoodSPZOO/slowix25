// Moduł Zamówienia: „do zamówienia” pogrupowane wg dostawcy (okno sweet spot / planowa / krytyczna,
// stan, szybkie „zamówione dziś”) oraz „zamówione – czekamy na dostawę” (dostawa zagrożona).

import { useCallback, useEffect, useMemo, useState, type CSSProperties } from "react";
import { Link, useSearchParams } from "react-router-dom";
import { CalendarClock, CircleAlert, Lock, PackageCheck, Settings, ShoppingCart, TriangleAlert, Truck } from "lucide-react";
import type { DoZamowienia, StanOstrzezenia, ZadanieKalendarza } from "../../api/typy";
import { usePolaczenie } from "../../kontekst/Polaczenie";
import { formatujDate } from "../../lib/formaty";
import { Przycisk, Segment } from "../../ui";
import { komunikatBledu, useOdswiezaniePoZdarzeniu } from "../kalendarz/dane";
import { dzisIso, wzgledny } from "../kalendarz/daty";
import { PanelZadania, useWykonanie } from "../kalendarz/PanelZadania";
import { EtykietaStanu, kolorTypu, STANY } from "../kalendarz/wyglad";
import "../kalendarz/kalendarz.css";
import "./zamowienia.css";

type Zakladka = "do_zamowienia" | "zamowione";
const FILTRY_STANU: (StanOstrzezenia | "wszystkie")[] = ["wszystkie", "czerwony", "pomaranczowy", "zolty", "spokojnie"];

function Material({ z }: { z: ZadanieKalendarza }) {
  return (
    <>
      <strong>{z.zamowienie?.kategoria_nazwa ?? z.tytul}</strong>
      {z.zamowienie?.opis && <div className="tekst-drugorzedny">{z.zamowienie.opis}</div>}
      {z.zamowienie?.zablokowane && !z.wykonano && (
        <div className="znak-blokady" title={`Klient musi potwierdzić pozycje do ${formatujDate(z.zamowienie.potwierdzic_do)}`}>
          <Lock size={11} aria-hidden /> czeka na potwierdzenie klienta{z.zamowienie.potwierdzic_do ? ` do ${formatujDate(z.zamowienie.potwierdzic_do)}` : ""}
        </div>
      )}
    </>
  );
}

function Zlecenie({ z }: { z: ZadanieKalendarza }) {
  return z.zlecenie_id ? <Link to={`/kalendarz/zlecenie/${z.zlecenie_id}`}>{z.zlecenie_nazwa}</Link> : <>—</>;
}

export function Zamowienia() {
  const { api } = usePolaczenie();
  const dzis = dzisIso();
  const [parametry, ustawParametry] = useSearchParams();
  const zakladka: Zakladka = parametry.get("widok") === "zamowione" ? "zamowione" : "do_zamowienia";
  const wybraneId = Number(parametry.get("zadanie")) || null;
  const [filtr, ustawFiltr] = useState<StanOstrzezenia | "wszystkie">("wszystkie");
  const [dane, ustawDane] = useState<DoZamowienia | null>(null);
  const [zamowione, ustawZamowione] = useState<ZadanieKalendarza[] | null>(null);
  const [blad, ustawBlad] = useState<string | null>(null);
  const [komunikat, ustawKomunikat] = useState<string | null>(null);

  const pobierz = useCallback(() => {
    if (!api) return;
    Promise.all([api.kalendarz.doZamowienia(), api.kalendarz.zadania({ typ: "zamowienie" })]).then(
      ([d, z]) => {
        ustawDane(d);
        ustawZamowione(z.filter((x) => x.wykonano && x.zamowienie?.status === "zamowione"));
        ustawBlad(null);
      },
      (e: unknown) => ustawBlad(komunikatBledu(e)),
    );
  }, [api]);
  useEffect(pobierz, [pobierz]);
  useOdswiezaniePoZdarzeniu(pobierz);

  const zmien = (p: Record<string, string | null>) =>
    ustawParametry((s) => {
      const n = new URLSearchParams(s);
      for (const [k, v] of Object.entries(p)) if (v === null) n.delete(k);
      else n.set(k, v);
      return n;
    });

  const { wykonaj, oknoBlokady, zapisywanie, blad: bladWykonania } = useWykonanie((z) => {
    ustawKomunikat(z.zamowienie?.status === "dostarczone" ? `Zapisano dostawę: ${z.zamowienie.kategoria_nazwa}.` : `Zapisano zamówienie: ${z.zamowienie?.kategoria_nazwa ?? z.tytul}.`);
    pobierz();
  });

  const wszystkie = useMemo(() => [...(dane?.grupy.flatMap((g) => g.pozycje) ?? []), ...(zamowione ?? [])], [dane, zamowione]);
  const wybrane = wszystkie.find((z) => z.id === wybraneId) ?? null;
  const grupy = useMemo(
    () =>
      (dane?.grupy ?? [])
        .map((g) => ({ ...g, pozycje: g.pozycje.filter((p) => filtr === "wszystkie" || p.stan.stan === filtr) }))
        .filter((g) => g.pozycje.length > 0),
    [dane, filtr],
  );
  const licznik = (s: StanOstrzezenia | "wszystkie") => dane?.grupy.flatMap((g) => g.pozycje).filter((p) => s === "wszystkie" || p.stan.stan === s).length ?? 0;
  const zagrozone = zamowione?.filter((z) => z.zamowienie?.dostawa_zagrozona).length ?? 0;
  const posortowaneZamowione = useMemo(
    () =>
      [...(zamowione ?? [])].sort(
        (a, b) =>
          Number(b.zamowienie?.dostawa_zagrozona) - Number(a.zamowienie?.dostawa_zagrozona) ||
          (a.zamowienie?.oczekiwana_dostawa ?? "").localeCompare(b.zamowienie?.oczekiwana_dostawa ?? ""),
      ),
    [zamowione],
  );

  return (
    <div className={`zamowienia${wybrane ? " zamowienia--z-panelem" : ""}`}>
      <header className="naglowek-modulu">
        <h1>Zamówienia materiałów</h1>
        <div className="naglowek-modulu__akcje">
          <Segment<Zakladka>
            etykieta="Lista"
            wartosc={zakladka}
            onZmiana={(w) => zmien({ widok: w === "zamowione" ? "zamowione" : null, zadanie: null })}
            opcje={[
              { wartosc: "do_zamowienia", etykieta: `Do zamówienia (${licznik("wszystkie")})`, ikona: ShoppingCart },
              { wartosc: "zamowione", etykieta: `Zamówione – czekamy na dostawę (${zamowione?.length ?? 0})`, ikona: Truck },
            ]}
          />
          <Link className="przycisk przycisk--tekstowy" to="/kalendarz/ustawienia#czasy-dostaw">
            <Settings size={16} aria-hidden /> Czasy dostaw
          </Link>
        </div>
      </header>

      {blad && (
        <div className="komunikat-stanu stan-tlo--czerwony" role="alert">
          <CircleAlert size={16} aria-hidden />
          <span>
            Nie udało się wczytać zamówień: {blad}{" "}
            <button type="button" className="link" onClick={pobierz}>
              Spróbuj ponownie
            </button>
          </span>
        </div>
      )}
      {(bladWykonania || komunikat) && (
        <div className={`komunikat-stanu stan-tlo--${bladWykonania ? "czerwony" : "spokojnie"}`} role="status">
          {bladWykonania ? <CircleAlert size={14} aria-hidden /> : <PackageCheck size={14} aria-hidden />}
          <span>{bladWykonania ?? komunikat}</span>
        </div>
      )}
      {dane && dane.nieaktualne_harmonogramy.length > 0 && (
        <div className="komunikat-stanu stan-tlo--pomaranczowy" role="note">
          <TriangleAlert size={16} aria-hidden />
          <span>
            Harmonogram nieaktualny (terminy zamówień mogą być błędne):{" "}
            {dane.nieaktualne_harmonogramy.map((n, i) => (
              <span key={n.zlecenie_id}>
                {i > 0 && "; "}
                <Link to={`/kalendarz/zlecenie/${n.zlecenie_id}`}>{n.zlecenie_nazwa}</Link> – {n.powod}
              </span>
            ))}
          </span>
        </div>
      )}

      <div className="zamowienia__uklad">
        <div className="zamowienia__lista">
          {zakladka === "do_zamowienia" && (
            <>
              <div className="filtr-stanow" role="group" aria-label="Filtr stanu">
                {FILTRY_STANU.map((s) => (
                  <button key={s} type="button" className="filtr-stanow__opcja" aria-pressed={filtr === s} onClick={() => ustawFiltr(s)}>
                    {s === "wszystkie" ? "Wszystkie" : STANY[s].tekst} ({licznik(s)})
                  </button>
                ))}
              </div>
              {dane === null && !blad && <p>Wczytywanie…</p>}
              {dane && grupy.length === 0 && (
                <div className="komunikat-stanu stan-tlo--neutralny">
                  <ShoppingCart size={16} aria-hidden />
                  <span>
                    {licznik("wszystkie") === 0
                      ? "Nie ma materiałów do zamówienia. Pozycje pojawiają się, gdy zlecenie ma materiały i policzony harmonogram (oś czasu zlecenia)."
                      : "Żadna pozycja nie ma wybranego stanu."}
                  </span>
                </div>
              )}
              {grupy.map((g) => (
                <section key={g.dostawca_id ?? "brak"} className="karta grupa-dostawcy" aria-label={`Dostawca: ${g.dostawca_nazwa ?? "bez dostawcy"}`}>
                  <header className="grupa-dostawcy__naglowek">
                    <h2>{g.dostawca_nazwa ?? "Bez dostawcy"}</h2>
                    <span className="tekst-drugorzedny">{g.pozycje.length} poz.</span>
                    <EtykietaStanu stan={g.stan} />
                  </header>
                  <table className="tabela tabela-zamowien">
                    <thead>
                      <tr>
                        <th>Materiał</th>
                        <th>Zlecenie</th>
                        <th className="kol-daty">Najlepiej zamówić</th>
                        <th className="kol-daty">Planowo</th>
                        <th className="kol-daty">Najpóźniej</th>
                        <th className="kol-stanu">Stan</th>
                        <th className="kol-akcji">
                          <span className="sr-only">Akcje</span>
                        </th>
                      </tr>
                    </thead>
                    <tbody>
                      {g.pozycje.map((z) => (
                        <tr key={z.id} className={z.id === wybraneId ? "wiersz-tabeli--zaznaczony" : ""} style={{ "--kolor": kolorTypu("zamowienie") } as CSSProperties}>
                          <td>
                            <Material z={z} />
                          </td>
                          <td>
                            <Zlecenie z={z} />
                          </td>
                          <td>{formatujDate(z.data_sweet_spot)}</td>
                          <td>
                            <strong>{formatujDate(z.data_planowa)}</strong>
                          </td>
                          <td>{formatujDate(z.data_krytyczna)}</td>
                          <td className="kolumna-stanu">
                            <EtykietaStanu stan={z.stan.stan} krotko />
                            <div className="tekst-drugorzedny">{z.stan.stan === "czerwony" ? `${wzgledny(z.stan.dni_do_terminu)}` : wzgledny(z.stan.dni_do_terminu)}</div>
                            {z.stan.nowy_termin_montazu && (
                              <div className="nowy-termin">
                                <CalendarClock size={11} aria-hidden /> montaż najwcześniej {formatujDate(z.stan.nowy_termin_montazu)}
                              </div>
                            )}
                          </td>
                          <td>
<div className="akcje-wiersza">
                            <Przycisk maly wariant="glowny" ikona={ShoppingCart} disabled={zapisywanie} onClick={() => void wykonaj(z, { wykonano: dzis, data_zamowienia: dzis })}>
                              Zamówione dziś
                            </Przycisk>
                            <Przycisk maly wariant="tekstowy" onClick={() => zmien({ zadanie: String(z.id) })}>
                              Szczegóły
                            </Przycisk>
                          </div>
</td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </section>
              ))}
              {dane && dane.bez_terminow.length > 0 && (
                <section className="karta grupa-dostawcy">
                  <header className="grupa-dostawcy__naglowek">
                    <h2>Materiały bez czasu dostawy</h2>
                    <span className="tekst-drugorzedny">nie mają terminów zamówienia</span>
                  </header>
                  <ul className="lista-bez-terminow">
                    {dane.bez_terminow.map((m) => (
                      <li key={m.id}>
                        <strong>{m.kategoria_nazwa}</strong>
                        {m.opis ? ` – ${m.opis}` : ""} · <Link to={`/kalendarz/zlecenie/${m.zlecenie_id}`}>zlecenie</Link>
                      </li>
                    ))}
                  </ul>
                  <p className="pole__pomoc">
                    <Link to="/kalendarz/ustawienia#czasy-dostaw">Ustaw czasy dostaw tych kategorii</Link>, a terminy policzą się same.
                  </p>
                </section>
              )}
            </>
          )}

          {zakladka === "zamowione" && (
            <>
              {zagrozone > 0 && (
                <div className="komunikat-stanu stan-tlo--czerwony" role="note">
                  <TriangleAlert size={16} aria-hidden />
                  <span>
                    {zagrozone} {zagrozone === 1 ? "dostawa może nie zdążyć" : "dostaw może nie zdążyć"} przed składaniem na warsztacie – skontaktuj się z dostawcą.
                  </span>
                </div>
              )}
              {zamowione && zamowione.length === 0 && (
                <div className="komunikat-stanu stan-tlo--neutralny">
                  <Truck size={16} aria-hidden />
                  <span>Nie czekamy na żadną dostawę.</span>
                </div>
              )}
              {zamowione && zamowione.length > 0 && (
                <div className="karta">
                  <table className="tabela tabela-zamowien">
                    <thead>
                      <tr>
                        <th>Materiał</th>
                        <th>Zlecenie</th>
                        <th>Dostawca</th>
                        <th className="kol-daty">Zamówiono</th>
                        <th className="kol-daty">Oczekiwana dostawa</th>
                        <th className="kol-daty">Potrzebne do</th>
                        <th className="kol-stanu">Stan dostawy</th>
                        <th className="kol-akcji">
                          <span className="sr-only">Akcje</span>
                        </th>
                      </tr>
                    </thead>
                    <tbody>
                      {posortowaneZamowione.map((z) => (
                        <tr key={z.id} className={z.id === wybraneId ? "wiersz-tabeli--zaznaczony" : ""}>
                          <td>
                            <Material z={z} />
                          </td>
                          <td>
                            <Zlecenie z={z} />
                          </td>
                          <td>{z.zamowienie?.dostawca_nazwa ?? "—"}</td>
                          <td>{formatujDate(z.zamowienie?.data_zamowienia)}</td>
                          <td>{formatujDate(z.zamowienie?.oczekiwana_dostawa)}</td>
                          <td>{formatujDate(z.zamowienie?.data_wymaganej_dostawy)}</td>
                          <td>
                            {z.zamowienie?.dostawa_zagrozona ? (
                              <EtykietaStanu stan="czerwony" title="Oczekiwana dostawa później niż potrzebna na warsztacie" />
                            ) : (
                              <EtykietaStanu stan="spokojnie" />
                            )}
                            {z.zamowienie?.dostawa_zagrozona && <div className="tekst-drugorzedny">dostawa zagrożona</div>}
                          </td>
                          <td>
<div className="akcje-wiersza">
                            <Przycisk maly wariant="glowny" ikona={PackageCheck} disabled={zapisywanie} onClick={() => void wykonaj(z, { wykonano: z.wykonano, data_dostawy: dzis })}>
                              Dostarczone dziś
                            </Przycisk>
                            <Przycisk maly wariant="tekstowy" onClick={() => zmien({ zadanie: String(z.id) })}>
                              Inna data…
                            </Przycisk>
                          </div>
</td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              )}
            </>
          )}
        </div>
        {wybrane && (
          <PanelZadania
            zadanie={wybrane}
            dzis={dzis}
            onZamknij={() => zmien({ zadanie: null })}
            onZmiana={() => {
              pobierz();
            }}
          />
        )}
      </div>
      {oknoBlokady}
    </div>
  );
}
