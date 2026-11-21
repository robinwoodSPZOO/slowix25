// Nagłówek oferty: daty, termin realizacji, adres inwestycji, klauzule końcowe, uwagi.

import { useEffect, useState } from "react";
import { ArrowDown, ArrowUp, Plus, Trash2 } from "lucide-react";
import type { DaneWersji, PelnaWersja } from "../../api/typy";
import { usePolaczenie } from "../../kontekst/Polaczenie";
import { Pole, Przycisk } from "../../ui";
import { BladApi } from "../../api/klient";
import { daneWersji, zmianaWersji, type ObslugaKonfliktu } from "./wspolne";

export function PanelNaglowka({
  dane,
  onDane,
  onBlad,
  onKonflikt,
}: {
  dane: PelnaWersja;
  onDane: (d: PelnaWersja) => void;
  onBlad: (b: string | null) => void;
  onKonflikt?: ObslugaKonfliktu;
}) {
  const { api } = usePolaczenie();
  const w = dane.wersja;
  const ro = w.tylko_do_odczytu;
  const [stan, ustawStan] = useState<{ baza: typeof w; szkic: DaneWersji }>(() => ({ baza: w, szkic: daneWersji(w) }));
  const [klauzuleSlownik, ustawKlauzuleSlownik] = useState<string[]>([]);
  // Nowe dane z serwera (zapis, inne stanowisko) zastępują szkic.
  if (stan.baza !== w) ustawStan({ baza: w, szkic: daneWersji(w) });
  const szkic = stan.baza === w ? stan.szkic : daneWersji(w);
  const ustawSzkic = (f: (s: DaneWersji) => DaneWersji) => ustawStan((x) => ({ ...x, szkic: f(x.szkic) }));
  useEffect(() => {
    api?.slowniki.pozycje("klauzula").then((p) => ustawKlauzuleSlownik(p.map((x) => x.wartosc)), () => {});
  }, [api]);

  if (!api) return null;

  const zapisz = async (zmiana: Partial<DaneWersji> = {}) => {
    const nowe = { ...szkic, ...zmiana };
    ustawSzkic(() => nowe);
    // Wysyłamy tylko zmienione pola (cudze zmiany innych pól nie zostaną nadpisane).
    const baza = daneWersji(w) as unknown as Record<string, unknown>;
    const zmienione = Object.fromEntries(
      Object.entries(nowe).filter(([k, v]) => JSON.stringify(v ?? null) !== JSON.stringify(baza[k] ?? null)),
    ) as Partial<DaneWersji>;
    if (Object.keys(zmienione).length === 0) return;
    const z = zmianaWersji(w, zmienione);
    try {
      onDane(await api.oferty.zmienWersje(w.id, z));
      onBlad(null);
    } catch (e) {
      if (e instanceof BladApi && e.kod === 409 && onKonflikt) {
        onKonflikt(e, async (akt) => onDane(await api.oferty.zmienWersje(w.id, { ...z, poprzednie: akt as Partial<DaneWersji> })));
      } else onBlad((e as Error).message);
    }
  };

  const tekst = (pole: "termin_realizacji" | "adres_inwestycji" | "uwagi" | "data_oferty" | "data_waznosci_cen") => ({
    value: szkic[pole] ?? "",
    disabled: ro,
    onChange: (e: { target: { value: string } }) => ustawSzkic((s) => ({ ...s, [pole]: e.target.value || null })),
    onBlur: () => void zapisz(),
  });

  const klauzule = szkic.klauzule;
  const ustawKlauzule = (k: string[]) => void zapisz({ klauzule: k });
  const brakujace = klauzuleSlownik.filter((k) => !klauzule.includes(k));

  return (
    <div className="panel-naglowka">
      <div className="panel-naglowka__dwie">
        <Pole etykieta="Data oferty" id="n-data">
          <input id="n-data" type="date" {...tekst("data_oferty")} />
        </Pole>
        <Pole etykieta="Ceny ważne do" id="n-waznosc">
          <input id="n-waznosc" type="date" {...tekst("data_waznosci_cen")} />
        </Pole>
      </div>
      <Pole etykieta="Termin realizacji" id="n-termin" pomoc="Np. „do 10 tygodni od podpisania umowy”">
        <input id="n-termin" {...tekst("termin_realizacji")} />
      </Pole>
      <Pole etykieta="Inwestycja (adres montażu)" id="n-adres">
        <input id="n-adres" {...tekst("adres_inwestycji")} />
      </Pole>
      <div className="pole">
        <span className="pole__etykieta">Klauzule końcowe (w kolejności w dokumencie)</span>
        <ol className="lista-klauzul">
          {klauzule.map((k, i) => (
            <li key={`${i}-${k.slice(0, 20)}`}>
              <textarea
                className="wejscie"
                rows={2}
                aria-label={`Klauzula ${i + 1}`}
                disabled={ro}
                value={k}
                onChange={(e) => ustawSzkic((s) => ({ ...s, klauzule: s.klauzule.map((x, j) => (j === i ? e.target.value : x)) }))}
                onBlur={() => void zapisz()}
              />
              {!ro && (
                <span className="lista-klauzul__akcje">
                  <Przycisk maly wariant="tekstowy" tylkoIkona ikona={ArrowUp} etykieta="W górę" disabled={i === 0} onClick={() => ustawKlauzule(zamien(klauzule, i, i - 1))} />
                  <Przycisk maly wariant="tekstowy" tylkoIkona ikona={ArrowDown} etykieta="W dół" disabled={i === klauzule.length - 1} onClick={() => ustawKlauzule(zamien(klauzule, i, i + 1))} />
                  <Przycisk maly wariant="tekstowy" tylkoIkona ikona={Trash2} etykieta="Usuń klauzulę" onClick={() => ustawKlauzule(klauzule.filter((_, j) => j !== i))} />
                </span>
              )}
            </li>
          ))}
        </ol>
        {!ro && (
          <div className="panel-naglowka__dodaj">
            <Przycisk maly wariant="tekstowy" ikona={Plus} onClick={() => ustawSzkic((s) => ({ ...s, klauzule: [...s.klauzule, ""] }))}>
              Własna klauzula
            </Przycisk>
            {brakujace.length > 0 && (
              <select className="wejscie wejscie--maly" aria-label="Dodaj klauzulę ze słownika" value="" onChange={(e) => e.target.value && ustawKlauzule([...klauzule, e.target.value])}>
                <option value="">Dodaj ze słownika…</option>
                {brakujace.map((k) => (
                  <option key={k} value={k}>
                    {k.length > 70 ? `${k.slice(0, 70)}…` : k}
                  </option>
                ))}
              </select>
            )}
          </div>
        )}
      </div>
      <Pole etykieta="Uwagi wewnętrzne" id="n-uwagi" pomoc="Nie trafiają do dokumentu">
        <textarea id="n-uwagi" rows={3} {...tekst("uwagi")} />
      </Pole>
    </div>
  );
}

function zamien<T>(t: T[], a: number, b: number): T[] {
  const w = [...t];
  [w[a], w[b]] = [w[b]!, w[a]!];
  return w;
}
