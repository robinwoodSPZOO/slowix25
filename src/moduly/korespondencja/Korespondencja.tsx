import { Zaslepka } from "../Zaslepka";

export function Korespondencja() {
  return (
    <Zaslepka tytul="Korespondencja" faza="Sieć i korespondencja">
      <ul>
        <li>Gmail: pobieranie wiadomości przez oficjalne API i przypisywanie do klienta po adresie e-mail.</li>
        <li>WhatsApp: import pliku „Eksportuj czat” na kartę klienta.</li>
        <li>Wysyłka ofert i umów oraz wysyłka zbiorcza (tylko do osób ze zgodą marketingową).</li>
      </ul>
    </Zaslepka>
  );
}
