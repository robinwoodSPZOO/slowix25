import { HashRouter, NavLink, Route, Routes } from "react-router-dom";
import { usePolaczenie } from "./kontekst/Polaczenie";
import { MODULY, type Modul } from "./moduly/rejestr";

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
  if (modul.wymagaPolaczenia && !api) return <BrakPolaczenia />;
  const K = modul.komponent;
  return <K />;
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

export function App() {
  return (
    <HashRouter>
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
