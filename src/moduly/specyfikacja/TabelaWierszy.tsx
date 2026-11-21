// Tabela wierszy jednej części: edycja w miejscu, obsługa z klawiatury, podpowiedzi ze słowników.
//
// Klawiatura: Tab / Shift+Tab – następna/poprzednia komórka (Tab w ostatniej komórce ostatniego
// wiersza dodaje wiersz), Enter – zapisz i dodaj wiersz poniżej, Ctrl+↑/↓ – wiersz wyżej/niżej,
// Ctrl+D – kopiuj materiał w dół, Ctrl+Delete – usuń wiersz, Esc – cofnij zmiany w wierszu.

import { Fragment, useEffect, useId, useMemo, useRef, useState, type FocusEvent, type KeyboardEvent } from "react";
import { ArrowDownToLine, CircleCheck, CircleHelp, CircleSlash, Link2, Package, Plus, Trash2, UserRound } from "lucide-react";
import type { Czesc, DaneWiersza, Podgrupa, Podpowiedzi, StatusWiersza, Wiersz, WynikZmiany } from "../../api/typy";
import { formatujKwote, parsujKwote } from "../../lib/formaty";
import { Przycisk } from "../../ui";
import {
  KOLUMNY,
  KOLUMNY_AGD,
  STATUSY_WIERSZA,
  czyZmieniony,
  daneZWiersza,
  formatujIlosc,
  parsujIlosc,
  podgrupaDlaRodzaju,
  podpowiedziDla,
  wierszePogrupowane,
  wierszeWidoczne,
  type Kolumna,
} from "./logika";

export const IKONY_STATUSU: Record<StatusWiersza, typeof CircleCheck> = {
  ustalone: CircleCheck,
  do_potwierdzenia: CircleHelp,
  po_stronie_klienta: UserRound,
  poza_wycena: CircleSlash,
};

interface Props {
  czesc: Czesc;
  agd: boolean;
  tylkoDoOdczytu: boolean;
  podpowiedzi: Podpowiedzi;
  /** Wykonuje zmianę na serwerze i podmienia specyfikację; zwraca wynik albo null przy błędzie. */
  wykonaj: (z: Promise<WynikZmiany>) => Promise<WynikZmiany | null>;
  api: {
    dodajWiersz: (czescId: number, d: DaneWiersza) => Promise<WynikZmiany>;
    zmienWiersz: (id: number, d: DaneWiersza) => Promise<WynikZmiany>;
    usunWiersz: (id: number) => Promise<WynikZmiany>;
    kopiujMaterialWDol: (id: number) => Promise<WynikZmiany>;
  };
}

type Szkice = Record<number, DaneWiersza>;

export function TabelaWierszy({ czesc, agd, tylkoDoOdczytu, podpowiedzi, wykonaj, api }: Props) {
  const [szkice, ustawSzkice] = useState<Szkice>({});
  const [doFokusu, ustawDoFokusu] = useState<{ id: number; kolumna: Kolumna } | null>(null);
  const tabela = useRef<HTMLTableElement>(null);
  const prefiks = useId();
  const kolumny = agd ? KOLUMNY_AGD : KOLUMNY;
  const widoczne = useMemo(() => wierszeWidoczne(czesc), [czesc]);
  const zapisywane = useRef(new Set<number>());

  // Po odświeżeniu z serwera usuwamy szkice wierszy, których już nie ma.
  useEffect(() => {
    ustawSzkice((s) => {
      const ids = new Set(czesc.wiersze.map((w) => w.id));
      const nowe: Szkice = {};
      let zmiana = false;
      for (const [k, v] of Object.entries(s)) {
        if (ids.has(Number(k))) nowe[Number(k)] = v;
        else zmiana = true;
      }
      return zmiana ? nowe : s;
    });
  }, [czesc]);

  useEffect(() => {
    if (!doFokusu) return;
    const el = tabela.current?.querySelector<HTMLElement>(`[data-komorka="${doFokusu.id}-${doFokusu.kolumna}"]`);
    if (el) {
      el.focus();
      if (el instanceof HTMLInputElement) el.select();
      ustawDoFokusu(null);
    }
  }, [doFokusu, czesc]);

  const dane = (w: Wiersz): DaneWiersza => ({ ...daneZWiersza(w), ...szkice[w.id] });

  const zmienPole = (w: Wiersz, pole: keyof DaneWiersza, wartosc: unknown) =>
    ustawSzkice((s) => ({ ...s, [w.id]: { ...dane(w), ...s[w.id], [pole]: wartosc } }));

  /** Zapis wiersza, jeśli się zmienił. */
  const zapisz = async (w: Wiersz): Promise<void> => {
    const szkic = szkice[w.id];
    if (!szkic || tylkoDoOdczytu || zapisywane.current.has(w.id)) return;
    if (!czyZmieniony(w, szkic)) {
      ustawSzkice((s) => {
        const { [w.id]: _pominiety, ...reszta } = s;
        void _pominiety;
        return reszta;
      });
      return;
    }
    const d: DaneWiersza = { ...daneZWiersza(w), ...szkic };
    // Podgrupa ze słownika, gdy wpisano rodzaj wyposażenia w pustym wierszu.
    if (szkic.rodzaj && szkic.rodzaj !== w.rodzaj && !w.rodzaj) {
      const pg = podgrupaDlaRodzaju(szkic.rodzaj, podpowiedzi);
      if (pg && !agd) d.podgrupa = pg;
    }
    zapisywane.current.add(w.id);
    const wynik = await wykonaj(api.zmienWiersz(w.id, d));
    zapisywane.current.delete(w.id);
    if (wynik) {
      ustawSzkice((s) => {
        const { [w.id]: _zapisany, ...reszta } = s;
        void _zapisany;
        return reszta;
      });
    }
  };

  const dodaj = async (po: Wiersz | null, podgrupa: Podgrupa) => {
    if (tylkoDoOdczytu) return;
    if (po) await zapisz(po);
    const w = await wykonaj(api.dodajWiersz(czesc.id, { podgrupa, po_wierszu_id: po?.id ?? null, ilosc: null }));
    if (w) ustawDoFokusu({ id: w.id, kolumna: "rodzaj" });
  };

  const usun = async (w: Wiersz) => {
    const i = widoczne.findIndex((x) => x.id === w.id);
    const sasiad = widoczne[i + 1] ?? widoczne[i - 1];
    await wykonaj(api.usunWiersz(w.id));
    if (sasiad) ustawDoFokusu({ id: sasiad.id, kolumna: "rodzaj" });
  };

  const kopiujWDol = async (w: Wiersz) => {
    await zapisz(w);
    await wykonaj(api.kopiujMaterialWDol(w.id));
  };

  const klawisz = (e: KeyboardEvent<HTMLElement>, w: Wiersz, kolumna: Kolumna) => {
    const i = widoczne.findIndex((x) => x.id === w.id);
    const k = kolumny.indexOf(kolumna);
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      void dodaj(w, w.podgrupa);
    } else if (e.key === "Tab" && !e.shiftKey && k === kolumny.length - 1 && i === widoczne.length - 1) {
      e.preventDefault();
      void dodaj(w, w.podgrupa);
    } else if (e.ctrlKey && (e.key === "ArrowDown" || e.key === "ArrowUp")) {
      e.preventDefault();
      const cel = widoczne[i + (e.key === "ArrowDown" ? 1 : -1)];
      if (cel) ustawDoFokusu({ id: cel.id, kolumna });
    } else if (e.ctrlKey && (e.key === "d" || e.key === "D")) {
      e.preventDefault();
      void kopiujWDol(w);
    } else if (e.ctrlKey && e.key === "Delete") {
      e.preventDefault();
      void usun(w);
    } else if (e.key === "Escape") {
      ustawSzkice((s) => {
        const { [w.id]: _cofniety, ...reszta } = s;
        void _cofniety;
        return reszta;
      });
    }
  };

  /** Zapis po opuszczeniu wiersza (fokus poza <tr>). */
  const opuszczenieWiersza = (e: FocusEvent<HTMLTableRowElement>, w: Wiersz) => {
    const cel = e.relatedTarget as Node | null;
    if (cel && e.currentTarget.contains(cel)) return;
    void zapisz(w);
  };

  const listaId = (kol: Kolumna, pg: Podgrupa) => `${prefiks}-${kol}-${pg}`;

  const komorkaTekstowa = (w: Wiersz, kol: "rodzaj" | "material" | "opis" | "jednostka", d: DaneWiersza) => (
    <input
      className="komorka"
      data-komorka={`${w.id}-${kol}`}
      value={(d[kol] as string | null) ?? ""}
      list={listaId(kol, w.podgrupa)}
      readOnly={tylkoDoOdczytu}
      aria-label={NAGLOWKI[kol]}
      onChange={(e) => zmienPole(w, kol, e.target.value)}
      onKeyDown={(e) => klawisz(e, w, kol)}
    />
  );

  const grupy = wierszePogrupowane(czesc);
  const podgrupyDoList: Podgrupa[] = ["glowna", "wyposazenie", "frezowanie"];

  return (
    <div className="tabela-wierszy">
      {podgrupyDoList.map((pg) =>
        (["rodzaj", "material", "opis", "jednostka"] as const).map((kol) => (
          <datalist key={`${kol}-${pg}`} id={listaId(kol, pg)}>
            {podpowiedziDla(kol, pg, agd, podpowiedzi).map((v) => (
              <option key={v} value={v} />
            ))}
          </datalist>
        )),
      )}
      <table className="tabela tabela--edycja" ref={tabela}>
        <colgroup>
          <col className="kol-rodzaj" />
          <col className="kol-material" />
          <col />
          <col className="kol-ilosc" />
          <col className="kol-jm" />
          <col className="kol-status" />
          {agd && <col className="kol-cena" />}
          <col className="kol-akcje" />
        </colgroup>
        <thead>
          <tr>
            {kolumny.map((k) => (
              <th key={k} className={k === "cena" || k === "ilosc" ? "kolumna-kwoty" : undefined}>
                {NAGLOWKI[k]}
              </th>
            ))}
            <th>
              <span className="sr-only">Akcje</span>
            </th>
          </tr>
        </thead>
        <tbody>
          {grupy.map((g) => (
            <Fragment key={g.podgrupa}>
              {g.podgrupa !== "glowna" && (
                <tr className="tabela-wierszy__grupa">
                  <td colSpan={kolumny.length + 1}>
                    <Package size={13} aria-hidden /> {g.etykieta}
                  </td>
                </tr>
              )}
              {g.wiersze.map((w) => {
                const d = dane(w);
                const Ikona = IKONY_STATUSU[d.status ?? "ustalone"];
                const zmieniony = !!szkice[w.id] && czyZmieniony(w, szkice[w.id]!);
                return (
                  <tr
                    key={w.id}
                    className={`tabela-wierszy__wiersz status-wiersza--${d.status}${zmieniony ? " tabela-wierszy__wiersz--zmieniony" : ""}`}
                    onBlur={(e) => opuszczenieWiersza(e, w)}
                  >
                    <td>{komorkaTekstowa(w, "rodzaj", d)}</td>
                    <td>
                      <div className="komorka-z-ikona">
                        {komorkaTekstowa(w, "material", d)}
                        {w.material_z_korpusu && !szkice[w.id]?.material && (
                          <span className="komorka-z-ikona__ikona" title="Materiał z korpusu – zmień ręcznie, aby odłączyć">
                            <Link2 size={13} aria-label="Materiał z korpusu" />
                          </span>
                        )}
                      </div>
                    </td>
                    <td>{komorkaTekstowa(w, "opis", d)}</td>
                    <td>
                      <input
                        className="komorka komorka--liczba"
                        data-komorka={`${w.id}-ilosc`}
                        inputMode="decimal"
                        aria-label="Ilość"
                        readOnly={tylkoDoOdczytu}
                        defaultValue={formatujIlosc(w.ilosc)}
                        key={`il-${w.id}-${w.ilosc}`}
                        onChange={(e) => {
                          const v = parsujIlosc(e.target.value);
                          e.target.setCustomValidity(v === undefined ? "Wpisz liczbę, np. 2 lub 2,5" : "");
                          if (v !== undefined) zmienPole(w, "ilosc", v);
                        }}
                        onKeyDown={(e) => klawisz(e, w, "ilosc")}
                      />
                    </td>
                    <td>{komorkaTekstowa(w, "jednostka", d)}</td>
                    <td>
                      <div className="komorka-status">
                        <Ikona size={14} aria-hidden />
                        <select
                          className="komorka"
                          data-komorka={`${w.id}-status`}
                          aria-label="Status"
                          disabled={tylkoDoOdczytu}
                          value={d.status ?? "ustalone"}
                          onChange={(e) => zmienPole(w, "status", e.target.value)}
                          onKeyDown={(e) => klawisz(e, w, "status")}
                        >
                          {STATUSY_WIERSZA.map((s) => (
                            <option key={s.wartosc} value={s.wartosc}>
                              {s.etykieta}
                            </option>
                          ))}
                        </select>
                      </div>
                    </td>
                    {agd && (
                      <td>
                        <input
                          className="komorka komorka--liczba"
                          data-komorka={`${w.id}-cena`}
                          aria-label="Cena jednostkowa"
                          inputMode="decimal"
                          readOnly={tylkoDoOdczytu}
                          key={`c-${w.id}-${w.cena_gr}`}
                          defaultValue={w.cena_gr === null ? "" : formatujKwote(w.cena_gr).replace(/\s?zł$/, "")}
                          onChange={(e) => {
                            const t = e.target.value.trim();
                            const v = t ? parsujKwote(t) : null;
                            e.target.setCustomValidity(t && v === null ? "Wpisz kwotę, np. 1 234,50" : "");
                            if (!t || v !== null) zmienPole(w, "cena_gr", v);
                          }}
                          onKeyDown={(e) => klawisz(e, w, "cena")}
                        />
                      </td>
                    )}
                    <td className="tabela-wierszy__akcje">
                      {!tylkoDoOdczytu && (
                        <>
                          <Przycisk
                            maly
                            wariant="tekstowy"
                            tylkoIkona
                            ikona={ArrowDownToLine}
                            etykieta="Kopiuj materiał w dół (Ctrl+D)"
                            disabled={!d.material}
                            tabIndex={-1}
                            onClick={() => void kopiujWDol(w)}
                          />
                          <Przycisk
                            maly
                            wariant="tekstowy"
                            tylkoIkona
                            ikona={Trash2}
                            etykieta="Usuń wiersz (Ctrl+Delete)"
                            tabIndex={-1}
                            onClick={() => void usun(w)}
                          />
                        </>
                      )}
                    </td>
                  </tr>
                );
              })}
            </Fragment>
          ))}
          {czesc.wiersze.length === 0 && (
            <tr>
              <td colSpan={kolumny.length + 1} className="tabela-wierszy__pusto">
                Część nie ma jeszcze wierszy.
              </td>
            </tr>
          )}
        </tbody>
      </table>
      {!tylkoDoOdczytu && (
        <div className="tabela-wierszy__dodaj">
          <Przycisk maly wariant="tekstowy" ikona={Plus} onClick={() => void dodaj(ostatni(czesc, "glowna"), "glowna")}>
            {agd ? "Dodaj pozycję" : "Dodaj element"}
          </Przycisk>
          {!agd && (
            <>
              <Przycisk maly wariant="tekstowy" ikona={Plus} onClick={() => void dodaj(ostatni(czesc, "wyposazenie"), "wyposazenie")}>
                Dodaj wyposażenie
              </Przycisk>
              <Przycisk maly wariant="tekstowy" ikona={Plus} onClick={() => void dodaj(ostatni(czesc, "frezowanie"), "frezowanie")}>
                Dodaj frezowanie
              </Przycisk>
            </>
          )}
          <span className="tabela-wierszy__skroty">Enter – nowy wiersz · Ctrl+↑/↓ – wiersze · Ctrl+D – materiał w dół · Esc – cofnij</span>
        </div>
      )}
    </div>
  );
}

function ostatni(c: Czesc, pg: Podgrupa): Wiersz | null {
  const w = c.wiersze.filter((x) => x.podgrupa === pg);
  return w[w.length - 1] ?? null;
}

const NAGLOWKI: Record<Kolumna, string> = {
  rodzaj: "Rodzaj",
  material: "Materiał",
  opis: "Opis",
  ilosc: "Ilość",
  jednostka: "j.m.",
  status: "Status",
  cena: "Cena jedn.",
};
