import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import { DostawcaPolaczenia } from "./kontekst/Polaczenie";
import { oproznijWszystkieSzkice } from "./lib/szkice";
import { etykietaOkna, obsluzZamykanieOkna } from "./lib/tauri";
import { Widzet } from "./widzet/Widzet";
import "./styles.css";

// To samo index.html obsługuje okno główne i okno-widżet; rozróżniamy po etykiecie okna Tauri.
const korzen = document.getElementById("root");
if (!korzen) throw new Error("Brak elementu #root");

etykietaOkna().then((etykieta) => {
  if (etykieta !== "widzet") void obsluzZamykanieOkna(oproznijWszystkieSzkice);
  createRoot(korzen).render(
    <StrictMode>
      <DostawcaPolaczenia>{etykieta === "widzet" ? <Widzet /> : <App />}</DostawcaPolaczenia>
    </StrictMode>,
  );
});
