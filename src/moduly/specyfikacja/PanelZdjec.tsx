// Zdjęcia przypięte do całej specyfikacji (oferty, umowy, aneksu), wybranej sekcji albo części.
// Wspólny dla ofert i umów – rozmawia tylko z API specyfikacji.

import { useRef, useState } from "react";
import { ImagePlus, Trash2 } from "lucide-react";
import type { KlientApi } from "../../api/klient";
import type { Specyfikacja, Zdjecie } from "../../api/typy";
import { usePolaczenie } from "../../kontekst/Polaczenie";
import { Przycisk, Segment } from "../../ui";
import type { KontekstWyboru } from "./EdytorSpecyfikacji";
import { useObrazek } from "./useObrazek";

type Cel = "oferta" | "sekcja" | "czesc";

export function PanelZdjec({
  specyfikacja: s,
  kontekst,
  onSpecyfikacja,
  onBlad,
}: {
  specyfikacja: Specyfikacja;
  kontekst: KontekstWyboru;
  onSpecyfikacja: (s: Specyfikacja) => void;
  onBlad: (b: string | null) => void;
}) {
  const { api } = usePolaczenie();
  const [cel, ustawCel] = useState<Cel>("oferta");
  const [wysylanie, ustawWysylanie] = useState(false);
  const plik = useRef<HTMLInputElement>(null);
  if (!api) return null;
  const ro = s.tylko_do_odczytu;
  const celEfektywny: Cel = cel === "czesc" && !kontekst.czesc ? (kontekst.sekcja ? "sekcja" : "oferta") : cel === "sekcja" && !kontekst.sekcja ? "oferta" : cel;

  const wyslij = async (pliki: FileList | null) => {
    if (!pliki?.length) return;
    ustawWysylanie(true);
    try {
      let ostatnia: Specyfikacja | null = null;
      for (const p of Array.from(pliki)) {
        const r = await api.specyfikacje.dodajZdjecie(s.id, p, {
          nazwa: p.name,
          sekcja_id: celEfektywny === "sekcja" ? kontekst.sekcja?.id : null,
          czesc_id: celEfektywny === "czesc" ? kontekst.czesc?.id : null,
        });
        ostatnia = r.specyfikacja;
      }
      if (ostatnia) onSpecyfikacja(ostatnia);
      onBlad(null);
    } catch (e) {
      onBlad((e as Error).message);
    } finally {
      ustawWysylanie(false);
      if (plik.current) plik.current.value = "";
    }
  };

  const opis = (sekcjaId: number | null, czescId: number | null) => {
    if (czescId) {
      for (const sek of s.sekcje) {
        const c = sek.czesci.find((x) => x.id === czescId);
        if (c) return `${sek.numer ?? ""} ${sek.nazwa} › ${c.nazwa}`;
      }
    }
    if (sekcjaId) {
      const sek = s.sekcje.find((x) => x.id === sekcjaId);
      if (sek) return `${sek.numer ?? ""} ${sek.nazwa}`;
    }
    return "Cała oferta";
  };

  return (
    <div className="panel-zdjec">
      {!ro && (
        <div className="panel-zdjec__dodaj">
          <span className="pole__etykieta">Przypnij do</span>
          <Segment<Cel>
            etykieta="Gdzie przypiąć zdjęcie"
            wartosc={celEfektywny}
            onZmiana={ustawCel}
            opcje={[
              { wartosc: "oferta", etykieta: "Oferty" },
              ...(kontekst.sekcja ? [{ wartosc: "sekcja" as Cel, etykieta: "Sekcji" }] : []),
              ...(kontekst.czesc ? [{ wartosc: "czesc" as Cel, etykieta: "Części" }] : []),
            ]}
          />
          <input ref={plik} type="file" accept="image/jpeg,image/png" multiple hidden onChange={(e) => void wyslij(e.target.files)} />
          <Przycisk ikona={ImagePlus} disabled={wysylanie} onClick={() => plik.current?.click()}>
            {wysylanie ? "Wysyłanie…" : "Dodaj zdjęcia (JPG, PNG)"}
          </Przycisk>
          <p className="pole__pomoc">Cel: {celEfektywny === "oferta" ? "cała oferta" : opis(kontekst.sekcja?.id ?? null, celEfektywny === "czesc" ? (kontekst.czesc?.id ?? null) : null)}</p>
        </div>
      )}
      {s.zdjecia.length === 0 && <p className="pole__pomoc">Brak zdjęć.</p>}
      <ul className="panel-zdjec__lista">
        {s.zdjecia.map((z) => (
          <li key={z.id} className="panel-zdjec__zdjecie">
            <Miniatura api={api} z={z} />
            <div className="panel-zdjec__opis">
              <span className="pole__pomoc">{opis(z.sekcja_id, z.czesc_id)}</span>
              <input
                className="wejscie wejscie--maly"
                aria-label="Podpis zdjęcia"
                placeholder="Podpis w dokumencie"
                defaultValue={z.podpis ?? ""}
                disabled={ro}
                onBlur={async (e) => {
                  const v = e.target.value.trim() || null;
                  if (v === z.podpis) return;
                  try {
                    onSpecyfikacja((await api.specyfikacje.zmienZdjecie(z.id, v)).specyfikacja);
                  } catch (er) {
                    onBlad((er as Error).message);
                  }
                }}
              />
            </div>
            {!ro && (
              <Przycisk
                maly
                wariant="tekstowy"
                tylkoIkona
                ikona={Trash2}
                etykieta="Odepnij zdjęcie"
                onClick={async () => {
                  try {
                    onSpecyfikacja((await api.specyfikacje.usunZdjecie(z.id)).specyfikacja);
                  } catch (er) {
                    onBlad((er as Error).message);
                  }
                }}
              />
            )}
          </li>
        ))}
      </ul>
    </div>
  );
}

function Miniatura({ api, z }: { api: KlientApi; z: Zdjecie }) {
  const src = useObrazek(api, z.plik_id);
  return src ? <img src={src} alt={z.podpis ?? z.nazwa} /> : <span className="panel-zdjec__brak" aria-label={z.nazwa} />;
}
