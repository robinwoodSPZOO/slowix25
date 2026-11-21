// Aneks do podpisanej umowy: data, zmiana kwoty i terminu, opis zmian, specyfikacja aneksu
// (kopia obowiązującej, edytowana wspólnym edytorem), lista różnic i dokument.

import { useCallback, useEffect, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { ArrowLeft, CircleAlert, CircleCheck, FileSignature, FileText, Send, Trash2 } from "lucide-react";
import type { Specyfikacja } from "../../api/typy";
import type { DaneAneksu, PelnyAneks, StatusAneksu } from "../../api/typyUmow";
import { usePolaczenie } from "../../kontekst/Polaczenie";
import { bladKwoty, formatujDate, formatujKwote, parsujKwote } from "../../lib/formaty";
import { EtykietaStatusu, Pole, Przycisk } from "../../ui";
import { EdytorSpecyfikacji, OknoKonfliktu } from "../specyfikacja";
import { opisHarmonogramu } from "./EkranUmowy";
import { nazwaPolaUmowy } from "./logika";
import { PoleTekstu } from "./pola";
import { STATUSY_ANEKSU } from "./statusy";
import { useFormularz } from "./useFormularz";
import { ListaRoznic, OknoDokumentu } from "./wspolne";

export function EkranAneksu() {
  const { id } = useParams();
  const aneksId = Number(id);
  const { api, ostatnieZdarzenie } = usePolaczenie();
  const nawiguj = useNavigate();
  const [dane, ustawDane] = useState<PelnyAneks | null>(null);
  const [blad, ustawBlad] = useState<string | null>(null);
  const [komunikat, ustawKomunikat] = useState<string | null>(null);
  const [dokument, ustawDokument] = useState(false);
  const [kwota, ustawKwote] = useState<string | null>(null);

  const wczytaj = useCallback(async () => {
    if (!api) return;
    try {
      ustawDane(await api.aneksy.pobierz(aneksId));
      ustawBlad(null);
    } catch (e) {
      ustawBlad((e as Error).message);
    }
  }, [api, aneksId]);
  const zdarzenie = ostatnieZdarzenie?.zasob === "umowy" ? ostatnieZdarzenie : null;
  useEffect(() => {
    if (!api) return;
    api.aneksy.pobierz(aneksId).then(
      (d) => {
        ustawDane(d);
        ustawBlad(null);
      },
      (e: Error) => ustawBlad(e.message),
    );
  }, [api, aneksId, zdarzenie]);

  const baza: DaneAneksu = dane
    ? { data: dane.aneks.data, opis_zmian: dane.aneks.opis_zmian, zmiana_ceny_gr: dane.aneks.zmiana_ceny_gr, nowy_termin_montazu: dane.aneks.nowy_termin_montazu }
    : { data: null, opis_zmian: "", zmiana_ceny_gr: 0, nowy_termin_montazu: null };
  const f = useFormularz<DaneAneksu>(
    baza,
    async (z) => {
      if (!api) return;
      ustawDane(await api.aneksy.zmien(aneksId, z));
    },
    ustawBlad,
  );
  const poZmianieSpecyfikacji = useCallback(
    (s: Specyfikacja) => {
      ustawDane((d) => (d ? { ...d, specyfikacja: s } : d));
      // lista zmian i różnice liczy serwer
      void wczytaj();
    },
    [wczytaj],
  );

  if (!api) return null;
  if (!dane) return <div className="ekran-umowy__ladowanie">{blad ? <p className="blad">{blad}</p> : "Wczytywanie aneksu…"}</div>;
  const a = dane.aneks;
  const st = STATUSY_ANEKSU[a.status];
  const ro = dane.tylko_do_odczytu;
  const tekstKwoty = kwota ?? (a.zmiana_ceny_gr ? (a.zmiana_ceny_gr / 100).toFixed(2).replace(".", ",") : "");
  const bladK = kwota !== null ? bladKwoty(kwota) : null;

  const status = async (s: StatusAneksu) => {
    if (s === "podpisany" && !confirm("Po podpisaniu aneksu obowiązuje jego specyfikacja, kwota i termin; plan płatności i harmonogram w kalendarzu zostaną przeliczone. Kontynuować?")) return;
    try {
      const w = await api.aneksy.zmienStatus(a.id, s);
      ustawDane(w);
      ustawKomunikat(opisHarmonogramu(w.harmonogram));
      ustawBlad(null);
    } catch (e) {
      ustawBlad((e as Error).message);
    }
  };

  return (
    <div className="ekran-umowy">
      <header className="ekran-oferty__naglowek">
        <div className="ekran-oferty__tytul">
          <Przycisk wariant="tekstowy" tylkoIkona ikona={ArrowLeft} etykieta="Wróć do umowy" onClick={() => nawiguj(`/umowy/${a.umowa_id}`)} />
          <div>
            <div className="ekran-oferty__numer">
              <h1>
                Aneks nr {a.nr} do umowy {dane.umowa_numer}
              </h1>
              <EtykietaStatusu stan={st.stan} ikona={st.ikona} tekst={st.etykieta} />
            </div>
            <div className="ekran-oferty__meta">
              <span>
                Wartość: {formatujKwote(a.poprzednia_wartosc_gr)} → {formatujKwote(dane.nowa_wartosc_gr)}
              </span>
              <span>· montaż do {formatujDate(a.nowy_termin_montazu ?? a.poprzedni_termin_montazu)}</span>
            </div>
          </div>
        </div>
        <div className="ekran-oferty__narzedzia">
          {a.status === "szkic" && (
            <Przycisk ikona={Send} onClick={() => void status("wyslany")}>
              Wyślij do klienta
            </Przycisk>
          )}
          {a.status === "wyslany" && (
            <Przycisk wariant="tekstowy" onClick={() => void status("szkic")}>
              Cofnij do szkicu
            </Przycisk>
          )}
          {a.status !== "podpisany" && (
            <Przycisk wariant="glowny" ikona={FileSignature} onClick={() => void status("podpisany")}>
              Podpisany
            </Przycisk>
          )}
          <span className="ekran-oferty__separator" aria-hidden />
          <Przycisk ikona={FileText} onClick={() => ustawDokument(true)}>
            Dokument aneksu
          </Przycisk>
          {a.status !== "podpisany" && (
            <Przycisk
              wariant="tekstowy"
              tylkoIkona
              ikona={Trash2}
              etykieta="Usuń aneks"
              onClick={async () => {
                if (!confirm(`Usunąć aneks nr ${a.nr}?`)) return;
                try {
                  await api.aneksy.usun(a.id);
                  nawiguj(`/umowy/${a.umowa_id}`);
                } catch (e) {
                  ustawBlad((e as Error).message);
                }
              }}
            />
          )}
        </div>
      </header>
      <div className="ekran-umowy__komunikaty">
        {blad && (
          <div className="komunikat-stanu status--czerwony" role="alert">
            <CircleAlert size={14} aria-hidden /> {blad}
          </div>
        )}
        {komunikat && (
          <div className="komunikat-stanu status--spokojnie" role="status">
            <CircleCheck size={14} aria-hidden /> {komunikat}
          </div>
        )}
      </div>
      <div className="ekran-aneksu">
        <section className="karta grupa-umowy ekran-aneksu__formularz" aria-label="Warunki aneksu">
          <h3>Warunki aneksu</h3>
          <div className="grupa-umowy__dwie">
            <PoleTekstu f={f} k="data" typ="date" etykieta="Data aneksu" blokada={ro} />
            <PoleTekstu f={f} k="nowy_termin_montazu" typ="date" etykieta="Nowy termin rozpoczęcia montażu" blokada={ro} pomoc="Puste = bez zmiany terminu" />
          </div>
          <Pole etykieta="Zmiana wynagrodzenia (+/−)" id="a-kwota" blad={bladK} pomoc={`Nowa wartość: ${formatujKwote(dane.nowa_wartosc_gr)}`}>
            <span className="pole-zl">
              <input
                id="a-kwota"
                inputMode="decimal"
                value={tekstKwoty}
                disabled={ro}
                onFocus={() => ustawKwote(tekstKwoty)}
                onChange={(e) => ustawKwote(e.target.value)}
                onBlur={() => {
                  if (kwota === null || bladKwoty(kwota)) return;
                  const n = kwota.trim() ? parsujKwote(kwota) : 0;
                  ustawKwote(null);
                  if (n !== null) f.zapisz("zmiana_ceny_gr", n);
                }}
              />
              <span aria-hidden>zł</span>
            </span>
          </Pole>
          <PoleTekstu f={f} k="opis_zmian" etykieta="Opis zmian (każda zmiana w osobnej linii)" wiele blokada={ro} />
          <h3>Lista zmian w dokumencie</h3>
          {dane.lista_zmian.length > 0 ? (
            <ol className="lista-zmian">
              {dane.lista_zmian.map((z, i) => (
                <li key={i}>{z}</li>
              ))}
            </ol>
          ) : (
            <p className="grupa-umowy__info">Brak zmian – opisz zmiany, zmień kwotę, termin albo specyfikację obok.</p>
          )}
        </section>
        <div className="ekran-aneksu__edytor">
          {dane.specyfikacja && (
            <EdytorSpecyfikacji
              specyfikacja={dane.specyfikacja}
              onZmiana={poZmianieSpecyfikacji}
              panel={() => (
                <div className="panel-roznic">
                  <h3>Różnice względem umowy</h3>
                  <ListaRoznic roznice={dane.roznice} pusta="Specyfikacja bez zmian." />
                </div>
              )}
            />
          )}
        </div>
      </div>
      {f.konflikt && (
        <OknoKonfliktu
          tytul="Konflikt zmian aneksu"
          konflikty={f.konflikt.konflikty}
          nazwaPola={nazwaPolaUmowy}
          onMoja={() => {
            const k = f.konflikt;
            f.zamknijKonflikt();
            if (!k) return;
            k.ponow(Object.fromEntries(k.konflikty.map((x) => [x.pole, x.aktualna]))).catch((e: Error) => ustawBlad(e.message));
          }}
          onCudza={() => {
            f.zamknijKonflikt();
            f.odrzucSzkic();
            void wczytaj();
          }}
        />
      )}
      {dokument && (
        <OknoDokumentu
          tytul={`Aneks nr ${a.nr} do umowy ${dane.umowa_numer}`}
          podglad={() => api.aneksy.polaDokumentu(a.id)}
          pobierz={(format) => api.aneksy.dokument(a.id, format)}
          onZamknij={() => ustawDokument(false)}
        />
      )}
    </div>
  );
}
