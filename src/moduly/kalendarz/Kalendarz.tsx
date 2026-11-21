import { Route, Routes } from "react-router-dom";
import { OsCzasu } from "./OsCzasu";
import { UstawieniaKalendarza } from "./UstawieniaKalendarza";
import { WidokKalendarza } from "./WidokKalendarza";
import "./kalendarz.css";

/** Moduł Kalendarz: siatka miesiąca/tygodnia, oś czasu zlecenia, ustawienia terminów. */
export function Kalendarz() {
  return (
    <Routes>
      <Route index element={<WidokKalendarza />} />
      <Route path="ustawienia" element={<UstawieniaKalendarza />} />
      <Route path="zlecenie/:id" element={<OsCzasu />} />
    </Routes>
  );
}
