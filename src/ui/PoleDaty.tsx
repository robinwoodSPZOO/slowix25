// Wspólne pole daty w polskim formacie dd.mm.rrrr – niezależne od języka przeglądarki / WebView2
// (natywne <input type="date"> pokazuje układ z ustawień systemu, np. mm/dd/yyyy).
//
//   <PoleDaty id="data-umowy" wartosc={iso} onZmiana={ustawIso} min="2026-01-01" max={dzis} />
//
// * `wartosc` / `onZmiana`: data ISO „RRRR-MM-DD”; "" – puste pole; inny tekst – wpis
//   niepoprawny (np. „31.02.2026”), który rodzic pokazuje jako błąd (`poprawnaDataIso` = false).
// * Wpisywanie ręczne: „5.3.2027”, „05.03.27”, „05032027”, „5-3-2027”, „2027-03-05”; kropki można
//   pomijać. Poprawny wpis jest od razu przekazywany; po wyjściu z pola – formatowany.
// * Klawiatura w polu: ↑/↓ – dzień wcześniej/później, PageUp/PageDown – miesiąc, Alt+↓ albo F4 –
//   kalendarz. W kalendarzu: strzałki – dni, PageUp/PageDown – miesiąc, Home/End – początek/koniec
//   tygodnia, Enter/Spacja – wybór, Esc – zamknięcie (fokus wraca do pola).
// * `min` / `max` (ISO) – dni spoza zakresu są wyłączone w kalendarzu.
// Etykietę i błąd dodaje `Pole` (src/ui/Pole.tsx) – jak dla zwykłego <input>.

import { useEffect, useId, useRef, useState, type KeyboardEvent } from "react";
import { CalendarDays, ChevronLeft, ChevronRight } from "lucide-react";

const MIESIACE = ["styczeń", "luty", "marzec", "kwiecień", "maj", "czerwiec", "lipiec", "sierpień", "wrzesień", "październik", "listopad", "grudzień"];
const DNI = ["pn", "wt", "śr", "cz", "pt", "sb", "nd"];
const DNI_PELNE = ["poniedziałek", "wtorek", "środa", "czwartek", "piątek", "sobota", "niedziela"];
const MIESIACE_D = ["stycznia", "lutego", "marca", "kwietnia", "maja", "czerwca", "lipca", "sierpnia", "września", "października", "listopada", "grudnia"];

/** Poprawna data ISO „RRRR-MM-DD” (istniejący dzień, lata 1900–2199). */
export function poprawnaDataIso(s: string): boolean {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(s);
  if (!m) return false;
  const [r, mi, d] = [Number(m[1]), Number(m[2]), Number(m[3])];
  if (r < 1900 || r > 2199) return false;
  const t = new Date(Date.UTC(r, mi - 1, d));
  return t.getUTCFullYear() === r && t.getUTCMonth() === mi - 1 && t.getUTCDate() === d;
}

/** ISO → „05.03.2027”; inne wartości bez zmian. */
export function formatPolski(iso: string): string {
  return poprawnaDataIso(iso) ? `${iso.slice(8, 10)}.${iso.slice(5, 7)}.${iso.slice(0, 4)}` : iso;
}

/** Tekst wpisany przez użytkownika → ISO albo null. Rok dwucyfrowy = 20RR. */
export function parsujDatePolska(tekst: string): string | null {
  const t = tekst.trim();
  if (t === "") return null;
  if (poprawnaDataIso(t)) return t;
  let d: string | undefined, m: string | undefined, r: string | undefined;
  const z = /^(\d{1,2})[.\-/ ](\d{1,2})[.\-/ ](\d{2}|\d{4})\.?$/.exec(t);
  if (z) [, d, m, r] = z;
  else {
    const c = /^(\d{2})(\d{2})(\d{2}|\d{4})$/.exec(t);
    if (c) [, d, m, r] = c;
  }
  if (!d || !m || !r) return null;
  const rok = r.length === 2 ? `20${r}` : r;
  const iso = `${rok}-${m.padStart(2, "0")}-${d.padStart(2, "0")}`;
  return poprawnaDataIso(iso) ? iso : null;
}

function utc(iso: string): Date {
  return new Date(Date.UTC(Number(iso.slice(0, 4)), Number(iso.slice(5, 7)) - 1, Number(iso.slice(8, 10))));
}
function iso(d: Date): string {
  return d.toISOString().slice(0, 10);
}
export function przesunDni(s: string, n: number): string {
  const d = utc(s);
  d.setUTCDate(d.getUTCDate() + n);
  return iso(d);
}
export function przesunMiesiace(s: string, n: number): string {
  const d = utc(s);
  const dzien = d.getUTCDate();
  const cel = new Date(Date.UTC(d.getUTCFullYear(), d.getUTCMonth() + n, 1));
  const ostatni = new Date(Date.UTC(cel.getUTCFullYear(), cel.getUTCMonth() + 1, 0)).getUTCDate();
  cel.setUTCDate(Math.min(dzien, ostatni));
  return iso(cel);
}
function dzienTyg(s: string): number {
  return (utc(s).getUTCDay() + 6) % 7;
}
function dzisLokalnie(): string {
  const t = new Date();
  return `${t.getFullYear()}-${String(t.getMonth() + 1).padStart(2, "0")}-${String(t.getDate()).padStart(2, "0")}`;
}

export interface PoleDatyProps {
  id?: string;
  wartosc: string;
  onZmiana: (wartosc: string) => void;
  min?: string;
  max?: string;
  disabled?: boolean;
  required?: boolean;
  /** Etykieta dostępności, gdy pole nie ma <label> (np. w tabeli). */
  etykieta?: string;
  className?: string;
  "aria-describedby"?: string;
  "aria-invalid"?: boolean;
  /** Tekst podpowiedzi w pustym polu. */
  placeholder?: string;
}

function Kalendarz({ wybrana, min, max, onWybierz, onZamknij, etykieta }: { wybrana: string; min?: string; max?: string; onWybierz: (iso: string) => void; onZamknij: () => void; etykieta: string }) {
  const [fokus, ustawFokus] = useState(wybrana);
  const ref = useRef<HTMLDivElement>(null);
  const dzis = dzisLokalnie();
  const pierwszy = `${fokus.slice(0, 8)}01`;
  const start = przesunDni(pierwszy, -dzienTyg(pierwszy));
  const dni = Array.from({ length: 42 }, (_, i) => przesunDni(start, i));
  const wylaczony = (d: string) => (!!min && d < min) || (!!max && d > max);
  useEffect(() => {
    ref.current?.querySelector<HTMLElement>(`[data-dzien="${fokus}"]`)?.focus();
  }, [fokus]);
  const klawisz = (e: KeyboardEvent<HTMLDivElement>) => {
    const ruch: Record<string, () => string> = {
      ArrowLeft: () => przesunDni(fokus, -1),
      ArrowRight: () => przesunDni(fokus, 1),
      ArrowUp: () => przesunDni(fokus, -7),
      ArrowDown: () => przesunDni(fokus, 7),
      PageUp: () => przesunMiesiace(fokus, e.shiftKey ? -12 : -1),
      PageDown: () => przesunMiesiace(fokus, e.shiftKey ? 12 : 1),
      Home: () => przesunDni(fokus, -dzienTyg(fokus)),
      End: () => przesunDni(fokus, 6 - dzienTyg(fokus)),
    };
    if (e.key in ruch) {
      e.preventDefault();
      ustawFokus(ruch[e.key]!());
    } else if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      onZamknij();
    } else if (e.key === "Tab") {
      // pułapka fokusu w małym oknie: Tab przełącza nawigację ↔ dni
      const el = ref.current?.querySelectorAll<HTMLElement>("button:not([disabled])");
      if (!el?.length) return;
      const lista = [...el].filter((b) => b.tabIndex >= 0);
      const i = lista.indexOf(document.activeElement as HTMLElement);
      e.preventDefault();
      lista[(i + (e.shiftKey ? -1 : 1) + lista.length) % lista.length]?.focus();
    }
  };
  const tytul = `${MIESIACE[Number(fokus.slice(5, 7)) - 1]} ${fokus.slice(0, 4)}`;
  return (
    <div className="pole-daty__kalendarz" role="dialog" aria-modal="true" aria-label={`${etykieta} – wybierz dzień`} ref={ref} onKeyDown={klawisz}>
      <div className="pole-daty__nawigacja">
        <button type="button" className="pole-daty__strzalka" aria-label="Poprzedni miesiąc (PageUp)" onClick={() => ustawFokus(przesunMiesiace(fokus, -1))}>
          <ChevronLeft size={16} aria-hidden />
        </button>
        <span className="pole-daty__miesiac" aria-live="polite">
          {tytul}
        </span>
        <button type="button" className="pole-daty__strzalka" aria-label="Następny miesiąc (PageDown)" onClick={() => ustawFokus(przesunMiesiace(fokus, 1))}>
          <ChevronRight size={16} aria-hidden />
        </button>
      </div>
      <div role="grid" aria-label={tytul} className="pole-daty__siatka">
        <div role="row" className="pole-daty__wiersz">
          {DNI.map((d, i) => (
            <span role="columnheader" key={d} aria-label={DNI_PELNE[i]} className="pole-daty__dzien-tyg">
              {d}
            </span>
          ))}
        </div>
        {Array.from({ length: 6 }, (_, w) => (
          <div role="row" key={w} className="pole-daty__wiersz">
            {dni.slice(w * 7, w * 7 + 7).map((d) => {
              const poza = d.slice(0, 7) !== fokus.slice(0, 7);
              const kl = ["pole-daty__dzien", poza ? "pole-daty__dzien--poza" : "", d === wybrana ? "pole-daty__dzien--wybrany" : "", d === dzis ? "pole-daty__dzien--dzis" : "", dzienTyg(d) >= 5 ? "pole-daty__dzien--weekend" : ""].filter(Boolean).join(" ");
              return (
                <span role="gridcell" key={d} aria-selected={d === wybrana}>
                  <button
                    type="button"
                    className={kl}
                    data-dzien={d}
                    tabIndex={d === fokus ? 0 : -1}
                    disabled={wylaczony(d)}
                    aria-label={`${DNI_PELNE[dzienTyg(d)]}, ${Number(d.slice(8, 10))} ${MIESIACE_D[Number(d.slice(5, 7)) - 1]} ${d.slice(0, 4)}${d === dzis ? " (dziś)" : ""}`}
                    onClick={() => onWybierz(d)}
                  >
                    {Number(d.slice(8, 10))}
                  </button>
                </span>
              );
            })}
          </div>
        ))}
      </div>
      <div className="pole-daty__stopka">
        <button type="button" className="link" disabled={wylaczony(dzis)} onClick={() => onWybierz(dzis)}>
          Dziś
        </button>
        <button type="button" className="link" onClick={onZamknij}>
          Zamknij
        </button>
      </div>
    </div>
  );
}

export function PoleDaty({ id, wartosc, onZmiana, min, max, disabled, required, etykieta, className, placeholder = "dd.mm.rrrr", ...aria }: PoleDatyProps) {
  const autoId = useId();
  const idPola = id ?? autoId;
  const [tekst, ustawTekst] = useState(() => formatPolski(wartosc));
  const [edycja, ustawEdycje] = useState(false);
  const [otwarte, ustawOtwarte] = useState(false);
  const pole = useRef<HTMLInputElement>(null);
  const kontener = useRef<HTMLDivElement>(null);
  // Zmiana wartości z zewnątrz (np. reset formularza) – gdy użytkownik nie pisze.
  const widoczny = edycja ? tekst : formatPolski(wartosc);

  useEffect(() => {
    if (!otwarte) return;
    const klik = (e: MouseEvent) => {
      if (!kontener.current?.contains(e.target as Node)) ustawOtwarte(false);
    };
    document.addEventListener("mousedown", klik);
    return () => document.removeEventListener("mousedown", klik);
  }, [otwarte]);

  const ustaw = (iso: string) => {
    ustawTekst(formatPolski(iso));
    onZmiana(iso);
  };
  const zamknij = (powrot = true) => {
    ustawOtwarte(false);
    if (powrot) pole.current?.focus();
  };
  const baza = poprawnaDataIso(wartosc) ? wartosc : (max && dzisLokalnie() > max ? max : min && dzisLokalnie() < min ? min : dzisLokalnie());

  const klawisz = (e: KeyboardEvent<HTMLInputElement>) => {
    if ((e.altKey && e.key === "ArrowDown") || e.key === "F4") {
      e.preventDefault();
      ustawOtwarte(true);
      return;
    }
    const krok: Record<string, (s: string) => string> = {
      ArrowUp: (s) => przesunDni(s, 1),
      ArrowDown: (s) => przesunDni(s, -1),
      PageUp: (s) => przesunMiesiace(s, 1),
      PageDown: (s) => przesunMiesiace(s, -1),
    };
    if (e.key in krok && !e.altKey) {
      e.preventDefault();
      let n = poprawnaDataIso(wartosc) ? krok[e.key]!(wartosc) : baza;
      if (min && n < min) n = min;
      if (max && n > max) n = max;
      ustawEdycje(false);
      ustaw(n);
    } else if (e.key === "Enter" && edycja) {
      const p = parsujDatePolska(tekst);
      if (p) ustawTekst(formatPolski(p));
    }
  };

  return (
    <div className={`pole-daty${className ? ` ${className}` : ""}`} ref={kontener}>
      <input
        ref={pole}
        id={idPola}
        className="wejscie pole-daty__wejscie"
        inputMode="numeric"
        autoComplete="off"
        value={widoczny}
        placeholder={placeholder}
        disabled={disabled}
        required={required}
        aria-label={etykieta}
        aria-describedby={aria["aria-describedby"]}
        aria-invalid={aria["aria-invalid"]}
        maxLength={10}
        onFocus={() => {
          ustawTekst(formatPolski(wartosc));
          ustawEdycje(true);
        }}
        onChange={(e) => {
          const t = e.target.value;
          ustawTekst(t);
          const p = parsujDatePolska(t);
          onZmiana(t.trim() === "" ? "" : (p ?? t));
        }}
        onBlur={() => {
          ustawEdycje(false);
          const p = parsujDatePolska(tekst);
          if (p) ustawTekst(formatPolski(p));
        }}
        onKeyDown={klawisz}
      />
      <button
        type="button"
        className="pole-daty__przycisk"
        aria-label={`Wybierz datę z kalendarza${etykieta ? `: ${etykieta}` : ""} (Alt+↓)`}
        title="Wybierz z kalendarza (Alt+↓)"
        aria-haspopup="dialog"
        aria-expanded={otwarte}
        disabled={disabled}
        onClick={() => (otwarte ? zamknij() : ustawOtwarte(true))}
      >
        <CalendarDays size={15} aria-hidden />
      </button>
      {otwarte && (
        <Kalendarz
          wybrana={baza}
          min={min}
          max={max}
          etykieta={etykieta ?? "Data"}
          onWybierz={(d) => {
            ustawEdycje(false);
            ustaw(d);
            zamknij();
          }}
          onZamknij={() => zamknij()}
        />
      )}
    </div>
  );
}
