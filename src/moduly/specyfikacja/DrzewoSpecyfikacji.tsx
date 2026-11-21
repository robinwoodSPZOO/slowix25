// Drzewo sekcji (pomieszczeń) i części (modułów) z akcjami: dodaj, z szablonu, zmień nazwę (F2 /
// dwuklik), kopiuj, usuń, przesuń (Alt+↑/↓).

import { useState, type KeyboardEvent } from "react";
import { ArrowDown, ArrowUp, Box, Copy, FolderInput, LayoutTemplate, Pencil, Plus, Refrigerator, Rows3, Trash2 } from "lucide-react";
import type { Sekcja, Specyfikacja, SzablonMebli, WynikZmiany } from "../../api/typy";
import type { KlientApi } from "../../api/klient";
import { ElementDrzewa, Okno, Pole, Przycisk } from "../../ui";
import { etykietaSekcji, liczbaDoPotwierdzenia, przesun, type Wybor } from "./logika";

interface Props {
  specyfikacja: Specyfikacja;
  wybor: Wybor;
  onWybor: (w: Wybor) => void;
  wykonaj: (z: Promise<WynikZmiany>) => Promise<WynikZmiany | null>;
  api: KlientApi;
  szablony: SzablonMebli[];
}

type Edycja = { typ: "sekcja" | "czesc"; id: number; nazwa: string } | null;
type DialogSzablonu = { tryb: "sekcja" } | { tryb: "czesc"; sekcjaId: number } | null;

export function DrzewoSpecyfikacji({ specyfikacja: s, wybor, onWybor, wykonaj, api, szablony }: Props) {
  const [zwiniete, ustawZwiniete] = useState<Set<number>>(new Set());
  const [edycja, ustawEdycje] = useState<Edycja>(null);
  const [dialog, ustawDialog] = useState<DialogSzablonu>(null);
  const [przenoszona, ustawPrzenoszona] = useState<{ id: number; nazwa: string; sekcjaId: number } | null>(null);
  const ro = s.tylko_do_odczytu;
  const sp = api.specyfikacje;

  const przelacz = (id: number) =>
    ustawZwiniete((z) => {
      const n = new Set(z);
      if (n.has(id)) n.delete(id);
      else n.add(id);
      return n;
    });

  const wybierzPo = (w: WynikZmiany | null, typ: "sekcja" | "czesc") => {
    if (w) onWybor({ typ, id: w.id });
  };

  const dodajSekcje = async (agd = false) => {
    const nazwa = agd ? "Sprzęt AGD" : "Nowe pomieszczenie";
    const w = await wykonaj(sp.dodajSekcje(s.id, { nazwa, typ: agd ? "agd" : "meble", stawka_vat: agd ? 23 : null }));
    if (w) {
      onWybor({ typ: "sekcja", id: w.id });
      ustawEdycje({ typ: "sekcja", id: w.id, nazwa });
    }
  };

  const dodajCzesc = async (sekcja: Sekcja) => {
    const nazwa = sekcja.typ === "agd" ? "Sprzęt" : "Nowa część";
    const w = await wykonaj(sp.dodajCzesc(sekcja.id, { nazwa }));
    if (w) {
      onWybor({ typ: "czesc", id: w.id });
      ustawEdycje({ typ: "czesc", id: w.id, nazwa });
    }
  };

  const zapiszNazwe = async () => {
    if (!edycja) return;
    const nazwa = edycja.nazwa.trim();
    ustawEdycje(null);
    if (!nazwa) return;
    if (edycja.typ === "sekcja") {
      const sek = s.sekcje.find((x) => x.id === edycja.id);
      if (sek && sek.nazwa !== nazwa) await wykonaj(sp.zmienSekcje(sek.id, { nazwa, poprzednie: { nazwa: sek.nazwa } }));
    } else {
      const cz = s.sekcje.flatMap((x) => x.czesci).find((x) => x.id === edycja.id);
      if (cz && cz.nazwa !== nazwa) await wykonaj(sp.zmienCzesc(cz.id, { nazwa, poprzednie: { nazwa: cz.nazwa } }));
    }
  };

  const usunSekcje = async (sek: Sekcja) => {
    if (!confirm(`Usunąć sekcję „${etykietaSekcji(sek)}” razem z częściami i wierszami?`)) return;
    await wykonaj(sp.usunSekcje(sek.id));
  };

  const usunCzesc = async (id: number, nazwa: string) => {
    if (!confirm(`Usunąć część „${nazwa}” razem z wierszami?`)) return;
    await wykonaj(sp.usunCzesc(id));
  };

  const przesunSekcje = async (id: number, krok: -1 | 1) => {
    const ids = przesun(
      s.sekcje.map((x) => x.id),
      id,
      krok,
    );
    if (ids) await wykonaj(sp.kolejnoscSekcji(s.id, ids));
  };

  const przesunCzesc = async (sek: Sekcja, id: number, krok: -1 | 1) => {
    const ids = przesun(
      sek.czesci.map((x) => x.id),
      id,
      krok,
    );
    if (ids) await wykonaj(sp.kolejnoscCzesci(sek.id, ids));
  };

  const klawiszElementu = (e: KeyboardEvent<HTMLDivElement>, typ: "sekcja" | "czesc", id: number, nazwa: string, sek: Sekcja) => {
    if (ro) return;
    if (e.key === "F2") {
      e.preventDefault();
      ustawEdycje({ typ, id, nazwa });
    } else if (e.altKey && (e.key === "ArrowUp" || e.key === "ArrowDown")) {
      e.preventDefault();
      const krok = e.key === "ArrowUp" ? -1 : 1;
      void (typ === "sekcja" ? przesunSekcje(id, krok) : przesunCzesc(sek, id, krok));
    } else if (e.key === "Delete") {
      e.preventDefault();
      void (typ === "sekcja" ? usunSekcje(sek) : usunCzesc(id, nazwa));
    }
  };

  const poleNazwy = (
    <input
      className="wejscie drzewo-specyfikacji__edycja"
      autoFocus
      aria-label="Nazwa"
      value={edycja?.nazwa ?? ""}
      onChange={(e) => ustawEdycje((x) => (x ? { ...x, nazwa: e.target.value } : x))}
      onClick={(e) => e.stopPropagation()}
      onKeyDown={(e) => {
        e.stopPropagation();
        if (e.key === "Enter") void zapiszNazwe();
        if (e.key === "Escape") ustawEdycje(null);
      }}
      onBlur={() => void zapiszNazwe()}
    />
  );

  return (
    <div className="drzewo-specyfikacji">
      <div className="drzewo-specyfikacji__naglowek">
        <h3>Specyfikacja</h3>
        {!ro && (
          <div className="drzewo-specyfikacji__przyciski">
            <Przycisk maly wariant="tekstowy" ikona={Plus} onClick={() => void dodajSekcje()} title="Nowe pomieszczenie (sekcja)">
              Sekcja
            </Przycisk>
            <Przycisk maly wariant="tekstowy" tylkoIkona ikona={LayoutTemplate} etykieta="Sekcja z szablonu" onClick={() => ustawDialog({ tryb: "sekcja" })} />
            <Przycisk maly wariant="tekstowy" tylkoIkona ikona={Refrigerator} etykieta="Sekcja sprzętu AGD" onClick={() => void dodajSekcje(true)} />
          </div>
        )}
      </div>
      <div role="tree" aria-label="Sekcje i części specyfikacji" className="drzewo-specyfikacji__lista">
        {s.sekcje.length === 0 && <p className="drzewo-specyfikacji__pusto">Dodaj pierwszą sekcję (pomieszczenie) – pustą albo z szablonu.</p>}
        {s.sekcje.map((sek, i) => {
          const rozwiniete = !zwiniete.has(sek.id);
          const doPotw = liczbaDoPotwierdzenia(sek.czesci.flatMap((c) => c.wiersze));
          return (
            <div key={sek.id} role="group">
              <ElementDrzewa
                poziom={0}
                ikona={sek.typ === "agd" ? Refrigerator : Rows3}
                etykieta={edycja?.typ === "sekcja" && edycja.id === sek.id ? poleNazwy : etykietaSekcji(sek)}
                zaznaczony={wybor?.typ === "sekcja" && wybor.id === sek.id}
                rozwiniety={rozwiniete}
                licznik={doPotw > 0 ? <span title={`${doPotw} do potwierdzenia`}>?{doPotw}</span> : undefined}
                onWybierz={() => onWybor({ typ: "sekcja", id: sek.id })}
                onPrzelacz={() => przelacz(sek.id)}
                onDwuklik={() => !ro && ustawEdycje({ typ: "sekcja", id: sek.id, nazwa: sek.nazwa })}
                onKlawisz={(e) => klawiszElementu(e, "sekcja", sek.id, sek.nazwa, sek)}
                akcje={
                  !ro && (
                    <>
                      <Przycisk maly wariant="tekstowy" tylkoIkona ikona={Plus} etykieta="Dodaj część" tabIndex={-1} onClick={() => void dodajCzesc(sek)} />
                      {sek.typ === "meble" && (
                        <Przycisk maly wariant="tekstowy" tylkoIkona ikona={LayoutTemplate} etykieta="Część z szablonu" tabIndex={-1} onClick={() => ustawDialog({ tryb: "czesc", sekcjaId: sek.id })} />
                      )}
                      <Przycisk maly wariant="tekstowy" tylkoIkona ikona={Pencil} etykieta="Zmień nazwę (F2)" tabIndex={-1} onClick={() => ustawEdycje({ typ: "sekcja", id: sek.id, nazwa: sek.nazwa })} />
                      <Przycisk maly wariant="tekstowy" tylkoIkona ikona={Copy} etykieta="Kopiuj sekcję" tabIndex={-1} onClick={() => void wykonaj(sp.kopiujSekcje(sek.id)).then((w) => wybierzPo(w, "sekcja"))} />
                      <Przycisk maly wariant="tekstowy" tylkoIkona ikona={ArrowUp} etykieta="W górę (Alt+↑)" tabIndex={-1} disabled={i === 0} onClick={() => void przesunSekcje(sek.id, -1)} />
                      <Przycisk maly wariant="tekstowy" tylkoIkona ikona={ArrowDown} etykieta="W dół (Alt+↓)" tabIndex={-1} disabled={i === s.sekcje.length - 1} onClick={() => void przesunSekcje(sek.id, 1)} />
                      <Przycisk maly wariant="tekstowy" tylkoIkona ikona={Trash2} etykieta="Usuń sekcję" tabIndex={-1} onClick={() => void usunSekcje(sek)} />
                    </>
                  )
                }
              />
              {rozwiniete &&
                sek.czesci.map((c, j) => (
                  <ElementDrzewa
                    key={c.id}
                    poziom={1}
                    ikona={Box}
                    etykieta={edycja?.typ === "czesc" && edycja.id === c.id ? poleNazwy : c.nazwa}
                    zaznaczony={wybor?.typ === "czesc" && wybor.id === c.id}
                    licznik={c.wiersze.length}
                    onWybierz={() => onWybor({ typ: "czesc", id: c.id })}
                    onDwuklik={() => !ro && ustawEdycje({ typ: "czesc", id: c.id, nazwa: c.nazwa })}
                    onKlawisz={(e) => klawiszElementu(e, "czesc", c.id, c.nazwa, sek)}
                    akcje={
                      !ro && (
                        <>
                          <Przycisk maly wariant="tekstowy" tylkoIkona ikona={Pencil} etykieta="Zmień nazwę (F2)" tabIndex={-1} onClick={() => ustawEdycje({ typ: "czesc", id: c.id, nazwa: c.nazwa })} />
                          <Przycisk maly wariant="tekstowy" tylkoIkona ikona={Copy} etykieta="Kopiuj część" tabIndex={-1} onClick={() => void wykonaj(sp.kopiujCzesc(c.id)).then((w) => wybierzPo(w, "czesc"))} />
                          {s.sekcje.length > 1 && (
                            <Przycisk maly wariant="tekstowy" tylkoIkona ikona={FolderInput} etykieta="Przenieś do innej sekcji" tabIndex={-1} onClick={() => ustawPrzenoszona({ id: c.id, nazwa: c.nazwa, sekcjaId: sek.id })} />
                          )}
                          <Przycisk maly wariant="tekstowy" tylkoIkona ikona={ArrowUp} etykieta="W górę (Alt+↑)" tabIndex={-1} disabled={j === 0} onClick={() => void przesunCzesc(sek, c.id, -1)} />
                          <Przycisk maly wariant="tekstowy" tylkoIkona ikona={ArrowDown} etykieta="W dół (Alt+↓)" tabIndex={-1} disabled={j === sek.czesci.length - 1} onClick={() => void przesunCzesc(sek, c.id, 1)} />
                          <Przycisk maly wariant="tekstowy" tylkoIkona ikona={Trash2} etykieta="Usuń część" tabIndex={-1} onClick={() => void usunCzesc(c.id, c.nazwa)} />
                        </>
                      )
                    }
                  />
                ))}
            </div>
          );
        })}
      </div>
      {przenoszona && (
        <Okno tytul={`Przenieś część „${przenoszona.nazwa}”`} onZamknij={() => ustawPrzenoszona(null)}>
          <p className="pole__pomoc">Wybierz sekcję, na której koniec trafi część:</p>
          <ul className="lista-celow">
            {s.sekcje
              .filter((x) => x.id !== przenoszona.sekcjaId)
              .map((x) => (
                <li key={x.id}>
                  <Przycisk
                    wariant="tekstowy"
                    ikona={FolderInput}
                    onClick={async () => {
                      const w = await wykonaj(sp.kolejnoscCzesci(x.id, [...x.czesci.map((c) => c.id), przenoszona.id]));
                      if (w) {
                        ustawPrzenoszona(null);
                        onWybor({ typ: "czesc", id: przenoszona.id });
                      }
                    }}
                  >
                    {etykietaSekcji(x)}
                  </Przycisk>
                </li>
              ))}
          </ul>
        </Okno>
      )}
      {dialog && (
        <DialogSzablonu
          tryb={dialog.tryb}
          szablony={szablony}
          onZamknij={() => ustawDialog(null)}
          onWstaw={async (szablonId, nazwa, czesci) => {
            const w =
              dialog.tryb === "sekcja"
                ? await wykonaj(sp.sekcjaZSzablonu(s.id, { szablon_id: szablonId, nazwa: nazwa || undefined, czesci }))
                : await wykonaj(sp.czescZSzablonu(dialog.sekcjaId, { szablon_id: szablonId, nazwa: nazwa || undefined }));
            if (w) {
              ustawDialog(null);
              if (dialog.tryb === "czesc") onWybor({ typ: "czesc", id: w.id });
              else {
                const pierwsza = w.specyfikacja.sekcje.find((x) => x.id === w.id)?.czesci[0];
                onWybor(pierwsza ? { typ: "czesc", id: pierwsza.id } : { typ: "sekcja", id: w.id });
              }
            }
          }}
        />
      )}
    </div>
  );
}

function DialogSzablonu({
  tryb,
  szablony,
  onZamknij,
  onWstaw,
}: {
  tryb: "sekcja" | "czesc";
  szablony: SzablonMebli[];
  onZamknij: () => void;
  onWstaw: (szablonId: number, nazwa: string, czesci?: string[]) => void;
}) {
  const [szablonId, ustawSzablon] = useState<number | null>(szablony[0]?.id ?? null);
  const szablon = szablony.find((x) => x.id === szablonId);
  const [nazwa, ustawNazwe] = useState("");
  const [czesci, ustawCzesci] = useState<string[] | null>(null);
  const wybraneCzesci = czesci ?? (szablon?.czesci.slice(0, 1) ?? []);

  return (
    <Okno
      tytul={tryb === "sekcja" ? "Nowa sekcja z szablonu" : "Nowa część z szablonu"}
      onZamknij={onZamknij}
      stopka={
        <>
          <Przycisk onClick={onZamknij}>Anuluj</Przycisk>
          <Przycisk wariant="glowny" disabled={!szablon} onClick={() => szablon && onWstaw(szablon.id, nazwa, tryb === "sekcja" ? wybraneCzesci : undefined)}>
            Wstaw
          </Przycisk>
        </>
      }
    >
      {szablony.length === 0 && <p>Brak szablonów. Zaimportuj plik szablonów w module Słowniki.</p>}
      <div className="lista-szablonow" role="listbox" aria-label="Szablony mebli">
        {szablony.map((sz) => (
          <button
            key={sz.id}
            type="button"
            role="option"
            aria-selected={sz.id === szablonId}
            className={`lista-szablonow__pozycja${sz.id === szablonId ? " lista-szablonow__pozycja--wybrana" : ""}`}
            onClick={() => {
              ustawSzablon(sz.id);
              ustawCzesci(null);
              ustawNazwe("");
            }}
          >
            <strong>{sz.nazwa}</strong>
            <span>
              {sz.wiersze.length} wierszy · {sz.czesci.slice(0, 3).join(", ")}
              {sz.zrodlo === "import" ? " · z importu" : ""}
            </span>
          </button>
        ))}
      </div>
      {szablon && tryb === "sekcja" && (
        <>
          <Pole etykieta="Nazwa sekcji" id="szablon-nazwa" pomoc={`Domyślnie: ${szablon.tytul_sekcji}`}>
            <input id="szablon-nazwa" value={nazwa} placeholder={szablon.tytul_sekcji} onChange={(e) => ustawNazwe(e.target.value)} />
          </Pole>
          <fieldset className="lista-czesci">
            <legend className="pole__etykieta">Części do utworzenia (każda z wierszami szablonu)</legend>
            {szablon.czesci.map((c) => (
              <label key={c}>
                <input
                  type="checkbox"
                  checked={wybraneCzesci.includes(c)}
                  onChange={(e) => ustawCzesci(e.target.checked ? [...wybraneCzesci, c] : wybraneCzesci.filter((x) => x !== c))}
                />
                {c}
              </label>
            ))}
          </fieldset>
        </>
      )}
      {szablon && tryb === "czesc" && (
        <Pole etykieta="Nazwa części" id="szablon-czesc">
          <input id="szablon-czesc" list="szablon-czesci-lista" value={nazwa} placeholder={szablon.czesci[0] ?? szablon.nazwa} onChange={(e) => ustawNazwe(e.target.value)} />
          <datalist id="szablon-czesci-lista">
            {szablon.czesci.map((c) => (
              <option key={c} value={c} />
            ))}
          </datalist>
        </Pole>
      )}
      {szablon && (
        <p className="pole__pomoc">
          Wiersze: {szablon.wiersze.map((w) => w.rodzaj).filter(Boolean).join(", ")}
        </p>
      )}
    </Okno>
  );
}
