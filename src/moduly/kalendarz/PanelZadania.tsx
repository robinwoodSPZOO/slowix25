// Panel szczegółów zadania: zlecenie, opis stanu z API, daty, akcje (wykonanie, zamówienie,
// dostawa, cofnięcie), blokada „do potwierdzenia” (409 → świadome „zamawiam mimo to”).

import { useEffect, useRef, useState, type CSSProperties } from "react";
import { Link } from "react-router-dom";
import { CalendarClock, CircleAlert, Lock, PackageCheck, Pencil, ShoppingCart, Trash2, TriangleAlert, Undo2, X } from "lucide-react";
import { BladApi } from "../../api/klient";
import type { OpisWierszaSpecyfikacji, Wykonanie, ZadanieKalendarza } from "../../api/typy";
import { usePolaczenie } from "../../kontekst/Polaczenie";
import { formatujDate } from "../../lib/formaty";
import { Okno, Pole, Przycisk } from "../../ui";
import { komunikatBledu } from "./dane";
import { poprawnaData, roznicaDni, wzgledny } from "./daty";
import { EtykietaStanu, IkonaTypu, kolorTypu, nazwaTypu } from "./wyglad";

const STATUSY_MATERIALU: Record<string, string> = {
  do_zamowienia: "Do zamówienia",
  zamowione: "Zamówione – czekamy na dostawę",
  dostarczone: "Dostarczone",
};

export function OknoBlokady({
  wiersze,
  potwierdzicDo,
  onAnuluj,
  onMimoTo,
  zapisywanie,
}: {
  wiersze: number[];
  potwierdzicDo?: string | null;
  onAnuluj: () => void;
  onMimoTo: () => void;
  zapisywanie?: boolean;
}) {
  const { api } = usePolaczenie();
  const [opisy, ustawOpisy] = useState<OpisWierszaSpecyfikacji[] | null>(null);
  useEffect(() => {
    if (!api || wiersze.length === 0) return;
    api.kalendarz.wierszeSpecyfikacji(wiersze).then(ustawOpisy, () => ustawOpisy([]));
  }, [api, wiersze]);
  return (
    <Okno
      tytul="Klient nie potwierdził pozycji – zamówić mimo to?"
      onZamknij={onAnuluj}
      stopka={
        <>
          <Przycisk onClick={onAnuluj}>Anuluj – poczekam na klienta</Przycisk>
          <Przycisk wariant="niebezpieczny" ikona={ShoppingCart} onClick={onMimoTo} disabled={zapisywanie}>
            Zamawiam mimo to
          </Przycisk>
        </>
      }
    >
      <p className="komunikat-stanu stan-tlo--pomaranczowy">
        <Lock size={14} aria-hidden />
        <span>
          W specyfikacji umowy są pozycje z tej kategorii materiału oznaczone „do potwierdzenia”
          {potwierdzicDo ? ` (klient ma potwierdzić do ${formatujDate(potwierdzicDo)})` : ""}. Jeśli zamówisz teraz, a klient zmieni
          zdanie, materiał może być do wymiany. Decyzja zostanie zapisana w historii zmian.
        </span>
      </p>
      <table className="tabela">
        <thead>
          <tr>
            <th>Sekcja / część</th>
            <th>Rodzaj</th>
            <th>Materiał</th>
            <th>Opis</th>
          </tr>
        </thead>
        <tbody>
          {opisy === null && (
            <tr>
              <td colSpan={4}>Wczytywanie pozycji…</td>
            </tr>
          )}
          {opisy?.map((w) => (
            <tr key={w.id}>
              <td>
                {w.sekcja} → {w.czesc}
              </td>
              <td>{w.rodzaj ?? "—"}</td>
              <td>{w.material ?? "—"}</td>
              <td>{w.opis ?? "—"}</td>
            </tr>
          ))}
          {opisy && opisy.length < wiersze.length && (
            <tr>
              <td colSpan={4}>Pozycje nr {wiersze.filter((id) => !opisy.some((o) => o.id === id)).join(", ")} (brak szczegółów)</td>
            </tr>
          )}
        </tbody>
      </table>
    </Okno>
  );
}

/** Wykonanie z obsługą 409 „do potwierdzenia” – wspólne dla panelu, modułu Zamówienia i Pulpitu. */
export function useWykonanie(onPo: (z: ZadanieKalendarza) => void) {
  const { api } = usePolaczenie();
  const [blokada, ustawBlokade] = useState<{ zadanie: ZadanieKalendarza; w: Wykonanie; wiersze: number[] } | null>(null);
  const [zapisywanie, ustawZapisywanie] = useState(false);
  const [blad, ustawBlad] = useState<string | null>(null);

  const wykonaj = async (zadanie: ZadanieKalendarza, w: Wykonanie) => {
    if (!api) return;
    ustawZapisywanie(true);
    ustawBlad(null);
    try {
      const z = await api.kalendarz.wykonanie(zadanie.id, w);
      ustawBlokade(null);
      onPo(z);
    } catch (e) {
      if (e instanceof BladApi && e.kod === 409 && e.kodBledu === "do_potwierdzenia") {
        ustawBlokade({ zadanie, w, wiersze: e.wierszeDoPotwierdzenia });
      } else {
        ustawBlad(komunikatBledu(e));
      }
    } finally {
      ustawZapisywanie(false);
    }
  };

  const oknoBlokady = blokada && (
    <OknoBlokady
      wiersze={blokada.wiersze}
      potwierdzicDo={blokada.zadanie.zamowienie?.potwierdzic_do}
      zapisywanie={zapisywanie}
      onAnuluj={() => ustawBlokade(null)}
      onMimoTo={() => void wykonaj(blokada.zadanie, { ...blokada.w, mimo_blokady: true })}
    />
  );
  return { wykonaj, oknoBlokady, zapisywanie, blad, ustawBlad };
}

function Wiersz({ etykieta, children }: { etykieta: string; children: React.ReactNode }) {
  return (
    <>
      <dt>{etykieta}</dt>
      <dd>{children}</dd>
    </>
  );
}

export function PasekOkna({ z }: { z: ZadanieKalendarza }) {
  // Mini-oś: sweet spot → planowa → krytyczna (z podpisami); kolor zamówienia z tokenu.
  const calosc = z.data_sweet_spot && z.data_krytyczna ? roznicaDni(z.data_sweet_spot, z.data_krytyczna) : 0;
  const plan = z.data_sweet_spot && z.data_planowa ? roznicaDni(z.data_sweet_spot, z.data_planowa) : 0;
  const poz = calosc > 0 ? Math.min(100, Math.max(0, (plan / calosc) * 100)) : 50;
  return (
    <div className="pasek-okna" style={{ "--kolor": kolorTypu("zamowienie") } as CSSProperties} aria-hidden>
      <div className="pasek-okna__tor">
        <span className="pasek-okna__znacznik pasek-okna__znacznik--planowa" style={{ left: `${poz}%` }} />
      </div>
      <div className="pasek-okna__podpisy">
        <span>
          Najlepiej zamówić
          <br />
          <strong>{formatujDate(z.data_sweet_spot)}</strong>
        </span>
        <span>
          Planowo
          <br />
          <strong>{formatujDate(z.data_planowa)}</strong>
        </span>
        <span>
          Najpóźniej
          <br />
          <strong>{formatujDate(z.data_krytyczna)}</strong>
        </span>
      </div>
    </div>
  );
}

export function PanelZadania({
  zadanie: z,
  dzis,
  onZamknij,
  onZmiana,
  onEdytuj,
}: {
  zadanie: ZadanieKalendarza;
  dzis: string;
  onZamknij: () => void;
  /** Po zapisie: nowe zadanie albo null (usunięte). */
  onZmiana: (z: ZadanieKalendarza | null) => void;
  onEdytuj?: (z: ZadanieKalendarza) => void;
}) {
  const { api } = usePolaczenie();
  const [dataWyk, ustawDataWyk] = useState(dzis);
  const [dataDostawy, ustawDataDostawy] = useState("");
  const [potwierdzUsun, ustawPotwierdzUsun] = useState(false);
  const [potwierdzCofnij, ustawPotwierdzCofnij] = useState(false);
  const { wykonaj, oknoBlokady, zapisywanie, blad, ustawBlad } = useWykonanie((n) => onZmiana(n));
  const ref = useRef<HTMLElement>(null);
  const zam = z.zamowienie;
  const jestZamowienie = z.typ === "zamowienie" && !!zam;

  // Nowe zadanie w panelu → fokus na nagłówek (czytniki ekranu), wartości domyślne.
  const [poprzednieId, ustawPoprzednieId] = useState(z.id);
  if (poprzednieId !== z.id) {
    ustawPoprzednieId(z.id);
    ustawDataWyk(dzis);
    ustawDataDostawy("");
    ustawPotwierdzUsun(false);
    ustawPotwierdzCofnij(false);
  }
  useEffect(() => {
    ref.current?.querySelector<HTMLElement>("h2")?.focus();
  }, [z.id]);
  useEffect(() => {
    const k = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !document.querySelector(".okno-tlo")) onZamknij();
    };
    window.addEventListener("keydown", k);
    return () => window.removeEventListener("keydown", k);
  }, [onZamknij]);

  const bladDaty = (d: string, pusta = false) =>
    pusta && d === "" ? null : !poprawnaData(d) ? "Wpisz datę" : d > dzis ? "Data nie może być z przyszłości" : null;

  const cofnijZamowienie = async () => {
    if (!api || !z.zamowienie_id) return;
    try {
      await api.materialy.cofnijZamowienie(z.zamowienie_id);
      onZmiana(await api.kalendarz.zadanieKalendarza(z.id));
      ustawPotwierdzCofnij(false);
    } catch (e) {
      ustawBlad(komunikatBledu(e));
    }
  };
  const usun = async () => {
    if (!api) return;
    try {
      await api.kalendarz.usunZadanie(z.id);
      onZmiana(null);
    } catch (e) {
      ustawBlad(komunikatBledu(e));
    }
  };

  const reczne = !z.wyliczone || !z.klucz;

  return (
    <aside className="panel-zadania" ref={ref} aria-label="Szczegóły zadania" style={{ "--kolor": kolorTypu(z.typ) } as CSSProperties}>
      <div className="panel-zadania__naglowek">
        <span className="panel-zadania__typ">
          <IkonaTypu typ={z.typ} rozmiar={14} />
          {nazwaTypu(z.typ)}
          {reczne && <span className="panel-zadania__reczne">· dodane ręcznie</span>}
        </span>
        <Przycisk wariant="tekstowy" tylkoIkona ikona={X} etykieta="Zamknij panel (Esc)" onClick={onZamknij} />
      </div>
      <h2 tabIndex={-1}>{z.tytul}</h2>
      <div className="panel-zadania__stan">
        <EtykietaStanu stan={z.stan.stan} />
        {z.stan.dni_do_terminu !== null && !z.wykonano && <span className="tekst-drugorzedny">{wzgledny(z.stan.dni_do_terminu)}</span>}
      </div>
      <p className="panel-zadania__opis">{z.stan.opis}</p>

      {z.stan.stan === "czerwony" && z.stan.nowy_termin_montazu && (
        <div className="komunikat-stanu stan-tlo--czerwony" role="note">
          <CalendarClock size={16} aria-hidden />
          <span>
            <strong>Nowy możliwy termin montażu: {formatujDate(z.stan.nowy_termin_montazu)}</strong>
            {z.stan.przesuniecie_montazu_dni ? ` (przesunięcie o ${z.stan.przesuniecie_montazu_dni} dni)` : ""}. Zamów jak najszybciej i
            uzgodnij z klientem nowy termin (aneks do umowy).
          </span>
        </div>
      )}
      {z.harmonogram_nieaktualny && (
        <div className="komunikat-stanu stan-tlo--pomaranczowy" role="note">
          <TriangleAlert size={16} aria-hidden />
          <span>
            <strong>Harmonogram nieaktualny</strong> – terminy mogą być błędne: {z.harmonogram_nieaktualny_powod ?? "ostatnie przeliczenie się nie udało"}.{" "}
            {z.zlecenie_id && <Link to={`/kalendarz/zlecenie/${z.zlecenie_id}`}>Popraw w zleceniu</Link>}
          </span>
        </div>
      )}
      {zam?.zablokowane && !z.wykonano && (
        <div className="komunikat-stanu stan-tlo--zolty" role="note">
          <Lock size={16} aria-hidden />
          <span>
            Czeka na potwierdzenie klienta{zam.potwierdzic_do ? ` do ${formatujDate(zam.potwierdzic_do)}` : ""} – w specyfikacji umowy jest{" "}
            {zam.wiersze_do_potwierdzenia.length} {zam.wiersze_do_potwierdzenia.length === 1 ? "pozycja" : "pozycji"} „do potwierdzenia”.
          </span>
        </div>
      )}
      {zam?.dostawa_zagrozona && (
        <div className="komunikat-stanu stan-tlo--czerwony" role="note">
          <TriangleAlert size={16} aria-hidden />
          <span>
            <strong>Dostawa zagrożona:</strong> oczekiwana {formatujDate(zam.oczekiwana_dostawa)}, a materiał jest potrzebny do{" "}
            {formatujDate(zam.data_wymaganej_dostawy)}.
          </span>
        </div>
      )}

      <dl className="panel-zadania__dane">
        {z.zlecenie_id ? (
          <Wiersz etykieta="Zlecenie">
            <Link to={`/kalendarz/zlecenie/${z.zlecenie_id}`}>{z.zlecenie_nazwa ?? `Zlecenie ${z.zlecenie_id}`}</Link>
          </Wiersz>
        ) : (
          <Wiersz etykieta="Zlecenie">bez zlecenia</Wiersz>
        )}
        {!jestZamowienie && (
          <Wiersz etykieta={z.data_do ? "Okres" : "Termin"}>
            {formatujDate(z.data_od)}
            {z.data_do && z.data_do !== z.data_od ? ` – ${formatujDate(z.data_do)}` : ""}
          </Wiersz>
        )}
        {jestZamowienie && zam && (
          <>
            <Wiersz etykieta="Materiał">
              {zam.kategoria_nazwa}
              {zam.opis ? ` – ${zam.opis}` : ""}
            </Wiersz>
            <Wiersz etykieta="Dostawca">{zam.dostawca_nazwa ?? "nie wybrano"}</Wiersz>
            <Wiersz etykieta="Czas dostawy">
              {zam.dni_min !== null && zam.dni_max !== null ? `${zam.dni_min}–${zam.dni_max} dni (średnio ${zam.dni_srednia})` : "nie ustawiono"}
            </Wiersz>
            <Wiersz etykieta="Potrzebne na warsztacie">{formatujDate(zam.data_wymaganej_dostawy)}</Wiersz>
            <Wiersz etykieta="Status">{STATUSY_MATERIALU[zam.status] ?? zam.status}</Wiersz>
            {zam.data_zamowienia && <Wiersz etykieta="Zamówiono">{formatujDate(zam.data_zamowienia)}</Wiersz>}
            {zam.oczekiwana_dostawa && !zam.data_dostawy && <Wiersz etykieta="Oczekiwana dostawa">{formatujDate(zam.oczekiwana_dostawa)}</Wiersz>}
            {zam.data_dostawy && <Wiersz etykieta="Dostarczono">{formatujDate(zam.data_dostawy)}</Wiersz>}
          </>
        )}
        {z.wykonano && !jestZamowienie && <Wiersz etykieta="Wykonano">{formatujDate(z.wykonano)}</Wiersz>}
        {z.uwagi && <Wiersz etykieta="Uwagi">{z.uwagi}</Wiersz>}
      </dl>
      {jestZamowienie && z.data_sweet_spot && <PasekOkna z={z} />}

      <div className="panel-zadania__akcje">
        {!z.wykonano && !jestZamowienie && (
          <form
            onSubmit={(e) => {
              e.preventDefault();
              if (!bladDaty(dataWyk)) void wykonaj(z, { wykonano: dataWyk });
            }}
          >
            <Pole etykieta="Data wykonania" id="data-wykonania" blad={bladDaty(dataWyk)}>
              <input id="data-wykonania" type="date" value={dataWyk} max={dzis} onChange={(e) => ustawDataWyk(e.target.value)} />
            </Pole>
            <Przycisk wariant="glowny" type="submit" ikona={PackageCheck} disabled={zapisywanie || !!bladDaty(dataWyk)}>
              Oznacz jako wykonane
            </Przycisk>
          </form>
        )}
        {!z.wykonano && jestZamowienie && (
          <form
            onSubmit={(e) => {
              e.preventDefault();
              if (bladDaty(dataWyk) || bladDaty(dataDostawy, true)) return;
              void wykonaj(z, { wykonano: dataWyk, data_zamowienia: dataWyk, ...(dataDostawy ? { data_dostawy: dataDostawy } : {}) });
            }}
          >
            <div className="pola-obok">
              <Pole etykieta="Data zamówienia" id="data-zamowienia" blad={bladDaty(dataWyk)}>
                <input id="data-zamowienia" type="date" value={dataWyk} max={dzis} onChange={(e) => ustawDataWyk(e.target.value)} />
              </Pole>
              <Pole etykieta="Data dostawy (jeśli już jest)" id="data-dostawy" blad={bladDaty(dataDostawy, true)}>
                <input id="data-dostawy" type="date" value={dataDostawy} max={dzis} onChange={(e) => ustawDataDostawy(e.target.value)} />
              </Pole>
            </div>
            <Przycisk wariant="glowny" type="submit" ikona={ShoppingCart} disabled={zapisywanie || !!bladDaty(dataWyk) || !!bladDaty(dataDostawy, true)}>
              {zam?.zablokowane ? "Zamówione (mimo blokady)…" : "Zamówione"}
            </Przycisk>
          </form>
        )}
        {z.wykonano && jestZamowienie && zam && (
          <>
            {zam.status !== "dostarczone" ? (
              <form
                onSubmit={(e) => {
                  e.preventDefault();
                  if (!bladDaty(dataDostawy)) void wykonaj(z, { wykonano: z.wykonano, data_dostawy: dataDostawy });
                }}
              >
                <Pole etykieta="Data dostawy" id="data-dostawy-2" blad={dataDostawy ? bladDaty(dataDostawy) : null}>
                  <input id="data-dostawy-2" type="date" value={dataDostawy} max={dzis} onChange={(e) => ustawDataDostawy(e.target.value)} />
                </Pole>
                <Przycisk wariant="glowny" type="submit" ikona={PackageCheck} disabled={zapisywanie || !dataDostawy || !!bladDaty(dataDostawy)}>
                  Zapisz dostawę
                </Przycisk>
              </form>
            ) : (
              <Przycisk ikona={Undo2} disabled={zapisywanie} onClick={() => void wykonaj(z, { wykonano: z.wykonano, data_dostawy: null })}>
                Usuń datę dostawy
              </Przycisk>
            )}
            {potwierdzCofnij ? (
              <div className="potwierdzenie" role="group" aria-label="Potwierdź cofnięcie zamówienia">
                <span>Cofnąć zamówienie? Daty zamówienia i dostawy zostaną wyczyszczone, a terminy przeliczone.</span>
                <Przycisk wariant="niebezpieczny" maly onClick={() => void cofnijZamowienie()}>
                  Tak, cofnij
                </Przycisk>
                <Przycisk maly onClick={() => ustawPotwierdzCofnij(false)}>
                  Nie
                </Przycisk>
              </div>
            ) : (
              <Przycisk wariant="tekstowy" ikona={Undo2} onClick={() => ustawPotwierdzCofnij(true)}>
                Cofnij zamówienie
              </Przycisk>
            )}
          </>
        )}
        {z.wykonano && !jestZamowienie && (
          <Przycisk ikona={Undo2} disabled={zapisywanie} onClick={() => void wykonaj(z, { wykonano: null })}>
            Cofnij wykonanie
          </Przycisk>
        )}
        {reczne && (
          <div className="panel-zadania__reczne-akcje">
            {onEdytuj && (
              <Przycisk wariant="tekstowy" ikona={Pencil} onClick={() => onEdytuj(z)}>
                Edytuj
              </Przycisk>
            )}
            {potwierdzUsun ? (
              <div className="potwierdzenie" role="group" aria-label="Potwierdź usunięcie">
                <span>Usunąć zadanie?</span>
                <Przycisk wariant="niebezpieczny" maly onClick={() => void usun()}>
                  Usuń
                </Przycisk>
                <Przycisk maly onClick={() => ustawPotwierdzUsun(false)}>
                  Nie
                </Przycisk>
              </div>
            ) : (
              <Przycisk wariant="tekstowy" ikona={Trash2} onClick={() => ustawPotwierdzUsun(true)}>
                Usuń
              </Przycisk>
            )}
          </div>
        )}
        {!reczne && (
          <p className="pole__pomoc">Termin wyliczony z umowy (liczony wstecz od montażu) – zmienisz go, edytując zlecenie: termin montażu, czas warsztatu.</p>
        )}
      </div>
      {blad && (
        <p className="komunikat-stanu stan-tlo--czerwony" role="alert">
          <CircleAlert size={14} aria-hidden />
          {blad}
        </p>
      )}
      {oknoBlokady}
    </aside>
  );
}
