import { useEffect, useState } from "react";
import { Route, Routes, useNavigate, useParams } from "react-router-dom";
import { TYPY_KLIENTA, type Klient } from "../../api/typy";
import { usePolaczenie } from "../../kontekst/Polaczenie";
import { FormularzKlienta } from "./FormularzKlienta";

function Lista() {
  const { api, ostatnieZdarzenie } = usePolaczenie();
  const [szukaj, ustawSzukaj] = useState("");
  const [klienci, ustawKlientow] = useState<Klient[]>([]);
  const [blad, ustawBlad] = useState<string | null>(null);
  const nawiguj = useNavigate();

  useEffect(() => {
    if (!api) return;
    const t = setTimeout(() => {
      api.klienci.lista(szukaj).then(
        (k) => {
          ustawKlientow(k);
          ustawBlad(null);
        },
        (e: Error) => ustawBlad(e.message),
      );
    }, 200);
    return () => clearTimeout(t);
    // Odświeżamy także po zdarzeniu z innego stanowiska.
  }, [api, szukaj, ostatnieZdarzenie]);

  return (
    <>
      <h1>Klienci</h1>
      <div className="pasek">
        <input placeholder="Szukaj (nazwa, e-mail, telefon)" value={szukaj} onChange={(e) => ustawSzukaj(e.target.value)} />
        <button onClick={() => nawiguj("/klienci/nowy")}>Nowy klient</button>
      </div>
      {blad && <p className="blad">{blad}</p>}
      <table>
        <thead>
          <tr>
            <th>Nazwa</th>
            <th>Typ</th>
            <th>E-mail</th>
            <th>Telefon</th>
            <th>Miasto</th>
          </tr>
        </thead>
        <tbody>
          {klienci.map((k) => (
            <tr key={k.id} onClick={() => nawiguj(`/klienci/${k.id}`)} style={{ cursor: "pointer" }}>
              <td>{k.nazwa}</td>
              <td>{TYPY_KLIENTA[k.typ]}</td>
              <td>{k.email ?? ""}</td>
              <td>{k.telefon ?? ""}</td>
              <td>{k.adres_miasto ?? ""}</td>
            </tr>
          ))}
          {klienci.length === 0 && !blad && (
            <tr>
              <td colSpan={5}>Brak klientów.</td>
            </tr>
          )}
        </tbody>
      </table>
    </>
  );
}

function Karta() {
  const { id } = useParams();
  return <FormularzKlienta id={id === "nowy" ? null : Number(id)} />;
}

export function ListaKlientow() {
  return (
    <Routes>
      <Route index element={<Lista />} />
      <Route path=":id" element={<Karta />} />
    </Routes>
  );
}
