// Wspólny edytor specyfikacji (oferta, umowa, aneks): drzewo po lewej, tabela wierszy pośrodku,
// opcjonalny panel po prawej (np. wycena oferty). Każda zmiana idzie od razu do API; serwer
// zwraca całe drzewo, które edytor przekazuje rodzicowi przez `onZmiana`.

import { useCallback, useEffect, useMemo, useState, type ReactNode } from "react";
import { Box, CircleAlert, Lock, PanelLeftClose, PanelLeftOpen, PanelRightClose, PanelRightOpen, Plus, X } from "lucide-react";
import type { BladApi } from "../../api/klient";
import type { Czesc, Podpowiedzi, Sekcja, Specyfikacja, SzablonMebli, WynikZmiany } from "../../api/typy";
import { usePolaczenie } from "../../kontekst/Polaczenie";
import { wczytajKopie, zapiszKopie } from "../../lib/szkice";
import { formatujKwote } from "../../lib/formaty";
import { Przycisk } from "../../ui";
import { DrzewoSpecyfikacji } from "./DrzewoSpecyfikacji";
import { TabelaWierszy } from "./TabelaWierszy";
import { etykietaSekcji, liczbaDoPotwierdzenia, poprawWybor, znajdzCzesc, type Wybor } from "./logika";
import "./specyfikacja.css";

export interface KontekstWyboru {
  wybor: Wybor;
  sekcja: Sekcja | null;
  czesc: Czesc | null;
}

export interface EdytorSpecyfikacjiProps {
  specyfikacja: Specyfikacja;
  /** Nowe drzewo po każdej zmianie na serwerze. */
  onZmiana: (s: Specyfikacja) => void;
  /** Trzecia kolumna (np. panel wyceny); dostaje bieżący wybór w drzewie. */
  panel?: (k: KontekstWyboru) => ReactNode;
  /** Dodatkowy powód blokady edycji (np. umowa podpisana). */
  tylkoDoOdczytu?: boolean;
  /** Treść paska „tylko do odczytu” (np. przycisk „Nowa wersja”). */
  akcjaBlokady?: ReactNode;
}

export function EdytorSpecyfikacji({ specyfikacja, onZmiana, panel, tylkoDoOdczytu, akcjaBlokady }: EdytorSpecyfikacjiProps) {
  const [zwiniete, ustawZwiniete] = useState<{ drzewo: boolean; panel: boolean }>(() => wczytajKopie("uklad-edytora") ?? { drzewo: false, panel: false });
  const przelaczUklad = (k: "drzewo" | "panel") =>
    ustawZwiniete((z) => {
      const n = { ...z, [k]: !z[k] };
      zapiszKopie("uklad-edytora", n);
      return n;
    });
  const { api, ostatnieZdarzenie } = usePolaczenie();
  const [wybor, ustawWybor] = useState<Wybor>(() => poprawWybor(specyfikacja, null));
  const [podpowiedzi, ustawPodpowiedzi] = useState<Podpowiedzi>({});
  const [szablony, ustawSzablony] = useState<SzablonMebli[]>([]);
  const [blad, ustawBlad] = useState<string | null>(null);
  const ro = tylkoDoOdczytu || specyfikacja.tylko_do_odczytu;
  const spec = useMemo(() => ({ ...specyfikacja, tylko_do_odczytu: ro }), [specyfikacja, ro]);

  // Słowniki i szablony odświeżamy po imporcie (zdarzenie z serwera).
  const zmianaSlownikow =
    ostatnieZdarzenie?.zasob === "slowniki" || ostatnieZdarzenie?.zasob === "szablony_mebli" ? ostatnieZdarzenie : null;
  useEffect(() => {
    if (!api) return;
    api.podpowiedzi().then(ustawPodpowiedzi, (e: Error) => ustawBlad(e.message));
    api.szablony.lista().then(ustawSzablony, (e: Error) => ustawBlad(e.message));
  }, [api, zmianaSlownikow]);

  const wybranyPoprawiony = poprawWybor(specyfikacja, wybor);
  if (JSON.stringify(wybranyPoprawiony) !== JSON.stringify(wybor)) ustawWybor(wybranyPoprawiony);

  const wykonaj = useCallback(
    async (z: Promise<WynikZmiany>, przechwyc?: (e: BladApi) => boolean) => {
      try {
        const w = await z;
        ustawBlad(null);
        onZmiana(w.specyfikacja);
        return w;
      } catch (e) {
        if (!(przechwyc && przechwyc(e as BladApi))) ustawBlad((e as Error).message);
        return null;
      }
    },
    [onZmiana],
  );
  const odswiez = useCallback(async () => {
    if (!api) return;
    try {
      onZmiana(await api.specyfikacje.pobierz(specyfikacja.id));
    } catch (e) {
      ustawBlad((e as Error).message);
    }
  }, [api, onZmiana, specyfikacja.id]);

  if (!api) return null;

  const znaleziona = wybor?.typ === "czesc" ? znajdzCzesc(specyfikacja, wybor.id) : null;
  const sekcja =
    znaleziona?.sekcja ?? (wybor?.typ === "sekcja" ? (specyfikacja.sekcje.find((s) => s.id === wybor.id) ?? null) : null);
  const czesc = znaleziona?.czesc ?? null;

  return (
    <div
      className={`edytor-specyfikacji${panel && !zwiniete.panel ? " edytor-specyfikacji--z-panelem" : ""}${zwiniete.drzewo ? " edytor-specyfikacji--bez-drzewa" : ""}`}
    >
      {!zwiniete.drzewo && (
        <aside className="edytor-specyfikacji__drzewo">
          <DrzewoSpecyfikacji specyfikacja={spec} wybor={wybor} onWybor={ustawWybor} wykonaj={wykonaj} api={api} szablony={szablony} />
        </aside>
      )}
      <section className="edytor-specyfikacji__srodek" aria-label="Wiersze specyfikacji">
        <div className="edytor-specyfikacji__przelaczniki">
          <Przycisk
            maly
            wariant="tekstowy"
            tylkoIkona
            ikona={zwiniete.drzewo ? PanelLeftOpen : PanelLeftClose}
            etykieta={zwiniete.drzewo ? "Pokaż drzewo specyfikacji" : "Zwiń drzewo specyfikacji"}
            aria-pressed={!zwiniete.drzewo}
            onClick={() => przelaczUklad("drzewo")}
          />
          {panel && (
            <Przycisk
              maly
              wariant="tekstowy"
              tylkoIkona
              ikona={zwiniete.panel ? PanelRightOpen : PanelRightClose}
              etykieta={zwiniete.panel ? "Pokaż panel (wycena, nagłówek, zdjęcia)" : "Zwiń panel boczny"}
              aria-pressed={!zwiniete.panel}
              onClick={() => przelaczUklad("panel")}
            />
          )}
        </div>
        {ro && (
          <div className="komunikat-stanu status--neutralny">
            <Lock size={14} aria-hidden />
            <span style={{ flex: 1 }}>
              Tylko do odczytu
              {specyfikacja.wlasciciel?.powod_blokady ? ` – ${specyfikacja.wlasciciel.powod_blokady}.` : "."}
            </span>
            {akcjaBlokady}
          </div>
        )}
        {blad && (
          <div className="komunikat-stanu status--czerwony" role="alert">
            <CircleAlert size={14} aria-hidden />
            <span style={{ flex: 1 }}>{blad}</span>
            <Przycisk maly wariant="tekstowy" tylkoIkona ikona={X} etykieta="Zamknij komunikat" onClick={() => ustawBlad(null)} />
          </div>
        )}
        {czesc && sekcja ? (
          <>
            <header className="edytor-specyfikacji__tytul">
              <span className="edytor-specyfikacji__okruszek">{etykietaSekcji(sekcja)}</span>
              <h2>{czesc.nazwa}</h2>
            </header>
            <TabelaWierszy
              key={czesc.id}
              czesc={czesc}
              agd={sekcja.typ === "agd"}
              tylkoDoOdczytu={ro}
              podpowiedzi={podpowiedzi}
              wykonaj={wykonaj}
              odswiez={odswiez}
              inneCzesci={specyfikacja.sekcje.flatMap((sk) =>
                sk.czesci
                  .filter((c) => c.id !== czesc.id && (sk.typ === "agd") === (sekcja.typ === "agd"))
                  .map((c) => ({ id: c.id, etykieta: `${etykietaSekcji(sk)} › ${c.nazwa}`, wiersze: c.wiersze.map((w) => w.id) })),
              )}
              api={api.specyfikacje}
            />
          </>
        ) : sekcja ? (
          <PodgladSekcji sekcja={sekcja} ro={ro} onWybierz={(id) => ustawWybor({ typ: "czesc", id })} onDodaj={async () => {
            const w = await wykonaj(api.specyfikacje.dodajCzesc(sekcja.id, { nazwa: sekcja.typ === "agd" ? "Sprzęt" : "Nowa część" }));
            if (w) ustawWybor({ typ: "czesc", id: w.id });
          }} />
        ) : (
          <div className="edytor-specyfikacji__pusto">
            <Box size={32} aria-hidden />
            <p>Specyfikacja jest pusta. Dodaj sekcję (pomieszczenie) w drzewie po lewej – pustą albo z szablonu.</p>
          </div>
        )}
      </section>
      {panel && !zwiniete.panel && <aside className="edytor-specyfikacji__panel">{panel({ wybor, sekcja, czesc })}</aside>}
    </div>
  );
}

function PodgladSekcji({ sekcja, ro, onWybierz, onDodaj }: { sekcja: Sekcja; ro: boolean; onWybierz: (id: number) => void; onDodaj: () => void }) {
  return (
    <>
      <header className="edytor-specyfikacji__tytul">
        <span className="edytor-specyfikacji__okruszek">{sekcja.typ === "agd" ? "Sprzęt AGD – liczony osobno" : "Sekcja"}</span>
        <h2>{etykietaSekcji(sekcja)}</h2>
      </header>
      <table className="tabela">
        <thead>
          <tr>
            <th>Część</th>
            <th>Wiersze</th>
            <th>Do potwierdzenia</th>
            <th className="kolumna-kwoty">Cena części</th>
          </tr>
        </thead>
        <tbody>
          {sekcja.czesci.map((c) => (
            <tr key={c.id} className="wiersz-tabeli wiersz-tabeli--klikalny" tabIndex={0} onClick={() => onWybierz(c.id)} onKeyDown={(e) => e.key === "Enter" && onWybierz(c.id)}>
              <td>{c.nazwa}</td>
              <td>{c.wiersze.length}</td>
              <td>{liczbaDoPotwierdzenia(c.wiersze) || "—"}</td>
              <td className="kolumna-kwoty">{formatujKwote(c.cena_gr)}</td>
            </tr>
          ))}
        </tbody>
      </table>
      {!ro && (
        <div className="tabela-wierszy__dodaj">
          <Przycisk maly wariant="tekstowy" ikona={Plus} onClick={onDodaj}>
            Dodaj część
          </Przycisk>
        </div>
      )}
    </>
  );
}
