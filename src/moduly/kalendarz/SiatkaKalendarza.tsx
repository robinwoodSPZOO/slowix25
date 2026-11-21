// Siatka miesiąca / tygodnia: dni (gridcell, strzałki, Enter), paski zadań w torach, „+N więcej”.

import { memo, useEffect, useLayoutEffect, useMemo, useRef, useState, type CSSProperties, type KeyboardEvent } from "react";
import { ChevronLeft, ChevronRight, Diamond, Lock, TriangleAlert } from "lucide-react";
import type { DzienWolny, ZadanieKalendarza } from "../../api/typy";
import { czyWeekend, dodajDni, DNI_TYGODNIA, DNI_TYGODNIA_KROTKO, pelnaData, roznicaDni } from "./daty";
import { pozycjaPlanowej, torowWWierszu, ukladTygodnia, type Segment } from "./uklad";
import { IkonaStanu, IkonaTypu, kolorTypu, nazwaTypu, STANY } from "./wyglad";

const RODZAJE_DNI: Record<string, string> = { swieto: "Dzień wolny", urlop: "Urlop", przerwa_dostawcow: "Przerwa dostawców", inne: "Dzień wolny" };

export const WYS_NAGLOWKA_DNIA = 26;
export const WYS_TORU_MIESIAC = 20;
export const WYS_TORU_TYDZIEN = 46;
export const WYS_NAPISU_WIECEJ = 18;

export interface PropsSiatki {
  tygodnie: string[];
  tryb: "miesiac" | "tydzien";
  /** Pierwszy dzień wyświetlanego miesiąca (dni spoza miesiąca – wyszarzone). */
  miesiac: string | null;
  zadania: ZadanieKalendarza[];
  dzis: string;
  swieta: Map<string, string>;
  dniWolne: Map<string, DzienWolny>;
  wybraneId: number | null;
  fokus: string;
  onFokus: (iso: string) => void;
  onOtworzDzien: (iso: string) => void;
  onWybierz: (z: ZadanieKalendarza) => void;
  etykieta: string;
}

function opisPaska(z: ZadanieKalendarza): string {
  const s = STANY[z.stan.stan]?.tekst ?? "";
  const zam =
    z.typ === "zamowienie" && z.data_planowa
      ? `, najlepiej zamówić od ${z.data_sweet_spot ?? "?"}, planowo ${z.data_planowa}, najpóźniej ${z.data_krytyczna ?? "?"}`
      : "";
  return `${nazwaTypu(z.typ)}: ${z.tytul}. ${s}${zam}. ${z.stan.opis}`;
}

const Pasek = memo(function Pasek({
  s,
  wybrany,
  tydzien,
  dzis,
  poczatekTygodnia,
  onWybierz,
}: {
  s: Segment;
  wybrany: boolean;
  tydzien: boolean;
  dzis: string;
  poczatekTygodnia: string;
  onWybierz: (z: ZadanieKalendarza) => void;
}) {
  const z = s.zadanie;
  const planowa = pozycjaPlanowej(s);
  const zamowienie = z.typ === "zamowienie";
  // Zamówienie: jaśniej od sweet spotu do daty planowej, ciemniej od planowej do krytycznej.
  let tlo: string | undefined;
  if (zamowienie && z.data_planowa) {
    const poPlanowej = planowa === null && z.data_planowa < poczatekTygodnia;
    tlo = planowa !== null ? `${planowa}%` : poPlanowej ? "0%" : "100%";
  }
  const klasy = [
    "pasek",
    zamowienie ? "pasek--okno" : "",
    `pasek--stan-${z.stan.stan}`,
    z.wykonano ? "pasek--wykonane" : "",
    z.harmonogram_nieaktualny ? "pasek--nieaktualny" : "",
    s.ciagnieSieZLewej ? "pasek--z-lewej" : "",
    s.ciagnieSieWPrawo ? "pasek--w-prawo" : "",
    wybrany ? "pasek--wybrany" : "",
  ]
    .filter(Boolean)
    .join(" ");
  const pokazStan = z.stan.stan !== "spokojnie";
  return (
    <button
      type="button"
      className={klasy}
      style={
        {
          gridColumn: `${s.kolOd + 1} / ${s.kolDo + 2}`,
          gridRow: s.tor + 2,
          "--kolor": kolorTypu(z.typ),
          ...(tlo ? { "--planowa": tlo } : {}),
        } as CSSProperties
      }
      tabIndex={tydzien ? 0 : -1}
      aria-label={opisPaska(z)}
      title={opisPaska(z)}
      aria-pressed={wybrany}
      data-zadanie={z.id}
      onClick={() => onWybierz(z)}
    >
      {s.ciagnieSieZLewej && <ChevronLeft className="pasek__ciag" size={11} aria-hidden />}
      <span className="pasek__ikona">
        <IkonaTypu typ={z.typ} rozmiar={12} />
      </span>
      <span className="pasek__tekst">
        <span className="pasek__tytul">{z.tytul}</span>
        {tydzien && (
          <span className="pasek__opis">
            {z.wykonano ? `Wykonano ${z.wykonano.slice(8, 10)}.${z.wykonano.slice(5, 7)}` : z.stan.opis}
          </span>
        )}
      </span>
      {z.zamowienie?.zablokowane && !z.wykonano && (
        <span className="pasek__znak" title="Czeka na potwierdzenie klienta">
          <Lock size={11} aria-hidden />
        </span>
      )}
      {z.harmonogram_nieaktualny && (
        <span className="pasek__znak" title="Harmonogram nieaktualny">
          <TriangleAlert size={11} aria-hidden />
        </span>
      )}
      {pokazStan && <IkonaStanu stan={z.stan.stan} rozmiar={12} />}
      {s.ciagnieSieWPrawo && <ChevronRight className="pasek__ciag" size={11} aria-hidden />}
      {planowa !== null && (
        <span className="pasek__planowa" style={{ left: `${planowa}%` }} title={`Data planowa: ${z.data_planowa}`}>
          <Diamond size={9} aria-hidden />
        </span>
      )}
      {z.typ !== "zamowienie" && z.termin < dzis && !z.wykonano && <span className="sr-only">po terminie</span>}
    </button>
  );
});

function Tydzien({
  poczatek,
  props,
  limitTorow,
}: {
  poczatek: string;
  props: PropsSiatki;
  limitTorow: number;
}) {
  const { zadania, dzis, swieta, dniWolne, miesiac, tryb, fokus, wybraneId, onWybierz, onFokus, onOtworzDzien } = props;
  const uklad = useMemo(() => ukladTygodnia(zadania, poczatek, limitTorow), [zadania, poczatek, limitTorow]);
  const tydzien = tryb === "tydzien";
  const torow = Math.max(uklad.torow, 1);
  const maUkryte = uklad.ukryte.some((n) => n > 0);
  const miesiacKlucz = miesiac?.slice(0, 7);
  return (
    <div
      role="row"
      className="tydzien"
      style={{ "--torow": torow, "--wiecej": maUkryte ? `${WYS_NAPISU_WIECEJ}px` : "0px" } as CSSProperties}
    >
      {Array.from({ length: 7 }, (_, k) => {
        const d = dodajDni(poczatek, k);
        const swieto = swieta.get(d);
        const wolny = dniWolne.get(d);
        const liczba = uklad.dni[k]?.length ?? 0;
        const klasy = [
          "komorka",
          d === dzis ? "komorka--dzis" : "",
          czyWeekend(d) ? "komorka--weekend" : "",
          swieto ? "komorka--swieto" : "",
          wolny ? "komorka--wolny" : "",
          miesiacKlucz && d.slice(0, 7) !== miesiacKlucz ? "komorka--poza" : "",
          d === fokus ? "komorka--fokus" : "",
        ]
          .filter(Boolean)
          .join(" ");
        const opisDnia = [
          pelnaData(d),
          d === dzis ? "dziś" : "",
          swieto ? `święto: ${swieto}` : "",
          wolny ? `${RODZAJE_DNI[wolny.rodzaj] ?? "dzień wolny"}${wolny.opis ? `: ${wolny.opis}` : ""}` : "",
          liczba ? `zadań: ${liczba}` : "brak zadań",
        ]
          .filter(Boolean)
          .join(", ");
        return (
          <div
            key={d}
            role="gridcell"
            className={klasy}
            style={{ gridColumn: k + 1 }}
            tabIndex={d === fokus ? 0 : -1}
            aria-label={opisDnia}
            aria-selected={d === fokus}
            data-dzien={d}
            onFocus={() => d !== fokus && onFokus(d)}
            onClick={(e) => {
              if (e.target === e.currentTarget) onFokus(d);
            }}
          >
            <div className="komorka__naglowek">
              <button type="button" tabIndex={-1} className="komorka__numer" onClick={() => onOtworzDzien(d)} aria-label={`Lista zadań: ${pelnaData(d)}`}>
                {tydzien ? `${DNI_TYGODNIA_KROTKO[k]} ${Number(d.slice(8, 10))}` : Number(d.slice(8, 10))}
              </button>
              {(swieto || wolny) && (
                <span className="komorka__swieto" title={swieto ?? wolny?.opis ?? RODZAJE_DNI[wolny?.rodzaj ?? "inne"]}>
                  {swieto ?? wolny?.opis ?? RODZAJE_DNI[wolny?.rodzaj ?? "inne"]}
                </span>
              )}
            </div>
          </div>
        );
      })}
      {uklad.segmenty.map((s) => (
        <Pasek
          key={`${s.zadanie.id}`}
          s={s}
          wybrany={s.zadanie.id === wybraneId}
          tydzien={tydzien}
          dzis={dzis}
          poczatekTygodnia={poczatek}
          onWybierz={onWybierz}
        />
      ))}
      {uklad.ukryte.map((n, k) =>
        n > 0 ? (
          <button
            key={`w${k}`}
            type="button"
            tabIndex={-1}
            className="tydzien__wiecej"
            style={{ gridColumn: k + 1, gridRow: torow + 2 }}
            onClick={() => onOtworzDzien(dodajDni(poczatek, k))}
          >
            +{n} więcej
          </button>
        ) : null,
      )}
    </div>
  );
}

export function SiatkaKalendarza(props: PropsSiatki) {
  const { tygodnie, tryb, fokus, onFokus, onOtworzDzien, etykieta } = props;
  const ref = useRef<HTMLDivElement>(null);
  const [limit, ustawLimit] = useState(3);
  const tydzien = tryb === "tydzien";

  // Liczba torów mieszczących się w wierszu miesiąca zależy od wysokości okna.
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el || tydzien) return;
    const policz = () => {
      const naglowek = el.querySelector<HTMLElement>(".siatka__dni-tygodnia")?.offsetHeight ?? 24;
      const wys = (el.clientHeight - naglowek) / Math.max(1, tygodnie.length);
      ustawLimit(torowWWierszu(wys, WYS_NAGLOWKA_DNIA, WYS_TORU_MIESIAC + 2, WYS_NAPISU_WIECEJ));
    };
    policz();
    const ro = new ResizeObserver(policz);
    ro.observe(el);
    return () => ro.disconnect();
  }, [tygodnie.length, tydzien]);

  // Fokus podąża za wybranym dniem (strzałki).
  const poprzedniFokus = useRef(fokus);
  useEffect(() => {
    if (poprzedniFokus.current === fokus) return;
    poprzedniFokus.current = fokus;
    const el = ref.current?.querySelector<HTMLElement>(`[data-dzien="${fokus}"]`);
    if (el && ref.current?.contains(document.activeElement)) el.focus();
  }, [fokus]);

  const klawisz = (e: KeyboardEvent<HTMLDivElement>) => {
    const cel = e.target as HTMLElement;
    if (!cel.matches('[role="gridcell"]')) return;
    const krok: Record<string, number> = { ArrowLeft: -1, ArrowRight: 1, ArrowUp: -7, ArrowDown: 7 };
    if (e.key in krok) {
      e.preventDefault();
      onFokus(dodajDni(fokus, krok[e.key]!));
    } else if (e.key === "Home") {
      e.preventDefault();
      onFokus(dodajDni(fokus, -((roznicaDni(tygodnie[0]!, fokus) % 7 + 7) % 7)));
    } else if (e.key === "End") {
      e.preventDefault();
      onFokus(dodajDni(fokus, 6 - ((roznicaDni(tygodnie[0]!, fokus) % 7 + 7) % 7)));
    } else if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      onOtworzDzien(fokus);
    }
  };

  return (
    <div
      ref={ref}
      role="grid"
      aria-label={etykieta}
      className={`siatka siatka--${tryb}`}
      onKeyDown={klawisz}
      style={{ "--wys-toru": `${tydzien ? WYS_TORU_TYDZIEN : WYS_TORU_MIESIAC}px`, "--wys-naglowka-dnia": `${WYS_NAGLOWKA_DNIA}px` } as CSSProperties}
    >
      <div role="row" className="siatka__dni-tygodnia">
        {DNI_TYGODNIA.map((d, i) => (
          <div role="columnheader" key={d} className={i >= 5 ? "weekend" : ""} aria-label={d}>
            {tydzien ? d : DNI_TYGODNIA_KROTKO[i]}
          </div>
        ))}
      </div>
      <div className="siatka__tygodnie">
        {tygodnie.map((t) => (
          <Tydzien key={t} poczatek={t} props={props} limitTorow={tydzien ? Infinity : limit} />
        ))}
      </div>
    </div>
  );
}
