import { useEffect, useState, type FormEvent } from "react";
import { useNavigate } from "react-router-dom";
import { TYPY_KLIENTA, type DaneKlienta, type TypKlienta } from "../../api/typy";
import { usePolaczenie } from "../../kontekst/Polaczenie";

const PUSTY: DaneKlienta = { nazwa: "", typ: "indywidualny", zgoda_marketing: false };

const POLA_TEKSTOWE: [keyof DaneKlienta, string][] = [
  ["nazwa", "Nazwa / imię i nazwisko *"],
  ["nip", "NIP"],
  ["email", "E-mail"],
  ["telefon", "Telefon"],
  ["adres_ulica", "Ulica i numer"],
  ["adres_kod", "Kod pocztowy"],
  ["adres_miasto", "Miejscowość"],
];

export function FormularzKlienta({ id }: { id: number | null }) {
  const { api } = usePolaczenie();
  const nawiguj = useNavigate();
  const [dane, ustawDane] = useState<DaneKlienta>(PUSTY);
  const [blad, ustawBlad] = useState<string | null>(null);

  useEffect(() => {
    if (!api || id === null) return;
    api.klienci.pobierz(id).then(
      (k) => ustawDane(k),
      (e: Error) => ustawBlad(e.message),
    );
  }, [api, id]);

  if (!api) return null;

  const zmien = (pole: keyof DaneKlienta, wartosc: unknown) => ustawDane((d) => ({ ...d, [pole]: wartosc }));

  const zapisz = async (e: FormEvent) => {
    e.preventDefault();
    try {
      if (id === null) await api.klienci.dodaj(dane);
      else await api.klienci.zmien(id, dane);
      nawiguj("/klienci");
    } catch (err) {
      ustawBlad((err as Error).message);
    }
  };

  const usun = async () => {
    if (id === null || !confirm("Usunąć klienta z listy? Oferty i umowy zostaną zachowane.")) return;
    try {
      await api.klienci.usun(id);
      nawiguj("/klienci");
    } catch (err) {
      ustawBlad((err as Error).message);
    }
  };

  return (
    <>
      <h1>{id === null ? "Nowy klient" : "Karta klienta"}</h1>
      {blad && <p className="blad">{blad}</p>}
      <form className="formularz" onSubmit={zapisz}>
        <label htmlFor="typ">Typ</label>
        <select id="typ" value={dane.typ ?? "indywidualny"} onChange={(e) => zmien("typ", e.target.value as TypKlienta)}>
          {Object.entries(TYPY_KLIENTA).map(([k, n]) => (
            <option key={k} value={k}>
              {n}
            </option>
          ))}
        </select>
        {POLA_TEKSTOWE.map(([pole, etykieta]) => (
          <FragmentPola key={pole} pole={pole} etykieta={etykieta} wartosc={(dane[pole] as string | null) ?? ""} zmien={zmien} />
        ))}
        <label htmlFor="uwagi">Uwagi</label>
        <textarea id="uwagi" rows={3} value={dane.uwagi ?? ""} onChange={(e) => zmien("uwagi", e.target.value)} />
        <label htmlFor="zgoda">Zgoda na kontakt marketingowy</label>
        <input id="zgoda" type="checkbox" checked={!!dane.zgoda_marketing} onChange={(e) => zmien("zgoda_marketing", e.target.checked)} />
        <div className="przyciski">
          <button type="submit">Zapisz</button>
          <button type="button" onClick={() => nawiguj("/klienci")}>
            Anuluj
          </button>
          {id !== null && (
            <button type="button" onClick={usun}>
              Usuń
            </button>
          )}
        </div>
      </form>
      {id !== null && (
        <p>
          <em>Oś czasu klienta (oferty, umowy, wpłaty, korespondencja) – kolejne fazy.</em>
        </p>
      )}
    </>
  );
}

function FragmentPola(p: {
  pole: keyof DaneKlienta;
  etykieta: string;
  wartosc: string;
  zmien: (pole: keyof DaneKlienta, w: unknown) => void;
}) {
  return (
    <>
      <label htmlFor={p.pole}>{p.etykieta}</label>
      <input id={p.pole} value={p.wartosc} onChange={(e) => p.zmien(p.pole, e.target.value)} required={p.pole === "nazwa"} />
    </>
  );
}
