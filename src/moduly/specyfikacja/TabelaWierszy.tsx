// Tabela wierszy jednej części: edycja w miejscu, obsługa z klawiatury, podpowiedzi ze słowników.
//
// Zapis: automatycznie ok. 1 s po ostatnim naciśnięciu klawisza, po opuszczeniu wiersza, Enterem,
// przy ukryciu/zamknięciu karty i (w Tauri) przed schowaniem okna lub „Zakończ”. Wysyłane są
// TYLKO zmienione pola razem z wartościami sprzed edycji (`poprzednie`) – serwer zwraca 409, gdy
// ktoś inny zmienił w międzyczasie to samo pole; wtedy użytkownik wybiera „moja / cudza”.
// Niezapisane szkice mają kopię w localStorage i są odtwarzane po ponownym otwarciu.
//
// Klawiatura: Tab / Shift+Tab – komórki (Tab w ostatniej komórce ostatniego wiersza dodaje
// wiersz), Enter – zapisz i dodaj wiersz poniżej, Ctrl+↑/↓ – przejdź do wiersza wyżej/niżej,
// Alt+↑/↓ – przesuń wiersz w górę/w dół, Ctrl+D – kopiuj materiał w dół, Ctrl+Delete – usuń
// wiersz (z „Cofnij”), Esc – odrzuć niezapisane zmiany wiersza.

import { Fragment, useCallback, useEffect, useId, useMemo, useRef, useState, type DragEvent, type FocusEvent, type KeyboardEvent } from "react";
import {
  ArrowDownToLine,
  CircleAlert,
  CircleCheck,
  CircleDashed,
  CircleHelp,
  CircleSlash,
  FolderInput,
  GripVertical,
  Link2,
  LoaderCircle,
  Package,
  Plus,
  Trash2,
  Undo2,
  UserRound,
} from "lucide-react";
import { BladApi } from "../../api/klient";
import type { Czesc, DaneWiersza, KonfliktPola, Podgrupa, Podpowiedzi, StatusWiersza, Wiersz, WynikZmiany, ZmianaCzesciowa } from "../../api/typy";
import { bladKwoty, formatujKwote, MAKS_ILOSC, parsujKwote } from "../../lib/formaty";
import { wczytajKopie, zapiszKopie, zarejestrujOproznianie } from "../../lib/szkice";
import { Okno, Przycisk } from "../../ui";
import { OknoKonfliktu } from "./OknoKonfliktu";
import {
  KOLUMNY,
  KOLUMNY_AGD,
  STATUSY_WIERSZA,
  daneZWiersza,
  formatujIlosc,
  nazwaPolaWiersza,
  parsujIlosc,
  podgrupaDlaRodzaju,
  podpowiedziDla,
  przesunWiersz,
  wierszePogrupowane,
  wierszeWidoczne,
  wstawPrzed,
  type Kolumna,
} from "./logika";

export const IKONY_STATUSU: Record<StatusWiersza, typeof CircleCheck> = {
  ustalone: CircleCheck,
  do_potwierdzenia: CircleHelp,
  po_stronie_klienta: UserRound,
  poza_wycena: CircleSlash,
};

/** Opóźnienie automatycznego zapisu po ostatnim naciśnięciu klawisza. */
export const OPOZNIENIE_ZAPISU_MS = 1000;

export interface ApiWierszy {
  dodajWiersz: (czescId: number, d: DaneWiersza) => Promise<WynikZmiany>;
  zmienWiersz: (id: number, d: ZmianaCzesciowa<DaneWiersza>) => Promise<WynikZmiany>;
  usunWiersz: (id: number) => Promise<WynikZmiany>;
  kopiujMaterialWDol: (id: number) => Promise<WynikZmiany>;
  kolejnoscWierszy: (czescId: number, ids: number[]) => Promise<WynikZmiany>;
}

interface Props {
  czesc: Czesc;
  agd: boolean;
  tylkoDoOdczytu: boolean;
  podpowiedzi: Podpowiedzi;
  /** Pozostałe części specyfikacji (cel „Przenieś do…”). */
  inneCzesci?: { id: number; etykieta: string; wiersze: number[] }[];
  /**
   * Wykonuje zmianę na serwerze i podmienia specyfikację; zwraca wynik albo null przy błędzie.
   * `przechwyc` może obsłużyć błąd samodzielnie (np. konflikt 409) – wtedy edytor nie pokazuje paska.
   */
  wykonaj: (z: Promise<WynikZmiany>, przechwyc?: (e: BladApi) => boolean) => Promise<WynikZmiany | null>;
  /** Pobiera aktualną specyfikację (po wyborze „cudzej” wartości w konflikcie). */
  odswiez?: () => Promise<void>;
  api: ApiWierszy;
}

type PoleTekstowe = "ilosc" | "cena";

/** Szkic wiersza: tylko zmienione pola, wartości sprzed edycji i surowe teksty liczb. */
export interface Szkic {
  pola: DaneWiersza;
  poprzednie: DaneWiersza;
  teksty: Partial<Record<PoleTekstowe, string>>;
}

type Szkice = Record<number, Szkic>;
type StanZapisu = "zapisano" | "zapisywanie" | "niezapisane" | "blad";

const POLE_TEKSTU: Record<PoleTekstowe, keyof DaneWiersza> = { ilosc: "ilosc", cena: "cena_gr" };

const norm = (v: unknown) => (v === "" || v === undefined ? null : v);

function bladTekstu(pole: PoleTekstowe, t: string): string | null {
  if (pole === "cena") return bladKwoty(t);
  const v = parsujIlosc(t);
  if (v === undefined) return "Wpisz liczbę, np. 2 lub 2,5";
  if (v !== null && v > MAKS_ILOSC) return "Najwyżej 1 000 000";
  return null;
}

function pustySzkic(s: Szkic | undefined): boolean {
  return !s || (Object.keys(s.pola).length === 0 && Object.keys(s.teksty).length === 0);
}

export function TabelaWierszy({ czesc, agd, tylkoDoOdczytu, podpowiedzi, inneCzesci = [], wykonaj, odswiez, api }: Props) {
  const kluczKopii = `wiersze:${czesc.id}`;
  const [szkice, ustawSzkice] = useState<Szkice>(() => {
    const k = wczytajKopie<Szkice>(kluczKopii) ?? {};
    const ids = new Set(czesc.wiersze.map((w) => w.id));
    return Object.fromEntries(Object.entries(k).filter(([id]) => ids.has(Number(id)))) as Szkice;
  });
  const szkiceRef = useRef(szkice);
  szkiceRef.current = szkice;
  const czescRef = useRef(czesc);
  czescRef.current = czesc;
  const [zapisywane, ustawZapisywane] = useState(0);
  const [stanBledu, ustawStanBledu] = useState(false);
  const [konflikt, ustawKonflikt] = useState<{ wiersz: number; konflikty: KonfliktPola[] } | null>(null);
  const [cofnij, ustawCofnij] = useState<{ dane: DaneWiersza; po: number | null; etykieta: string } | null>(null);
  const [przenoszony, ustawPrzenoszony] = useState<Wiersz | null>(null);
  const [ciagniety, ustawCiagniety] = useState<number | null>(null);
  const [doFokusu, ustawDoFokusu] = useState<{ id: number; kolumna: Kolumna } | null>(null);
  const tabela = useRef<HTMLTableElement>(null);
  const timery = useRef(new Map<number, ReturnType<typeof setTimeout>>());
  const wToku = useRef(new Set<number>());
  /** Zapis zlecony w trakcie innego zapisu tego wiersza – wykonywany zaraz po nim. */
  const oczekujace = useRef(new Map<number, Record<string, unknown> | undefined>());
  const konfliktRef = useRef(konflikt);
  konfliktRef.current = konflikt;
  const prefiks = useId();
  const kolumny = agd ? KOLUMNY_AGD : KOLUMNY;
  const widoczne = useMemo(() => wierszeWidoczne(czesc), [czesc]);

  // Kopia awaryjna szkiców w localStorage.
  useEffect(() => {
    zapiszKopie(kluczKopii, Object.fromEntries(Object.entries(szkice).filter(([, s]) => !pustySzkic(s))));
  }, [szkice, kluczKopii]);

  useEffect(() => {
    if (!doFokusu) return;
    const el = tabela.current?.querySelector<HTMLElement>(`[data-komorka="${doFokusu.id}-${doFokusu.kolumna}"]`);
    if (el) {
      el.focus();
      if (el instanceof HTMLInputElement) el.select();
      ustawDoFokusu(null);
    }
  }, [doFokusu, czesc]);

  /** Zapis szkicu wiersza (tylko poprawne, zmienione pola). */
  const zapisz = useCallback(
    async (id: number, wymusPoprzednie?: Record<string, unknown>): Promise<void> => {
      const t = timery.current.get(id);
      if (t) clearTimeout(t);
      timery.current.delete(id);
      const szkic = szkiceRef.current[id];
      const w = czescRef.current.wiersze.find((x) => x.id === id);
      if (!szkic || tylkoDoOdczytu) return;
      if (wToku.current.has(id)) {
        oczekujace.current.set(id, { ...oczekujace.current.get(id), ...wymusPoprzednie });
        return;
      }
      if (!w) {
        // Wiersz usunięty (także na innym stanowisku) – szkic nie ma już czego zapisać.
        ustawSzkice((s) => {
          const reszta = { ...s };
          delete reszta[id];
          return reszta;
        });
        return;
      }
      const baza = daneZWiersza(w);
      const pola: Record<string, unknown> = {};
      for (const [k, v] of Object.entries(szkic.pola)) {
        if (norm(v) !== norm((baza as Record<string, unknown>)[k])) pola[k] = v;
      }
      if (Object.keys(pola).length === 0) {
        // Nic do zapisania (np. wrócono do wartości z serwera) – zostają tylko błędne teksty.
        ustawSzkice((s) => {
          const x = s[id];
          if (!x) return s;
          const reszta = { ...s };
          if (Object.keys(x.teksty).length === 0) delete reszta[id];
          else reszta[id] = { ...x, pola: {}, poprzednie: {} };
          return reszta;
        });
        return;
      }
      // Podgrupa ze słownika, gdy wpisano rodzaj wyposażenia w pustym wierszu.
      if (typeof pola.rodzaj === "string" && !w.rodzaj && !agd && !("podgrupa" in pola)) {
        const pg = podgrupaDlaRodzaju(pola.rodzaj, podpowiedzi);
        if (pg && pg !== w.podgrupa) pola.podgrupa = pg;
      }
      const poprzednie: Record<string, unknown> = {};
      for (const k of Object.keys(pola)) poprzednie[k] = k in szkic.poprzednie ? (szkic.poprzednie as Record<string, unknown>)[k] : (baza as Record<string, unknown>)[k];
      // „Zachowaj moją” po konflikcie: wartości z serwera jako poprzednie.
      for (const [k, v] of Object.entries(wymusPoprzednie ?? {})) if (k in pola) poprzednie[k] = v;
      wToku.current.add(id);
      ustawZapisywane((n) => n + 1);
      let konfliktowy = false;
      const wynik = await wykonaj(api.zmienWiersz(id, { ...(pola as DaneWiersza), poprzednie: poprzednie as DaneWiersza }), (e) => {
        if (e.kod === 409 && e.konflikty.length > 0) {
          konfliktowy = true;
          // Użytkownik już wybrał „Zachowaj moją” (zapis czeka w kolejce) – nie pytamy drugi raz.
          const f = oczekujace.current.get(id);
          if (!(f && Object.keys(f).length > 0)) ustawKonflikt({ wiersz: id, konflikty: e.konflikty });
          return true;
        }
        return false;
      });
      wToku.current.delete(id);
      ustawZapisywane((n) => n - 1);
      if (oczekujace.current.has(id)) {
        const f = oczekujace.current.get(id);
        oczekujace.current.delete(id);
        if (!konfliktowy || (f && Object.keys(f).length > 0)) setTimeout(() => void zapiszRef.current(id, f), 0);
      }
      ustawStanBledu(!wynik && !konfliktowy);
      if (wynik) {
        // Usuwamy z szkicu pola zapisane (chyba że w trakcie zapisu zmieniono je ponownie).
        ustawSzkice((s) => {
          const x = s[id];
          if (!x) return s;
          const nowePola: Record<string, unknown> = { ...x.pola };
          const nowePoprzednie: Record<string, unknown> = { ...x.poprzednie };
          const noweTeksty = { ...x.teksty };
          for (const [k, v] of Object.entries(pola)) {
            if (norm(nowePola[k]) === norm(v)) {
              delete nowePola[k];
              delete nowePoprzednie[k];
              if (k === "ilosc") delete noweTeksty.ilosc;
              if (k === "cena_gr") delete noweTeksty.cena;
            }
          }
          if (k(nowePola) === 0 && k(noweTeksty) === 0) {
            const reszta = { ...s };
            delete reszta[id];
            return reszta;
          }
          return { ...s, [id]: { pola: nowePola as DaneWiersza, poprzednie: nowePoprzednie as DaneWiersza, teksty: noweTeksty } };
        });
      }
    },
    [agd, api, podpowiedzi, tylkoDoOdczytu, wykonaj],
  );

  // Stabilne odwołanie do najnowszego `zapisz` – rejestracja i sprzątanie tylko raz (inaczej
  // każda zmiana danych rodzica wywoływałaby zapis „przy odmontowaniu” i podwójne żądania).
  const zapiszRef = useRef(zapisz);
  useEffect(() => {
    zapiszRef.current = zapisz;
  }, [zapisz]);
  useEffect(() => {
    const zapiszWszystkie = async () => {
      await Promise.all(Object.keys(szkiceRef.current).map((id) => zapiszRef.current(Number(id))));
    };
    const wyrejestruj = zarejestrujOproznianie(zapiszWszystkie);
    const t = timery.current;
    return () => {
      wyrejestruj();
      for (const x of t.values()) clearTimeout(x);
      void zapiszWszystkie();
    };
  }, []);

  // Szkice odtworzone z kopii – zapis po chwili.
  useEffect(() => {
    for (const id of Object.keys(szkiceRef.current)) zaplanuj(Number(id));
  }, []);

  function zaplanuj(id: number) {
    const t = timery.current.get(id);
    if (t) clearTimeout(t);
    timery.current.set(
      id,
      setTimeout(() => void zapiszRef.current(id), OPOZNIENIE_ZAPISU_MS),
    );
  }

  const dane = (w: Wiersz): DaneWiersza => ({ ...daneZWiersza(w), ...szkice[w.id]?.pola });

  const zmienPole = (w: Wiersz, pole: keyof DaneWiersza, wartosc: unknown) => {
    ustawSzkice((s) => {
      const x = s[w.id] ?? { pola: {}, poprzednie: {}, teksty: {} };
      const baza = daneZWiersza(w) as Record<string, unknown>;
      const poprzednie = { ...x.poprzednie } as Record<string, unknown>;
      if (!(pole in poprzednie)) poprzednie[pole] = baza[pole];
      return { ...s, [w.id]: { ...x, pola: { ...x.pola, [pole]: wartosc }, poprzednie: poprzednie as DaneWiersza } };
    });
    zaplanuj(w.id);
  };

  const zmienTekst = (w: Wiersz, pole: PoleTekstowe, tekst: string) => {
    const blad = bladTekstu(pole, tekst);
    ustawSzkice((s) => {
      const x = s[w.id] ?? { pola: {}, poprzednie: {}, teksty: {} };
      const klucz = POLE_TEKSTU[pole];
      const baza = daneZWiersza(w) as Record<string, unknown>;
      const pola = { ...x.pola } as Record<string, unknown>;
      const poprzednie = { ...x.poprzednie } as Record<string, unknown>;
      if (!(klucz in poprzednie)) poprzednie[klucz] = baza[klucz];
      if (blad) delete pola[klucz];
      else pola[klucz] = pole === "cena" ? (tekst.trim() ? parsujKwote(tekst) : null) : parsujIlosc(tekst);
      return { ...s, [w.id]: { pola: pola as DaneWiersza, poprzednie: poprzednie as DaneWiersza, teksty: { ...x.teksty, [pole]: tekst } } };
    });
    zaplanuj(w.id);
  };

  const odrzucSzkic = (id: number) => {
    const t = timery.current.get(id);
    if (t) clearTimeout(t);
    ustawSzkice((s) => {
      const reszta = { ...s };
      delete reszta[id];
      return reszta;
    });
  };

  const dodaj = async (po: Wiersz | null, podgrupa: Podgrupa) => {
    if (tylkoDoOdczytu) return;
    if (po) await zapisz(po.id);
    const w = await wykonaj(api.dodajWiersz(czesc.id, { podgrupa, po_wierszu_id: po?.id ?? null, ilosc: null }));
    if (w) ustawDoFokusu({ id: w.id, kolumna: "rodzaj" });
  };

  const usun = async (w: Wiersz) => {
    const i = widoczne.findIndex((x) => x.id === w.id);
    const sasiad = widoczne[i + 1] ?? widoczne[i - 1];
    const poprzedni = czesc.wiersze[czesc.wiersze.findIndex((x) => x.id === w.id) - 1] ?? null;
    const d = dane(w);
    odrzucSzkic(w.id);
    const wynik = await wykonaj(api.usunWiersz(w.id));
    if (wynik) {
      ustawCofnij({
        dane: { ...d, podgrupa: w.podgrupa, material_z_korpusu: w.material_z_korpusu },
        po: poprzedni?.id ?? null,
        etykieta: w.rodzaj || "(bez rodzaju)",
      });
      if (sasiad) ustawDoFokusu({ id: sasiad.id, kolumna: "rodzaj" });
    }
  };

  // Pasek „Cofnij” znika po 8 s.
  useEffect(() => {
    if (!cofnij) return;
    const t = setTimeout(() => ustawCofnij(null), 8000);
    return () => clearTimeout(t);
  }, [cofnij]);

  const przywroc = async () => {
    if (!cofnij) return;
    const c = cofnij;
    ustawCofnij(null);
    const po = c.po !== null && czesc.wiersze.some((x) => x.id === c.po) ? c.po : null;
    await wykonaj(api.dodajWiersz(czesc.id, { ...c.dane, po_wierszu_id: po }));
  };

  const przesunKlawiszem = async (w: Wiersz, krok: -1 | 1, kolumna: Kolumna) => {
    await zapisz(w.id);
    const ids = przesunWiersz(czesc, w.id, krok);
    if (!ids) return;
    const r = await wykonaj(api.kolejnoscWierszy(czesc.id, ids));
    if (r) ustawDoFokusu({ id: w.id, kolumna });
  };

  const przeniesDo = async (w: Wiersz, celId: number) => {
    const cel = inneCzesci.find((c) => c.id === celId);
    if (!cel) return;
    await zapisz(w.id);
    const r = await wykonaj(api.kolejnoscWierszy(cel.id, [...cel.wiersze, w.id]));
    if (r) ustawPrzenoszony(null);
  };

  const upusc = async (e: DragEvent<HTMLTableRowElement>, cel: Wiersz) => {
    e.preventDefault();
    const id = Number(e.dataTransfer.getData("text/x-robinwood-wiersz") || ciagniety);
    ustawCiagniety(null);
    if (!id) return;
    const ids = wstawPrzed(czesc, id, cel.id);
    if (ids) await wykonaj(api.kolejnoscWierszy(czesc.id, ids));
  };

  const kopiujWDol = async (w: Wiersz) => {
    await zapisz(w.id);
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
    } else if (e.altKey && (e.key === "ArrowDown" || e.key === "ArrowUp")) {
      e.preventDefault();
      if (!tylkoDoOdczytu) void przesunKlawiszem(w, e.key === "ArrowDown" ? 1 : -1, kolumna);
    } else if (e.ctrlKey && (e.key === "ArrowDown" || e.key === "ArrowUp")) {
      e.preventDefault();
      const cel = widoczne[i + (e.key === "ArrowDown" ? 1 : -1)];
      if (cel) ustawDoFokusu({ id: cel.id, kolumna });
    } else if (e.ctrlKey && (e.key === "d" || e.key === "D")) {
      e.preventDefault();
      void kopiujWDol(w);
    } else if (e.ctrlKey && e.key === "Delete") {
      e.preventDefault();
      if (!tylkoDoOdczytu) void usun(w);
    } else if (e.key === "Escape") {
      odrzucSzkic(w.id);
    }
  };

  /** Zapis po opuszczeniu wiersza (fokus poza <tr>). */
  const opuszczenieWiersza = (e: FocusEvent<HTMLTableRowElement>, w: Wiersz) => {
    const cel = e.relatedTarget as Node | null;
    if (cel && e.currentTarget.contains(cel)) return;
    // Otwarte okno konfliktu tego wiersza – zapis dopiero po decyzji użytkownika.
    if (konfliktRef.current?.wiersz === w.id) return;
    void zapisz(w.id);
  };

  const listaId = (kol: Kolumna, pg: Podgrupa) => `${prefiks}-${kol}-${pg}`;

  const komorkaTekstowa = (w: Wiersz, kol: "rodzaj" | "material" | "opis" | "jednostka", d: DaneWiersza) => {
    const wartosc = (d[kol] as string | null) ?? "";
    return (
      <input
        className="komorka"
        data-komorka={`${w.id}-${kol}`}
        value={wartosc}
        title={wartosc.length > 12 ? wartosc : undefined}
        list={listaId(kol, w.podgrupa)}
        readOnly={tylkoDoOdczytu}
        aria-label={NAGLOWKI[kol]}
        onChange={(e) => zmienPole(w, kol, e.target.value)}
        onKeyDown={(e) => klawisz(e, w, kol)}
      />
    );
  };

  const komorkaLiczby = (w: Wiersz, pole: PoleTekstowe) => {
    const tekst = szkice[w.id]?.teksty[pole];
    const pokazany = tekst ?? (pole === "ilosc" ? formatujIlosc(w.ilosc) : w.cena_gr === null ? "" : formatujKwote(w.cena_gr).replace(/\s?zł$/, ""));
    const blad = tekst !== undefined ? bladTekstu(pole, tekst) : null;
    return (
      <input
        className="komorka komorka--liczba"
        data-komorka={`${w.id}-${pole}`}
        inputMode="decimal"
        aria-label={pole === "ilosc" ? "Ilość" : "Cena jednostkowa"}
        aria-invalid={blad ? true : undefined}
        title={blad ?? undefined}
        readOnly={tylkoDoOdczytu}
        value={pokazany}
        onChange={(e) => zmienTekst(w, pole, e.target.value)}
        onKeyDown={(e) => klawisz(e, w, pole)}
      />
    );
  };

  // Błędy wpisów (pokazywane pod tabelą – nie znikają po cichu).
  const bledyWpisow = czesc.wiersze.flatMap((w) =>
    (Object.entries(szkice[w.id]?.teksty ?? {}) as [PoleTekstowe, string][])
      .map(([p, t]) => ({ w, p, b: bladTekstu(p, t) }))
      .filter((x) => x.b)
      .map((x) => `Wiersz „${x.w.rodzaj || "(bez rodzaju)"}”, ${x.p === "ilosc" ? "Ilość" : "Cena"}: ${x.b} – ta wartość nie została zapisana.`),
  );
  // Szkice wierszy usuniętych na serwerze nie liczą się (i nie są zapisywane).
  const niezapisane = czesc.wiersze.filter((w) => !pustySzkic(szkice[w.id])).length;
  const stan: StanZapisu = zapisywane > 0 ? "zapisywanie" : stanBledu ? "blad" : niezapisane > 0 ? "niezapisane" : "zapisano";

  const grupy = wierszePogrupowane(czesc);
  const podgrupyDoList: Podgrupa[] = ["glowna", "wyposazenie", "frezowanie"];
  const konfliktWiersz = konflikt ? czesc.wiersze.find((x) => x.id === konflikt.wiersz) : null;

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
      {!tylkoDoOdczytu && <WskaznikZapisu stan={stan} niezapisane={niezapisane} />}
      <div className="tabela-wierszy__przewijanie">
        <table className={`tabela tabela--edycja${agd ? " tabela--agd" : ""}`} ref={tabela}>
          <colgroup>
            {!tylkoDoOdczytu && <col className="kol-uchwyt" />}
            <col className="kol-rodzaj" />
            <col className="kol-material" />
            <col className="kol-opis" />
            <col className="kol-ilosc" />
            <col className="kol-jm" />
            <col className="kol-status" />
            {agd && <col className="kol-cena" />}
            <col className="kol-akcje" />
          </colgroup>
          <thead>
            <tr>
              {!tylkoDoOdczytu && (
                <th>
                  <span className="sr-only">Przeciągnij</span>
                </th>
              )}
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
                    <td colSpan={kolumny.length + (tylkoDoOdczytu ? 1 : 2)}>
                      <Package size={13} aria-hidden /> {g.etykieta}
                    </td>
                  </tr>
                )}
                {g.wiersze.map((w) => {
                  const d = dane(w);
                  const Ikona = IKONY_STATUSU[d.status ?? "ustalone"];
                  const zmieniony = !pustySzkic(szkice[w.id]);
                  const etykietaStatusu = STATUSY_WIERSZA.find((s) => s.wartosc === d.status)?.etykieta ?? "";
                  const zIkona = w.material_z_korpusu && !(szkice[w.id]?.pola && "material" in szkice[w.id]!.pola);
                  return (
                    <tr
                      key={w.id}
                      className={`tabela-wierszy__wiersz status-wiersza--${d.status}${zmieniony ? " tabela-wierszy__wiersz--zmieniony" : ""}${ciagniety === w.id ? " tabela-wierszy__wiersz--ciagniety" : ""}`}
                      draggable={ciagniety === w.id}
                      onDragStart={(e) => {
                        e.dataTransfer.setData("text/x-robinwood-wiersz", String(w.id));
                        e.dataTransfer.effectAllowed = "move";
                      }}
                      onDragEnd={() => ustawCiagniety(null)}
                      onDragOver={(e) => ciagniety !== null && e.preventDefault()}
                      onDrop={(e) => void upusc(e, w)}
                      onBlur={(e) => opuszczenieWiersza(e, w)}
                    >
                      {!tylkoDoOdczytu && (
                        <td className="tabela-wierszy__uchwyt">
                          <span
                            role="button"
                            tabIndex={-1}
                            aria-label="Przeciągnij, aby zmienić kolejność (albo Alt+↑/↓)"
                            title="Przeciągnij, aby zmienić kolejność (albo Alt+↑/↓)"
                            onMouseDown={() => ustawCiagniety(w.id)}
                            onMouseUp={() => ustawCiagniety(null)}
                          >
                            <GripVertical size={14} aria-hidden />
                          </span>
                        </td>
                      )}
                      <td>{komorkaTekstowa(w, "rodzaj", d)}</td>
                      <td>
                        <div className={`komorka-z-ikona${zIkona ? " komorka-z-ikona--z-ikona" : ""}`}>
                          {komorkaTekstowa(w, "material", d)}
                          {zIkona && (
                            <span className="komorka-z-ikona__ikona" title="Materiał z korpusu – zmień ręcznie, aby odłączyć">
                              <Link2 size={13} aria-label="Materiał z korpusu" />
                            </span>
                          )}
                        </div>
                      </td>
                      <td>{komorkaTekstowa(w, "opis", d)}</td>
                      <td>{komorkaLiczby(w, "ilosc")}</td>
                      <td>{komorkaTekstowa(w, "jednostka", d)}</td>
                      <td>
                        <div className="komorka-status" title={etykietaStatusu}>
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
                      {agd && <td>{komorkaLiczby(w, "cena")}</td>}
                      <td className="tabela-wierszy__akcje">
                        {!tylkoDoOdczytu && (
                          <>
                            <Przycisk maly wariant="tekstowy" tylkoIkona ikona={ArrowDownToLine} etykieta="Kopiuj materiał w dół (Ctrl+D)" disabled={!d.material} tabIndex={-1} onClick={() => void kopiujWDol(w)} />
                            {inneCzesci.length > 0 && (
                              <Przycisk maly wariant="tekstowy" tylkoIkona ikona={FolderInput} etykieta="Przenieś do innej części" tabIndex={-1} onClick={() => ustawPrzenoszony(w)} />
                            )}
                            <Przycisk maly wariant="tekstowy" tylkoIkona ikona={Trash2} etykieta="Usuń wiersz (Ctrl+Delete)" tabIndex={-1} onClick={() => void usun(w)} />
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
                <td colSpan={kolumny.length + (tylkoDoOdczytu ? 1 : 2)} className="tabela-wierszy__pusto">
                  Część nie ma jeszcze wierszy.
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
      {bledyWpisow.length > 0 && (
        <ul className="tabela-wierszy__bledy" role="alert">
          {bledyWpisow.map((b) => (
            <li key={b} className="komunikat-stanu status--czerwony">
              <CircleAlert size={14} aria-hidden /> {b}
            </li>
          ))}
        </ul>
      )}
      {cofnij && (
        <div className="pasek-cofnij" role="status">
          <span>Usunięto wiersz „{cofnij.etykieta}”.</span>
          <Przycisk maly ikona={Undo2} onClick={() => void przywroc()}>
            Cofnij
          </Przycisk>
        </div>
      )}
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
          <span className="tabela-wierszy__skroty">Enter – nowy wiersz · Ctrl+↑/↓ – przejdź · Alt+↑/↓ – przesuń · Ctrl+D – materiał w dół · Ctrl+Del – usuń · Esc – odrzuć zmiany</span>
        </div>
      )}
      {przenoszony && (
        <Okno tytul={`Przenieś wiersz „${przenoszony.rodzaj || "(bez rodzaju)"}”`} onZamknij={() => ustawPrzenoszony(null)}>
          <p className="pole__pomoc">Wybierz część, na której koniec trafi wiersz:</p>
          <ul className="lista-celow">
            {inneCzesci.map((c) => (
              <li key={c.id}>
                <Przycisk wariant="tekstowy" ikona={FolderInput} onClick={() => void przeniesDo(przenoszony, c.id)}>
                  {c.etykieta}
                </Przycisk>
              </li>
            ))}
          </ul>
        </Okno>
      )}
      {konflikt && (
        <OknoKonfliktu
          tytul={`Konflikt w wierszu „${konfliktWiersz?.rodzaj || "(bez rodzaju)"}”`}
          konflikty={konflikt.konflikty}
          nazwaPola={nazwaPolaWiersza}
          formatuj={(p, v) => (v === null || v === undefined || v === "" ? "(puste)" : p === "cena_gr" ? formatujKwote(Number(v)) : p === "ilosc" ? formatujIlosc(Number(v)) : String(v))}
          onMoja={() => {
            // Moja: zapis ponownie, z cudzą wartością jako „poprzednią”.
            const { wiersz, konflikty } = konflikt;
            ustawSzkice((s) => {
              const x = s[wiersz];
              if (!x) return s;
              const poprzednie = { ...x.poprzednie } as Record<string, unknown>;
              for (const k of konflikty) poprzednie[k.pole] = k.aktualna;
              return { ...s, [wiersz]: { ...x, poprzednie: poprzednie as DaneWiersza } };
            });
            ustawKonflikt(null);
            void zapisz(wiersz, Object.fromEntries(konflikty.map((k) => [k.pole, k.aktualna])));
          }}
          onCudza={() => {
            const { wiersz, konflikty } = konflikt;
            ustawSzkice((s) => {
              const x = s[wiersz];
              if (!x) return s;
              const pola = { ...x.pola } as Record<string, unknown>;
              const poprzednie = { ...x.poprzednie } as Record<string, unknown>;
              const teksty = { ...x.teksty };
              for (const k of konflikty) {
                delete pola[k.pole];
                delete poprzednie[k.pole];
                if (k.pole === "ilosc") delete teksty.ilosc;
                if (k.pole === "cena_gr") delete teksty.cena;
              }
              return { ...s, [wiersz]: { pola: pola as DaneWiersza, poprzednie: poprzednie as DaneWiersza, teksty } };
            });
            ustawKonflikt(null);
            void odswiez?.();
            setTimeout(() => void zapisz(wiersz), 0);
          }}
        />
      )}
    </div>
  );
}

function k(o: object): number {
  return Object.keys(o).length;
}

function WskaznikZapisu({ stan, niezapisane }: { stan: StanZapisu; niezapisane: number }) {
  const opis = {
    zapisywanie: { ikona: LoaderCircle, tekst: "Zapisywanie…", klasa: "status--neutralny" },
    zapisano: { ikona: CircleCheck, tekst: "Zapisano", klasa: "status--spokojnie" },
    niezapisane: { ikona: CircleDashed, tekst: `Niezapisane (${niezapisane})`, klasa: "status--zolty" },
    blad: { ikona: CircleAlert, tekst: "Niezapisane – błąd zapisu", klasa: "status--czerwony" },
  }[stan];
  const I = opis.ikona;
  return (
    <div className={`wskaznik-zapisu ${opis.klasa}`} role="status" aria-live="polite" data-stan={stan}>
      <I size={13} aria-hidden /> {opis.tekst}
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
