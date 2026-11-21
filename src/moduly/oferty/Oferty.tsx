import { Route, Routes } from "react-router-dom";
import { EkranOferty } from "./EkranOferty";
import { ListaOfert } from "./ListaOfert";
import { UstawieniaOfert } from "./UstawieniaOfert";
import "./oferty.css";

/** Moduł Oferty: lista, ekran oferty (edytor + wycena + wersje), dane firmy do dokumentów. */
export function Oferty() {
  return (
    <Routes>
      <Route index element={<ListaOfert />} />
      <Route path="ustawienia" element={<UstawieniaOfert />} />
      <Route path=":id" element={<EkranOferty />} />
    </Routes>
  );
}
