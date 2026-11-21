// Panel wyceny: poziom wyceny, ceny części/sekcji/całości, rabaty, VAT, AGD, suma brutto.
// Marża i koszt własny tylko w rozwijanym panelu wewnętrznym (nigdy w dokumentach).

import { useState } from "react";
import { CircleAlert, Eye, EyeOff, Lock, Refrigerator } from "lucide-react";
import type { DaneWersji, PelnaWersja, PoziomWyceny, Specyfikacja, TrybCen } from "../../api/typy";
import { usePolaczenie } from "../../kontekst/Polaczenie";
import { formatujKwote } from "../../lib/formaty";
import { Przycisk, Segment } from "../../ui";
import { PoleKwoty, daneWersji } from "./wspolne";

interface Props {
  dane: PelnaWersja;
  onDane: (d: PelnaWersja) => void;
  onBlad: (b: string | null) => void;
  onSpecyfikacja: (s: Specyfikacja) => void;
}

const procent = (bp: number) => (bp / 100).toLocaleString("pl-PL", { maximumFractionDigits: 2 });

export function PanelWyceny({ dane, onDane, onBlad, onSpecyfikacja }: Props) {
  const { api } = usePolaczenie();
  const [wewnetrzne, ustawWewnetrzne] = useState(false);
  const [rabatTekst, ustawRabatTekst] = useState<string | null>(null);
  const { wersja: w, wycena: wy, specyfikacja: s } = dane;
  const ro = !w.najnowsza;
  if (!api) return null;

  const zapisz = async (zmiana: Partial<DaneWersji>) => {
    try {
      onDane(await api.oferty.zmienWersje(w.id, { ...daneWersji(w), ...zmiana }));
      onBlad(null);
    } catch (e) {
      onBlad((e as Error).message);
    }
  };

  const cenaSekcji = async (sekcjaId: number, cena: number | null) => {
    const sek = s.sekcje.find((x) => x.id === sekcjaId);
    if (!sek) return;
    try {
      const r = await api.specyfikacje.zmienSekcje(sek.id, { nazwa: sek.nazwa, typ: sek.typ, wariant: sek.wariant, cena_gr: cena, stawka_vat: sek.stawka_vat });
      onSpecyfikacja(r.specyfikacja);
    } catch (e) {
      onBlad((e as Error).message);
    }
  };

  const cenaCzesci = async (czescId: number, nazwa: string, cena: number | null) => {
    try {
      const r = await api.specyfikacje.zmienCzesc(czescId, { nazwa, cena_gr: cena });
      onSpecyfikacja(r.specyfikacja);
    } catch (e) {
      onBlad((e as Error).message);
    }
  };

  const zapiszRabatProcent = () => {
    if (rabatTekst === null) return;
    const v = Number(rabatTekst.replace(",", ".").replace("%", "").trim() || "0");
    ustawRabatTekst(null);
    if (Number.isFinite(v) && v >= 0 && v <= 100) {
      const bp = Math.round(v * 100);
      if (bp !== w.rabat_calosc_bp) void zapisz({ rabat_calosc_bp: bp });
    } else onBlad("Rabat procentowy: od 0 do 100");
  };

  const netto = w.tryb_cen === "netto";
  const meble = wy.sekcje.filter((x) => !x.agd);
  const agd = wy.sekcje.filter((x) => x.agd);

  return (
    <div className="panel-wyceny">
      {ro && (
        <div className="komunikat-stanu status--neutralny">
          <Lock size={14} aria-hidden /> Wersja archiwalna – wycena tylko do podglądu.
        </div>
      )}
      <div className="panel-wyceny__grupa">
        <span className="pole__etykieta">Cena wpisywana dla</span>
        <Segment<PoziomWyceny>
          etykieta="Poziom wyceny"
          wartosc={w.poziom_wyceny}
          wylaczony={ro}
          onZmiana={(p) => void zapisz({ poziom_wyceny: p })}
          opcje={[
            { wartosc: "czesc", etykieta: "Części" },
            { wartosc: "sekcja", etykieta: "Sekcji" },
            { wartosc: "calosc", etykieta: "Całości" },
          ]}
        />
      </div>
      <div className="panel-wyceny__grupa panel-wyceny__rzad">
        <div>
          <span className="pole__etykieta">VAT</span>
          <Segment<8 | 23> etykieta="Stawka VAT" wartosc={w.stawka_vat} wylaczony={ro} onZmiana={(v) => void zapisz({ stawka_vat: v })} opcje={[{ wartosc: 8, etykieta: "8%" }, { wartosc: 23, etykieta: "23%" }]} />
        </div>
        <div>
          <span className="pole__etykieta">Ceny wpisane</span>
          <Segment<TrybCen> etykieta="Tryb cen" wartosc={w.tryb_cen} wylaczony={ro} onZmiana={(t) => void zapisz({ tryb_cen: t })} opcje={[{ wartosc: "brutto", etykieta: "Brutto" }, { wartosc: "netto", etykieta: "Netto" }]} />
        </div>
      </div>

      <section className="panel-wyceny__sekcje" aria-label="Ceny">
        {meble.map((sek) => (
          <div key={sek.sekcja_id} className="panel-wyceny__sekcja">
            <div className="panel-wyceny__wiersz panel-wyceny__wiersz--sekcja">
              <span className="panel-wyceny__nazwa" title={sek.nazwa}>
                {sek.numer} {sek.nazwa}
              </span>
              {w.poziom_wyceny === "sekcja" ? (
                <PoleKwoty etykieta={`Cena sekcji ${sek.nazwa}`} wartosc={s.sekcje.find((x) => x.id === sek.sekcja_id)?.cena_gr ?? null} wylaczone={ro} onZapisz={(c) => void cenaSekcji(sek.sekcja_id, c)} />
              ) : (
                <span className="panel-wyceny__kwota">{sek.suma_gr === null ? "w cenie całości" : formatujKwote(sek.suma_gr)}</span>
              )}
            </div>
            {w.poziom_wyceny === "czesc" &&
              (s.sekcje.find((x) => x.id === sek.sekcja_id)?.czesci ?? []).map((c) => (
                <div key={c.id} className="panel-wyceny__wiersz panel-wyceny__wiersz--czesc">
                  <span className="panel-wyceny__nazwa" title={c.nazwa}>
                    {c.nazwa}
                  </span>
                  <PoleKwoty etykieta={`Cena części ${c.nazwa}`} wartosc={c.cena_gr} wylaczone={ro} onZapisz={(v) => void cenaCzesci(c.id, c.nazwa, v)} />
                </div>
              ))}
            {sek.suma_gr !== null && (
              <div className="panel-wyceny__wiersz panel-wyceny__suma-sekcji">
                <span>Suma za meble</span>
                <span className="panel-wyceny__kwota">{formatujKwote(sek.suma_gr)}</span>
              </div>
            )}
          </div>
        ))}
        {w.poziom_wyceny === "calosc" && (
          <div className="panel-wyceny__wiersz panel-wyceny__wiersz--sekcja">
            <label htmlFor="cena-calosc" className="panel-wyceny__nazwa">
              Cena za całość mebli
            </label>
            <PoleKwoty id="cena-calosc" wartosc={w.cena_calosc_gr} wylaczone={ro} onZapisz={(c) => void zapisz({ cena_calosc_gr: c })} />
          </div>
        )}
        {s.sekcje.length === 0 && <p className="pole__pomoc">Dodaj sekcje specyfikacji, aby wpisać ceny.</p>}
      </section>

      <section className="panel-wyceny__grupa" aria-label="Rabaty">
        <div className="panel-wyceny__wiersz">
          <label htmlFor="rabat-termin" className="panel-wyceny__nazwa">
            Rabat za termin
          </label>
          <PoleKwoty id="rabat-termin" wartosc={w.rabat_termin_gr} wymagane wylaczone={ro} onZapisz={(c) => void zapisz({ rabat_termin_gr: c ?? 0 })} />
        </div>
        <div className="panel-wyceny__wiersz">
          <label htmlFor="rabat-calosc" className="panel-wyceny__nazwa">
            Rabat za całość
          </label>
          <span className="pole-kwoty">
            <input
              id="rabat-calosc"
              className="wejscie wejscie--kwota"
              inputMode="decimal"
              disabled={ro}
              value={rabatTekst ?? procent(w.rabat_calosc_bp)}
              onChange={(e) => ustawRabatTekst(e.target.value)}
              onBlur={zapiszRabatProcent}
              onKeyDown={(e) => e.key === "Enter" && zapiszRabatProcent()}
            />
            <span className="pole-kwoty__waluta">%</span>
          </span>
        </div>
      </section>

      {agd.length > 0 && (
        <section className="panel-wyceny__grupa" aria-label="Sprzęt AGD">
          {agd.map((a) => (
            <div key={a.sekcja_id} className="panel-wyceny__wiersz">
              <span className="panel-wyceny__nazwa">
                <Refrigerator size={13} aria-hidden /> {a.nazwa} (VAT {a.stawka_vat}%)
              </span>
              <span className="panel-wyceny__kwota">{formatujKwote(a.suma_gr)}</span>
            </div>
          ))}
          <p className="pole__pomoc">AGD liczone osobno: cena × ilość z wierszy sekcji, bez rabatów.</p>
        </section>
      )}

      <section className="panel-wyceny__podsumowanie" aria-label="Podsumowanie">
        <Linia etykieta={`Suma za meble${netto ? " (netto)" : ""}`} kwota={wy.suma_meble_gr} />
        {wy.rabat_termin_gr > 0 && <Linia etykieta="Rabat za termin" kwota={-wy.rabat_termin_gr} />}
        {wy.rabat_calosc_gr > 0 && <Linia etykieta={`Rabat za całość (${procent(wy.rabat_calosc_bp)}%)`} kwota={-wy.rabat_calosc_gr} />}
        {(wy.rabat_termin_gr > 0 || wy.rabat_calosc_gr > 0) && <Linia etykieta="Meble po rabatach" kwota={wy.meble_po_rabatach_gr} />}
        {agd.length > 0 && <Linia etykieta={`Sprzęt AGD${netto ? " (netto)" : ""}`} kwota={wy.suma_agd_gr} />}
        <Linia etykieta="Netto" kwota={wy.suma_netto_gr} wyciszona />
        <Linia etykieta={`VAT (${[wy.meble.stawka, ...wy.agd.map((a) => a.stawka)].filter((v, i, t) => t.indexOf(v) === i).join("% / ")}%)`} kwota={wy.suma_vat_gr} wyciszona />
        <div className="panel-wyceny__brutto">
          <span>SUMA brutto</span>
          <strong>{formatujKwote(wy.suma_brutto_gr)}</strong>
        </div>
      </section>

      {wy.ostrzezenia.length > 0 && (
        <ul className="panel-wyceny__ostrzezenia" aria-label="Ostrzeżenia wyceny">
          {wy.ostrzezenia.map((o) => (
            <li key={o} className="komunikat-stanu status--zolty">
              <CircleAlert size={14} aria-hidden /> {o}
            </li>
          ))}
        </ul>
      )}

      <section className="panel-wyceny__wewnetrzne">
        <Przycisk maly wariant="tekstowy" ikona={wewnetrzne ? EyeOff : Eye} aria-expanded={wewnetrzne} onClick={() => ustawWewnetrzne((x) => !x)}>
          {wewnetrzne ? "Ukryj dane wewnętrzne" : "Dane wewnętrzne (koszt, marża)"}
        </Przycisk>
        {wewnetrzne && (
          <div className="panel-wyceny__grupa">
            <div className="panel-wyceny__wiersz">
              <label htmlFor="koszt-wlasny" className="panel-wyceny__nazwa">
                Koszt własny (netto)
              </label>
              <PoleKwoty id="koszt-wlasny" wartosc={w.koszt_wlasny_gr} wylaczone={ro} onZapisz={(c) => void zapisz({ koszt_wlasny_gr: c })} />
            </div>
            <Linia etykieta="Przychód netto mebli" kwota={wy.meble.netto_gr} />
            <div className="panel-wyceny__wiersz">
              <span className="panel-wyceny__nazwa">Marża</span>
              <span className="panel-wyceny__kwota">
                {wy.wewnetrzne.marza_gr === null ? "— (wpisz koszt)" : `${formatujKwote(wy.wewnetrzne.marza_gr)}${wy.wewnetrzne.marza_bp !== null ? ` · ${procent(wy.wewnetrzne.marza_bp)}%` : ""}`}
              </span>
            </div>
            <p className="pole__pomoc">Dane wewnętrzne nie trafiają do PDF ani DOCX.</p>
          </div>
        )}
      </section>
    </div>
  );
}

function Linia({ etykieta, kwota, wyciszona }: { etykieta: string; kwota: number; wyciszona?: boolean }) {
  return (
    <div className={`panel-wyceny__wiersz${wyciszona ? " panel-wyceny__wiersz--wyciszony" : ""}`}>
      <span className="panel-wyceny__nazwa">{etykieta}</span>
      <span className="panel-wyceny__kwota">{formatujKwote(kwota)}</span>
    </div>
  );
}
