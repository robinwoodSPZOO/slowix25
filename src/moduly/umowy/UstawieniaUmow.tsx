// Ustawienia umów (format numeru, terminy, warianty płatności, domyślne czasy produkcji)
// oraz szablony dokumentów (wgrywanie DOCX, raport pól, instrukcja przygotowania).

import { useEffect, useRef, useState, type FormEvent } from "react";
import { useNavigate } from "react-router-dom";
import { ArrowLeft, Building2, CircleAlert, CircleCheck, Download, RotateCcw, Upload } from "lucide-react";
import type { OpisPola, RodzajSzablonu, StanSzablonu, UstawieniaUmow as Ustawienia } from "../../api/typyUmow";
import { usePolaczenie } from "../../kontekst/Polaczenie";
import { EtykietaStatusu, Pole, Przycisk } from "../../ui";
import { zapiszPlik } from "../oferty/pobieranie";
import { KROKI_SZABLONU } from "./instrukcja";
import { numerZFormatu } from "./logika";

export function UstawieniaUmow() {
  const { api } = usePolaczenie();
  const nawiguj = useNavigate();
  const [u, ustawU] = useState<Ustawienia | null>(null);
  const [szablony, ustawSzablony] = useState<StanSzablonu[]>([]);
  const [opisy, ustawOpisy] = useState<OpisPola[]>([]);
  const [blad, ustawBlad] = useState<string | null>(null);
  const [zapisano, ustawZapisano] = useState(false);

  useEffect(() => {
    if (!api) return;
    api.umowy.ustawienia().then(ustawU, (e: Error) => ustawBlad(e.message));
    api.szablonyDokumentow.lista().then(ustawSzablony, (e: Error) => ustawBlad(e.message));
    api.szablonyDokumentow.opisyPol().then(ustawOpisy, () => {});
  }, [api]);

  if (!api || !u) return <p>{blad ?? "Wczytywanie…"}</p>;

  const liczba = (k: keyof Ustawienia, min = 0, max = 365) => ({
    id: `uu-${k}`,
    type: "number",
    min,
    max,
    value: String(u[k]),
    onChange: (e: { target: { value: string } }) => {
      ustawZapisano(false);
      ustawU({ ...u, [k]: Number(e.target.value) });
    },
  });

  const zapisz = async (e: FormEvent) => {
    e.preventDefault();
    try {
      ustawU(await api.umowy.zapiszUstawienia(u));
      ustawZapisano(true);
      ustawBlad(null);
    } catch (er) {
      ustawBlad((er as Error).message);
    }
  };

  const zamienSzablon = (s: StanSzablonu) => ustawSzablony((l) => l.map((x) => (x.rodzaj === s.rodzaj ? s : x)));

  return (
    <div className="ustawienia-umow">
      <Przycisk wariant="tekstowy" ikona={ArrowLeft} onClick={() => nawiguj("/umowy")}>
        Umowy
      </Przycisk>
      <h1>Ustawienia umów i szablony dokumentów</h1>
      <div className="ustawienia-umow__kolumny">
        <form className="karta ustawienia-ofert__formularz" onSubmit={zapisz}>
          <h2>Numeracja i domyślne wartości</h2>
          <Pole etykieta="Format numeru umowy" id="uu-format" pomoc={`{N}/{NN}/{NNN} – numer w roku, {RRRR}/{RR} – rok. Przykład: ${numerZFormatu(u.format_numeru, new Date().getFullYear(), 7)}`}>
            <input
              id="uu-format"
              value={u.format_numeru}
              onChange={(e) => {
                ustawZapisano(false);
                ustawU({ ...u, format_numeru: e.target.value });
              }}
            />
          </Pole>
          <Pole etykieta="Domyślna miejscowość zawarcia" id="uu-miejscowosc">
            <input
              id="uu-miejscowosc"
              value={u.miejscowosc}
              onChange={(e) => {
                ustawZapisano(false);
                ustawU({ ...u, miejscowosc: e.target.value });
              }}
            />
          </Pole>
          <div className="ustawienia-ofert__rzad">
            <Pole etykieta="Dni na brakujące informacje" id="uu-dni_na_informacje">
              <input {...liczba("dni_na_informacje")} />
            </Pole>
            <Pole etykieta="Zadatek – dni od zawarcia" id="uu-termin_zadatku_dni" pomoc="Dni kalendarzowe">
              <input {...liczba("termin_zadatku_dni")} />
            </Pole>
            <Pole etykieta="Zadatek – podpowiedź %" id="uu-zadatek_procent">
              <input {...liczba("zadatek_procent", 0, 100)} />
            </Pole>
          </div>
          <h2>Warianty płatności (do potwierdzenia)</h2>
          <p className="pole__pomoc">
            A: zadatek + pozostała część najpóźniej N dni roboczych przed rozpoczęciem montażu. B: zadatek + kwota przed montażem + pozostałość w terminie N dni roboczych od prawidłowego oddania przedmiotu umowy. Suma rat zawsze równa
            się wartości umowy.
          </p>
          <div className="ustawienia-ofert__rzad">
            <Pole etykieta="A: dni rob. przed montażem" id="uu-wariant_a_dni_przed_montazem">
              <input {...liczba("wariant_a_dni_przed_montazem", 0, 60)} />
            </Pole>
            <Pole etykieta="B: dni rob. przed montażem" id="uu-wariant_b_dni_przed_montazem">
              <input {...liczba("wariant_b_dni_przed_montazem", 0, 60)} />
            </Pole>
            <Pole etykieta="B: dni rob. po odbiorze" id="uu-wariant_b_dni_po_odbiorze">
              <input {...liczba("wariant_b_dni_po_odbiorze", 0, 60)} />
            </Pole>
          </div>
          <h2>Produkcja (domyślnie dla nowej umowy)</h2>
          <div className="ustawienia-ofert__rzad">
            <Pole etykieta="Czas na warsztacie (dni)" id="uu-czas_warsztat_dni">
              <input {...liczba("czas_warsztat_dni")} />
            </Pole>
            <Pole etykieta="Czas montażu (dni)" id="uu-czas_montazu_dni">
              <input {...liczba("czas_montazu_dni", 1)} />
            </Pole>
          </div>
          {blad && <p className="blad">{blad}</p>}
          <div className="ustawienia-ofert__przyciski">
            <Przycisk type="submit" wariant="glowny">
              Zapisz ustawienia
            </Przycisk>
            {zapisano && <EtykietaStatusu stan="spokojnie" ikona={CircleCheck} tekst="Zapisano" />}
            <Przycisk wariant="tekstowy" ikona={Building2} onClick={() => nawiguj("/oferty/ustawienia")}>
              Dane firmy (bank, rachunek)
            </Przycisk>
          </div>
        </form>

        <div className="ustawienia-umow__szablony">
          {szablony.map((s) => (
            <KartaSzablonu key={s.rodzaj} stan={s} onStan={zamienSzablon} onBlad={ustawBlad} />
          ))}
        </div>
      </div>

      <section className="karta ustawienia-umow__instrukcja">
        <h2>Jak przygotować szablon z dotychczasowej umowy</h2>
        <ol>
          {KROKI_SZABLONU.map((k) => (
            <li key={k.tytul}>
              <strong>{k.tytul}.</strong> {k.opis}
            </li>
          ))}
        </ol>
        <h3>Pola szablonu</h3>
        <table className="tabela tabela--zwarta">
          <thead>
            <tr>
              <th>Pole</th>
              <th>Co wstawia program</th>
              <th>Dokument</th>
            </tr>
          </thead>
          <tbody>
            {opisy.map((o) => (
              <tr key={o.nazwa}>
                <td>
                  <code>{o.rodzaj === "warunek" ? `[[${o.nazwa}]] … [[/${o.nazwa}]]` : `[${o.nazwa}]`}</code>
                </td>
                <td>
                  {o.opis}
                  {o.rodzaj === "blok" && " – w osobnym akapicie"}
                </td>
                <td>{o.dokument === "oba" ? "umowa, aneks" : o.dokument}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </section>
    </div>
  );
}

const NAZWY: Record<RodzajSzablonu, string> = { umowa: "Szablon umowy", aneks: "Szablon aneksu" };

function KartaSzablonu({ stan, onStan, onBlad }: { stan: StanSzablonu; onStan: (s: StanSzablonu) => void; onBlad: (b: string | null) => void }) {
  const { api } = usePolaczenie();
  const plik = useRef<HTMLInputElement>(null);
  const [trwa, ustawTrwa] = useState(false);
  if (!api) return null;
  const r = stan.raport;
  const wgraj = async (f: File) => {
    ustawTrwa(true);
    try {
      onStan(await api.szablonyDokumentow.wgraj(stan.rodzaj, f, f.name));
      onBlad(null);
    } catch (e) {
      onBlad((e as Error).message);
    } finally {
      ustawTrwa(false);
      if (plik.current) plik.current.value = "";
    }
  };
  return (
    <section className="karta grupa-umowy karta-szablonu" aria-label={NAZWY[stan.rodzaj]}>
      <h3>{NAZWY[stan.rodzaj]}</h3>
      {stan.zrodlo === "wlasny" ? (
        <EtykietaStatusu stan="spokojnie" ikona={CircleCheck} tekst={`Własny: ${stan.meta?.nazwa_pliku ?? ""}`} />
      ) : (
        <EtykietaStatusu stan="zolty" ikona={CircleAlert} tekst="Przykładowy (wbudowany) – wgraj własny" />
      )}
      {stan.meta && <p className="grupa-umowy__mikro">Wgrano {new Date(stan.meta.wgrano).toLocaleString("pl-PL")}, {(stan.meta.rozmiar / 1024).toFixed(0)} KB</p>}
      <p className="grupa-umowy__info">
        Rozpoznane pola: {r.pola.length}
        {r.kontrolki ? ` (w tym kontrolki treści: ${r.kontrolki})` : ""}; bloki warunkowe: {r.bloki_warunkowe.length ? r.bloki_warunkowe.join(", ") : "brak"}
        {r.do_uzupelnienia ? `; „[uzupełnij]”: ${r.do_uzupelnienia}` : ""}.
      </p>
      {r.nieznane.length > 0 && (
        <p className="komunikat-stanu status--zolty">
          <CircleAlert size={14} aria-hidden /> Pola nieznane (zostaną bez zmian – sprawdź pisownię): {r.nieznane.map((n) => `[${n}]`).join(", ")}
        </p>
      )}
      {r.ostrzezenia.map((o) => (
        <p key={o} className="grupa-umowy__mikro">
          {o}
        </p>
      ))}
      <div className="grupa-umowy__wiersz">
        <input ref={plik} type="file" accept=".docx,.docm,application/vnd.openxmlformats-officedocument.wordprocessingml.document" hidden onChange={(e) => e.target.files?.[0] && void wgraj(e.target.files[0])} />
        <Przycisk wariant="glowny" ikona={Upload} disabled={trwa} onClick={() => plik.current?.click()}>
          {trwa ? "Wgrywanie…" : "Wgraj szablon"}
        </Przycisk>
        <Przycisk
          ikona={Download}
          onClick={async () => {
            try {
              const { nazwa, plik: b } = await api.szablonyDokumentow.plik(stan.rodzaj);
              zapiszPlik(b, nazwa);
            } catch (e) {
              onBlad((e as Error).message);
            }
          }}
        >
          Pobierz
        </Przycisk>
        {stan.zrodlo === "wlasny" && (
          <Przycisk
            wariant="tekstowy"
            ikona={RotateCcw}
            onClick={async () => {
              if (!confirm("Usunąć własny szablon i wrócić do przykładowego?")) return;
              try {
                onStan(await api.szablonyDokumentow.usun(stan.rodzaj));
              } catch (e) {
                onBlad((e as Error).message);
              }
            }}
          >
            Przywróć przykładowy
          </Przycisk>
        )}
      </div>
    </section>
  );
}
