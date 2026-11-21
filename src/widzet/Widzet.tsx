// Okno-widżet na pulpicie (ok. 360×520, bez ramki): zaległe, dziś i 7 dni w kolorach zadań, liczniki
// stanów, stan połączenia. Odświeżanie: zdarzenia WebSocket + co 5 minut. Bez serwera – ostatnio
// pobrane dane (pamięć lokalna) tylko do odczytu, z godziną synchronizacji.

import { useCallback, useEffect, useMemo, useRef, useState, type CSSProperties } from "react";
import { EyeOff, Lock, RefreshCw, SquareArrowOutUpRight, TriangleAlert, Wifi, WifiOff } from "lucide-react";
import type { ZadanieKalendarza } from "../api/typy";
import { usePolaczenie } from "../kontekst/Polaczenie";
import { otworzWGlownymOknie, ukryjWidzet } from "../lib/tauri";
import { komunikatBledu, useOdswiezaniePoZdarzeniu } from "../moduly/kalendarz/dane";
import { dodajDni, dzisIso, pelnaData } from "../moduly/kalendarz/daty";
import { licznikiStanow } from "../moduly/kalendarz/filtry";
import { IkonaStanu, IkonaTypu, kolorTypu, nazwaTypu, STANY } from "../moduly/kalendarz/wyglad";
import { ukladWidzetu, wczytajPamiec, zapiszPamiec, type PamiecWidzetu } from "./logika";
import "../moduly/kalendarz/kalendarz.css";
import "./widzet.css";

const CO_ILE_MS = 5 * 60 * 1000;
const MAKS_ZALEGLYCH = 4;

function godzina(iso: string): string {
  const d = new Date(iso);
  return d.toLocaleString("pl-PL", { hour: "2-digit", minute: "2-digit", day: "2-digit", month: "2-digit" });
}

function Pozycja({ z }: { z: ZadanieKalendarza }) {
  return (
    <li>
      <button
        type="button"
        className={`pozycja-widzetu pozycja-widzetu--${z.stan.stan}`}
        style={{ "--kolor": kolorTypu(z.typ) } as CSSProperties}
        onClick={() => void otworzWGlownymOknie(`/kalendarz?zadanie=${z.id}&data=${z.termin}`)}
        title={`${nazwaTypu(z.typ)}: ${z.tytul}\n${z.stan.opis}`}
      >
        <span className="pozycja-widzetu__ikona">
          <IkonaTypu typ={z.typ} rozmiar={12} />
        </span>
        <span className="pozycja-widzetu__tekst">
          <span className="pozycja-widzetu__tytul">{z.tytul}</span>
          <span className="pozycja-widzetu__opis">{z.stan.opis}</span>
        </span>
        {z.zamowienie?.zablokowane && <Lock size={11} aria-label="czeka na potwierdzenie klienta" />}
        {z.stan.stan !== "spokojnie" && <IkonaStanu stan={z.stan.stan} rozmiar={13} />}
      </button>
    </li>
  );
}

export function Widzet() {
  const { api, naZywo } = usePolaczenie();
  const [pamiec, ustawPamiec] = useState<PamiecWidzetu | null>(() => wczytajPamiec());
  const [blad, ustawBlad] = useState<string | null>(null);
  const [odswiezanie, ustawOdswiezanie] = useState(false);
  const [dzis, ustawDzis] = useState(dzisIso());
  const trwa = useRef(false);
  const [wszystkieZalegle, ustawWszystkieZalegle] = useState(false);

  useEffect(() => {
    document.body.classList.add("cialo-widzetu");
    return () => document.body.classList.remove("cialo-widzetu");
  }, []);

  const pobierz = useCallback(async () => {
    if (!api || trwa.current) return;
    trwa.current = true;
    ustawOdswiezanie(true);
    const d = dzisIso();
    ustawDzis(d);
    try {
      const [tydzien, alerty] = await Promise.all([
        api.kalendarz.zadania({ od: d, do: dodajDni(d, 7), tylko_niewykonane: true }),
        api.kalendarz.zadania({ stan: "czerwony,pomaranczowy", tylko_niewykonane: true }),
      ]);
      const mapa = new Map<number, ZadanieKalendarza>();
      for (const z of [...alerty, ...tydzien]) mapa.set(z.id, z);
      const p = { czas: new Date().toISOString(), dzis: d, zadania: [...mapa.values()] };
      zapiszPamiec(p);
      ustawPamiec(p);
      ustawBlad(null);
    } catch (e) {
      ustawBlad(komunikatBledu(e));
    } finally {
      trwa.current = false;
      ustawOdswiezanie(false);
    }
  }, [api]);

  useEffect(() => {
    void Promise.resolve().then(pobierz);
    const t = setInterval(() => void pobierz(), CO_ILE_MS);
    const naFokus = () => void pobierz();
    window.addEventListener("focus", naFokus);
    return () => {
      clearInterval(t);
      window.removeEventListener("focus", naFokus);
    };
  }, [pobierz]);
  useOdswiezaniePoZdarzeniu(() => void pobierz());
  // Po ponownym połączeniu gniazda zdarzeń – od razu świeże dane.
  useEffect(() => {
    if (naZywo) void Promise.resolve().then(pobierz);
  }, [naZywo, pobierz]);

  const offline = !api || !!blad;
  const uklad = useMemo(() => (pamiec ? ukladWidzetu(pamiec.zadania, dzis) : null), [pamiec, dzis]);
  const liczniki = licznikiStanow(pamiec?.zadania ?? []);
  const nieaktualnaData = pamiec && pamiec.dzis !== dzis;

  return (
    <div className={`widzet${offline ? " widzet--offline" : ""}`}>
      <header className="widzet__naglowek" data-tauri-drag-region>
        <div className="widzet__tytul" data-tauri-drag-region>
          <strong data-tauri-drag-region>ROBINWOOD</strong>
          <span data-tauri-drag-region>{pelnaData(dzis)}</span>
        </div>
        <span className={`widzet__polaczenie${offline ? " widzet__polaczenie--brak" : ""}`} title={offline ? "Brak połączenia z serwerem" : "Połączono z serwerem"}>
          {offline ? <WifiOff size={14} aria-hidden /> : <Wifi size={14} aria-hidden />}
          <span className="sr-only">{offline ? "Brak połączenia z serwerem" : "Połączono z serwerem"}</span>
        </span>
        <button type="button" className="widzet__przycisk" onClick={() => void pobierz()} title="Odśwież" aria-label="Odśwież">
          <RefreshCw size={14} className={odswiezanie ? "obrot" : undefined} aria-hidden />
        </button>
        <button type="button" className="widzet__przycisk" onClick={() => void ukryjWidzet()} title="Ukryj widżet (przywrócisz z ikony w zasobniku)" aria-label="Ukryj widżet">
          <EyeOff size={14} aria-hidden />
        </button>
      </header>

      <div className="widzet__liczniki" aria-label="Pozycje wymagające uwagi">
        {(["czerwony", "pomaranczowy", "zolty"] as const).map((s) => {
          const I = STANY[s].ikona;
          return (
            <span key={s} className={`widzet__licznik stan-tlo--${s}`} title={STANY[s].tekst}>
              <I size={13} aria-hidden />
              <strong>{liczniki[s]}</strong>
              <span>{STANY[s].krotko}</span>
            </span>
          );
        })}
      </div>

      {offline && (
        <div className="widzet__offline" role="status">
          <TriangleAlert size={13} aria-hidden />
          <span>
            {!api ? "Program nie jest połączony z serwerem." : "Brak połączenia z serwerem."}{" "}
            {pamiec ? `Dane z ${godzina(pamiec.czas)} – tylko do odczytu.` : "Brak zapisanych danych."}
          </span>
        </div>
      )}
      {nieaktualnaData && !offline && <div className="widzet__offline">Dane z poprzedniego dnia – odświeżanie…</div>}

      <div className="widzet__lista">
        {uklad && uklad.zalegle.length > 0 && (
          <section>
            <h2 className="widzet__dzien widzet__dzien--zalegle">Zaległe i krytyczne ({uklad.zalegle.length})</h2>
            <ul>
              {(wszystkieZalegle ? uklad.zalegle : uklad.zalegle.slice(0, MAKS_ZALEGLYCH)).map((z) => (
                <Pozycja key={z.id} z={z} />
              ))}
            </ul>
            {uklad.zalegle.length > MAKS_ZALEGLYCH && (
              <button type="button" className="widzet__wiecej" aria-expanded={wszystkieZalegle} onClick={() => ustawWszystkieZalegle((w) => !w)}>
                {wszystkieZalegle ? "Zwiń" : `Pokaż wszystkie zaległe (+${uklad.zalegle.length - MAKS_ZALEGLYCH})`}
              </button>
            )}
          </section>
        )}
        {uklad?.dni.map((d) => (
          <section key={d.data}>
            <h2 className="widzet__dzien">{d.etykieta}</h2>
            {d.zadania.length === 0 ? (
              <p className="widzet__pusto">Brak zadań na dziś.</p>
            ) : (
              <ul>
                {d.zadania.map((z) => (
                  <Pozycja key={z.id} z={z} />
                ))}
              </ul>
            )}
          </section>
        ))}
        {!uklad && <p className="widzet__pusto">{offline ? "Połącz program z serwerem w Ustawieniach." : "Wczytywanie…"}</p>}
        {uklad && uklad.dni.length === 1 && uklad.dni[0]!.zadania.length === 0 && uklad.zalegle.length === 0 && (
          <p className="widzet__pusto">W najbliższych 7 dniach nie ma terminów.</p>
        )}
      </div>

      <footer className="widzet__stopka">
        <span>{pamiec ? `Synchronizacja: ${godzina(pamiec.czas)}` : ""}</span>
        <button type="button" className="widzet__otworz" onClick={() => void otworzWGlownymOknie("/kalendarz")}>
          <SquareArrowOutUpRight size={13} aria-hidden /> Otwórz kalendarz
        </button>
      </footer>
    </div>
  );
}
