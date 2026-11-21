// Ręczne dodawanie i edycja własnego zadania (z kopią szkicu – zamknięcie okna nie gubi wpisów).

import { useState } from "react";
import { CircleAlert, History } from "lucide-react";
import type { DaneZadaniaRecznego, ZadanieKalendarza, Zlecenie } from "../../api/typy";
import { usePolaczenie } from "../../kontekst/Polaczenie";
import { Okno, Pole, Przycisk, PoleDaty } from "../../ui";
import { komunikatBledu } from "./dane";
import { poprawnaData } from "./daty";
import { useSzkic } from "./szkic";
import { nazwaTypu, TYPY_W_LEGENDZIE } from "./wyglad";

interface Formularz {
  typ: string;
  tytul: string;
  data_od: string;
  data_do: string;
  zlecenie_id: string;
  uwagi: string;
}

export function OknoZadania({
  zadanie,
  dzien,
  zlecenia,
  onZamknij,
  onZapisano,
}: {
  zadanie: ZadanieKalendarza | null;
  dzien: string;
  zlecenia: Zlecenie[];
  onZamknij: () => void;
  onZapisano: (z: ZadanieKalendarza) => void;
}) {
  const { api } = usePolaczenie();
  const [poczatkowa] = useState<Formularz>(() => ({
    typ: zadanie?.typ ?? "inne",
    tytul: zadanie?.tytul ?? "",
    data_od: zadanie?.data_od ?? dzien,
    data_do: zadanie?.data_do ?? "",
    zlecenie_id: zadanie?.zlecenie_id ? String(zadanie.zlecenie_id) : "",
    uwagi: zadanie?.uwagi ?? "",
  }));
  const { wartosc, zmien, wyczysc, przywrocono } = useSzkic<Formularz>(`kalendarz:zadanie:${zadanie?.id ?? "nowe"}`, poczatkowa);
  const f = wartosc ?? poczatkowa;
  const [blad, ustawBlad] = useState<string | null>(null);
  const [zapisywanie, ustawZapisywanie] = useState(false);
  const [proba, ustawProbe] = useState(false);

  const ustaw = <K extends keyof Formularz>(k: K, v: Formularz[K]) => zmien({ ...f, [k]: v });
  const bledy = {
    tytul: f.tytul.trim() === "" ? "Wpisz, co trzeba zrobić" : null,
    data_od: !poprawnaData(f.data_od) ? "Wpisz datę" : null,
    data_do: f.data_do && !poprawnaData(f.data_do) ? "Wpisz datę" : f.data_do && f.data_do < f.data_od ? "Koniec nie może być przed początkiem" : null,
  };
  const poprawny = !bledy.tytul && !bledy.data_od && !bledy.data_do;

  const zapisz = async () => {
    ustawProbe(true);
    if (!api || !poprawny) return;
    const d: DaneZadaniaRecznego = {
      typ: f.typ,
      tytul: f.tytul.trim(),
      data_od: f.data_od,
      data_do: f.data_do || null,
      zlecenie_id: f.zlecenie_id ? Number(f.zlecenie_id) : null,
      uwagi: f.uwagi.trim() || null,
    };
    ustawZapisywanie(true);
    try {
      const z = zadanie ? await api.kalendarz.zmienZadanie(zadanie.id, d) : await api.kalendarz.dodajZadanie(d);
      wyczysc();
      onZapisano(z);
    } catch (e) {
      ustawBlad(komunikatBledu(e));
    } finally {
      ustawZapisywanie(false);
    }
  };

  return (
    <Okno
      tytul={zadanie ? "Edycja zadania" : "Nowe zadanie w kalendarzu"}
      onZamknij={onZamknij}
      stopka={
        <>
          {przywrocono && (
            <Przycisk wariant="tekstowy" onClick={() => wyczysc()}>
              Odrzuć przywrócone zmiany
            </Przycisk>
          )}
          <Przycisk onClick={onZamknij}>Anuluj</Przycisk>
          <Przycisk wariant="glowny" onClick={() => void zapisz()} disabled={zapisywanie}>
            {zadanie ? "Zapisz zmiany" : "Dodaj zadanie"}
          </Przycisk>
        </>
      }
    >
      {przywrocono && (
        <p className="komunikat-stanu stan-tlo--zolty">
          <History size={14} aria-hidden />
          Przywrócono niezapisane zmiany z poprzedniego otwarcia.
        </p>
      )}
      <form
        className="formularz-siatka"
        onSubmit={(e) => {
          e.preventDefault();
          void zapisz();
        }}
      >
        <Pole etykieta="Rodzaj" id="zad-typ" pomoc="Kolor i ikona w kalendarzu zależą od rodzaju">
          <select id="zad-typ" value={f.typ} onChange={(e) => ustaw("typ", e.target.value)}>
            {TYPY_W_LEGENDZIE.map((t) => (
              <option key={t} value={t}>
                {nazwaTypu(t)}
              </option>
            ))}
          </select>
        </Pole>
        <Pole etykieta="Zlecenie (opcjonalnie)" id="zad-zlecenie">
          <select id="zad-zlecenie" value={f.zlecenie_id} onChange={(e) => ustaw("zlecenie_id", e.target.value)}>
            <option value="">— bez zlecenia —</option>
            {zlecenia.map((z) => (
              <option key={z.id} value={z.id}>
                {z.nazwa}
              </option>
            ))}
          </select>
        </Pole>
        <Pole etykieta="Co trzeba zrobić" id="zad-tytul" blad={proba ? bledy.tytul : null} className="pole--szerokie">
          <input id="zad-tytul" value={f.tytul} maxLength={300} autoFocus onChange={(e) => ustaw("tytul", e.target.value)} />
        </Pole>
        <Pole etykieta="Data (od)" id="zad-od" blad={proba ? bledy.data_od : null}>
          <PoleDaty id="zad-od" wartosc={f.data_od} onZmiana={(v) => ustaw("data_od", v)} />
        </Pole>
        <Pole etykieta="Do (opcjonalnie, zadanie wielodniowe)" id="zad-do" blad={bledy.data_do}>
          <PoleDaty id="zad-do" wartosc={f.data_do} min={f.data_od} onZmiana={(v) => ustaw("data_do", v)} />
        </Pole>
        <Pole etykieta="Uwagi" id="zad-uwagi" className="pole--szerokie">
          <textarea id="zad-uwagi" rows={3} value={f.uwagi} onChange={(e) => ustaw("uwagi", e.target.value)} />
        </Pole>
        <button type="submit" hidden />
      </form>
      {blad && (
        <p className="komunikat-stanu stan-tlo--czerwony" role="alert">
          <CircleAlert size={14} aria-hidden />
          {blad}
        </p>
      )}
    </Okno>
  );
}
