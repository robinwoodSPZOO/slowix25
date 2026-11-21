// Ustawienia kalendarza (wspólne dla wszystkich stanowisk – zapis na serwerze): czasy dostaw
// (widełki kategorii i par kategoria + dostawca, statystyka rzeczywistych dostaw), dostawcy, odstępy
// i tryb liczenia dni, progi ostrzeżeń, dni wolne, import kategorie-materialow.csv.
// Po każdym zapisie, który przelicza terminy, pokazujemy przeliczone i nieprzeliczone zlecenia.

import { useCallback, useEffect, useMemo, useState, type ChangeEvent, type ReactNode } from "react";
import { Link } from "react-router-dom";
import { ArrowLeft, CircleAlert, CircleCheck, History, Plus, Save, Trash2, TriangleAlert, Upload, Wand2 } from "lucide-react";
import type {
  CzasDostawy,
  Dostawca,
  DzienWolny,
  KategoriaMaterialu,
  RaportImportuKategorii,
  RodzajDniaWolnego,
  StatystykaPary,
  Swieto,
  UstawieniaTerminow,
  WynikPrzeliczenia,
  Zlecenie,
} from "../../api/typy";
import { usePolaczenie } from "../../kontekst/Polaczenie";
import { formatujDate } from "../../lib/formaty";
import { EtykietaStatusu, Pole, Przycisk, Segment } from "../../ui";
import { komunikatBledu, useOdswiezaniePoZdarzeniu } from "./dane";
import { dodajDni, dzisIso, poprawnaData, rokMiesiac } from "./daty";
import { parsujKategorieCsv, type WynikParsowania } from "./importCsv";
import { useSzkic } from "./szkic";

const RODZAJE: Record<RodzajDniaWolnego, string> = {
  urlop: "Urlop / przerwa w firmie",
  przerwa_dostawcow: "Przerwa u dostawców",
  swieto: "Dodatkowe święto",
  inne: "Inny dzień wolny",
};

function Komunikat({ rodzaj, children }: { rodzaj: "ok" | "blad" | "uwaga"; children: ReactNode }) {
  const Ikona = rodzaj === "ok" ? CircleCheck : rodzaj === "blad" ? CircleAlert : TriangleAlert;
  const tlo = rodzaj === "ok" ? "spokojnie" : rodzaj === "blad" ? "czerwony" : "zolty";
  return (
    <div className={`komunikat-stanu stan-tlo--${tlo}`} role={rodzaj === "blad" ? "alert" : "status"}>
      <Ikona size={14} aria-hidden />
      <span>{children}</span>
    </div>
  );
}

/** Wynik przeliczenia wszystkich zleceń po zapisie ustawień. */
export function RaportPrzeliczenia({ wynik, naglowek }: { wynik: WynikPrzeliczenia; naglowek: string }) {
  const n = wynik.przeliczone_zlecenia.length;
  return (
    <div className="raport-przeliczenia">
      <Komunikat rodzaj={wynik.nieprzeliczone_zlecenia.length ? "uwaga" : "ok"}>
        {naglowek} Przeliczono terminy {n} {n === 1 ? "zlecenia" : "zleceń"}
        {wynik.nieprzeliczone_zlecenia.length ? `, nie udało się przeliczyć ${wynik.nieprzeliczone_zlecenia.length}:` : "."}
      </Komunikat>
      {wynik.nieprzeliczone_zlecenia.length > 0 && (
        <ul>
          {wynik.nieprzeliczone_zlecenia.map((z) => (
            <li key={z.zlecenie_id}>
              <Link to={`/kalendarz/zlecenie/${z.zlecenie_id}`}>{z.zlecenie_nazwa}</Link> – {z.powod}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

function liczba(t: string, min = 0, max = 365): number | null {
  if (t.trim() === "" || !/^\d+$/.test(t.trim())) return null;
  const n = Number(t);
  return n >= min && n <= max ? n : null;
}

// ---------------------------------------------------------------------------------------------
// Czasy dostaw
// ---------------------------------------------------------------------------------------------

function WierszKategorii({ k, dostawcy, par, onZapisano }: { k: KategoriaMaterialu; dostawcy: Dostawca[]; par: number; onZapisano: (komunikat: string) => void }) {
  const { api } = usePolaczenie();
  const [min, ustawMin] = useState(String(k.dni_dostawy_min));
  const [max, ustawMax] = useState(String(k.dni_dostawy_max));
  const [dost, ustawDost] = useState(k.domyslny_dostawca_id ? String(k.domyslny_dostawca_id) : "");
  const [blad, ustawBlad] = useState<string | null>(null);
  const [poprzednie, ustawPoprzednie] = useState(k);
  if (poprzednie !== k) {
    ustawPoprzednie(k);
    ustawMin(String(k.dni_dostawy_min));
    ustawMax(String(k.dni_dostawy_max));
    ustawDost(k.domyslny_dostawca_id ? String(k.domyslny_dostawca_id) : "");
  }
  const a = liczba(min);
  const b = liczba(max);
  const bladPol = a === null || b === null ? "0–365 dni" : a > b ? "min > max" : null;
  const zmienione = min !== String(k.dni_dostawy_min) || max !== String(k.dni_dostawy_max) || dost !== (k.domyslny_dostawca_id ? String(k.domyslny_dostawca_id) : "");
  const nieUstawiono = k.dni_dostawy_min === 0 && k.dni_dostawy_max === 0;
  const zapisz = async () => {
    if (!api || bladPol || a === null || b === null) return;
    try {
      await api.czasyDostaw.widelkiKategorii(k.id, { dni_dostawy_min: a, dni_dostawy_max: b, domyslny_dostawca_id: dost ? Number(dost) : null });
      ustawBlad(null);
      onZapisano(`Zapisano czas dostawy: ${k.nazwa}.`);
    } catch (e) {
      ustawBlad(komunikatBledu(e));
    }
  };
  return (
    <tr>
      <th scope="row">
        {k.nazwa}
        {k.opis && <div className="tekst-drugorzedny">{k.opis}</div>}
      </th>
      <td>
        <select className="wejscie wejscie--maly" aria-label={`Domyślny dostawca: ${k.nazwa}`} value={dost} onChange={(e) => ustawDost(e.target.value)}>
          <option value="">— brak —</option>
          {dostawcy.map((d) => (
            <option key={d.id} value={d.id}>
              {d.nazwa}
            </option>
          ))}
        </select>
      </td>
      <td>
        <input className="wejscie wejscie--maly wejscie--dni" aria-label={`Minimalny czas dostawy: ${k.nazwa}`} inputMode="numeric" value={min} onChange={(e) => ustawMin(e.target.value)} />
      </td>
      <td>
        <input className="wejscie wejscie--maly wejscie--dni" aria-label={`Maksymalny czas dostawy: ${k.nazwa}`} inputMode="numeric" value={max} onChange={(e) => ustawMax(e.target.value)} />
      </td>
      <td className="kolumna-kwoty">{a !== null && b !== null && !bladPol && (a > 0 || b > 0) ? Math.floor((a + b) / 2) : "—"}</td>
      <td>
        {bladPol ? (
          <EtykietaStatusu stan="czerwony" ikona={CircleAlert} tekst={bladPol} />
        ) : nieUstawiono && !zmienione && par > 0 ? (
          <EtykietaStatusu stan="neutralny" ikona={CircleCheck} tekst={`tylko u dostawców (${par})`} title="Widełki ustawione dla par kategoria + dostawca (tabela niżej); materiały bez dostawcy nie mają terminów" />
        ) : nieUstawiono && !zmienione ? (
          <EtykietaStatusu stan="zolty" ikona={TriangleAlert} tekst="nie ustawiono" title="Materiały tej kategorii nie mają terminów zamówienia" />
        ) : (
          <EtykietaStatusu stan="spokojnie" ikona={CircleCheck} tekst="ustawiono" />
        )}
        {blad && <div className="pole__blad">{blad}</div>}
      </td>
      <td>
        {zmienione && (
          <Przycisk maly wariant="glowny" ikona={Save} disabled={!!bladPol} onClick={() => void zapisz()}>
            Zapisz
          </Przycisk>
        )}
      </td>
    </tr>
  );
}

function WierszPary({ c, onWynik }: { c: CzasDostawy; onWynik: (w: WynikPrzeliczenia | null, k: string) => void }) {
  const { api } = usePolaczenie();
  const [min, ustawMin] = useState(String(c.dni_min));
  const [max, ustawMax] = useState(String(c.dni_max));
  const [blad, ustawBlad] = useState<string | null>(null);
  const [poprzednie, ustawPoprzednie] = useState(c);
  if (poprzednie !== c) {
    ustawPoprzednie(c);
    ustawMin(String(c.dni_min));
    ustawMax(String(c.dni_max));
  }
  const a = liczba(min);
  const b = liczba(max);
  const bladPol = a === null || b === null ? "0–365 dni" : a > b ? "min > max" : null;
  const zmienione = min !== String(c.dni_min) || max !== String(c.dni_max);
  return (
    <tr>
      <th scope="row">{c.kategoria_nazwa}</th>
      <td>{c.dostawca_nazwa}</td>
      <td>
        <input className="wejscie wejscie--maly wejscie--dni" aria-label={`Min: ${c.kategoria_nazwa} – ${c.dostawca_nazwa}`} value={min} onChange={(e) => ustawMin(e.target.value)} />
      </td>
      <td>
        <input className="wejscie wejscie--maly wejscie--dni" aria-label={`Max: ${c.kategoria_nazwa} – ${c.dostawca_nazwa}`} value={max} onChange={(e) => ustawMax(e.target.value)} />
      </td>
      <td className="kolumna-kwoty">{a !== null && b !== null && !bladPol ? Math.floor((a + b) / 2) : "—"}</td>
      <td className="kolumna-kwoty">{c.dni_srednia_rzeczywista !== null ? c.dni_srednia_rzeczywista.toFixed(1) : "—"}</td>
      <td className="akcje-wiersza">
        {bladPol && <EtykietaStatusu stan="czerwony" ikona={CircleAlert} tekst={bladPol} />}
        {zmienione && (
          <Przycisk
            maly
            wariant="glowny"
            ikona={Save}
            disabled={!!bladPol}
            onClick={async () => {
              if (!api || a === null || b === null) return;
              try {
                const w = await api.czasyDostaw.zmien(c.id, { kategoria_id: c.kategoria_id, dostawca_id: c.dostawca_id, dni_min: a, dni_max: b });
                onWynik(w, `Zapisano: ${c.kategoria_nazwa} – ${c.dostawca_nazwa}.`);
              } catch (e) {
                ustawBlad(komunikatBledu(e));
              }
            }}
          >
            Zapisz
          </Przycisk>
        )}
        <Przycisk
          maly
          wariant="tekstowy"
          tylkoIkona
          ikona={Trash2}
          etykieta={`Usuń czas dostawy ${c.kategoria_nazwa} – ${c.dostawca_nazwa}`}
          onClick={async () => {
            if (!api || !window.confirm(`Usunąć czas dostawy ${c.kategoria_nazwa} – ${c.dostawca_nazwa}? Materiały użyją widełek kategorii.`)) return;
            try {
              await api.czasyDostaw.usun(c.id);
              onWynik(null, "Usunięto czas dostawy – terminy przeliczone.");
            } catch (e) {
              ustawBlad(komunikatBledu(e));
            }
          }}
        />
        {blad && <div className="pole__blad">{blad}</div>}
      </td>
    </tr>
  );
}

function SekcjaCzasowDostaw({
  kategorie,
  dostawcy,
  czasy,
  statystyka,
  odswiez,
  onWynik,
}: {
  kategorie: KategoriaMaterialu[];
  dostawcy: Dostawca[];
  czasy: CzasDostawy[];
  statystyka: StatystykaPary[];
  odswiez: () => void;
  onWynik: (w: WynikPrzeliczenia | null, k: string) => void;
}) {
  const { api } = usePolaczenie();
  const [nowa, ustawNowa] = useState({ kategoria: "", dostawca: "", min: "", max: "" });
  const [blad, ustawBlad] = useState<string | null>(null);
  const nieustawione = kategorie.filter((k) => k.dni_dostawy_min === 0 && k.dni_dostawy_max === 0 && !czasy.some((c) => c.kategoria_id === k.id)).length;
  const doKorekty = statystyka.filter((s) => s.sugerowana_korekta);

  const przyjmij = async (s: StatystykaPary) => {
    if (!api) return;
    const w = s.statystyka.sugerowane_widelki;
    try {
      if (s.dostawca_id !== null) {
        const para = czasy.find((c) => c.kategoria_id === s.kategoria_id && c.dostawca_id === s.dostawca_id);
        const d = { kategoria_id: s.kategoria_id, dostawca_id: s.dostawca_id, dni_min: w.min, dni_max: w.max };
        const r = para ? await api.czasyDostaw.zmien(para.id, d) : await api.czasyDostaw.dodaj(d);
        onWynik(r, `Przyjęto widełki ${w.min}–${w.max} dni: ${s.kategoria_nazwa} – ${s.dostawca_nazwa}.`);
      } else {
        const k = kategorie.find((x) => x.id === s.kategoria_id);
        await api.czasyDostaw.widelkiKategorii(s.kategoria_id, { dni_dostawy_min: w.min, dni_dostawy_max: w.max, domyslny_dostawca_id: k?.domyslny_dostawca_id ?? null });
        onWynik(null, `Przyjęto widełki ${w.min}–${w.max} dni: ${s.kategoria_nazwa}.`);
      }
    } catch (e) {
      ustawBlad(komunikatBledu(e));
    }
  };

  const dodaj = async () => {
    const a = liczba(nowa.min);
    const b = liczba(nowa.max);
    if (!api || !nowa.kategoria || !nowa.dostawca || a === null || b === null || a > b) {
      ustawBlad("Wybierz kategorię i dostawcę oraz wpisz czasy 0–365 dni (min ≤ max).");
      return;
    }
    try {
      const r = await api.czasyDostaw.dodaj({ kategoria_id: Number(nowa.kategoria), dostawca_id: Number(nowa.dostawca), dni_min: a, dni_max: b });
      ustawNowa({ kategoria: "", dostawca: "", min: "", max: "" });
      ustawBlad(null);
      onWynik(r, "Dodano czas dostawy.");
    } catch (e) {
      ustawBlad(komunikatBledu(e));
    }
  };

  return (
    <section id="czasy-dostaw" className="karta sekcja-ustawien">
      <h2>Czasy dostaw materiałów</h2>
      <p className="pole__pomoc">
        Widełki „od–do” w dniach (zgodnie z trybem liczenia dni). Najlepiej zamówić (sweet spot) = przy najdłuższej dostawie, planowo = przy średniej
        (zaokrąglonej w dół), najpóźniej (krytyczna) = przy najkrótszej. 0–0 oznacza „nie ustawiono” – materiały bez terminów.
      </p>
      {nieustawione > 0 && <Komunikat rodzaj="uwaga">{nieustawione} kategorii nie ma ustawionego czasu dostawy.</Komunikat>}
      <div className="tabela-przewijana">
        <table className="tabela">
          <thead>
            <tr>
              <th>Kategoria materiału</th>
              <th>Domyślny dostawca</th>
              <th>Min (dni)</th>
              <th>Max (dni)</th>
              <th className="kolumna-kwoty">Średnia</th>
              <th>Stan</th>
              <th />
            </tr>
          </thead>
          <tbody>
            {kategorie.map((k) => (
              <WierszKategorii key={k.id} k={k} dostawcy={dostawcy} par={czasy.filter((c) => c.kategoria_id === k.id).length} onZapisano={(t) => onWynik(null, t)} />
            ))}
          </tbody>
        </table>
      </div>

      <h3>Czasy u konkretnych dostawców (mają pierwszeństwo przed widełkami kategorii)</h3>
      <div className="tabela-przewijana">
        <table className="tabela">
          <thead>
            <tr>
              <th>Kategoria</th>
              <th>Dostawca</th>
              <th>Min</th>
              <th>Max</th>
              <th className="kolumna-kwoty">Średnia</th>
              <th className="kolumna-kwoty">Rzeczywista średnia</th>
              <th />
            </tr>
          </thead>
          <tbody>
            {czasy.map((c) => (
              <WierszPary key={c.id} c={c} onWynik={onWynik} />
            ))}
            <tr className="wiersz-dodawania">
              <td>
                <select className="wejscie wejscie--maly" aria-label="Kategoria nowego czasu dostawy" value={nowa.kategoria} onChange={(e) => ustawNowa({ ...nowa, kategoria: e.target.value })}>
                  <option value="">kategoria…</option>
                  {kategorie.map((k) => (
                    <option key={k.id} value={k.id}>
                      {k.nazwa}
                    </option>
                  ))}
                </select>
              </td>
              <td>
                <select className="wejscie wejscie--maly" aria-label="Dostawca nowego czasu dostawy" value={nowa.dostawca} onChange={(e) => ustawNowa({ ...nowa, dostawca: e.target.value })}>
                  <option value="">dostawca…</option>
                  {dostawcy.map((d) => (
                    <option key={d.id} value={d.id}>
                      {d.nazwa}
                    </option>
                  ))}
                </select>
              </td>
              <td>
                <input className="wejscie wejscie--maly wejscie--dni" aria-label="Min (dni)" value={nowa.min} onChange={(e) => ustawNowa({ ...nowa, min: e.target.value })} />
              </td>
              <td>
                <input className="wejscie wejscie--maly wejscie--dni" aria-label="Max (dni)" value={nowa.max} onChange={(e) => ustawNowa({ ...nowa, max: e.target.value })} />
              </td>
              <td colSpan={3}>
                <Przycisk maly ikona={Plus} onClick={() => void dodaj()}>
                  Dodaj
                </Przycisk>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
      {blad && <Komunikat rodzaj="blad">{blad}</Komunikat>}

      <h3>Rzeczywiste czasy dostaw (z dat zamówienia i dostawy)</h3>
      {statystyka.length === 0 ? (
        <p className="pole__pomoc">Brak zamówień z wpisaną datą dostawy – statystyka pojawi się po pierwszych dostawach.</p>
      ) : (
        <div className="tabela-przewijana">
          <table className="tabela">
            <thead>
              <tr>
                <th>Kategoria – dostawca</th>
                <th>Ustawione</th>
                <th>Rzeczywiste (liczba dostaw)</th>
                <th>Sugerowane</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {statystyka.map((s) => (
                <tr key={`${s.kategoria_id}-${s.dostawca_id ?? 0}`}>
                  <th scope="row">
                    {s.kategoria_nazwa} – {s.dostawca_nazwa ?? "bez dostawcy"}
                  </th>
                  <td>{s.aktualne_widelki ? `${s.aktualne_widelki.min}–${s.aktualne_widelki.max} dni` : "nie ustawiono"}</td>
                  <td>
                    {s.statystyka.min}–{s.statystyka.max} dni, średnio {s.statystyka.srednia.toFixed(1)} ({s.statystyka.liczba})
                  </td>
                  <td>
                    {s.statystyka.sugerowane_widelki.min}–{s.statystyka.sugerowane_widelki.max} dni
                  </td>
                  <td>
                    {s.sugerowana_korekta ? (
                      <Przycisk maly ikona={Wand2} onClick={() => void przyjmij(s)}>
                        Przyjmij sugerowane widełki
                      </Przycisk>
                    ) : (
                      <span className="tekst-drugorzedny">zgodne</span>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
      {doKorekty.length > 0 && (
        <p className="pole__pomoc">
          {doKorekty.length} {doKorekty.length === 1 ? "para odbiega" : "par odbiega"} od ustawionych widełek – program nie zmienia ich sam; zdecyduj przyciskiem.
        </p>
      )}
      <Przycisk wariant="tekstowy" maly onClick={odswiez}>
        Odśwież
      </Przycisk>
    </section>
  );
}

// ---------------------------------------------------------------------------------------------
// Dostawcy
// ---------------------------------------------------------------------------------------------

function SekcjaDostawcow({ dostawcy, onZmiana }: { dostawcy: Dostawca[]; onZmiana: () => void }) {
  const { api } = usePolaczenie();
  const [nowy, ustawNowy] = useState({ nazwa: "", email: "", telefon: "" });
  const [edycja, ustawEdycje] = useState<Dostawca | null>(null);
  const [blad, ustawBlad] = useState<string | null>(null);
  const zapisz = async () => {
    if (!api) return;
    try {
      if (edycja) {
        await api.dostawcy.zmien(edycja.id, { nazwa: edycja.nazwa, email: edycja.email || null, telefon: edycja.telefon || null, uwagi: edycja.uwagi, aktywny: edycja.aktywny });
        ustawEdycje(null);
      } else {
        if (!nowy.nazwa.trim()) return ustawBlad("Wpisz nazwę dostawcy.");
        await api.dostawcy.dodaj({ nazwa: nowy.nazwa, email: nowy.email || null, telefon: nowy.telefon || null });
        ustawNowy({ nazwa: "", email: "", telefon: "" });
      }
      ustawBlad(null);
      onZmiana();
    } catch (e) {
      ustawBlad(komunikatBledu(e));
    }
  };
  return (
    <section id="dostawcy" className="karta sekcja-ustawien">
      <h2>Dostawcy</h2>
      <div className="tabela-przewijana">
        <table className="tabela">
          <thead>
            <tr>
              <th>Nazwa</th>
              <th>E-mail</th>
              <th>Telefon</th>
              <th>Aktywny</th>
              <th />
            </tr>
          </thead>
          <tbody>
            {dostawcy.map((d) =>
              edycja?.id === d.id ? (
                <tr key={d.id}>
                  <td>
                    <input className="wejscie wejscie--maly" aria-label="Nazwa" value={edycja.nazwa} onChange={(e) => ustawEdycje({ ...edycja, nazwa: e.target.value })} />
                  </td>
                  <td>
                    <input className="wejscie wejscie--maly" aria-label="E-mail" value={edycja.email ?? ""} onChange={(e) => ustawEdycje({ ...edycja, email: e.target.value })} />
                  </td>
                  <td>
                    <input className="wejscie wejscie--maly" aria-label="Telefon" value={edycja.telefon ?? ""} onChange={(e) => ustawEdycje({ ...edycja, telefon: e.target.value })} />
                  </td>
                  <td>
                    <input type="checkbox" aria-label="Aktywny" checked={edycja.aktywny} onChange={(e) => ustawEdycje({ ...edycja, aktywny: e.target.checked })} />
                  </td>
                  <td className="akcje-wiersza">
                    <Przycisk maly wariant="glowny" onClick={() => void zapisz()}>
                      Zapisz
                    </Przycisk>
                    <Przycisk maly onClick={() => ustawEdycje(null)}>
                      Anuluj
                    </Przycisk>
                  </td>
                </tr>
              ) : (
                <tr key={d.id}>
                  <th scope="row">{d.nazwa}</th>
                  <td>{d.email ?? "—"}</td>
                  <td>{d.telefon ?? "—"}</td>
                  <td>{d.aktywny ? "tak" : "nie"}</td>
                  <td>
                    <Przycisk maly wariant="tekstowy" onClick={() => ustawEdycje(d)}>
                      Zmień
                    </Przycisk>
                  </td>
                </tr>
              ),
            )}
            <tr className="wiersz-dodawania">
              <td>
                <input className="wejscie wejscie--maly" placeholder="nazwa nowego dostawcy" aria-label="Nazwa nowego dostawcy" value={nowy.nazwa} onChange={(e) => ustawNowy({ ...nowy, nazwa: e.target.value })} />
              </td>
              <td>
                <input className="wejscie wejscie--maly" placeholder="e-mail" aria-label="E-mail nowego dostawcy" value={nowy.email} onChange={(e) => ustawNowy({ ...nowy, email: e.target.value })} />
              </td>
              <td>
                <input className="wejscie wejscie--maly" placeholder="telefon" aria-label="Telefon nowego dostawcy" value={nowy.telefon} onChange={(e) => ustawNowy({ ...nowy, telefon: e.target.value })} />
              </td>
              <td />
              <td>
                <Przycisk maly ikona={Plus} onClick={() => void zapisz()} disabled={!!edycja}>
                  Dodaj
                </Przycisk>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
      {blad && <Komunikat rodzaj="blad">{blad}</Komunikat>}
    </section>
  );
}

// ---------------------------------------------------------------------------------------------
// Odstępy, tryb liczenia dni, progi
// ---------------------------------------------------------------------------------------------

type PolaUstawien = Record<keyof UstawieniaTerminow, string>;
const POLA_LICZBOWE: { klucz: Exclude<keyof UstawieniaTerminow, "liczenie_dni">; etykieta: string; pomoc: string }[] = [
  { klucz: "odstep_materialy_dni", etykieta: "Materiały na warsztacie przed składaniem", pomoc: "Ile dni przed rozpoczęciem składania mają być wszystkie materiały" },
  { klucz: "odstep_rysunki_dni", etykieta: "Rysunki przed zamówieniem materiałów", pomoc: "Rysunki wykonawcze gotowe N dni przed najwcześniejszym planowym zamówieniem" },
  { klucz: "odstep_pomiar_dni", etykieta: "Pomiar przed rysunkami", pomoc: "Pomiar końcowy N dni przed terminem rysunków" },
  { klucz: "odstep_potwierdzenie_montazu_dni", etykieta: "Potwierdzenie montażu klientowi", pomoc: "Zadanie „potwierdź termin montażu” N dni przed montażem" },
  { klucz: "odstep_pomieszczenie_gotowe_dni", etykieta: "Pomieszczenie gotowe do montażu", pomoc: "Termin dla klienta N dni przed montażem" },
  { klucz: "prog_pomaranczowy_dni", etykieta: "Próg pomarańczowy (zamówienia)", pomoc: "Niezamówiony materiał robi się pomarańczowy N dni przed datą krytyczną" },
  { klucz: "prog_zbliza_sie_dni", etykieta: "Próg żółty (pozostałe zadania)", pomoc: "Pomiar, rysunki, terminy klienta – żółte N dni przed terminem" },
];

function SekcjaOdstepow({ ustawienia, onWynik }: { ustawienia: UstawieniaTerminow | null; onWynik: (w: WynikPrzeliczenia, k: string) => void }) {
  const { api } = usePolaczenie();
  const poczatkowe = useMemo<PolaUstawien | null>(
    () => ustawienia && (Object.fromEntries(Object.entries(ustawienia).map(([k, v]) => [k, String(v)])) as PolaUstawien),
    [ustawienia],
  );
  const szkic = useSzkic<PolaUstawien>("kalendarz:ustawienia", poczatkowe);
  const f = szkic.wartosc;
  const [blad, ustawBlad] = useState<string | null>(null);
  const [zapisywanie, ustawZapisywanie] = useState(false);
  if (!f || !ustawienia) return null;
  const bledy = Object.fromEntries(POLA_LICZBOWE.map((p) => [p.klucz, liczba(f[p.klucz], 0, 365) === null ? "0–365" : null]));
  const poprawny = Object.values(bledy).every((b) => !b);
  const zapisz = async () => {
    if (!api || !poprawny) return;
    const zmiana: Partial<UstawieniaTerminow> = {};
    if (f.liczenie_dni !== ustawienia.liczenie_dni) zmiana.liczenie_dni = f.liczenie_dni as UstawieniaTerminow["liczenie_dni"];
    for (const p of POLA_LICZBOWE) if (Number(f[p.klucz]) !== ustawienia[p.klucz]) zmiana[p.klucz] = Number(f[p.klucz]);
    if (Object.keys(zmiana).length === 0) return szkic.wyczysc();
    ustawZapisywanie(true);
    try {
      const w = await api.kalendarz.zmienUstawienia(zmiana);
      szkic.wyczysc();
      ustawBlad(null);
      onWynik(w, "Zapisano ustawienia terminów.");
    } catch (e) {
      ustawBlad(komunikatBledu(e));
    } finally {
      ustawZapisywanie(false);
    }
  };
  return (
    <section id="odstepy" className="karta sekcja-ustawien">
      <h2>Liczenie dni, odstępy i progi ostrzeżeń</h2>
      {szkic.przywrocono && (
        <Komunikat rodzaj="uwaga">
          <History size={12} aria-hidden /> Przywrócono niezapisane zmiany.{" "}
          <button type="button" className="link" onClick={() => szkic.wyczysc()}>
            Odrzuć
          </button>
        </Komunikat>
      )}
      <div className="tryb-dni">
        <span className="pole__etykieta" id="tryb-dni-etykieta">
          Jak liczyć dni
        </span>
        <Segment
          etykieta="Jak liczyć dni"
          wartosc={f.liczenie_dni}
          onZmiana={(w) => szkic.zmien({ ...f, liczenie_dni: w })}
          opcje={[
            { wartosc: "robocze", etykieta: "Dni robocze" },
            { wartosc: "kalendarzowe", etykieta: "Dni kalendarzowe" },
          ]}
        />
        <p className="pole__pomoc">
          {f.liczenie_dni === "robocze"
            ? "Dni robocze: soboty, niedziele, święta ustawowe i Twoje dni wolne nie są liczone. „5 dni dostawy” od poniedziałku = materiał w następny poniedziałek. Wybierz, jeśli dostawcy podają czasy w dniach roboczych."
            : "Dni kalendarzowe: liczy się każdy dzień, ale termin wypadający w dzień wolny przesuwa się na wcześniejszy dzień roboczy. Wybierz, jeśli dostawcy podają czasy w dniach kalendarzowych."}{" "}
          Dotyczy czasów dostaw, warsztatu, montażu, odstępów i progów.
        </p>
      </div>
      <div className="siatka-pol">
        {POLA_LICZBOWE.map((p) => (
          <Pole key={p.klucz} etykieta={`${p.etykieta} (dni)`} id={`ust-${p.klucz}`} pomoc={p.pomoc} blad={bledy[p.klucz]}>
            <input id={`ust-${p.klucz}`} inputMode="numeric" value={f[p.klucz]} onChange={(e: ChangeEvent<HTMLInputElement>) => szkic.zmien({ ...f, [p.klucz]: e.target.value })} />
          </Pole>
        ))}
      </div>
      <div className="przyciski-sekcji">
        <Przycisk wariant="glowny" ikona={Save} disabled={!poprawny || zapisywanie || !szkic.zmieniono} onClick={() => void zapisz()}>
          {zapisywanie ? "Zapisywanie i przeliczanie…" : "Zapisz i przelicz terminy"}
        </Przycisk>
        {szkic.zmieniono && (
          <Przycisk wariant="tekstowy" onClick={() => szkic.wyczysc()}>
            Anuluj zmiany
          </Przycisk>
        )}
      </div>
      {blad && <Komunikat rodzaj="blad">{blad}</Komunikat>}
    </section>
  );
}

// ---------------------------------------------------------------------------------------------
// Dni wolne
// ---------------------------------------------------------------------------------------------

function SekcjaDniWolnych({ dni, swieta, onWynik }: { dni: DzienWolny[]; swieta: Swieto[]; onWynik: (w: WynikPrzeliczenia | null, k: string) => void }) {
  const { api } = usePolaczenie();
  const [f, ustawF] = useState({ od: "", do: "", rodzaj: "urlop" as RodzajDniaWolnego, opis: "" });
  const [blad, ustawBlad] = useState<string | null>(null);
  const [pokazSwieta, ustawPokazSwieta] = useState(false);
  const bladOd = f.od && !poprawnaData(f.od) ? "Wpisz datę" : null;
  const bladDo = f.do && (!poprawnaData(f.do) || f.do < f.od) ? "Koniec nie może być przed początkiem" : null;
  const dodaj = async () => {
    if (!api || !f.od || bladOd || bladDo) return ustawBlad("Wpisz datę początku (i ewentualnie końca) zakresu.");
    try {
      if (f.do && f.do !== f.od) {
        const w = await api.kalendarz.dodajZakresDniWolnych({ od: f.od, do: f.do, rodzaj: f.rodzaj, opis: f.opis || null });
        onWynik(w, `Dodano ${w.dni} dni wolnych.`);
      } else {
        const w = await api.kalendarz.ustawDzienWolny(f.od, { rodzaj: f.rodzaj, opis: f.opis || null });
        onWynik(w, `Dodano dzień wolny ${formatujDate(f.od)}.`);
      }
      ustawF({ od: "", do: "", rodzaj: f.rodzaj, opis: "" });
      ustawBlad(null);
    } catch (e) {
      ustawBlad(komunikatBledu(e));
    }
  };
  // Grupowanie kolejnych dni o tym samym opisie w zakresy.
  const grupy = useMemo(() => {
    const g: { od: string; do: string; rodzaj: RodzajDniaWolnego; opis: string | null }[] = [];
    for (const d of [...dni].sort((a, b) => a.data.localeCompare(b.data))) {
      const ost = g[g.length - 1];
      if (ost && ost.rodzaj === d.rodzaj && ost.opis === d.opis && dodajDni(ost.do, 1) === d.data) ost.do = d.data;
      else g.push({ od: d.data, do: d.data, rodzaj: d.rodzaj, opis: d.opis });
    }
    return g;
  }, [dni]);
  return (
    <section id="dni-wolne" className="karta sekcja-ustawien">
      <h2>Dni wolne firmy</h2>
      <p className="pole__pomoc">Soboty, niedziele i święta ustawowe program zna sam. Tu dopisz urlopy, przerwy u dostawców i inne dni bez pracy – terminy je ominą.</p>
      <div className="formularz-dni">
        <Pole etykieta="Od" id="dw-od" blad={bladOd}>
          <input id="dw-od" type="date" value={f.od} onChange={(e) => ustawF({ ...f, od: e.target.value })} />
        </Pole>
        <Pole etykieta="Do (dla zakresu)" id="dw-do" blad={bladDo}>
          <input id="dw-do" type="date" min={f.od} value={f.do} onChange={(e) => ustawF({ ...f, do: e.target.value })} />
        </Pole>
        <Pole etykieta="Rodzaj" id="dw-rodzaj">
          <select id="dw-rodzaj" value={f.rodzaj} onChange={(e) => ustawF({ ...f, rodzaj: e.target.value as RodzajDniaWolnego })}>
            {Object.entries(RODZAJE).map(([k, v]) => (
              <option key={k} value={k}>
                {v}
              </option>
            ))}
          </select>
        </Pole>
        <Pole etykieta="Opis" id="dw-opis">
          <input id="dw-opis" value={f.opis} maxLength={200} placeholder="np. urlop letni" onChange={(e) => ustawF({ ...f, opis: e.target.value })} />
        </Pole>
        <Przycisk ikona={Plus} onClick={() => void dodaj()}>
          Dodaj
        </Przycisk>
      </div>
      {blad && <Komunikat rodzaj="blad">{blad}</Komunikat>}
      {grupy.length === 0 ? (
        <p className="pole__pomoc">Brak dodanych dni wolnych.</p>
      ) : (
        <table className="tabela">
          <thead>
            <tr>
              <th>Termin</th>
              <th>Rodzaj</th>
              <th>Opis</th>
              <th />
            </tr>
          </thead>
          <tbody>
            {grupy.map((g) => (
              <tr key={g.od}>
                <td>{g.od === g.do ? formatujDate(g.od) : `${formatujDate(g.od)} – ${formatujDate(g.do)}`}</td>
                <td>{RODZAJE[g.rodzaj] ?? g.rodzaj}</td>
                <td>{g.opis ?? "—"}</td>
                <td>
                  <Przycisk
                    maly
                    wariant="tekstowy"
                    ikona={Trash2}
                    onClick={async () => {
                      if (!api) return;
                      try {
                        if (g.od === g.do) {
                          await api.kalendarz.usunDzienWolny(g.od);
                          onWynik(null, "Usunięto dzień wolny – terminy przeliczone.");
                        } else {
                          onWynik(await api.kalendarz.usunZakresDniWolnych(g.od, g.do), "Usunięto zakres dni wolnych.");
                        }
                      } catch (e) {
                        ustawBlad(komunikatBledu(e));
                      }
                    }}
                  >
                    Usuń
                  </Przycisk>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
      <button type="button" className="link" aria-expanded={pokazSwieta} onClick={() => ustawPokazSwieta((p) => !p)}>
        {pokazSwieta ? "Ukryj" : "Pokaż"} święta ustawowe ({swieta.length})
      </button>
      {pokazSwieta && (
        <ul className="lista-swiat">
          {swieta.map((s) => (
            <li key={s.data}>
              <strong>{formatujDate(s.data)}</strong> {s.nazwa}
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

// ---------------------------------------------------------------------------------------------
// Import CSV
// ---------------------------------------------------------------------------------------------

function SekcjaImportu({ onWynik }: { onWynik: (w: WynikPrzeliczenia, k: string) => void }) {
  const { api } = usePolaczenie();
  const [plik, ustawPlik] = useState<string | null>(null);
  const [parsowanie, ustawParsowanie] = useState<WynikParsowania | null>(null);
  const [raport, ustawRaport] = useState<RaportImportuKategorii | null>(null);
  const [blad, ustawBlad] = useState<string | null>(null);
  const [trwa, ustawTrwa] = useState(false);
  const wczytaj = async (e: ChangeEvent<HTMLInputElement>) => {
    const p = e.target.files?.[0];
    e.target.value = "";
    if (!p) return;
    ustawRaport(null);
    ustawBlad(null);
    if (p.size > 2_000_000) return ustawBlad("Plik jest za duży (ponad 2 MB).");
    ustawPlik(p.name);
    ustawParsowanie(parsujKategorieCsv(await p.text()));
  };
  const importuj = async () => {
    if (!api || !parsowanie || parsowanie.wiersze.length === 0) return;
    ustawTrwa(true);
    try {
      const r = await api.czasyDostaw.importuj(parsowanie.wiersze);
      ustawRaport(r);
      ustawParsowanie(null);
      onWynik(r, "Zaimportowano plik.");
    } catch (e) {
      ustawBlad(komunikatBledu(e));
    } finally {
      ustawTrwa(false);
    }
  };
  const pominiete = [...(parsowanie?.pominiete ?? []), ...(raport?.pominiete ?? [])].sort((a, b) => a.wiersz - b.wiersz);
  return (
    <section id="import" className="karta sekcja-ustawien">
      <h2>Import kategorii i czasów dostaw z pliku</h2>
      <p className="pole__pomoc">
        Plik <code>kategorie-materialow.csv</code>: kolumny <code>kategoria; przykladowe_materialy; liczba_dokumentow; dostawca; czas_dostawy_min_dni; czas_dostawy_max_dni</code>,
        separator średnik, UTF-8. Brakujące kategorie i dostawcy zostaną dodani, istniejące – uzupełnione. Wiersz z dostawcą ustawia czas tej pary, bez dostawcy – widełki
        kategorii.
      </p>
      <label className="przycisk przycisk--drugorzedny wybor-pliku">
        <Upload size={16} aria-hidden />
        Wybierz plik CSV…
        <input type="file" accept=".csv,text/csv" onChange={(e) => void wczytaj(e)} />
      </label>
      {parsowanie?.blad && <Komunikat rodzaj="blad">{parsowanie.blad}</Komunikat>}
      {parsowanie && !parsowanie.blad && (
        <div className="podglad-importu">
          <p>
            <strong>{plik}</strong>: {parsowanie.wiersze.length} wierszy do importu, {parsowanie.pominiete.length} pominiętych.
          </p>
          <div className="tabela-przewijana tabela-przewijana--niska">
            <table className="tabela">
              <thead>
                <tr>
                  <th>Wiersz</th>
                  <th>Kategoria</th>
                  <th>Dostawca</th>
                  <th>Czas dostawy</th>
                </tr>
              </thead>
              <tbody>
                {parsowanie.wiersze.slice(0, 200).map((w) => (
                  <tr key={w.wiersz}>
                    <td>{w.wiersz}</td>
                    <td>{w.kategoria}</td>
                    <td>{w.dostawca ?? "—"}</td>
                    <td>{w.czas_dostawy_min_dni !== null ? `${w.czas_dostawy_min_dni}–${w.czas_dostawy_max_dni} dni` : "nie podano"}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          <Przycisk wariant="glowny" ikona={Upload} onClick={() => void importuj()} disabled={trwa || parsowanie.wiersze.length === 0}>
            {trwa ? "Importowanie…" : `Importuj ${parsowanie.wiersze.length} wierszy`}
          </Przycisk>
        </div>
      )}
      {raport && (
        <Komunikat rodzaj="ok">
          Kategorie: dodano {raport.kategorie_dodane}, uzupełniono {raport.kategorie_zaktualizowane}. Dostawcy: dodano {raport.dostawcy_dodani}. Ustawiono czasów dostaw:{" "}
          {raport.czasy_dostaw_ustawione}.{raport.bez_czasu_dostawy.length > 0 && ` Bez czasu dostawy (do uzupełnienia): wiersze ${raport.bez_czasu_dostawy.join(", ")}.`}
        </Komunikat>
      )}
      {pominiete.length > 0 && (
        <>
          <h3>Pominięte wiersze</h3>
          <table className="tabela tabela-bledow">
            <thead>
              <tr>
                <th>Wiersz</th>
                <th>Powód</th>
              </tr>
            </thead>
            <tbody>
              {pominiete.map((p, i) => (
                <tr key={`${p.wiersz}-${i}`}>
                  <td>{p.wiersz}</td>
                  <td>{p.powod}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </>
      )}
      {blad && <Komunikat rodzaj="blad">{blad}</Komunikat>}
    </section>
  );
}

// ---------------------------------------------------------------------------------------------

export function UstawieniaKalendarza() {
  const { api } = usePolaczenie();
  const [kategorie, ustawKategorie] = useState<KategoriaMaterialu[]>([]);
  const [dostawcy, ustawDostawcow] = useState<Dostawca[]>([]);
  const [czasy, ustawCzasy] = useState<CzasDostawy[]>([]);
  const [statystyka, ustawStatystyke] = useState<StatystykaPary[]>([]);
  const [ustawienia, ustawUstawienia] = useState<UstawieniaTerminow | null>(null);
  const [dni, ustawDni] = useState<DzienWolny[]>([]);
  const [swieta, ustawSwieta] = useState<Swieto[]>([]);
  const [blad, ustawBlad] = useState<string | null>(null);
  const [wynik, ustawWynik] = useState<{ w: WynikPrzeliczenia | null; k: string; nieaktualne: Zlecenie[] } | null>(null);
  const dzis = dzisIso();

  const pobierz = useCallback(() => {
    if (!api) return;
    Promise.all([
      api.slowniki.kategorieMaterialow(),
      api.dostawcy.lista(),
      api.czasyDostaw.lista(),
      api.kalendarz.statystykaDostaw(),
      api.kalendarz.ustawienia(),
      api.kalendarz.dniWolne(dodajDni(dzis, -60), dodajDni(dzis, 730)),
      api.kalendarz.swieta(rokMiesiac(dzis).rok),
    ]).then(
      ([k, d, c, s, u, w, sw]) => {
        ustawKategorie(k);
        ustawDostawcow(d);
        ustawCzasy(c);
        ustawStatystyke(s);
        ustawUstawienia(u);
        ustawDni(w);
        ustawSwieta(sw);
        ustawBlad(null);
      },
      (e: unknown) => ustawBlad(komunikatBledu(e)),
    );
  }, [api, dzis]);
  useEffect(pobierz, [pobierz]);
  useOdswiezaniePoZdarzeniu(pobierz);

  // Gdy trasa nie zwraca raportu (widełki kategorii, usunięcia) – pokazujemy zlecenia ze znacznikiem nieaktualności.
  const poZapisie = async (w: WynikPrzeliczenia | null, k: string) => {
    let nieaktualne: Zlecenie[] = [];
    if (!w && api) {
      try {
        nieaktualne = (await api.zlecenia.lista()).filter((z) => z.harmonogram_nieaktualny);
      } catch {
        /* raport bez listy */
      }
    }
    ustawWynik({ w, k, nieaktualne });
    pobierz();
  };

  return (
    <div className="ustawienia-kalendarza">
      <header className="naglowek-modulu">
        <div>
          <Link to="/kalendarz" className="link-wstecz">
            <ArrowLeft size={14} aria-hidden /> Kalendarz
          </Link>
          <h1>Ustawienia kalendarza</h1>
          <p className="tekst-drugorzedny">Wspólne dla wszystkich stanowisk. Powiadomienia, autostart i widżet ustawiasz osobno na każdym komputerze w module Ustawienia.</p>
        </div>
      </header>
      <nav className="spis-sekcji" aria-label="Sekcje ustawień">
        <a href="#czasy-dostaw" onClick={(e) => (e.preventDefault(), document.getElementById("czasy-dostaw")?.scrollIntoView())}>
          Czasy dostaw
        </a>
        <a href="#dostawcy" onClick={(e) => (e.preventDefault(), document.getElementById("dostawcy")?.scrollIntoView())}>
          Dostawcy
        </a>
        <a href="#odstepy" onClick={(e) => (e.preventDefault(), document.getElementById("odstepy")?.scrollIntoView())}>
          Liczenie dni i odstępy
        </a>
        <a href="#dni-wolne" onClick={(e) => (e.preventDefault(), document.getElementById("dni-wolne")?.scrollIntoView())}>
          Dni wolne
        </a>
        <a href="#import" onClick={(e) => (e.preventDefault(), document.getElementById("import")?.scrollIntoView())}>
          Import z pliku
        </a>
      </nav>
      {blad && <Komunikat rodzaj="blad">{blad}</Komunikat>}
      {wynik && (
        <div className="wynik-zapisu" aria-live="polite">
          {wynik.w ? (
            <RaportPrzeliczenia wynik={wynik.w} naglowek={wynik.k} />
          ) : (
            <Komunikat rodzaj={wynik.nieaktualne.length ? "uwaga" : "ok"}>
              {wynik.k} Terminy zleceń przeliczono.
              {wynik.nieaktualne.length > 0 && (
                <>
                  {" "}
                  Nieaktualne harmonogramy:{" "}
                  {wynik.nieaktualne.map((z, i) => (
                    <span key={z.id}>
                      {i > 0 && ", "}
                      <Link to={`/kalendarz/zlecenie/${z.id}`}>{z.nazwa}</Link> ({z.harmonogram_nieaktualny_powod})
                    </span>
                  ))}
                </>
              )}
            </Komunikat>
          )}
        </div>
      )}
      <SekcjaCzasowDostaw kategorie={kategorie} dostawcy={dostawcy} czasy={czasy} statystyka={statystyka} odswiez={pobierz} onWynik={(w, k) => void poZapisie(w, k)} />
      <SekcjaDostawcow dostawcy={dostawcy} onZmiana={pobierz} />
      <SekcjaOdstepow ustawienia={ustawienia} onWynik={(w, k) => void poZapisie(w, k)} />
      <SekcjaDniWolnych dni={dni} swieta={swieta} onWynik={(w, k) => void poZapisie(w, k)} />
      <SekcjaImportu onWynik={(w, k) => void poZapisie(w, k)} />
    </div>
  );
}
