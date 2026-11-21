// Słowniki podpowiedzi i szablony mebli: podgląd oraz import z plików użytkownika
// (slowniki.json, szablony-mebli.json – np. z folderu „program-dane”).

import { useEffect, useRef, useState } from "react";
import { CircleCheck, FileUp, LayoutTemplate, TriangleAlert } from "lucide-react";
import type { KategoriaMaterialu, PozycjaSlownika, RaportImportu, Slownik, SzablonMebli } from "../../api/typy";
import { usePolaczenie } from "../../kontekst/Polaczenie";
import { EtykietaStatusu, Przycisk, WierszTabeli } from "../../ui";
import "../oferty/oferty.css";
import "./slowniki.css";

type Rodzaj = "slowniki" | "szablony";

export function Slowniki() {
  const { api, ostatnieZdarzenie } = usePolaczenie();
  const [slowniki, ustawSlowniki] = useState<Slownik[]>([]);
  const [kod, ustawKod] = useState<string | null>(null);
  const [pozycje, ustawPozycje] = useState<PozycjaSlownika[]>([]);
  const [szablony, ustawSzablony] = useState<SzablonMebli[]>([]);
  const [kategorie, ustawKategorie] = useState<KategoriaMaterialu[]>([]);
  const [raport, ustawRaport] = useState<{ rodzaj: Rodzaj; r: RaportImportu } | null>(null);
  const [blad, ustawBlad] = useState<string | null>(null);
  const wejscie = useRef<HTMLInputElement>(null);
  const [rodzajImportu, ustawRodzajImportu] = useState<Rodzaj>("slowniki");

  useEffect(() => {
    if (!api) return;
    api.slowniki.lista().then(ustawSlowniki, (e: Error) => ustawBlad(e.message));
    api.szablony.lista().then(ustawSzablony, (e: Error) => ustawBlad(e.message));
    api.slowniki.kategorieMaterialow().then(ustawKategorie, (e: Error) => ustawBlad(e.message));
  }, [api, ostatnieZdarzenie, raport]);

  const wybrany = kod ?? slowniki[0]?.kod ?? null;
  useEffect(() => {
    if (!api || !wybrany) return;
    api.slowniki.pozycje(wybrany).then(ustawPozycje, (e: Error) => ustawBlad(e.message));
  }, [api, wybrany, raport]);

  if (!api) return null;

  const importuj = async (pliki: FileList | null) => {
    const plik = pliki?.[0];
    if (!plik) return;
    try {
      const json: unknown = JSON.parse(await plik.text());
      const r = rodzajImportu === "slowniki" ? await api.importy.slowniki(json) : await api.importy.szablony(json);
      ustawRaport({ rodzaj: rodzajImportu, r });
      ustawBlad(null);
      ustawSzablony(await api.szablony.lista());
    } catch (e) {
      ustawBlad(e instanceof SyntaxError ? `Plik nie jest poprawnym JSON-em: ${e.message}` : (e as Error).message);
    } finally {
      if (wejscie.current) wejscie.current.value = "";
    }
  };

  const wybierzPlik = (r: Rodzaj) => {
    ustawRodzajImportu(r);
    setTimeout(() => wejscie.current?.click(), 0);
  };

  const suma = raport ? Object.values(raport.r.dodane).reduce((a, b) => a + b, 0) : 0;

  return (
    <div className="slowniki">
      <div className="lista-ofert__naglowek">
        <h1>Słowniki i szablony</h1>
        <div className="lista-ofert__akcje">
          <Przycisk ikona={FileUp} onClick={() => wybierzPlik("slowniki")}>
            Importuj słowniki (slowniki.json)
          </Przycisk>
          <Przycisk ikona={LayoutTemplate} onClick={() => wybierzPlik("szablony")}>
            Importuj szablony (szablony-mebli.json)
          </Przycisk>
          <input ref={wejscie} type="file" accept=".json,application/json" hidden onChange={(e) => void importuj(e.target.files)} />
        </div>
      </div>
      <p className="pole__pomoc">
        Import dopisuje nowe pozycje i nie usuwa istniejących; powtórny import tego samego pliku niczego nie dubluje. Szablon o tym samym kodzie jest nadpisywany.
      </p>
      {blad && (
        <div className="komunikat-stanu status--czerwony" role="alert">
          <TriangleAlert size={14} aria-hidden /> {blad}
        </div>
      )}
      {raport && (
        <div className="komunikat-stanu status--spokojnie" role="status">
          <CircleCheck size={14} aria-hidden />
          <div>
            <strong>
              Zaimportowano {raport.rodzaj === "slowniki" ? "słowniki" : "szablony"}: {suma} nowych
              {raport.r.zaktualizowane ? `, ${raport.r.zaktualizowane} zaktualizowanych` : ""}, {raport.r.pominiete} pominiętych (już były).
            </strong>
            {Object.keys(raport.r.dodane).length > 0 && (
              <div>
                {Object.entries(raport.r.dodane)
                  .map(([k, v]) => `${k}: ${v}`)
                  .join(" · ")}
              </div>
            )}
            {raport.r.nowe_kategorie.length > 0 && <div>Nowe kategorie materiałów: {raport.r.nowe_kategorie.join(", ")}</div>}
            {raport.r.ostrzezenia.map((o) => (
              <div key={o}>{o}</div>
            ))}
          </div>
        </div>
      )}
      <div className="slowniki__uklad">
        <nav className="karta slowniki__lista" aria-label="Słowniki">
          {slowniki.map((s) => (
            <button key={s.kod} type="button" className={`slowniki__pozycja${s.kod === wybrany ? " slowniki__pozycja--wybrana" : ""}`} onClick={() => ustawKod(s.kod)}>
              {s.nazwa}
            </button>
          ))}
        </nav>
        <div className="karta slowniki__tabela">
          <table className="tabela">
            <thead>
              <tr>
                <th>Wartość</th>
                <th>Producent</th>
                <th>Dekor / kod</th>
                <th>Podgrupa</th>
              </tr>
            </thead>
            <tbody>
              {pozycje.map((p) => (
                <WierszTabeli key={p.id}>
                  <td>{p.wartosc}</td>
                  <td>{p.producent ?? ""}</td>
                  <td>{p.dekor ?? ""}</td>
                  <td>{p.podgrupa === "wyposazenie" ? "Wyposażenie" : p.podgrupa === "frezowanie" ? "Frezowanie" : ""}</td>
                </WierszTabeli>
              ))}
              {pozycje.length === 0 && (
                <tr>
                  <td colSpan={4} className="tabela-wierszy__pusto">
                    Słownik jest pusty.
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
        <div className="karta slowniki__szablony">
          <h3>Szablony mebli ({szablony.length})</h3>
          <ul>
            {szablony.map((s) => (
              <li key={s.id}>
                <strong>{s.nazwa}</strong>{" "}
                {s.zrodlo === "import" ? <EtykietaStatusu stan="akcent" ikona={FileUp} tekst="import" /> : null}
                <div className="pole__pomoc">
                  {s.czesci.slice(0, 4).join(", ")} · {s.wiersze.length} wierszy
                </div>
              </li>
            ))}
          </ul>
        </div>
      </div>
      <h2>Kategorie materiałów i czasy dostaw</h2>
      <div className="karta">
        <table className="tabela">
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
      </div>
      <p className="pole__pomoc">Edycja słowników, dostawców i widełek czasów dostaw – kolejne fazy.</p>
    </div>
  );
}
