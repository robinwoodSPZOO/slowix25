import { Zaslepka } from "../Zaslepka";

export function Oferty() {
  return (
    <Zaslepka tytul="Oferty" faza="Generator Ofert">
      <ul>
        <li>Edytor specyfikacji: sekcja → część → wiersze Rodzaj | Materiał | Opis | Ilość → Wyposażenie.</li>
        <li>Status wiersza: ustalone / do potwierdzenia / po stronie klienta / poza wyceną; kategoria materiału.</li>
        <li>Wycena na poziomie modułu, sekcji lub całości; rabaty; VAT 8% / 23%; zdjęcia; wersje v1, v2…</li>
        <li>Eksport do PDF i DOCX.</li>
      </ul>
    </Zaslepka>
  );
}
