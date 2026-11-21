// Ekran umowy: nagłówek ze statusem i akcjami; zakładki Umowa (skrócona forma), Specyfikacja
// (wspólny edytor + różnice względem oferty), Płatności i wpłaty, Aneksy.

import { useCallback, useEffect, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { ArrowLeft, CircleAlert, CircleCheck, FilePlus2, FileText, Lock, Trash2, TriangleAlert } from "lucide-react";
import type { Klient, Specyfikacja } from "../../api/typy";
import type { DaneUmowy, PelnaUmowa, RozniceUmowy, StatusUmowy, WynikHarmonogramu } from "../../api/typyUmow";
import { usePolaczenie } from "../../kontekst/Polaczenie";
import { formatujDate, formatujKwote } from "../../lib/formaty";
import { EtykietaStatusu, Przycisk, Segment } from "../../ui";
import { EdytorSpecyfikacji, OknoKonfliktu } from "../specyfikacja";
import { FormularzUmowy } from "./FormularzUmowy";
import { daneUmowy, nazwaPolaUmowy } from "./logika";
import { PanelPlatnosci } from "./PanelPlatnosci";
import { STATUSY_ANEKSU, STATUSY_UMOWY, dozwolonePrzejscia, etykietaPrzejscia } from "./statusy";
import { useFormularz } from "./useFormularz";
import { ListaRoznic, OknoDokumentu, OknoStatusu } from "./wspolne";

type Zakladka = "umowa" | "specyfikacja" | "platnosci" | "aneksy";

export function EkranUmowy() {
  const { id } = useParams();
  const umowaId = Number(id);
  const { api, ostatnieZdarzenie } = usePolaczenie();
  const nawiguj = useNavigate();
  const [dane, ustawDane] = useState<PelnaUmowa | null>(null);
  const [klienci, ustawKlientow] = useState<Klient[]>([]);
  const [roznice, ustawRoznice] = useState<RozniceUmowy | null>(null);
  const [blad, ustawBlad] = useState<string | null>(null);
  const [zakladka, ustawZakladke] = useState<Zakladka>("umowa");
  const [dokument, ustawDokument] = useState(false);
  const [status, ustawStatus] = useState<StatusUmowy | null>(null);
  const [komunikat, ustawKomunikat] = useState<string | null>(null);

  const wczytaj = useCallback(async () => {
    if (!api) return;
    try {
      ustawDane(await api.umowy.pobierz(umowaId));
      ustawBlad(null);
    } catch (e) {
      ustawBlad((e as Error).message);
    }
  }, [api, umowaId]);

  const zdarzenie = ostatnieZdarzenie?.zasob === "umowy" && ostatnieZdarzenie.id === umowaId ? ostatnieZdarzenie : null;
  useEffect(() => {
    if (!api) return;
    api.umowy.pobierz(umowaId).then(
      (d) => {
        ustawDane(d);
        ustawBlad(null);
      },
      (e: Error) => ustawBlad(e.message),
    );
  }, [api, umowaId, zdarzenie]);
  const wczytajKlientow = useCallback(() => {
    api?.klienci.lista().then(ustawKlientow, () => {});
  }, [api]);
  useEffect(wczytajKlientow, [wczytajKlientow]);
  const specId = dane?.umowa.specyfikacja_id;
  const wersjaSpec = dane?.specyfikacja;
  useEffect(() => {
    if (!api || !specId) return;
    api.umowy.roznice(umowaId).then(ustawRoznice, () => ustawRoznice(null));
  }, [api, umowaId, specId, wersjaSpec]);

  const pusteDane = dane ? daneUmowy(dane.umowa) : ({} as DaneUmowy);
  const f = useFormularz<DaneUmowy>(
    pusteDane,
    async (z) => {
      if (!api) return;
      ustawDane(await api.umowy.zmien(umowaId, z));
    },
    ustawBlad,
  );

  const poZmianieSpecyfikacji = useCallback((s: Specyfikacja) => ustawDane((d) => (d ? { ...d, specyfikacja: s } : d)), []);

  if (!api) return null;
  if (!dane) return <div className="ekran-umowy__ladowanie">{blad ? <p className="blad">{blad}</p> : "Wczytywanie umowy…"}</div>;

  const u = dane.umowa;
  const st = STATUSY_UMOWY[u.status];
  const przejscia = dozwolonePrzejscia(u.status);
  const glowne = przejscia.filter((p) => p !== "anulowana" && p !== "szkic");
  const mozliwyAneks = ["podpisana", "w_realizacji", "odebrana"].includes(u.status);
  const otwartyAneks = dane.aneksy.find((a) => a.status !== "podpisany");

  const zmienStatus = async (na: StatusUmowy, data: string | null, powod: string | null) => {
    ustawStatus(null);
    try {
      const w = await api.umowy.zmienStatus(u.id, na, data, powod);
      ustawDane(w);
      ustawBlad(null);
      ustawKomunikat(opisHarmonogramu(w.harmonogram));
    } catch (e) {
      ustawBlad((e as Error).message);
    }
  };

  const utworzAneks = async () => {
    if (otwartyAneks) {
      nawiguj(`/umowy/aneksy/${otwartyAneks.id}`);
      return;
    }
    try {
      const a = await api.umowy.dodajAneks(u.id);
      nawiguj(`/umowy/aneksy/${a.aneks.id}`);
    } catch (e) {
      ustawBlad((e as Error).message);
    }
  };

  const liczbaRoznic = roznice?.roznice.length ?? 0;

  return (
    <div className="ekran-umowy">
      <header className="ekran-oferty__naglowek">
        <div className="ekran-oferty__tytul">
          <Przycisk wariant="tekstowy" tylkoIkona ikona={ArrowLeft} etykieta="Wróć do listy umów" onClick={() => nawiguj("/umowy")} />
          <div>
            <div className="ekran-oferty__numer">
              <h1>Umowa {u.numer}</h1>
              <EtykietaStatusu stan={st.stan} ikona={st.ikona} tekst={st.etykieta} />
            </div>
            <div className="ekran-oferty__meta">
              <span>{u.klient_nazwa}</span>
              <span>· {formatujDate(u.data_zawarcia)}</span>
              <span>· {formatujKwote(u.wartosc_brutto_gr)}</span>
              {u.montaz_do && <span>· montaż do {formatujDate(u.montaz_do)}</span>}
            </div>
          </div>
        </div>
        <div className="ekran-oferty__narzedzia">
          {glowne.map((p) => (
            <Przycisk key={p} wariant={p === "podpisana" ? "glowny" : "drugorzedny"} ikona={STATUSY_UMOWY[p].ikona} onClick={() => ustawStatus(p)}>
              {etykietaPrzejscia(u.status, p)}
            </Przycisk>
          ))}
          {przejscia.includes("szkic") && (
            <Przycisk wariant="tekstowy" onClick={() => ustawStatus("szkic")}>
              Cofnij do szkicu
            </Przycisk>
          )}
          <span className="ekran-oferty__separator" aria-hidden />
          <Przycisk ikona={FileText} onClick={() => ustawDokument(true)}>
            Dokument
          </Przycisk>
          {mozliwyAneks && (
            <Przycisk ikona={FilePlus2} onClick={() => void utworzAneks()}>
              {otwartyAneks ? `Aneks nr ${otwartyAneks.nr}` : "Utwórz aneks"}
            </Przycisk>
          )}
          {u.status === "szkic" && (
            <Przycisk
              wariant="tekstowy"
              tylkoIkona
              ikona={Trash2}
              etykieta="Usuń szkic umowy"
              onClick={async () => {
                if (!confirm(`Usunąć szkic umowy ${u.numer}?`)) return;
                try {
                  await api.umowy.usun(u.id);
                  nawiguj("/umowy");
                } catch (e) {
                  ustawBlad((e as Error).message);
                }
              }}
            />
          )}
          {przejscia.includes("anulowana") && (
            <Przycisk wariant="tekstowy" onClick={() => ustawStatus("anulowana")}>
              Anuluj umowę
            </Przycisk>
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
            <Przycisk maly wariant="tekstowy" onClick={() => ustawKomunikat(null)}>
              OK
            </Przycisk>
          </div>
        )}
        {u.powod_blokady && (
          <div className="komunikat-stanu status--neutralny">
            <Lock size={14} aria-hidden /> {u.powod_blokady}. Czasy produkcji i uwagi wewnętrzne można zmieniać.
          </div>
        )}
        {u.status === "szkic" && dane.braki.length > 0 && (
          <div className="komunikat-stanu status--zolty">
            <TriangleAlert size={14} aria-hidden /> Do wysłania/podpisania uzupełnij: {dane.braki.join("; ")}
          </div>
        )}
      </div>
      <div className="ekran-umowy__zakladki">
        <Segment<Zakladka>
          etykieta="Widok umowy"
          wartosc={zakladka}
          onZmiana={ustawZakladke}
          opcje={[
            { wartosc: "umowa", etykieta: "Umowa" },
            { wartosc: "specyfikacja", etykieta: `Specyfikacja${liczbaRoznic ? ` · ${liczbaRoznic} różnic` : ""}` },
            { wartosc: "platnosci", etykieta: "Płatności i wpłaty" },
            { wartosc: "aneksy", etykieta: `Aneksy (${dane.aneksy.length})` },
          ]}
        />
      </div>
      <div className={`ekran-umowy__tresc${zakladka === "specyfikacja" ? " ekran-umowy__tresc--edytor" : ""}`}>
        {zakladka === "umowa" && <FormularzUmowy dane={dane} f={f} klienci={klienci} onKlienci={wczytajKlientow} onDane={ustawDane} onBlad={ustawBlad} />}
        {zakladka === "specyfikacja" && (
          <EdytorSpecyfikacji
            specyfikacja={dane.specyfikacja}
            onZmiana={poZmianieSpecyfikacji}
            akcjaBlokady={
              mozliwyAneks ? (
                <Przycisk maly wariant="glowny" ikona={FilePlus2} onClick={() => void utworzAneks()}>
                  {otwartyAneks ? `Otwórz aneks nr ${otwartyAneks.nr}` : "Utwórz aneks"}
                </Przycisk>
              ) : undefined
            }
            panel={() => (
              <div className="panel-roznic">
                <h3>Różnice względem oferty</h3>
                {u.oferta_numer ? (
                  <ListaRoznic roznice={roznice?.roznice ?? []} zrodlo={roznice?.zrodlo} pusta="Specyfikacja zgodna z ofertą źródłową." />
                ) : (
                  <p className="grupa-umowy__info">Umowa utworzona od zera – bez oferty źródłowej.</p>
                )}
              </div>
            )}
          />
        )}
        {zakladka === "platnosci" && <PanelPlatnosci dane={dane} onDane={ustawDane} onBlad={ustawBlad} />}
        {zakladka === "aneksy" && (
          <section className="karta grupa-umowy lista-aneksow">
            <h3>Aneksy</h3>
            <table className="tabela">
              <thead>
                <tr>
                  <th>Nr</th>
                  <th>Data</th>
                  <th>Opis zmian</th>
                  <th className="kolumna-kwoty">Zmiana kwoty</th>
                  <th>Nowy termin</th>
                  <th>Status</th>
                </tr>
              </thead>
              <tbody>
                {dane.aneksy.map((a) => {
                  const s = STATUSY_ANEKSU[a.status];
                  return (
                    <tr key={a.id} className="wiersz-klikalny" onClick={() => nawiguj(`/umowy/aneksy/${a.id}`)}>
                      <td>
                        <strong>{a.nr}</strong>
                      </td>
                      <td>{formatujDate(a.data)}</td>
                      <td>{a.opis_zmian || "—"}</td>
                      <td className="kolumna-kwoty">{a.zmiana_ceny_gr ? formatujKwote(a.zmiana_ceny_gr) : "—"}</td>
                      <td>{formatujDate(a.nowy_termin_montazu)}</td>
                      <td>
                        <EtykietaStatusu stan={s.stan} ikona={s.ikona} tekst={s.etykieta} />
                      </td>
                    </tr>
                  );
                })}
                {dane.aneksy.length === 0 && (
                  <tr>
                    <td colSpan={6} className="tabela-wierszy__pusto">
                      Brak aneksów. {mozliwyAneks ? "Aneks zmienia specyfikację, kwotę lub termin podpisanej umowy." : "Aneks można sporządzić do podpisanej umowy."}
                    </td>
                  </tr>
                )}
              </tbody>
            </table>
            {mozliwyAneks && (
              <Przycisk wariant="glowny" ikona={FilePlus2} onClick={() => void utworzAneks()}>
                {otwartyAneks ? `Otwórz aneks nr ${otwartyAneks.nr}` : "Utwórz aneks"}
              </Przycisk>
            )}
          </section>
        )}
      </div>
      {f.konflikt && (
        <OknoKonfliktu
          tytul="Konflikt zmian umowy"
          konflikty={f.konflikt.konflikty}
          nazwaPola={nazwaPolaUmowy}
          onMoja={() => {
            const k = f.konflikt;
            f.zamknijKonflikt();
            if (!k) return;
            const akt = Object.fromEntries(k.konflikty.map((x) => [x.pole, x.aktualna]));
            k.ponow(akt).catch((e: Error) => ustawBlad(e.message));
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
          tytul={`Dokument umowy ${u.numer}`}
          podglad={() => api.umowy.polaDokumentu(u.id)}
          pobierz={(format) => api.umowy.dokument(u.id, format)}
          onZamknij={() => ustawDokument(false)}
        />
      )}
      {status && (
        <OknoStatusu
          z={u.status}
          na={status}
          dataDomyslna={status === "podpisana" ? u.data_zawarcia : null}
          onZamknij={() => ustawStatus(null)}
          onZatwierdz={(d, p) => void zmienStatus(status, d, p)}
        />
      )}
    </div>
  );
}

export function opisHarmonogramu(h: WynikHarmonogramu | null): string | null {
  if (!h) return null;
  const materialy = h.materialy_dodane ? `, dodano ${h.materialy_dodane} zamówień materiałów` : "";
  return h.wygenerowano
    ? `Zlecenie zapisane w kalendarzu${materialy}; harmonogram wygenerowany (nowych zadań: ${h.dodane}, zmienionych: ${h.zmienione}).`
    : `Zlecenie zapisane${materialy}, ale harmonogramu nie udało się policzyć: ${h.powod ?? "brak danych"}.`;
}
