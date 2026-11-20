import { useEffect, useState } from "react";
import type { KategoriaMaterialu, PozycjaSlownika, Slownik } from "../../api/typy";
import { usePolaczenie } from "../../kontekst/Polaczenie";

export function Slowniki() {
  const { api } = usePolaczenie();
  const [slowniki, ustawSlowniki] = useState<Slownik[]>([]);
  const [wybrany, ustawWybrany] = useState<string>("rodzaj_elementu");
  const [pozycje, ustawPozycje] = useState<PozycjaSlownika[]>([]);
  const [kategorie, ustawKategorie] = useState<KategoriaMaterialu[]>([]);
  const [blad, ustawBlad] = useState<string | null>(null);

  useEffect(() => {
    if (!api) return;
    api.slowniki.lista().then(ustawSlowniki, (e: Error) => ustawBlad(e.message));
    api.slowniki.kategorieMaterialow().then(ustawKategorie, (e: Error) => ustawBlad(e.message));
  }, [api]);

  useEffect(() => {
    if (!api) return;
    api.slowniki.pozycje(wybrany).then(ustawPozycje, (e: Error) => ustawBlad(e.message));
  }, [api, wybrany]);

  return (
    <>
      <h1>Słowniki</h1>
      {blad && <p className="blad">{blad}</p>}
      <div className="pasek">
        <select value={wybrany} onChange={(e) => ustawWybrany(e.target.value)}>
          {slowniki.map((s) => (
            <option key={s.kod} value={s.kod}>
              {s.nazwa}
            </option>
          ))}
        </select>
      </div>
      <ul>
        {pozycje.map((p) => (
          <li key={p.id}>{[p.wartosc, p.producent, p.dekor].filter(Boolean).join(" · ")}</li>
        ))}
        {pozycje.length === 0 && <li>Słownik jest pusty.</li>}
      </ul>
      <h2>Kategorie materiałów i czasy dostaw</h2>
      <table>
        <thead>
          <tr>
            <th>Kategoria</th>
            <th>Dostawa od (dni)</th>
            <th>Dostawa do (dni)</th>
          </tr>
        </thead>
        <tbody>
          {kategorie.map((k) => (
            <tr key={k.id}>
              <td>{k.nazwa}</td>
              <td>{k.dni_dostawy_min || "—"}</td>
              <td>{k.dni_dostawy_max || "—"}</td>
            </tr>
          ))}
        </tbody>
      </table>
      <p>
        <em>Edycja słowników, dostawców i widełek czasów dostaw – faza Fundament / Kalendarz.</em>
      </p>
    </>
  );
}
