import { useEffect, useState } from "react";
import { usePolaczenie } from "../../kontekst/Polaczenie";
import { UstawieniaPowiadomien } from "../kalendarz/UstawieniaPowiadomien";
import {
  autostart,
  powiadomienieTestowe,
  przelaczWidzet,
  wTauri,
  wyszukajSerwery,
  zapiszKonfiguracje,
  type Konfiguracja,
  type Polaczenie,
  type ZnalezionySerwer,
} from "../../lib/tauri";

export function Ustawienia() {
  const { polaczenie } = usePolaczenie();
  if (!polaczenie) return <p>Wczytywanie…</p>;
  return <FormularzUstawien polaczenie={polaczenie} />;
}

function FormularzUstawien({ polaczenie }: { polaczenie: Polaczenie }) {
  const [konf, ustawKonf] = useState<Konfiguracja>(polaczenie.konfiguracja);
  const [znalezione, ustawZnalezione] = useState<ZnalezionySerwer[] | null>(null);
  const [szukam, ustawSzukam] = useState(false);
  const [autostartWl, ustawAutostart] = useState<boolean | null>(null);
  const [blad, ustawBlad] = useState<string | null>(null);

  useEffect(() => {
    autostart().then((a) => ustawAutostart(a ? a.wlaczony : null), () => ustawAutostart(null));
  }, []);

  const zmien = <K extends keyof Konfiguracja>(pole: K, w: Konfiguracja[K]) => ustawKonf({ ...konf, [pole]: w });

  const szukaj = async () => {
    ustawSzukam(true);
    try {
      ustawZnalezione(await wyszukajSerwery());
    } catch (e) {
      ustawBlad(String(e));
    } finally {
      ustawSzukam(false);
    }
  };

  const zapisz = async () => {
    try {
      await zapiszKonfiguracje(konf); // aplikacja uruchomi się ponownie
    } catch (e) {
      ustawBlad(String(e));
    }
  };

  const przelaczAutostart = async (w: boolean) => {
    const a = await autostart();
    if (!a) return;
    await a.ustaw(w);
    ustawAutostart(w);
  };

  return (
    <>
      <h1>Ustawienia</h1>
      {blad && <p className="blad">{blad}</p>}
      {polaczenie.blad_serwera && <p className="blad">Serwer nie wystartował: {polaczenie.blad_serwera}</p>}
      {!wTauri() && <p>Tryb przeglądarkowy: adres i klucz API ustawiasz zmiennymi VITE_API_URL i VITE_API_KLUCZ.</p>}

      <h2>Tryb pracy tego komputera</h2>
      <form className="formularz" onSubmit={(e) => e.preventDefault()}>
        <label htmlFor="tryb">Tryb</label>
        <select id="tryb" value={konf.tryb} onChange={(e) => zmien("tryb", e.target.value as Konfiguracja["tryb"])}>
          <option value="nieskonfigurowany">— wybierz —</option>
          <option value="serwer">Serwer (ten komputer trzyma bazę)</option>
          <option value="klient">Klient (łączy się z serwerem w sieci)</option>
        </select>
        <label htmlFor="nazwa">Nazwa stanowiska</label>
        <input id="nazwa" value={konf.nazwa_stanowiska} onChange={(e) => zmien("nazwa_stanowiska", e.target.value)} />

        {konf.tryb === "serwer" && (
          <>
            <label htmlFor="port">Port</label>
            <input id="port" type="number" value={konf.port} onChange={(e) => zmien("port", Number(e.target.value))} />
            <label>Folder danych</label>
            <span>{polaczenie.folder_danych ?? "domyślny folder danych aplikacji"}</span>
            <label>Klucz dla stanowisk</label>
            <code>{konf.klucz_api ?? "zostanie wygenerowany przy zapisie"}</code>
          </>
        )}

        {konf.tryb === "klient" && (
          <>
            <label htmlFor="adres">Adres serwera</label>
            <input
              id="adres"
              placeholder="np. 192.168.1.10:47800"
              value={konf.adres_serwera ?? ""}
              onChange={(e) => zmien("adres_serwera", e.target.value)}
            />
            <label htmlFor="klucz">Klucz z serwera</label>
            <input id="klucz" value={konf.klucz_api ?? ""} onChange={(e) => zmien("klucz_api", e.target.value)} />
            <span />
            <div>
              <button type="button" onClick={szukaj} disabled={szukam}>
                {szukam ? "Szukam…" : "Znajdź serwer w sieci"}
              </button>
              {znalezione && znalezione.length === 0 && <p>Nie znaleziono serwera – wpisz adres ręcznie.</p>}
              <ul>
                {znalezione?.map((s) => (
                  <li key={s.adres_api}>
                    <button type="button" onClick={() => zmien("adres_serwera", s.adres_api)}>
                      {s.nazwa} ({s.adres_api})
                    </button>
                  </li>
                ))}
              </ul>
            </div>
          </>
        )}

        <div className="przyciski">
          <button type="button" onClick={zapisz} disabled={!wTauri()}>
            Zapisz i uruchom ponownie
          </button>
        </div>
      </form>

      <h2>System</h2>
      <div className="pasek">
        <label>
          <input
            type="checkbox"
            disabled={autostartWl === null}
            checked={!!autostartWl}
            onChange={(e) => przelaczAutostart(e.target.checked)}
          />{" "}
          Uruchamiaj z systemem Windows (bez okna – tylko ikona w zasobniku i widżet na pulpicie)
        </label>
      </div>
      <div className="pasek">
        <button onClick={() => powiadomienieTestowe().catch((e) => ustawBlad(String(e)))}>Powiadomienie testowe</button>
        <button onClick={() => przelaczWidzet()}>Pokaż / ukryj widżet</button>
      </div>
      <UstawieniaPowiadomien />
    </>
  );
}
