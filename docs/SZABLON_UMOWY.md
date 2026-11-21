# Szablon umowy i aneksu – jak przygotować z dotychczasowego wzorca

Instrukcja dla użytkownika (ta sama treść jest w programie: *Umowy → Ustawienia i szablony*). Szablon to zwykły
dokument Word wgrywany w programie; przechowuje go serwer w swoim folderze danych – **nie jest częścią repozytorium**.
Bez wgranego szablonu program używa wbudowanego, fikcyjnego szablonu przykładowego (z napisem „SZABLON PRZYKŁADOWY”).

## Kroki

1. **Zrób kopię wzorca jako .docx.** Otwórz swój wzorzec umowy w Wordzie i zapisz kopię jako „Dokument programu Word (*.docx)”. Makra nie są potrzebne – program sam wypełnia pola (plik .docm też zadziała; makra i przycisk formularza zostaną usunięte z gotowego dokumentu).
2. **Zostaw pola w nawiasach kwadratowych.** Pola zmienne zapisuje się w tekście jak dotąd: [numer umowy], [klient], [adres montażu], [wartość], [kwota słownie]… Wielkość liter i polskie znaki nie mają znaczenia. Kontrolki treści z dotychczasowego formularza są rozpoznawane automatycznie po nazwie. Miejsca do ręcznego wpisania możesz zostawić jako [uzupełnij] – program ich nie zmienia.
3. **Wstaw tabele jednym polem.** W załączniku nr 1 usuń przykładową tabelę specyfikacji i w jej miejsce wpisz w osobnym akapicie [specyfikacja]. Harmonogram płatności: kontrolkę treści „Harmonogram płatności” z dotychczasowego formularza można zostawić (program wstawi w niej tabelę rat), a w zwykłym tekście wpisz akapit [harmonogram płatności]. Tabelę etapów w załączniku nr 2 zastąp akapitem [harmonogram etapów].
4. **Oznacz załączniki opcjonalne.** Załączniki 2, 3 i 6 otocz znacznikami w osobnych akapitach: [[załącznik 3]] przed pierwszym akapitem załącznika i [[/załącznik 3]] po ostatnim. Gdy załącznik nie jest zaznaczony w umowie, cały fragment znika. Protokół odbioru otocz [[załącznik 5]] … [[/załącznik 5]] – z tego fragmentu powstaje osobny protokół z saldem.
5. **§9 VAT i warianty płatności.** Kratki „☐” przy rodzajach nieruchomości zamień na [VAT lokal], [VAT dom], [VAT inny] (program wstawi ☒ albo ☐), miejsce na powierzchnię – na [powierzchnia], a podstawę – na [podstawa powierzchni]. Tekst tylko dla jednego wariantu płatności otocz [[wariant A]] … [[/wariant A]] albo [[wariant B]] … [[/wariant B]].
6. **Kwoty i daty.** Kwoty są wstawiane bez „zł” (np. 48 352,00) – „zł” zostaw w tekście szablonu. Kwoty słownie mają postać „… złote 00/100” (z poprawną odmianą). Daty – DD.MM.RRRR. Dane firmy (nazwa, adres, NIP, bank, rachunek) pochodzą z Oferty → Dane firmy, nie z szablonu.
7. **Wgraj i sprawdź.** Wgraj plik przyciskiem „Wgraj szablon”. Raport pokaże rozpoznane pola i „pola nieznane” (zwykle literówki). Następnie otwórz dowolną umowę → Dokument: lista pól pokaże wartości i pola puste, a pobrany DOCX porównaj z wzorcem.
8. **Aneks.** Szablon aneksu przygotuj tak samo (pola [numer aneksu], [data aneksu], [lista zmian], [nowa wartość], [nowy termin montażu], [specyfikacja]) albo korzystaj z wbudowanego przykładu.

## Przykład fragmentu szablonu

```
UMOWA NR [numer umowy]
zawarta w dniu [data umowy] w [miejscowość] pomiędzy [wykonawca] a [klient], [adres klienta].
Wynagrodzenie: [wartość] zł brutto (słownie: [kwota słownie]).
[[wariant B]]
Kwota przed montażem: [kwota przed montażem] zł, pozostała część: [termin pozostałej kwoty].
[[/wariant B]]
[harmonogram płatności]
[VAT lokal] lokal mieszkalny   [VAT dom] budynek mieszkalny jednorodzinny   [VAT inny] inny
[[załącznik 3]]
Załącznik nr 3 – Cennik montażu AGD …
[[/załącznik 3]]
```

Pola blokowe (`[specyfikacja]`, `[harmonogram płatności]`, `[harmonogram etapów]`, `[lista zmian]`) i znaczniki bloków `[[…]]` muszą stać **w osobnych akapitach**.

## Pola tekstowe

| Pole | Co wstawia program |
| --- | --- |
| `[numer umowy]` | numer umowy, np. 07/2026 |
| `[miejscowość]` | miejsce zawarcia umowy |
| `[data umowy]` | data zawarcia (DD.MM.RRRR) |
| `[klient]` | zamawiający – imię i nazwisko albo nazwa |
| `[adres klienta]` | adres zamawiającego w jednej linii |
| `[e-mail]` | e-mail zamawiającego |
| `[telefon]` | telefon zamawiającego |
| `[nip klienta]` | NIP zamawiającego (firma) |
| `[osoby kontaktowe]` | osoby kontaktowe zamawiającego (po przecinku) |
| `[adres montażu]` | adres miejsca montażu |
| `[piętro]` | piętro |
| `[kod domofonu]` | kod domofonu |
| `[parking]` | informacja o parkingu |
| `[wniesienie]` | winda i możliwość wniesienia |
| `[liczba dni]` | dni kalendarzowe na przekazanie brakujących informacji |
| `[wartość]` | wartość umowy brutto, np. 48 352,00 (bez „zł” – dopisz w szablonie) |
| `[kwota słownie]` | wartość umowy słownie, np. „czterdzieści osiem tysięcy … złote 00/100” |
| `[stawka VAT]` | 8 albo 23 (bez znaku %) |
| `[zadatek]` | kwota zadatku |
| `[zadatek słownie]` | zadatek słownie |
| `[termin zadatku]` | data, do której należy wpłacić zadatek |
| `[wariant A lub B]` | wybrany wariant płatności: A albo B |
| `[kwota przed montażem]` | wariant B: kwota płatna przed montażem |
| `[kwota przed montażem słownie]` | wariant B: kwota przed montażem słownie |
| `[termin kwoty przed montażem]` | wariant B: termin kwoty przed montażem |
| `[pozostała kwota]` | ostatnia rata (A: przed montażem, B: po odbiorze) |
| `[pozostała kwota słownie]` | ostatnia rata słownie |
| `[termin pozostałej kwoty]` | termin ostatniej raty |
| `[termin montażu]` | planowany termin rozpoczęcia montażu („do”) |
| `[nazwa zabudowy]` | nazwy sekcji specyfikacji, np. „Zabudowa kuchni, Szafa” |
| `[rodzaj nieruchomości]` | §9 VAT: lokal mieszkalny / budynek mieszkalny jednorodzinny / inny |
| `[powierzchnia]` | §9 VAT: powierzchnia użytkowa w m² |
| `[podstawa powierzchni]` | §9 VAT: podstawa określenia powierzchni |
| `[VAT lokal]` | pole wyboru ☒/☐ – lokal mieszkalny |
| `[VAT dom]` | pole wyboru ☒/☐ – budynek mieszkalny jednorodzinny |
| `[VAT inny]` | pole wyboru ☒/☐ – inny |
| `[data odbioru]` | data protokołu odbioru |
| `[wpłacono]` | suma wpłat |
| `[pozostało]` | pozostało do zapłaty |
| `[termin płatności]` | termin najbliższej niezapłaconej raty |
| `[odbiór bez uwag]` | pole wyboru ☒/☐ – odbiór bez zastrzeżeń |
| `[odbiór z uwagami]` | pole wyboru ☒/☐ – odbiór z uwagami |
| `[uwagi do odbioru]` | uwagi z protokołu odbioru |
| `[data zgody]` | data zgody na zdjęcia (= data umowy) |
| `[wykonawca]` | nazwa firmy (Ustawienia → Dane firmy) |
| `[adres wykonawcy]` | adres firmy |
| `[nip wykonawcy]` | NIP firmy |
| `[telefon wykonawcy]` | telefon firmy |
| `[e-mail wykonawcy]` | e-mail firmy |
| `[bank]` | nazwa banku firmy |
| `[numer rachunku]` | numer rachunku firmy |
| `[swift]` | kod SWIFT/BIC |
| `[data wydruku]` | dzisiejsza data |
| `[numer aneksu]` | numer kolejny aneksu do umowy |
| `[data aneksu]` | data aneksu |
| `[opis zmian]` | opis zmian wpisany w aneksie |
| `[zmiana kwoty]` | zmiana wynagrodzenia (np. +1 200,00 albo -500,00) |
| `[poprzednia wartość]` | wartość umowy przed aneksem |
| `[nowa wartość]` | wartość umowy po aneksie |
| `[nowa wartość słownie]` | wartość po aneksie słownie |
| `[poprzedni termin montażu]` | termin przed aneksem |
| `[nowy termin montażu]` | termin po aneksie (albo dotychczasowy) |

## Pola blokowe i bloki warunkowe

| Zapis | Znaczenie | Dokument |
| --- | --- | --- |
| `[specyfikacja]` | tabele specyfikacji (sekcje → części → Rodzaj | Materiał | Opis | Ilość) | oba |
| `[harmonogram płatności]` | tabela rat: zadatek, raty, terminy, suma | umowa |
| `[harmonogram etapów]` | tabela: Lp. | nazwa pracy | termin rozpoczęcia | kwota | suma | umowa |
| `[lista zmian]` | numerowana lista zmian aneksu (kwota, termin, specyfikacja) | aneks |
| `[[załącznik 2]] … [[/załącznik 2]]` | [[załącznik 2]] … [[/załącznik 2]] – harmonogram etapów / projekt (gdy zaznaczony) | umowa |
| `[[załącznik 3]] … [[/załącznik 3]]` | [[załącznik 3]] … [[/załącznik 3]] – cennik montażu AGD | umowa |
| `[[załącznik 5]] … [[/załącznik 5]]` | [[załącznik 5]] … [[/załącznik 5]] – protokół odbioru (z niego powstaje osobny protokół) | umowa |
| `[[załącznik 6]] … [[/załącznik 6]]` | [[załącznik 6]] … [[/załącznik 6]] – zgoda na zdjęcia | umowa |
| `[[wariant A]] … [[/wariant A]]` | [[wariant A]] … [[/wariant A]] – tekst tylko dla wariantu A (analogicznie [[wariant B]]) | umowa |
| `[[stawka 8]] … [[/stawka 8]]` | [[stawka 8]] … [[/stawka 8]] – np. oświadczenie VAT tylko przy 8% (analogicznie [[stawka 23]]) | umowa |

## Zasady techniczne (dla porządku)

* Wielkość liter, polskie znaki i wielokrotne spacje w nazwach pól nie mają znaczenia (`[Wartosc]` = `[wartość]`).
* Word często dzieli tekst pola na kilka fragmentów (np. po poprawce pisowni) – program je scala; formatowanie pola bierze z pierwszego znaku.
* Kontrolki treści (Deweloper → Kontrolka zawartości) o nazwie lub tagu pola są zastępowane wartością – można zostawić te z dotychczasowego formularza.
* Pola nieznane programowi zostają w dokumencie bez zmian i są wypisane w raporcie po wgraniu oraz w oknie „Dokument”.
* Pola bez wartości są zastępowane „………………” do uzupełnienia ręcznie; ich lista jest widoczna przed pobraniem dokumentu.
* Pola działają też w nagłówku i stopce (np. numer umowy na każdej stronie); pola blokowe – tylko w treści.
* Plik .docm jest akceptowany – w wygenerowanym dokumencie makra i własna wstążka są usuwane.
* PDF powstaje z gotowego DOCX przez Microsoft Word albo LibreOffice zainstalowane na komputerze-serwerze; bez nich pobierz DOCX i zapisz go jako PDF w Wordzie.
