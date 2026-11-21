// Skrócona forma umowy: grupy Umowa, Klient, Miejsce montażu, Terminy, Pieniądze, VAT,
// Załączniki, Produkcja (wewnętrznie). Każde pole zapisuje się przy opuszczeniu.

import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import {
  Banknote,
  CalendarClock,
  CircleAlert,
  CircleCheck,
  ExternalLink,
  Factory,
  FileText,
  MapPin,
  Paperclip,
  Plus,
  Receipt,
  Trash2,
  TriangleAlert,
  UserPlus,
  UserRound,
} from "lucide-react";
import type { Klient } from "../../api/typy";
import type { DaneUmowy, Etap, OsobaKontaktowa, PelnaUmowa } from "../../api/typyUmow";
import { usePolaczenie } from "../../kontekst/Polaczenie";
import { formatujDate, formatujKwote, parsujKwote } from "../../lib/formaty";
import { EtykietaStatusu, Okno, Pole, Przycisk, Segment } from "../../ui";
import { ostrzezeniePowierzchni, parsujCalkowita, parsujDziesietna, poleZablokowane } from "./logika";
import { PoleKwoty, PoleLiczby, PoleTekstu } from "./pola";
import { RODZAJE_NIERUCHOMOSCI, ZALACZNIKI } from "./statusy";
import type { useFormularz } from "./useFormularz";

type F = ReturnType<typeof useFormularz<DaneUmowy>>;

function Grupa({ tytul, ikona: Ikona, children, className }: { tytul: string; ikona: typeof FileText; children: React.ReactNode; className?: string }) {
  return (
    <section className={`karta grupa-umowy${className ? ` ${className}` : ""}`} aria-label={tytul}>
      <h3>
        <Ikona size={16} aria-hidden /> {tytul}
      </h3>
      <div className="grupa-umowy__pola">{children}</div>
    </section>
  );
}

export function FormularzUmowy({
  dane,
  f,
  klienci,
  onKlienci,
  onDane,
  onBlad,
}: {
  dane: PelnaUmowa;
  f: F;
  klienci: Klient[];
  onKlienci: () => void;
  onDane: (p: PelnaUmowa) => void;
  onBlad: (b: string | null) => void;
}) {
  const u = dane.umowa;
  const ro = (p: keyof DaneUmowy) => poleZablokowane(u, p);
  const klient = klienci.find((k) => k.id === f.wartosc("klient_id"));
  const [nowyKlient, ustawNowyKlient] = useState(false);
  const wartosc = f.wartosc("wartosc_brutto_gr");
  const rodzaj = f.wartosc("vat_rodzaj_nieruchomosci");
  const pow = f.wartosc("vat_powierzchnia_m2");
  const ostrzezeniePow = ostrzezeniePowierzchni(rodzaj, pow);
  const zalaczniki = f.wartosc("zalaczniki") ?? [];

  return (
    <div className="formularz-umowy">
      <Grupa tytul="Umowa" ikona={FileText}>
        <PoleTekstu f={f} k="numer" etykieta="Numer umowy" blokada={ro("numer")} pomoc="Nadany automatycznie; można zmienić (musi być unikalny)" />
        <div className="grupa-umowy__dwie">
          <PoleTekstu f={f} k="miejscowosc" etykieta="Miejscowość" blokada={ro("miejscowosc")} />
          <PoleTekstu f={f} k="data_zawarcia" typ="date" etykieta="Data umowy" blokada={ro("data_zawarcia")} />
        </div>
        {u.oferta_numer && (
          <p className="grupa-umowy__info">
            Z oferty{" "}
            <Link to={`/oferty/${u.oferta_id}`}>
              {u.oferta_numer} v{u.nr_wersji_oferty}
            </Link>
          </p>
        )}
      </Grupa>

      <Grupa tytul="Klient" ikona={UserRound}>
        <Pole etykieta="Zamawiający (z bazy klientów)" id="u-klient">
          <div className="grupa-umowy__wiersz">
            <select id="u-klient" value={f.wartosc("klient_id") ?? ""} disabled={ro("klient_id")} onChange={(e) => f.zapisz("klient_id", Number(e.target.value))}>
              {!klient && <option value={f.wartosc("klient_id") ?? ""}>{u.klient_nazwa ?? "—"}</option>}
              {klienci.map((k) => (
                <option key={k.id} value={k.id}>
                  {k.nazwa}
                </option>
              ))}
            </select>
            <Przycisk tylkoIkona ikona={UserPlus} etykieta="Nowy klient" disabled={ro("klient_id")} onClick={() => ustawNowyKlient(true)} />
          </div>
        </Pole>
        {klient && (
          <dl className="grupa-umowy__dane">
            <dt>Adres</dt>
            <dd>{[klient.adres_ulica, [klient.adres_kod, klient.adres_miasto].filter(Boolean).join(" ")].filter(Boolean).join(", ") || "—"}</dd>
            <dt>E-mail</dt>
            <dd>{klient.email ?? "—"}</dd>
            <dt>Telefon</dt>
            <dd>{klient.telefon ?? "—"}</dd>
            {klient.nip && (
              <>
                <dt>NIP</dt>
                <dd>{klient.nip}</dd>
              </>
            )}
          </dl>
        )}
        {klient && (
          <Link className="grupa-umowy__link" to={`/klienci/${klient.id}`}>
            <ExternalLink size={12} aria-hidden /> Edytuj dane w bazie klientów
          </Link>
        )}
        {klient && <OsobyKontaktowe klientId={klient.id} onBlad={onBlad} />}
        {u.migawka !== null && <p className="grupa-umowy__info">Dane stron w dokumencie są zamrożone z chwili wysłania/podpisania umowy.</p>}
      </Grupa>

      <Grupa tytul="Miejsce montażu" ikona={MapPin}>
        <PoleTekstu f={f} k="montaz_ulica" etykieta="Adres (ulica, nr)" blokada={ro("montaz_ulica")} />
        <div className="grupa-umowy__dwie grupa-umowy__dwie--kod">
          <PoleTekstu f={f} k="montaz_kod" etykieta="Kod" blokada={ro("montaz_kod")} />
          <PoleTekstu f={f} k="montaz_miasto" etykieta="Miejscowość" blokada={ro("montaz_miasto")} />
        </div>
        <div className="grupa-umowy__dwie">
          <PoleTekstu f={f} k="montaz_pietro" etykieta="Piętro" blokada={ro("montaz_pietro")} />
          <PoleTekstu f={f} k="montaz_kod_domofonu" etykieta="Kod domofonu" blokada={ro("montaz_kod_domofonu")} />
        </div>
        <PoleTekstu f={f} k="montaz_parking" etykieta="Parking" blokada={ro("montaz_parking")} />
        <PoleTekstu f={f} k="montaz_winda_wniesienie" etykieta="Winda / możliwość wniesienia" wiele blokada={ro("montaz_winda_wniesienie")} />
      </Grupa>

      <Grupa tytul="Terminy" ikona={CalendarClock}>
        <PoleTekstu f={f} k="montaz_do" typ="date" etykieta="Rozpoczęcie montażu „do”" blokada={ro("montaz_do")} pomoc="Planowany termin rozpoczęcia montażu – z niego liczy się kalendarz" />
        <PoleLiczby f={f} k="dni_na_brakujace_informacje" etykieta="Dni na brakujące informacje" jednostka="dni kal." parsuj={(t) => parsujCalkowita(t)} blokada={ro("dni_na_brakujace_informacje")} pomoc="Od podpisania umowy; termin pozycji „do potwierdzenia”" />
        <PoleTekstu f={f} k="termin_zadatku" typ="date" etykieta="Termin zadatku (opcjonalnie)" blokada={ro("termin_zadatku")} pomoc="Puste = wg ustawień (np. 3 dni kalendarzowe od zawarcia)" />
      </Grupa>

      <Grupa tytul="Pieniądze" ikona={Banknote} className="grupa-umowy--szeroka">
        <div className="grupa-umowy__dwie">
          <PoleKwoty f={f} k="wartosc_brutto_gr" etykieta="Wartość brutto" blokada={ro("wartosc_brutto_gr")} />
          <PoleKwoty
            f={f}
            k="zadatek_gr"
            etykieta="Zadatek"
            blokada={ro("zadatek_gr")}
            pomoc={wartosc && f.wartosc("zadatek_gr") ? `${Math.round(((f.wartosc("zadatek_gr") ?? 0) / wartosc) * 100)}% wartości` : undefined}
          />
        </div>
        <div className="pole">
          <span className="pole__etykieta">Wariant płatności</span>
          <Segment<"A" | "B">
            etykieta="Wariant płatności"
            wartosc={f.wartosc("wariant_platnosci") ?? "A"}
            onZmiana={(w) => !ro("wariant_platnosci") && f.zapisz("wariant_platnosci", w)}
            opcje={[
              { wartosc: "A", etykieta: "A – reszta przed montażem" },
              { wartosc: "B", etykieta: "B – część po odbiorze" },
            ]}
          />
        </div>
        {f.wartosc("wariant_platnosci") === "B" && (
          <PoleKwoty f={f} k="kwota_przed_montazem_gr" etykieta="Kwota przed montażem" blokada={ro("kwota_przed_montazem_gr")} />
        )}
        <PodgladPlanu dane={dane} />
      </Grupa>

      <Grupa tytul="VAT – oświadczenie (§9)" ikona={Receipt}>
        <div className="pole">
          <span className="pole__etykieta">Stawka VAT</span>
          <Segment<8 | 23>
            etykieta="Stawka VAT"
            wartosc={f.wartosc("stawka_vat") ?? 23}
            onZmiana={(s) => !ro("stawka_vat") && f.zapisz("stawka_vat", s)}
            opcje={[
              { wartosc: 8, etykieta: "8%" },
              { wartosc: 23, etykieta: "23%" },
            ]}
          />
        </div>
        <Pole etykieta="Rodzaj nieruchomości" id="u-rodzaj">
          <select id="u-rodzaj" value={rodzaj ?? ""} disabled={ro("vat_rodzaj_nieruchomosci")} onChange={(e) => f.zapisz("vat_rodzaj_nieruchomosci", (e.target.value || null) as DaneUmowy["vat_rodzaj_nieruchomosci"])}>
            <option value="">— nie wybrano —</option>
            {RODZAJE_NIERUCHOMOSCI.map((r) => (
              <option key={r.wartosc} value={r.wartosc}>
                {r.etykieta}
                {r.limit ? ` (limit ${r.limit} m²)` : ""}
              </option>
            ))}
          </select>
        </Pole>
        <PoleLiczby f={f} k="vat_powierzchnia_m2" etykieta="Powierzchnia użytkowa" jednostka="m²" parsuj={(t) => parsujDziesietna(t)} blokada={ro("vat_powierzchnia_m2")} />
        <PoleTekstu f={f} k="vat_podstawa_powierzchni" etykieta="Podstawa określenia powierzchni" pomoc="Np. akt notarialny, projekt budowlany" blokada={ro("vat_podstawa_powierzchni")} />
        {ostrzezeniePow && (
          <p className="komunikat-stanu status--pomaranczowy">
            <TriangleAlert size={14} aria-hidden /> {ostrzezeniePow}
          </p>
        )}
        {dane.ostrzezenia
          .filter((o) => o.includes("8%") && !o.includes("limit"))
          .map((o) => (
            <p key={o} className="komunikat-stanu status--zolty">
              <CircleAlert size={14} aria-hidden /> {o}
            </p>
          ))}
      </Grupa>

      <Grupa tytul="Załączniki" ikona={Paperclip}>
        <ul className="lista-zalacznikow">
          {ZALACZNIKI.map((z) => {
            const zaznaczony = z.staly || zalaczniki.includes(z.nr);
            return (
              <li key={z.nr}>
                <label>
                  <input
                    type="checkbox"
                    checked={zaznaczony}
                    disabled={z.staly || ro("zalaczniki")}
                    onChange={() => f.zapisz("zalaczniki", zaznaczony ? zalaczniki.filter((x) => x !== z.nr) : [...zalaczniki, z.nr])}
                  />
                  Nr {z.nr} – {z.nazwa}
                  {z.staly && <span className="grupa-umowy__mikro"> (zawsze)</span>}
                </label>
              </li>
            );
          })}
        </ul>
        {zalaczniki.includes("2") && <EdytorEtapow dane={dane} onDane={onDane} onBlad={onBlad} />}
      </Grupa>

      <Grupa tytul="Produkcja (wewnętrznie)" ikona={Factory}>
        <div className="grupa-umowy__dwie">
          <PoleLiczby f={f} k="czas_warsztat_dni" etykieta="Czas na warsztacie" jednostka="dni" parsuj={(t) => parsujCalkowita(t)} />
          <PoleLiczby f={f} k="czas_montazu_dni" etykieta="Czas montażu" jednostka="dni" parsuj={(t) => parsujCalkowita(t, 1)} />
        </div>
        <PoleTekstu f={f} k="uwagi" etykieta="Uwagi wewnętrzne" wiele pomoc="Nie trafiają do dokumentu" />
        {dane.zlecenie ? (
          <p className="grupa-umowy__info">
            Zlecenie w kalendarzu: montaż od {formatujDate(dane.zlecenie.montaz_od)}, zadań: {dane.zlecenie.zadania}, materiałów: {dane.zlecenie.materialy}.{" "}
            <Link to="/kalendarz">Kalendarz</Link>
            {dane.zlecenie.harmonogram_nieaktualny_powod && (
              <span className="komunikat-stanu status--pomaranczowy">
                <TriangleAlert size={14} aria-hidden /> Harmonogram nieaktualny: {dane.zlecenie.harmonogram_nieaktualny_powod}
              </span>
            )}
          </p>
        ) : (
          <p className="grupa-umowy__info">Zlecenie i harmonogram powstaną przy oznaczeniu umowy jako podpisanej.</p>
        )}
      </Grupa>

      {nowyKlient && (
        <OknoNowegoKlienta
          onZamknij={() => ustawNowyKlient(false)}
          onDodany={(k) => {
            ustawNowyKlient(false);
            onKlienci();
            f.zapisz("klient_id", k.id);
          }}
        />
      )}
    </div>
  );
}

/** Podgląd planu płatności (wylicza serwer z bieżących danych) z kontrolą sumy. */
export function PodgladPlanu({ dane }: { dane: PelnaUmowa }) {
  const plan = dane.plan;
  const wartosc = dane.umowa.wartosc_brutto_gr ?? 0;
  return (
    <div className="plan-platnosci">
      <table className="tabela tabela--zwarta">
        <thead>
          <tr>
            <th>Płatność</th>
            <th>Termin</th>
            <th className="kolumna-kwoty">Kwota</th>
          </tr>
        </thead>
        <tbody>
          {plan.pozycje.map((p) => (
            <tr key={p.rodzaj}>
              <td>{p.opis}</td>
              <td title={p.termin_opis}>
                {p.termin ? formatujDate(p.termin) : "—"}
                {p.szacunkowy && <span className="grupa-umowy__mikro"> (szac.)</span>}
                <div className="grupa-umowy__mikro">{p.termin_opis}</div>
              </td>
              <td className="kolumna-kwoty">{formatujKwote(p.kwota_gr)}</td>
            </tr>
          ))}
          {plan.pozycje.length === 0 && (
            <tr>
              <td colSpan={3} className="tabela-wierszy__pusto">
                Uzupełnij wartość i wariant, aby zobaczyć raty.
              </td>
            </tr>
          )}
        </tbody>
        <tfoot>
          <tr>
            <th colSpan={2}>Suma / wartość umowy</th>
            <th className="kolumna-kwoty">
              {formatujKwote(plan.suma_gr)} / {formatujKwote(wartosc)}
            </th>
          </tr>
        </tfoot>
      </table>
      {plan.zgodny ? (
        <EtykietaStatusu stan="spokojnie" ikona={CircleCheck} tekst="Suma płatności = wartość umowy" />
      ) : (
        plan.bledy.map((b) => (
          <p key={b} className="komunikat-stanu status--czerwony">
            <CircleAlert size={14} aria-hidden /> {b}
          </p>
        ))
      )}
      {plan.ostrzezenia.map((o) => (
        <p key={o} className="grupa-umowy__mikro">
          {o}
        </p>
      ))}
    </div>
  );
}

function OsobyKontaktowe({ klientId, onBlad }: { klientId: number; onBlad: (b: string | null) => void }) {
  const { api } = usePolaczenie();
  const [osoby, ustawOsoby] = useState<OsobaKontaktowa[]>([]);
  const [nowa, ustawNowa] = useState({ imie_nazwisko: "", telefon: "", email: "" });
  useEffect(() => {
    api?.umowy.osoby(klientId).then(ustawOsoby, () => ustawOsoby([]));
  }, [api, klientId]);
  if (!api) return null;
  const zapisz = async (lista: OsobaKontaktowa[]) => {
    try {
      ustawOsoby(await api.umowy.zapiszOsoby(klientId, lista));
      onBlad(null);
    } catch (e) {
      onBlad((e as Error).message);
    }
  };
  return (
    <div className="osoby-kontaktowe">
      <span className="pole__etykieta">Osoby kontaktowe</span>
      {osoby.length > 0 && (
        <ul>
          {osoby.map((o, i) => (
            <li key={o.id ?? i}>
              <span>
                {o.imie_nazwisko}
                {o.telefon ? ` · ${o.telefon}` : ""}
                {o.email ? ` · ${o.email}` : ""}
              </span>
              <Przycisk maly wariant="tekstowy" tylkoIkona ikona={Trash2} etykieta={`Usuń: ${o.imie_nazwisko}`} onClick={() => void zapisz(osoby.filter((_, j) => j !== i))} />
            </li>
          ))}
        </ul>
      )}
      <div className="osoby-kontaktowe__nowa">
        <input className="wejscie" placeholder="Imię i nazwisko" aria-label="Imię i nazwisko osoby kontaktowej" value={nowa.imie_nazwisko} onChange={(e) => ustawNowa({ ...nowa, imie_nazwisko: e.target.value })} />
        <input className="wejscie" placeholder="Telefon" aria-label="Telefon osoby kontaktowej" value={nowa.telefon} onChange={(e) => ustawNowa({ ...nowa, telefon: e.target.value })} />
        <Przycisk
          maly
          ikona={Plus}
          disabled={!nowa.imie_nazwisko.trim()}
          onClick={() => {
            void zapisz([...osoby, { imie_nazwisko: nowa.imie_nazwisko.trim(), telefon: nowa.telefon.trim() || null, email: nowa.email.trim() || null, rola: null }]);
            ustawNowa({ imie_nazwisko: "", telefon: "", email: "" });
          }}
        >
          Dodaj
        </Przycisk>
      </div>
    </div>
  );
}

export function OknoNowegoKlienta({ onZamknij, onDodany }: { onZamknij: () => void; onDodany: (k: Klient) => void }) {
  const { api } = usePolaczenie();
  const [d, ustawD] = useState({ nazwa: "", adres_ulica: "", adres_kod: "", adres_miasto: "", email: "", telefon: "", nip: "" });
  const [blad, ustawBlad] = useState<string | null>(null);
  const pole = (k: keyof typeof d) => ({ id: `nk-${k}`, value: d[k], onChange: (e: { target: { value: string } }) => ustawD({ ...d, [k]: e.target.value }) });
  const dodaj = async () => {
    if (!api) return;
    try {
      const n = (v: string) => (v.trim() ? v.trim() : null);
      const k = await api.klienci.dodaj({
        nazwa: d.nazwa.trim(),
        adres_ulica: n(d.adres_ulica),
        adres_kod: n(d.adres_kod),
        adres_miasto: n(d.adres_miasto),
        email: n(d.email),
        telefon: n(d.telefon),
        nip: n(d.nip),
        typ: d.nip.trim() ? "firma" : "indywidualny",
      } as Parameters<typeof api.klienci.dodaj>[0]);
      onDodany(k);
    } catch (e) {
      ustawBlad((e as Error).message);
    }
  };
  return (
    <Okno
      tytul="Nowy klient"
      onZamknij={onZamknij}
      stopka={
        <>
          <Przycisk onClick={onZamknij}>Anuluj</Przycisk>
          <Przycisk wariant="glowny" disabled={!d.nazwa.trim()} onClick={() => void dodaj()}>
            Dodaj klienta
          </Przycisk>
        </>
      }
    >
      <Pole etykieta="Imię i nazwisko albo nazwa firmy" id="nk-nazwa">
        <input {...pole("nazwa")} />
      </Pole>
      <Pole etykieta="Adres (ulica, nr)" id="nk-adres_ulica">
        <input {...pole("adres_ulica")} />
      </Pole>
      <div className="grupa-umowy__dwie grupa-umowy__dwie--kod">
        <Pole etykieta="Kod" id="nk-adres_kod">
          <input {...pole("adres_kod")} />
        </Pole>
        <Pole etykieta="Miejscowość" id="nk-adres_miasto">
          <input {...pole("adres_miasto")} />
        </Pole>
      </div>
      <div className="grupa-umowy__dwie">
        <Pole etykieta="E-mail" id="nk-email">
          <input type="email" {...pole("email")} />
        </Pole>
        <Pole etykieta="Telefon" id="nk-telefon">
          <input {...pole("telefon")} />
        </Pole>
      </div>
      <Pole etykieta="NIP (firma)" id="nk-nip">
        <input {...pole("nip")} />
      </Pole>
      {blad && <p className="blad">{blad}</p>}
    </Okno>
  );
}

/** Harmonogram etapowy (załącznik nr 2): etapy z terminem i kwotą, kontrola sumy. */
function EdytorEtapow({ dane, onDane, onBlad }: { dane: PelnaUmowa; onDane: (p: PelnaUmowa) => void; onBlad: (b: string | null) => void }) {
  const { api } = usePolaczenie();
  const ro = dane.umowa.tylko_do_odczytu;
  const [etapy, ustawEtapy] = useState<(Etap & { kwota: string })[]>(() => dane.etapy.map((e) => ({ ...e, kwota: (e.kwota_gr / 100).toFixed(2).replace(".", ",") })));
  const [zmienione, ustawZmienione] = useState(false);
  const suma = etapy.reduce((s, e) => s + (parsujKwote(e.kwota) ?? 0), 0);
  const wartosc = dane.umowa.wartosc_brutto_gr ?? 0;
  const zmien = (i: number, z: Partial<Etap & { kwota: string }>) => {
    ustawZmienione(true);
    ustawEtapy((l) => l.map((e, j) => (j === i ? { ...e, ...z } : e)));
  };
  const zapisz = async () => {
    if (!api) return;
    try {
      onDane(await api.umowy.zapiszEtapy(dane.umowa.id, etapy.map((e) => ({ nazwa: e.nazwa, termin: e.termin || null, kwota_gr: parsujKwote(e.kwota) ?? 0 }))));
      ustawZmienione(false);
      onBlad(null);
    } catch (e) {
      onBlad((e as Error).message);
    }
  };
  return (
    <div className="etapy">
      <span className="pole__etykieta">Harmonogram etapów (załącznik nr 2)</span>
      <table className="tabela tabela--zwarta">
        <thead>
          <tr>
            <th>Lp.</th>
            <th>Nazwa pracy</th>
            <th>Termin</th>
            <th className="kolumna-kwoty">Kwota</th>
            <th aria-label="Akcje" />
          </tr>
        </thead>
        <tbody>
          {etapy.map((e, i) => (
            <tr key={i}>
              <td>{i + 1}.</td>
              <td>
                <input className="wejscie" aria-label={`Nazwa etapu ${i + 1}`} value={e.nazwa} disabled={ro} onChange={(x) => zmien(i, { nazwa: x.target.value })} />
              </td>
              <td>
                <input className="wejscie" type="date" aria-label={`Termin etapu ${i + 1}`} value={e.termin ?? ""} disabled={ro} onChange={(x) => zmien(i, { termin: x.target.value || null })} />
              </td>
              <td>
                <input className="wejscie etapy__kwota" aria-label={`Kwota etapu ${i + 1}`} value={e.kwota} disabled={ro} onChange={(x) => zmien(i, { kwota: x.target.value })} />
              </td>
              <td>
                <Przycisk maly wariant="tekstowy" tylkoIkona ikona={Trash2} etykieta={`Usuń etap ${i + 1}`} disabled={ro} onClick={() => (ustawZmienione(true), ustawEtapy((l) => l.filter((_, j) => j !== i)))} />
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      <div className="etapy__stopka">
        <Przycisk maly ikona={Plus} disabled={ro} onClick={() => (ustawZmienione(true), ustawEtapy((l) => [...l, { nazwa: "", termin: null, kwota_gr: 0, kwota: "" }]))}>
          Etap
        </Przycisk>
        <span className={suma === wartosc ? "etapy__suma etapy__suma--ok" : "etapy__suma"}>
          {suma === wartosc ? <CircleCheck size={14} aria-hidden /> : <TriangleAlert size={14} aria-hidden />}
          Suma {formatujKwote(suma)} {suma === wartosc ? "= wartość umowy" : `(różnica ${formatujKwote(wartosc - suma)})`}
        </span>
        <Przycisk maly wariant="glowny" disabled={ro || !zmienione} onClick={() => void zapisz()}>
          Zapisz etapy
        </Przycisk>
      </div>
    </div>
  );
}
