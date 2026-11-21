// Płatności umowy: plan (zapisany po wyjściu ze szkicu), saldo i zaległości, rejestr wpłat,
// protokół odbioru (załącznik nr 5) z saldem.

import { useState } from "react";
import { CircleAlert, CircleCheck, ClipboardCheck, FileDown, FileText, Plus, Trash2 } from "lucide-react";
import type { PelnaUmowa, SposobWplaty } from "../../api/typyUmow";
import { usePolaczenie } from "../../kontekst/Polaczenie";
import { bladKwoty, formatujDate, formatujKwote, parsujKwote } from "../../lib/formaty";
import { EtykietaStatusu, Pole, Przycisk } from "../../ui";
import { zapiszPlik } from "../oferty/pobieranie";
import { PodgladPlanu } from "./FormularzUmowy";
import { SPOSOBY_WPLATY } from "./statusy";

const dzisIso = () => new Date().toISOString().slice(0, 10);

export function PanelPlatnosci({ dane, onDane, onBlad }: { dane: PelnaUmowa; onDane: (p: PelnaUmowa) => void; onBlad: (b: string | null) => void }) {
  const { api } = usePolaczenie();
  const u = dane.umowa;
  const [wplata, ustawWplate] = useState({ data: dzisIso(), kwota: "", sposob: "przelew" as SposobWplaty, uwagi: "" });
  const [protokol, ustawProtokol] = useState({ data: dane.protokol?.data ?? dzisIso(), uwagi: dane.protokol?.uwagi ?? "", bez: dane.protokol?.bez_zastrzezen ?? true });
  const [pobieranie, ustawPobieranie] = useState<string | null>(null);
  if (!api) return null;
  const s = dane.saldo;
  const bladWplaty = wplata.kwota ? bladKwoty(wplata.kwota) : null;
  const moznaWplacac = u.status !== "szkic";
  const mozliwyProtokol = ["podpisana", "w_realizacji", "odebrana", "zamknieta"].includes(u.status);

  const dodajWplate = async () => {
    const kwota = parsujKwote(wplata.kwota);
    if (!kwota) return;
    try {
      onDane(await api.umowy.dodajWplate(u.id, { data: wplata.data, kwota_gr: kwota, sposob: wplata.sposob, uwagi: wplata.uwagi.trim() || null }));
      ustawWplate({ ...wplata, kwota: "", uwagi: "" });
      onBlad(null);
    } catch (e) {
      onBlad((e as Error).message);
    }
  };

  const pobierzProtokol = async (format: "docx" | "pdf") => {
    ustawPobieranie(format);
    try {
      const { nazwa, plik } = await api.umowy.protokol(u.id, format);
      zapiszPlik(plik, nazwa);
      onBlad(null);
    } catch (e) {
      onBlad((e as Error).message);
    } finally {
      ustawPobieranie(null);
    }
  };

  return (
    <div className="panel-platnosci">
      <section className="karta grupa-umowy">
        <h3>Saldo</h3>
        <div className="saldo">
          <div>
            <span>Wartość umowy</span>
            <strong>{formatujKwote(s.wartosc_gr)}</strong>
          </div>
          <div>
            <span>Wpłacono</span>
            <strong>{formatujKwote(s.wplacono_gr)}</strong>
          </div>
          <div>
            <span>Pozostało</span>
            <strong>{formatujKwote(s.pozostalo_gr)}</strong>
          </div>
          <div>
            <span>Zaległe</span>
            <strong className={s.zalegle_gr > 0 ? "saldo__zalegle" : undefined}>{formatujKwote(s.zalegle_gr)}</strong>
          </div>
        </div>
        {s.zalegle_gr > 0 ? (
          <p className="komunikat-stanu status--czerwony">
            <CircleAlert size={14} aria-hidden /> Zaległość {formatujKwote(s.zalegle_gr)} – raty po terminie bez pokrycia wpłatami.
          </p>
        ) : s.nastepna ? (
          <p className="grupa-umowy__info">
            Najbliższa płatność: {s.nastepna.opis} – {formatujKwote(s.nastepna.do_zaplaty_gr)} do {formatujDate(s.nastepna.termin)}.
          </p>
        ) : (
          s.wartosc_gr > 0 && <EtykietaStatusu stan="spokojnie" ikona={CircleCheck} tekst="Rozliczone" />
        )}
        <table className="tabela tabela--zwarta">
          <thead>
            <tr>
              <th>Rata</th>
              <th>Termin</th>
              <th className="kolumna-kwoty">Kwota</th>
              <th className="kolumna-kwoty">Do zapłaty</th>
            </tr>
          </thead>
          <tbody>
            {s.pozycje.map((p, i) => (
              <tr key={i} className={p.zalegla ? "wiersz-zalegly" : undefined}>
                <td>{p.opis}</td>
                <td>
                  {formatujDate(p.termin)}
                  {p.zalegla && <span className="grupa-umowy__mikro"> po terminie</span>}
                </td>
                <td className="kolumna-kwoty">{formatujKwote(p.kwota_gr)}</td>
                <td className="kolumna-kwoty">{formatujKwote(p.do_zaplaty_gr)}</td>
              </tr>
            ))}
          </tbody>
        </table>
        {dane.platnosci.length === 0 && <p className="grupa-umowy__mikro">Plan zostanie zapisany (i trafi do kalendarza) przy wysłaniu lub podpisaniu umowy. Podgląd:</p>}
        {dane.platnosci.length === 0 && <PodgladPlanu dane={dane} />}
      </section>

      <section className="karta grupa-umowy">
        <h3>Wpłaty</h3>
        <table className="tabela tabela--zwarta">
          <thead>
            <tr>
              <th>Data</th>
              <th>Forma</th>
              <th>Uwagi</th>
              <th className="kolumna-kwoty">Kwota</th>
              <th aria-label="Akcje" />
            </tr>
          </thead>
          <tbody>
            {dane.wplaty.map((w) => (
              <tr key={w.id}>
                <td>{formatujDate(w.data)}</td>
                <td>{SPOSOBY_WPLATY.find((x) => x.wartosc === w.sposob)?.etykieta ?? "—"}</td>
                <td>{w.uwagi ?? ""}</td>
                <td className="kolumna-kwoty">{formatujKwote(w.kwota_gr)}</td>
                <td>
                  <Przycisk
                    maly
                    wariant="tekstowy"
                    tylkoIkona
                    ikona={Trash2}
                    etykieta={`Usuń wpłatę z ${formatujDate(w.data)}`}
                    onClick={async () => {
                      if (!confirm(`Usunąć wpłatę ${formatujKwote(w.kwota_gr)} z ${formatujDate(w.data)}?`)) return;
                      try {
                        onDane(await api.umowy.usunWplate(w.id));
                      } catch (e) {
                        onBlad((e as Error).message);
                      }
                    }}
                  />
                </td>
              </tr>
            ))}
            {dane.wplaty.length === 0 && (
              <tr>
                <td colSpan={5} className="tabela-wierszy__pusto">
                  Brak wpłat.
                </td>
              </tr>
            )}
          </tbody>
        </table>
        {moznaWplacac ? (
          <div className="nowa-wplata">
            <Pole etykieta="Data" id="w-data">
              <input id="w-data" type="date" value={wplata.data} onChange={(e) => ustawWplate({ ...wplata, data: e.target.value })} />
            </Pole>
            <Pole etykieta="Kwota" id="w-kwota" blad={bladWplaty}>
              <span className="pole-zl">
                <input id="w-kwota" inputMode="decimal" value={wplata.kwota} onChange={(e) => ustawWplate({ ...wplata, kwota: e.target.value })} />
                <span aria-hidden>zł</span>
              </span>
            </Pole>
            <Pole etykieta="Forma" id="w-sposob">
              <select id="w-sposob" value={wplata.sposob} onChange={(e) => ustawWplate({ ...wplata, sposob: e.target.value as SposobWplaty })}>
                {SPOSOBY_WPLATY.map((x) => (
                  <option key={x.wartosc} value={x.wartosc}>
                    {x.etykieta}
                  </option>
                ))}
              </select>
            </Pole>
            <Pole etykieta="Uwagi" id="w-uwagi">
              <input id="w-uwagi" value={wplata.uwagi} onChange={(e) => ustawWplate({ ...wplata, uwagi: e.target.value })} />
            </Pole>
            <Przycisk wariant="glowny" ikona={Plus} disabled={!wplata.kwota || !!bladWplaty || !parsujKwote(wplata.kwota)} onClick={() => void dodajWplate()}>
              Dodaj wpłatę
            </Przycisk>
          </div>
        ) : (
          <p className="grupa-umowy__mikro">Wpłaty rejestruje się po wysłaniu albo podpisaniu umowy.</p>
        )}
      </section>

      <section className="karta grupa-umowy">
        <h3>
          <ClipboardCheck size={16} aria-hidden /> Protokół odbioru (załącznik nr 5)
        </h3>
        {mozliwyProtokol ? (
          <>
            <div className="nowa-wplata">
              <Pole etykieta="Data odbioru" id="p-data">
                <input id="p-data" type="date" value={protokol.data} onChange={(e) => ustawProtokol({ ...protokol, data: e.target.value })} />
              </Pole>
              <Pole etykieta="Uwagi" id="p-uwagi">
                <input id="p-uwagi" value={protokol.uwagi} onChange={(e) => ustawProtokol({ ...protokol, uwagi: e.target.value })} />
              </Pole>
              <label className="nowa-wplata__przelacznik">
                <input type="checkbox" checked={protokol.bez} onChange={(e) => ustawProtokol({ ...protokol, bez: e.target.checked })} /> bez zastrzeżeń
              </label>
              <Przycisk
                wariant="glowny"
                ikona={ClipboardCheck}
                onClick={async () => {
                  try {
                    onDane(await api.umowy.zapiszProtokol(u.id, { data: protokol.data, uwagi: protokol.uwagi.trim() || null, bez_zastrzezen: protokol.bez }));
                    onBlad(null);
                  } catch (e) {
                    onBlad((e as Error).message);
                  }
                }}
              >
                {dane.protokol ? "Zapisz zmiany protokołu" : "Zapisz protokół (umowa → odebrana)"}
              </Przycisk>
            </div>
            {dane.protokol && (
              <div className="grupa-umowy__wiersz">
                <span className="grupa-umowy__info">
                  Protokół z {formatujDate(dane.protokol.data)}: wpłacono {formatujKwote(s.wplacono_gr)}, pozostało {formatujKwote(Math.max(0, s.pozostalo_gr))}
                  {s.nastepna?.termin ? `, termin płatności ${formatujDate(s.nastepna.termin)}` : ""}.
                </span>
                <Przycisk maly ikona={FileText} disabled={pobieranie !== null} onClick={() => void pobierzProtokol("docx")}>
                  {pobieranie === "docx" ? "Tworzenie…" : "DOCX"}
                </Przycisk>
                <Przycisk maly ikona={FileDown} disabled={pobieranie !== null} onClick={() => void pobierzProtokol("pdf")}>
                  {pobieranie === "pdf" ? "Tworzenie…" : "PDF"}
                </Przycisk>
              </div>
            )}
          </>
        ) : (
          <p className="grupa-umowy__mikro">Protokół sporządza się dla umowy podpisanej.</p>
        )}
      </section>
    </div>
  );
}
