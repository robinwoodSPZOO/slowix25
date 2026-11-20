import { Zaslepka } from "../Zaslepka";

export function Umowy() {
  return (
    <Zaslepka tytul="Umowy" faza="Generator Umów">
      <ul>
        <li>Umowa z zaakceptowanej oferty (kopia specyfikacji z listą różnic) albo od zera.</li>
        <li>Skrócony formularz: dane, miejsce montażu, terminy, płatności wariant A/B, VAT, załączniki.</li>
        <li>Aneksy, rejestr wpłat, protokół odbioru z saldem.</li>
      </ul>
    </Zaslepka>
  );
}
