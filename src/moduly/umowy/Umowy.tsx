import { Route, Routes } from "react-router-dom";
import { EkranAneksu } from "./EkranAneksu";
import { EkranUmowy } from "./EkranUmowy";
import { ListaUmow } from "./ListaUmow";
import { UstawieniaUmow } from "./UstawieniaUmow";
import "../oferty/oferty.css";
import "./umowy.css";

/** Moduł Umowy: lista, ekran umowy (skrócona forma, specyfikacja, płatności, aneksy), aneksy, ustawienia i szablony. */
export function Umowy() {
  return (
    <Routes>
      <Route index element={<ListaUmow />} />
      <Route path="ustawienia" element={<UstawieniaUmow />} />
      <Route path="aneksy/:id" element={<EkranAneksu />} />
      <Route path=":id" element={<EkranUmowy />} />
    </Routes>
  );
}
