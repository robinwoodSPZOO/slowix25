// Ustawienia powiadomień TEGO stanowiska (zapis lokalny): poranne podsumowanie, przypomnienia N dni
// przed terminem, alerty pomarańczowe/czerwone, wyciszone typy zadań; podgląd i czyszczenie historii.

import { useState } from "react";
import { Bell, Eye, RotateCcw } from "lucide-react";
import type { Powiadomienie } from "../../lib/powiadomienia";
import { poprawnaGodzina } from "../../lib/powiadomienia";
import { podgladPowiadomien, wczytajUstawieniaPowiadomien, wyczyscHistoriePowiadomien, zapiszUstawieniaPowiadomien } from "../../lib/planerPowiadomien";
import { pokazPowiadomienie } from "../../lib/tauri";
import { usePolaczenie } from "../../kontekst/Polaczenie";
import { Pole, Przycisk } from "../../ui";
import { komunikatBledu } from "./dane";
import { IkonaTypu, nazwaTypu, TYPY_W_LEGENDZIE } from "./wyglad";
import "./kalendarz.css";

function parsujProgi(t: string): number[] | null {
  const czesci = t.split(/[,;\s]+/).filter(Boolean);
  if (czesci.length === 0) return [];
  const liczby = czesci.map((c) => (/^\d+$/.test(c) ? Number(c) : NaN));
  if (liczby.some((n) => Number.isNaN(n) || n > 60)) return null;
  return [...new Set(liczby)].sort((a, b) => b - a);
}

export function UstawieniaPowiadomien() {
  const { api } = usePolaczenie();
  const [u, ustawU] = useState(wczytajUstawieniaPowiadomien);
  const [progi, ustawProgi] = useState(u.dniPrzed.join(", "));
  const [podglad, ustawPodglad] = useState<Powiadomienie[] | null>(null);
  const [komunikat, ustawKomunikat] = useState<string | null>(null);
  const zmien = (n: typeof u) => {
    ustawU(n);
    zapiszUstawieniaPowiadomien(n);
    ustawKomunikat("Zapisano na tym komputerze.");
  };
  const bladProgow = parsujProgi(progi) === null ? "Wpisz liczby dni oddzielone przecinkami, np. 7, 3, 1 (najwyżej 60)" : null;

  return (
    <section className="ustawienia-powiadomien" aria-labelledby="naglowek-powiadomien">
      <h2 id="naglowek-powiadomien">Powiadomienia o terminach (ten komputer)</h2>
      <p className="pole__pomoc">Każde stanowisko ma własne ustawienia. Powiadomienia pokazują się w Windows także wtedy, gdy okno programu jest schowane do zasobnika.</p>
      <label className="przelacznik">
        <input type="checkbox" checked={u.wlaczone} onChange={(e) => zmien({ ...u, wlaczone: e.target.checked })} /> Pokazuj powiadomienia
      </label>
      <fieldset disabled={!u.wlaczone} className="ustawienia-powiadomien__pola">
        <div className="ustawienia-powiadomien__wiersz">
          <label className="przelacznik">
            <input type="checkbox" checked={u.podsumowanie} onChange={(e) => zmien({ ...u, podsumowanie: e.target.checked })} /> Poranne podsumowanie dnia
          </label>
          <Pole etykieta="o godzinie" id="godz-podsumowania" pomoc="Od tej godziny przychodzą też przypomnienia">
            <input
              id="godz-podsumowania"
              type="time"
              value={u.godzinaPodsumowania}
              onChange={(e) => poprawnaGodzina(e.target.value) && zmien({ ...u, godzinaPodsumowania: e.target.value })}
            />
          </Pole>
        </div>
        <div className="ustawienia-powiadomien__wiersz">
          <label className="przelacznik">
            <input type="checkbox" checked={u.przypomnienia} onChange={(e) => zmien({ ...u, przypomnienia: e.target.checked })} /> Przypomnienia przed terminem
          </label>
          <Pole etykieta="ile dni przed (oddziel przecinkami)" id="progi-dni" blad={bladProgow}>
            <input
              id="progi-dni"
              value={progi}
              onChange={(e) => {
                ustawProgi(e.target.value);
                const p = parsujProgi(e.target.value);
                if (p) zmien({ ...u, dniPrzed: p });
              }}
            />
          </Pole>
        </div>
        <label className="przelacznik">
          <input type="checkbox" checked={u.alertPomaranczowy} onChange={(e) => zmien({ ...u, alertPomaranczowy: e.target.checked })} /> Alert, gdy zadanie zrobi się
          pomarańczowe („zbliżamy się do krytycznego momentu”)
        </label>
        <label className="przelacznik">
          <input type="checkbox" checked={u.alertCzerwony} onChange={(e) => zmien({ ...u, alertCzerwony: e.target.checked })} /> Alert, gdy zadanie zrobi się czerwone (po
          terminie krytycznym)
        </label>
        <div>
          <span className="pole__etykieta">Wycisz typy zadań</span>
          <div className="ustawienia-powiadomien__typy">
            {TYPY_W_LEGENDZIE.map((t) => (
              <label key={t} className="przelacznik">
                <input
                  type="checkbox"
                  checked={u.wyciszoneTypy.includes(t)}
                  onChange={(e) => zmien({ ...u, wyciszoneTypy: e.target.checked ? [...u.wyciszoneTypy, t] : u.wyciszoneTypy.filter((x) => x !== t) })}
                />
                <IkonaTypu typ={t} rozmiar={12} /> {nazwaTypu(t)}
              </label>
            ))}
          </div>
        </div>
      </fieldset>
      <div className="pasek">
        <Przycisk
          ikona={Eye}
          disabled={!api}
          onClick={async () => {
            if (!api) return;
            try {
              ustawPodglad(await podgladPowiadomien(api));
            } catch (e) {
              ustawKomunikat(komunikatBledu(e));
            }
          }}
        >
          Co zostałoby teraz pokazane?
        </Przycisk>
        <Przycisk
          ikona={RotateCcw}
          onClick={() => {
            wyczyscHistoriePowiadomien();
            ustawKomunikat("Wyczyszczono historię – aktualne przypomnienia pokażą się ponownie w ciągu minuty.");
          }}
        >
          Pokaż przypomnienia jeszcze raz
        </Przycisk>
        {komunikat && (
          <span className="tekst-drugorzedny" role="status">
            {komunikat}
          </span>
        )}
      </div>
      {podglad && (
        <div className="podglad-powiadomien">
          {podglad.length === 0 ? (
            <p className="pole__pomoc">Teraz nic nowego – wszystkie aktualne powiadomienia już były pokazane (albo nie ma terminów).</p>
          ) : (
            <ul>
              {podglad.map((p) => (
                <li key={p.klucz}>
                  <Bell size={12} aria-hidden /> <strong>{p.tytul}</strong> – {p.tresc}{" "}
                  <button type="button" className="link" onClick={() => void pokazPowiadomienie(p.tytul, p.tresc)}>
                    pokaż próbnie
                  </button>
                </li>
              ))}
            </ul>
          )}
        </div>
      )}
    </section>
  );
}
