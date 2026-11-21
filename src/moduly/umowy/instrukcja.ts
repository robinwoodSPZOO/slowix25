// Instrukcja przygotowania szablonu umowy z dotychczasowego wzorca (ta sama treść jest w
// docs/SZABLON_UMOWY.md). Lista pól pochodzi z serwera (GET /szablony-dokumentow/pola).

export const KROKI_SZABLONU: { tytul: string; opis: string }[] = [
  {
    tytul: "Zrób kopię wzorca jako .docx",
    opis: "Otwórz swój wzorzec umowy w Wordzie i zapisz kopię jako „Dokument programu Word (*.docx)”. Makra nie są potrzebne – program sam wypełnia pola (plik .docm też zadziała; makra i przycisk formularza zostaną usunięte z gotowego dokumentu).",
  },
  {
    tytul: "Zostaw pola w nawiasach kwadratowych",
    opis: "Pola zmienne zapisuje się w tekście jak dotąd: [numer umowy], [klient], [adres montażu], [wartość], [kwota słownie]… Wielkość liter i polskie znaki nie mają znaczenia. Kontrolki treści z dotychczasowego formularza są rozpoznawane automatycznie po nazwie. Miejsca do ręcznego wpisania możesz zostawić jako [uzupełnij] – program ich nie zmienia.",
  },
  {
    tytul: "Wstaw tabele jednym polem",
    opis: "W załączniku nr 1 usuń przykładową tabelę specyfikacji i w jej miejsce wpisz w osobnym akapicie [specyfikacja]. Harmonogram płatności: kontrolkę treści „Harmonogram płatności” z dotychczasowego formularza można zostawić (program wstawi w niej tabelę rat), a w zwykłym tekście wpisz akapit [harmonogram płatności]. Tabelę etapów w załączniku nr 2 zastąp akapitem [harmonogram etapów].",
  },
  {
    tytul: "Oznacz załączniki opcjonalne",
    opis: "Załączniki 2, 3 i 6 otocz znacznikami w osobnych akapitach: [[załącznik 3]] przed pierwszym akapitem załącznika i [[/załącznik 3]] po ostatnim. Gdy załącznik nie jest zaznaczony w umowie, cały fragment znika. Protokół odbioru otocz [[załącznik 5]] … [[/załącznik 5]] – z tego fragmentu powstaje osobny protokół z saldem.",
  },
  {
    tytul: "§9 VAT i warianty płatności",
    opis: "Kratki „☐” przy rodzajach nieruchomości zamień na [VAT lokal], [VAT dom], [VAT inny] (program wstawi ☒ albo ☐), miejsce na powierzchnię – na [powierzchnia], a podstawę – na [podstawa powierzchni]. Tekst tylko dla jednego wariantu płatności otocz [[wariant A]] … [[/wariant A]] albo [[wariant B]] … [[/wariant B]].",
  },
  {
    tytul: "Kwoty i daty",
    opis: "Kwoty są wstawiane bez „zł” (np. 48 352,00) – „zł” zostaw w tekście szablonu. Kwoty słownie mają postać „… złote 00/100” (z poprawną odmianą). Daty – DD.MM.RRRR. Dane firmy (nazwa, adres, NIP, bank, rachunek) pochodzą z Oferty → Dane firmy, nie z szablonu.",
  },
  {
    tytul: "Wgraj i sprawdź",
    opis: "Wgraj plik przyciskiem „Wgraj szablon”. Raport pokaże rozpoznane pola i „pola nieznane” (zwykle literówki). Następnie otwórz dowolną umowę → Dokument: lista pól pokaże wartości i pola puste, a pobrany DOCX porównaj z wzorcem.",
  },
  {
    tytul: "Aneks",
    opis: "Szablon aneksu przygotuj tak samo (pola [numer aneksu], [data aneksu], [lista zmian], [nowa wartość], [nowy termin montażu], [specyfikacja]) albo korzystaj z wbudowanego przykładu.",
  },
];
