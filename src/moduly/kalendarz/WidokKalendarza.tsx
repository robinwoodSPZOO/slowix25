// Widok kalendarza: miesiąc / tydzień, filtry, legenda, panel szczegółów zadania, lista dnia.
// Stan widoku w adresie (?widok=&data=&zadanie=) – widżet i powiadomienia otwierają konkretne zadanie.

import { useCallback, useEffect, useMemo, useState, type CSSProperties } from "react";
import { Link, useSearchParams } from "react-router-dom";
import { CalendarDays, ChevronLeft, ChevronRight, CircleAlert, Diamond, Filter, Info, Plus, RefreshCw, Settings, TriangleAlert, X } from "lucide-react";
import type { ZadanieKalendarza } from "../../api/typy";
import { usePolaczenie } from "../../kontekst/Polaczenie";
import { wczytajKopie, zapiszKopie } from "../../lib/szkice";
import { Okno, Przycisk, Segment } from "../../ui";
import { komunikatBledu, useDaneKalendarza, useZleceniaIKlienci } from "./dane";
import { dodajDni, dodajMiesiace, dzisIso, nazwaMiesiaca, nazwaTygodnia, pelnaData, pierwszyDzienMiesiaca, poczatekTygodnia, poprawnaData, siatkaMiesiaca } from "./daty";
import { filtrujZadania, liczbaAktywnych, PUSTE_FILTRY, wgPilnosci, type Filtry } from "./filtry";
import { OknoZadania } from "./OknoZadania";
import { PanelZadania } from "./PanelZadania";
import { SiatkaKalendarza } from "./SiatkaKalendarza";
import { zakresZadania } from "./uklad";
import { EtykietaStanu, IkonaTypu, kolorTypu, nazwaTypu, TYPY_W_LEGENDZIE, ZnacznikTypu } from "./wyglad";

type Tryb = "miesiac" | "tydzien";
const KLUCZ_FILTROW = "kalendarz:filtry";

function Legenda() {
  return (
    <div className="legenda-kalendarza" aria-label="Legenda">
      <span className="legenda-kalendarza__grupa">
        {TYPY_W_LEGENDZIE.map((t) => (
          <ZnacznikTypu key={t} typ={t} />
        ))}
      </span>
      <span className="legenda-kalendarza__grupa">
        {(["zolty", "pomaranczowy", "czerwony"] as const).map((s) => (
          <EtykietaStanu key={s} stan={s} />
        ))}
      </span>
      <span className="legenda-kalendarza__okno" style={{ "--kolor": kolorTypu("zamowienie") } as CSSProperties}>
        <span className="legenda-kalendarza__pasek">
          <Diamond size={9} aria-hidden />
        </span>
        okno zamówienia: najlepiej zamówić → <Diamond size={10} aria-hidden /> data planowa → najpóźniej (krytyczna)
      </span>
    </div>
  );
}

function ListaDnia({ dzien, zadania, onWybierz, onZamknij, onDodaj }: { dzien: string; zadania: ZadanieKalendarza[]; onWybierz: (z: ZadanieKalendarza) => void; onZamknij: () => void; onDodaj: () => void }) {
  const lista = zadania
    .filter((z) => {
      const r = zakresZadania(z);
      return r.od <= dzien && dzien <= r.do;
    })
    .sort(wgPilnosci);
  return (
    <Okno
      tytul={pelnaData(dzien)}
      onZamknij={onZamknij}
      stopka={
        <Przycisk ikona={Plus} onClick={onDodaj}>
          Dodaj zadanie w tym dniu
        </Przycisk>
      }
    >
      {lista.length === 0 && <p>Brak zadań w tym dniu (przy obecnych filtrach).</p>}
      <ul className="lista-dnia">
        {lista.map((z) => (
          <li key={z.id}>
            <button type="button" className="lista-dnia__pozycja" style={{ "--kolor": kolorTypu(z.typ) } as CSSProperties} onClick={() => onWybierz(z)}>
              <span className="lista-dnia__ikona">
                <IkonaTypu typ={z.typ} />
              </span>
              <span className="lista-dnia__tekst">
                <strong>{z.tytul}</strong>
                <span>
                  {nazwaTypu(z.typ)} · {z.stan.opis}
                </span>
              </span>
              <EtykietaStanu stan={z.stan.stan} krotko />
            </button>
          </li>
        ))}
      </ul>
    </Okno>
  );
}

export function WidokKalendarza() {
  const { api } = usePolaczenie();
  const [parametry, ustawParametry] = useSearchParams();
  const dzis = dzisIso();
  const tryb: Tryb = parametry.get("widok") === "tydzien" ? "tydzien" : "miesiac";
  const dataParam = parametry.get("data");
  const kotwica = dataParam && poprawnaData(dataParam) ? dataParam : dzis;
  const wybraneId = Number(parametry.get("zadanie")) || null;

  const zmienParametry = useCallback(
    (z: Record<string, string | null>) => {
      ustawParametry(
        (p) => {
          const n = new URLSearchParams(p);
          for (const [k, v] of Object.entries(z)) if (v === null) n.delete(k);
          else n.set(k, v);
          return n;
        },
        { replace: true },
      );
    },
    [ustawParametry],
  );

  const { tygodnie, od, doDnia, miesiac, tytul } = useMemo(() => {
    if (tryb === "tydzien") {
      const p = poczatekTygodnia(kotwica);
      return { tygodnie: [p], od: p, doDnia: dodajDni(p, 6), miesiac: null, tytul: nazwaTygodnia(p) };
    }
    const s = siatkaMiesiaca(kotwica);
    return { tygodnie: s.tygodnie, od: s.od, doDnia: s.do, miesiac: pierwszyDzienMiesiaca(kotwica), tytul: nazwaMiesiaca(kotwica) };
  }, [tryb, kotwica]);

  const dane = useDaneKalendarza(od, doDnia);
  const { zlecenia, klienci } = useZleceniaIKlienci();
  const [filtry, ustawFiltry] = useState<Filtry>(() => ({ ...PUSTE_FILTRY, ...(wczytajKopie<Filtry>(KLUCZ_FILTROW) ?? {}) }));
  useEffect(() => zapiszKopie(KLUCZ_FILTROW, filtry), [filtry]);
  const [pokazFiltry, ustawPokazFiltry] = useState(false);
  const [fokus, ustawFokus] = useState(kotwica);
  const [dzienListy, ustawDzienListy] = useState<string | null>(null);
  const [oknoZadania, ustawOknoZadania] = useState<{ zadanie: ZadanieKalendarza | null; dzien: string } | null>(null);
  const [zadanieSpozaZakresu, ustawZadanieSpoza] = useState<ZadanieKalendarza | null>(null);
  const [bladZadania, ustawBladZadania] = useState<string | null>(null);

  const klientZlecenia = useMemo(() => new Map(zlecenia.map((z) => [z.id, z.klient_id])), [zlecenia]);
  const widoczne = useMemo(() => filtrujZadania(dane.zadania, filtry, klientZlecenia), [dane.zadania, filtry, klientZlecenia]);

  const wybrane = useMemo(() => {
    if (!wybraneId) return null;
    return dane.zadania.find((z) => z.id === wybraneId) ?? (zadanieSpozaZakresu?.id === wybraneId ? zadanieSpozaZakresu : null);
  }, [wybraneId, dane.zadania, zadanieSpozaZakresu]);

  // Zadanie z adresu (widżet, Pulpit, Zamówienia) spoza widocznego zakresu – pobieramy i przechodzimy do jego daty.
  useEffect(() => {
    if (!api || !wybraneId || dane.ladowanie) return;
    if (dane.zadania.some((z) => z.id === wybraneId) || zadanieSpozaZakresu?.id === wybraneId) return;
    api.kalendarz.zadanieKalendarza(wybraneId).then(
      (z) => {
        ustawZadanieSpoza(z);
        if (!dataParam) zmienParametry({ data: z.termin });
      },
      (e: unknown) => ustawBladZadania(komunikatBledu(e)),
    );
  }, [api, wybraneId, dane.ladowanie, dane.zadania, zadanieSpozaZakresu, dataParam, zmienParametry]);

  const przejdz = (d: string) => {
    zmienParametry({ data: d });
    ustawFokus(d);
  };
  const przesun = (k: number) => przejdz(tryb === "tydzien" ? dodajDni(kotwica, 7 * k) : dodajMiesiace(kotwica, k));
  const ustawFokusDnia = (d: string) => {
    ustawFokus(d);
    if (d < od || d > doDnia) zmienParametry({ data: d });
  };
  const wybierz = (z: ZadanieKalendarza) => {
    ustawDzienListy(null);
    zmienParametry({ zadanie: String(z.id) });
  };
  const zamknijPanel = useCallback(() => zmienParametry({ zadanie: null }), [zmienParametry]);

  // Skróty: PageUp/PageDown – poprzedni/następny okres, T – dziś (poza polami formularzy).
  useEffect(() => {
    const k = (e: KeyboardEvent) => {
      const cel = e.target as HTMLElement;
      if (cel.closest("input, select, textarea, .okno-tlo")) return;
      if (e.key === "PageUp") przesun(-1);
      else if (e.key === "PageDown") przesun(1);
      else return;
      e.preventDefault();
    };
    window.addEventListener("keydown", k);
    return () => window.removeEventListener("keydown", k);
  });

  const aktywneFiltry = liczbaAktywnych(filtry);
  const przelaczTyp = (t: string) =>
    ustawFiltry((f) => ({ ...f, typy: f.typy.includes(t) ? f.typy.filter((x) => x !== t) : [...f.typy, t] }));
  const nieaktualne = useMemo(() => {
    const m = new Map<number, string>();
    for (const z of dane.zadania) if (z.harmonogram_nieaktualny && z.zlecenie_id) m.set(z.zlecenie_id, z.zlecenie_nazwa ?? `Zlecenie ${z.zlecenie_id}`);
    return [...m];
  }, [dane.zadania]);

  return (
    <div className={`kalendarz${wybrane ? " kalendarz--z-panelem" : ""}`}>
      <header className="kalendarz__naglowek">
        <h1>Kalendarz</h1>
        <div className="kalendarz__nawigacja" role="group" aria-label="Nawigacja po kalendarzu">
          <Przycisk tylkoIkona ikona={ChevronLeft} etykieta={tryb === "tydzien" ? "Poprzedni tydzień (PageUp)" : "Poprzedni miesiąc (PageUp)"} onClick={() => przesun(-1)} />
          <Przycisk onClick={() => przejdz(dzis)}>Dziś</Przycisk>
          <Przycisk tylkoIkona ikona={ChevronRight} etykieta={tryb === "tydzien" ? "Następny tydzień (PageDown)" : "Następny miesiąc (PageDown)"} onClick={() => przesun(1)} />
          <h2 className="kalendarz__okres" aria-live="polite">
            {tytul}
          </h2>
          {dane.ladowanie && <RefreshCw className="obrot" size={14} aria-label="Wczytywanie" />}
        </div>
        <div className="kalendarz__akcje">
          <Segment<Tryb>
            etykieta="Widok"
            wartosc={tryb}
            onZmiana={(w) => zmienParametry({ widok: w === "miesiac" ? null : w, data: fokus })}
            opcje={[
              { wartosc: "miesiac", etykieta: "Miesiąc" },
              { wartosc: "tydzien", etykieta: "Tydzień" },
            ]}
          />
          <Przycisk ikona={Filter} onClick={() => ustawPokazFiltry((p) => !p)} aria-expanded={pokazFiltry} aria-controls="filtry-kalendarza">
            Filtry{aktywneFiltry ? ` (${aktywneFiltry})` : ""}
          </Przycisk>
          <Przycisk wariant="glowny" ikona={Plus} onClick={() => ustawOknoZadania({ zadanie: null, dzien: fokus })}>
            Dodaj zadanie
          </Przycisk>
          <Link className="przycisk przycisk--tekstowy" to="/kalendarz/ustawienia">
            <Settings size={16} aria-hidden />
            Ustawienia
          </Link>
        </div>
      </header>

      {pokazFiltry && (
        <div className="filtry-kalendarza" id="filtry-kalendarza">
          <div className="filtry-kalendarza__typy" role="group" aria-label="Typ zadania">
            {TYPY_W_LEGENDZIE.map((t) => {
              const aktywny = filtry.typy.length === 0 || filtry.typy.includes(t);
              return (
                <button
                  key={t}
                  type="button"
                  className={`przelacznik-typu${aktywny ? " przelacznik-typu--wl" : ""}`}
                  aria-pressed={filtry.typy.includes(t)}
                  style={{ "--kolor": kolorTypu(t) } as CSSProperties}
                  onClick={() => przelaczTyp(t)}
                >
                  <IkonaTypu typ={t} rozmiar={12} />
                  {nazwaTypu(t)}
                </button>
              );
            })}
          </div>
          <label className="filtry-kalendarza__pole">
            Zlecenie
            <select className="wejscie wejscie--maly" value={filtry.zlecenieId ?? ""} onChange={(e) => ustawFiltry((f) => ({ ...f, zlecenieId: e.target.value ? Number(e.target.value) : null }))}>
              <option value="">wszystkie</option>
              {zlecenia.map((z) => (
                <option key={z.id} value={z.id}>
                  {z.nazwa}
                </option>
              ))}
            </select>
          </label>
          <label className="filtry-kalendarza__pole">
            Klient
            <select className="wejscie wejscie--maly" value={filtry.klientId ?? ""} onChange={(e) => ustawFiltry((f) => ({ ...f, klientId: e.target.value ? Number(e.target.value) : null }))}>
              <option value="">wszyscy</option>
              {klienci.map((k) => (
                <option key={k.id} value={k.id}>
                  {k.nazwa}
                </option>
              ))}
            </select>
          </label>
          <label className="filtry-kalendarza__pole filtry-kalendarza__pole--check">
            <input type="checkbox" checked={filtry.tylkoNiewykonane} onChange={(e) => ustawFiltry((f) => ({ ...f, tylkoNiewykonane: e.target.checked }))} />
            tylko niewykonane
          </label>
          <label className="filtry-kalendarza__pole filtry-kalendarza__pole--check">
            <input type="checkbox" checked={filtry.tylkoZOstrzezeniem} onChange={(e) => ustawFiltry((f) => ({ ...f, tylkoZOstrzezeniem: e.target.checked }))} />
            tylko z ostrzeżeniem
          </label>
          {aktywneFiltry > 0 && (
            <Przycisk wariant="tekstowy" maly ikona={X} onClick={() => ustawFiltry(PUSTE_FILTRY)}>
              Wyczyść filtry
            </Przycisk>
          )}
        </div>
      )}

      {dane.blad && (
        <div className="komunikat-stanu stan-tlo--czerwony" role="alert">
          <CircleAlert size={16} aria-hidden />
          <span>
            Nie udało się wczytać kalendarza: {dane.blad}{" "}
            <button type="button" className="link" onClick={dane.odswiez}>
              Spróbuj ponownie
            </button>
          </span>
        </div>
      )}
      {bladZadania && (
        <div className="komunikat-stanu stan-tlo--zolty" role="alert">
          <Info size={16} aria-hidden />
          <span>Nie można otworzyć zadania: {bladZadania}</span>
        </div>
      )}
      {nieaktualne.length > 0 && (
        <div className="komunikat-stanu stan-tlo--pomaranczowy" role="note">
          <TriangleAlert size={16} aria-hidden />
          <span>
            Harmonogram nieaktualny – terminy mogą być błędne:{" "}
            {nieaktualne.map(([id, n], i) => (
              <span key={id}>
                {i > 0 && ", "}
                <Link to={`/kalendarz/zlecenie/${id}`}>{n}</Link>
              </span>
            ))}
          </span>
        </div>
      )}
      {!dane.ladowanie && !dane.blad && dane.zadania.length === 0 && (
        <div className="komunikat-stanu stan-tlo--neutralny pusty-kalendarz">
          <CalendarDays size={18} aria-hidden />
          <span>
            <strong>Brak terminów w tym okresie.</strong> Terminy powstają same z umów: po wpisaniu w zleceniu terminu montażu i czasu
            pracy na warsztacie program liczy wstecz pomiar, rysunki, zamówienia materiałów (wg czasów dostaw z{" "}
            <Link to="/kalendarz/ustawienia">ustawień kalendarza</Link>), składanie i montaż. Własne zadania dodasz przyciskiem „Dodaj
            zadanie”.
          </span>
        </div>
      )}
      {!dane.ladowanie && dane.zadania.length > 0 && widoczne.length === 0 && (
        <div className="komunikat-stanu stan-tlo--neutralny">
          <Filter size={16} aria-hidden />
          <span>
            Żadne z {dane.zadania.length} zadań nie pasuje do filtrów.{" "}
            <button type="button" className="link" onClick={() => ustawFiltry(PUSTE_FILTRY)}>
              Wyczyść filtry
            </button>
          </span>
        </div>
      )}

      <div className="kalendarz__glowna">
        <SiatkaKalendarza
          tygodnie={tygodnie}
          tryb={tryb}
          miesiac={miesiac}
          zadania={widoczne}
          dzis={dzis}
          swieta={dane.swieta}
          dniWolne={dane.dniWolne}
          wybraneId={wybraneId}
          fokus={fokus}
          onFokus={ustawFokusDnia}
          onOtworzDzien={ustawDzienListy}
          onWybierz={wybierz}
          etykieta={`Kalendarz – ${tytul}. Strzałki – zmiana dnia, Enter – lista zadań dnia.`}
        />
        {wybrane && (
          <PanelZadania
            zadanie={wybrane}
            dzis={dzis}
            onZamknij={zamknijPanel}
            onZmiana={(z) => {
              if (!z) zamknijPanel();
              else if (zadanieSpozaZakresu?.id === z.id) ustawZadanieSpoza(z);
              dane.odswiez();
            }}
            onEdytuj={(z) => ustawOknoZadania({ zadanie: z, dzien: z.data_od })}
          />
        )}
      </div>
      <Legenda />
      <p className="sr-only" aria-live="polite">
        {dane.ladowanie ? "Wczytywanie kalendarza" : `Widoczne zadania: ${widoczne.length}`}
      </p>

      {dzienListy && (
        <ListaDnia
          dzien={dzienListy}
          zadania={widoczne}
          onWybierz={wybierz}
          onZamknij={() => ustawDzienListy(null)}
          onDodaj={() => {
            ustawOknoZadania({ zadanie: null, dzien: dzienListy });
            ustawDzienListy(null);
          }}
        />
      )}
      {oknoZadania && (
        <OknoZadania
          zadanie={oknoZadania.zadanie}
          dzien={oknoZadania.dzien}
          zlecenia={zlecenia}
          onZamknij={() => ustawOknoZadania(null)}
          onZapisano={(z) => {
            ustawOknoZadania(null);
            dane.odswiez();
            zmienParametry({ zadanie: String(z.id) });
          }}
        />
      )}
    </div>
  );
}
