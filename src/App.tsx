import { useEffect } from "react";
import { HashRouter, NavLink, Route, Routes, useLocation, useNavigate } from "react-router-dom";
import { WifiOff } from "lucide-react";
import { GranicaBledow } from "./ui";
import { usePolaczenie } from "./kontekst/Polaczenie";
import { MODULY, type Modul } from "./moduly/rejestr";
import { usePlanerPowiadomien } from "./lib/planerPowiadomien";
import { nasluchujNawigacji } from "./lib/tauri";

function BrakPolaczenia() {
  return (
    <div className="komunikat">
      <h2>Brak połączenia z serwerem</h2>
      <p>
        Ustaw tryb pracy w module <NavLink to="/ustawienia">Ustawienia</NavLink>: ten komputer jako
        serwer albo połączenie z serwerem w sieci.
      </p>
    </div>
  );
}

function Strona({ modul }: { modul: Modul }) {
  const { api } = usePolaczenie();
  const { pathname } = useLocation();
  if (modul.wymagaPolaczenia && !api) return <BrakPolaczenia />;
  const K = modul.komponent;
  return (
    <GranicaBledow nazwa={modul.nazwa} reset={pathname}>
      <K />
    </GranicaBledow>
  );
}

/** W czasie przerwy w połączeniu z serwerem (gniazdo zdarzeń) – dyskretny pasek nad treścią. */
function PasekPrzerwy() {
  const { api, naZywo } = usePolaczenie();
  if (!api || naZywo !== false) return null;
  return (
    <div className="pasek-przerwy" role="status">
      <WifiOff size={14} aria-hidden /> Brak połączenia z serwerem – dane mogą być nieaktualne. Program łączy się ponownie…
    </div>
  );
}

function PasekStanu() {
  const { polaczenie } = usePolaczenie();
  if (!polaczenie) return <span>Wczytywanie…</span>;
  const { konfiguracja: k } = polaczenie;
  const tryb =
    k.tryb === "serwer"
      ? `Serwer${polaczenie.serwer_dziala ? " (działa)" : " (nie działa)"}`
      : k.tryb === "klient"
        ? `Klient → ${k.adres_serwera ?? "?"}`
        : "Nieskonfigurowany";
  return (
    <span>
      {k.nazwa_stanowiska} · {tryb}
    </span>
  );
}

/** Kalendarz (Faza 4b): przejścia z widżetu do zadania i planer powiadomień Windows. */
function UslugiKalendarza() {
  const przejdz = useNavigate();
  usePlanerPowiadomien();
  useEffect(() => {
    let koniec: (() => void) | undefined;
    let aktywny = true;
    void nasluchujNawigacji((s) => przejdz(s)).then((k) => (aktywny ? (koniec = k) : k()));
    return () => {
      aktywny = false;
      koniec?.();
    };
  }, [przejdz]);
  return null;
}

export function App() {
  return (
    <HashRouter>
      <UslugiKalendarza />
      <div className="uklad">
        <nav className="menu">
          <div className="logo">ROBINWOOD</div>
          {MODULY.map((m) => (
            <NavLink key={m.sciezka} to={m.sciezka} end={m.sciezka === "/"}>
              {m.nazwa}
            </NavLink>
          ))}
          <div className="stan">
            <PasekStanu />
          </div>
        </nav>
        <main className="tresc">
          <PasekPrzerwy />
          <Routes>
            {MODULY.map((m) => (
              <Route key={m.sciezka} path={m.sciezka === "/" ? "/" : `${m.sciezka}/*`} element={<Strona modul={m} />} />
            ))}
          </Routes>
        </main>
      </div>
    </HashRouter>
  );
}
