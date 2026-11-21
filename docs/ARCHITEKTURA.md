# Architektura programu ROBINWOOD

Dokument dla kolejnych agentów i programistów. Stan: Faza 0 (szkielet) + Faza 2 (Generator Ofert, rozdz. 7–9).

## 1. Stos

| Warstwa | Wybór | Powód |
| --- | --- | --- |
| Aplikacja okienkowa | **Tauri 2** (Rust + systemowy WebView2) | Mały instalator, natywny zasobnik, autostart, powiadomienia, kilka okien (widżet) |
| Interfejs | **React 19 + TypeScript + Vite** | Popularny stos, łatwe przeniesienie projektów z Figmy |
| Serwer API | **axum 0.8** (Rust) w tym samym procesie co aplikacja | Jeden program na wszystkich komputerach, bez osobnej usługi do instalowania |
| Baza | **SQLite** (sqlx 0.8, tryb WAL) na komputerze-serwerze | Zero administracji, jeden plik, łatwa kopia zapasowa |
| Migracje | `sqlx::migrate!` – pliki SQL wkompilowane w program | Każda wersja programu sama aktualizuje schemat przy starcie |
| Wykrywanie w sieci | **mDNS / DNS-SD** (`mdns-sd`), usługa `_robinwood._tcp.local.` | Klienci znajdują serwer bez wpisywania adresu |
| Odświeżanie na żywo | **WebSocket** `/api/v1/zdarzenia` | Zmiana na jednym stanowisku pojawia się na pozostałych w ciągu sekund |
| Instalator | NSIS (Tauri bundler), budowany w GitHub Actions na `windows-latest` | Instalacja dla bieżącego użytkownika, po polsku |

Rozważane alternatywy: Electron (większy instalator, osobny serwer w Node), PostgreSQL (wymaga instalacji i administracji serwera bazy – zbędne przy kilku stanowiskach), osobna usługa Windows dla serwera (więcej instalacji; samodzielny program `robinwood-serwer` jest gotowy na wypadek, gdyby serwerem miał być mini PC bez okna).

## 2. Tryb serwer / klient

```
┌───────────── Komputer-serwer ─────────────┐          ┌──── Komputer-klient ────┐
│ ROBINWOOD (Tauri)                          │          │ ROBINWOOD (Tauri)        │
│  ├─ okno (React) ──HTTP──► 127.0.0.1:47800 │◄──HTTP───│  okno (React)            │
│  ├─ API axum 0.0.0.0:47800 ◄───────────────┼──WS──────│  (adres z mDNS / ręczny) │
│  ├─ SQLite: <folder danych>/robinwood.db   │          └──────────────────────────┘
│  └─ ogłoszenie mDNS _robinwood._tcp        │
└────────────────────────────────────────────┘
```

* Tryb zapisany jest lokalnie w `%APPDATA%\pl.robinwood.program\konfiguracja.json` (`tryb`, `nazwa_stanowiska`, `port`, `folder_danych`, `adres_serwera`, `klucz_api`). Zmiana trybu = zapis + restart aplikacji.
* **Interfejs zawsze rozmawia z API HTTP** – także na serwerze (przez 127.0.0.1). Dzięki temu każdy moduł działa identycznie na serwerze i na kliencie. Komendy Tauri (`invoke`) służą WYŁĄCZNIE do spraw lokalnego komputera: konfiguracja, wykrywanie serwerów, powiadomienia, okna, autostart.
* **Dostęp** chroni klucz API (32 znaki, generowany przy wyborze trybu serwer): nagłówek `Authorization: Bearer <klucz>`, dla WebSocket parametr `?klucz=`. `GET /api/v1/status` jest jawny (test połączenia).
* Ruch w sieci lokalnej jest nieszyfrowany (HTTP). Akceptowalne w zaufanej sieci firmowej z hasłem Wi-Fi; jeśli sieć jest współdzielona, do rozważenia TLS z certyfikatem samopodpisanym (otwarty problem).
* Windows pyta o zgodę zapory przy pierwszym nasłuchu na 0.0.0.0 – trzeba zezwolić dla sieci prywatnej. mDNS bywa blokowany przez izolację klientów w routerach Wi-Fi – wtedy adres wpisuje się ręcznie.
* Gdy serwer jest niedostępny, klient pokazuje „Brak połączenia”. Pamięć podręczna tylko do odczytu (kalendarz offline) – do zrobienia w fazie Kalendarz.

### Okna, zasobnik, autostart, powiadomienia

* Okna (`src-tauri/tauri.conf.json`): `main` (główne) i `widzet` (320×420, bez ramki, bez paska zadań, `alwaysOnBottom` – „przypięte do pulpitu”). Oba ładują to samo `index.html`; `src/main.tsx` wybiera widok po etykiecie okna.
* Zamknięcie okna chowa je (`on_window_event`); wyjście przez menu zasobnika *Zakończ*.
* Autostart: `tauri-plugin-autostart` z argumentem `--zminimalizowany` – start bez okna głównego, z widżetem. Włączany w *Ustawienia*.
* Powiadomienia: `tauri-plugin-notification`, moduł `src-tauri/src/powiadomienia.rs` (na razie tylko testowe).
* `tauri-plugin-single-instance`: drugie uruchomienie tylko pokazuje okno.

## 3. Schemat danych

Plik: `crates/robinwood-serwer/migrations/0001_schemat_poczatkowy.sql`.

Konwencje: nazwy po polsku bez polskich znaków; kwoty w **groszach** (`*_gr`, INTEGER); rabat procentowy w setnych procenta (`*_bp`); daty `TEXT 'RRRR-MM-DD'`, znaczniki czasu ISO UTC; statusy jako TEXT z `CHECK`; klucze obce włączone na połączeniu.

| Obszar | Tabele |
| --- | --- |
| Ustawienia | `ustawienia` (m.in. odstępy 7 dni, dni robocze/kalendarzowe), `dni_wolne` |
| Klienci | `klienci` (typ, dane kontaktowe, polecający architekt, zgoda marketingowa, usunięcie miękkie), `osoby_kontaktowe`, `etykiety`, `klienci_etykiety`, `inwestycje` (miejsce montażu: piętro, domofon, parking, wniesienie) |
| Materiały | `dostawcy`, `kategorie_materialow` (widełki `dni_dostawy_min/max`, domyślny dostawca), `czasy_dostaw` (para kategoria + dostawca, średnia rzeczywista), `slowniki`, `pozycje_slownika` |
| Specyfikacja | `specyfikacje` → `sekcje_specyfikacji` (pomieszczenie; typ meble/AGD, wariant, cena sekcji) → `czesci_specyfikacji` (moduł, cena modułu) → `wiersze_specyfikacji` (podgrupa główna/wyposażenie/frezowanie, Rodzaj, Materiał, Opis, Ilość + jednostka, status, termin potwierdzenia, kategoria materiału); `zdjecia_specyfikacji`, `pliki` |
| Oferty | `oferty` (numer, klient, status) → `wersje_oferty` (każda wersja ma własną specyfikację; poziom wyceny, rabaty, VAT, koszt własny, ważność cen) |
| Umowy | `umowy` (numer `NN/RRRR` unikalny w roku, z wersji oferty lub od zera, własna kopia specyfikacji, pola skróconego formularza, wariant A/B), `platnosci_planowane`, `wplaty`, `aneksy` (z nową wersją specyfikacji), `protokoly_odbioru` |
| Produkcja i kalendarz | `zlecenia` (etap, montaż od, czas warsztatu i montażu, nadpisania odstępów), `zamowienia_materialow` (daty sweet spot / planowa / krytyczna, faktyczne daty zamówienia i dostawy), `zadania_kalendarza` (typ = kolor, wykonano, wyliczone/ręczne) |
| Korespondencja | `korespondencja` (gmail / whatsapp / ręczna, unikalne `zewnetrzne_id`), `zalaczniki_korespondencji` |
| Audyt | `dziennik_zmian` |

Migracja `0002_oferty_specyfikacja.sql` (Faza 2) dodaje: `ident` w sekcjach/częściach/wierszach (stały identyfikator przenoszony przy kopiowaniu – po nim porównujemy wersje), `wiersze_specyfikacji.cena_gr` (cena jednostkowa pozycji AGD) i `material_z_korpusu` (materiał wypełniony automatycznie), `wersje_oferty.adres_inwestycji`, `tryb_cen` (brutto/netto), `zmieniono`, `pozycje_slownika.podgrupa` i `zrodlo`, tabelę `szablony_mebli`, słowniki `material`, `dekor`, `opis`, `agd`, `agd_marka`, `jednostka`, ogólne klauzule końcowe oraz klucze ustawień `firma_*` i `oferta_*`. **Uwaga:** `wersje_oferty.klauzule` przechowuje JSON z listą **treści** klauzul (kopia w chwili tworzenia wersji), nie identyfikatorów.

Kluczowa decyzja: **specyfikacja jest osobnym obiektem**, do którego wskazuje wersja oferty, umowa i aneks. Przejście oferta → umowa = skopiowanie specyfikacji (`specyfikacje.zrodlo_id` pamięta źródło), co pozwala pokazać różnice.

## 4. API

Prefiks `/api/v1`. Błędy: JSON `{ "blad": "komunikat po polsku" }` z kodem 401/404/422/500.

| Metoda i ścieżka | Opis |
| --- | --- |
| `GET /status` | nazwa serwera, wersja, `wersja_api` (bez klucza) |
| `GET /klienci?szukaj=` | lista (nazwa, e-mail, telefon) |
| `POST /klienci` | dodanie → 201 |
| `GET/PUT/DELETE /klienci/{id}` | odczyt, pełna zmiana, usunięcie miękkie → 204 |
| `GET /slowniki`, `GET /slowniki/{kod}` | słowniki i ich pozycje |
| `GET /kategorie-materialow` | kategorie z widełkami dostaw |
| `GET /zdarzenia?klucz=` | WebSocket: `{"zasob":"klienci","id":1,"operacja":"zmiana"}`; `zasob:"*"` = odśwież wszystko |

Faza 2 (trasy w `crates/robinwood-serwer/src/api_oferty.rs`, dołączone do routera jedną linią w `api.rs`):

| Metoda i ścieżka | Opis |
| --- | --- |
| `GET /specyfikacje/{id}` | drzewo: sekcje → części → wiersze, zdjęcia, właściciel, `tylko_do_odczytu` |
| `POST /specyfikacje/{id}/sekcje`, `PUT/DELETE /sekcje/{id}` | sekcje (numeracja „01, 02…” automatyczna; sekcje AGD bez numeru) |
| `POST /specyfikacje/{id}/sekcje/kolejnosc` `{ids}` | kolejność sekcji |
| `POST /sekcje/{id}/czesci`, `PUT/DELETE /czesci/{id}` | części |
| `POST /sekcje/{id}/czesci/kolejnosc` `{ids}` | kolejność części; id z innej sekcji tej samej specyfikacji = przeniesienie |
| `POST /czesci/{id}/wiersze` (`po_wierszu_id` = wstaw za), `PUT/DELETE /wiersze/{id}` | wiersze |
| `POST /czesci/{id}/wiersze/kolejnosc` `{ids}` | kolejność / przeniesienie wierszy |
| `POST /sekcje/{id}/kopiuj`, `POST /czesci/{id}/kopiuj` `{cel_id?}` | kopia (nowe `ident`) w tym samym albo innym miejscu |
| `POST /wiersze/{id}/kopiuj-material` | „kopiuj materiał w dół” |
| `POST /specyfikacje/{id}/z-szablonu`, `POST /sekcje/{id}/z-szablonu` | nowa sekcja / część z szablonu mebli |
| `POST /specyfikacje/{id}/zdjecia?sekcja_id=&czesc_id=&nazwa=&podpis=` | treść = bajty JPG/PNG (≤ 20 MB) → 201 |
| `PUT/DELETE /zdjecia/{id}`, `GET /pliki/{id}` | podpis, odpięcie; plik (także `?klucz=` dla `<img>`) |
| `GET /oferty?status=&klient_id=&szukaj=`, `POST /oferty` | lista (najnowsza wersja + suma brutto), nowa oferta z v1 |
| `GET/PUT/DELETE /oferty/{id}`, `PUT /oferty/{id}/status` | szczegóły, zmiana inwestora, usunięcie, status |
| `POST /oferty/{id}/wersje` `{z_wersji_id?}` | nowa wersja = kopia (specyfikacja + parametry) |
| `GET /oferty/{id}/porownanie?z=&do=` | różnice między wersjami |
| `GET/PUT /wersje-oferty/{id}` | nagłówek, parametry wyceny, **wycena**, specyfikacja |
| `GET /wersje-oferty/{id}/eksport/{pdf,docx}?zdjecia=0/1` | dokument oferty |
| `GET /szablony-mebli`, `GET /slowniki-podpowiedzi` | szablony; wszystkie słowniki jednym zapytaniem |
| `POST /import/slowniki`, `POST /import/szablony-mebli` | import plików użytkownika (JSON w treści) |
| `GET/PUT /ustawienia/firma` | dane wykonawcy i domyślne wartości oferty |

Wszystkie zmiany specyfikacji zwracają `{id, specyfikacja}` – całe aktualne drzewo – i rozsyłają zdarzenie `specyfikacje` (+ `oferty` dla właściciela).

## 5. Konwencje dla kolejnych faz

**Gdzie dokładać kod**

| Moduł | Rust (serwer) | Interfejs |
| --- | --- | --- |
| Oferty | `crates/robinwood-serwer/src/oferty.rs` (+ `specyfikacje.rs` wspólny z umowami) | `src/moduly/oferty/`, wspólny edytor specyfikacji w `src/moduly/specyfikacja/` |
| Umowy | `.../src/umowy.rs`, `wplaty.rs`, `aneksy.rs` | `src/moduly/umowy/` |
| Kalendarz | `.../src/kalendarz/` (czysta logika liczenia terminów w osobnym module bez bazy, testowana jednostkowo; święta PL) | `src/moduly/kalendarz/`, `src/widzet/` |
| Korespondencja | `.../src/korespondencja/` (Gmail API, parser eksportu WhatsApp) | `src/moduly/korespondencja/` |
| Powiadomienia | logika „co i kiedy” po stronie serwera lub klienta, wyświetlanie: `src-tauri/src/powiadomienia.rs` | – |

**Zasady**

1. Nowy zasób = plik w `crates/robinwood-serwer/src/` (model + funkcje na `SqlitePool`) + trasy w `api.rs` + testy (jednostkowe w pliku i HTTP w `tests/`) + metody w `src/api/klient.ts` + typy w `src/api/typy.ts`.
2. Każda zmiana danych: wpis w `dziennik_zmian` i `stan.zdarzenia.wyslij(...)`, żeby inne stanowiska się odświeżyły.
3. Zmiany schematu **tylko nowymi migracjami** `000N_opis.sql`; nigdy nie edytuj migracji już wypuszczonej (sqlx sprawdza sumy kontrolne).
4. Nowy moduł interfejsu = folder w `src/moduly/` + wpis w `src/moduly/rejestr.tsx`.
5. Pieniądze w groszach (`formatujKwote`, `parsujKwote` w `src/lib/formaty.ts`), daty w ISO, wyświetlanie `DD.MM.RRRR`.
6. Kolory typów zadań: tokeny `--zadanie-montaz/-warsztat/-zamowienie/-rysunki/-pomiar` w `src/ui/tokeny.css` (montaż #0072B2, warsztat #8A5A2B, zamówienie #00896A, rysunki #B8578F, pomiar #2B8FCB). Są zarezerwowane – nie używać ich do niczego innego. `src/lib/kolory.ts` z Fazy 0 ma jeszcze stare wartości – do podmiany przez moduł Kalendarz.
7. Wygląd: tokeny i podstawowe komponenty w `src/ui/` (rozdz. 9). Nowe ekrany składamy z nich; statusy i stany zawsze ikoną + tekstem (lucide-react).
8. Interfejs i komunikaty po polsku.
9. **Repozytorium jest publiczne**: żadnych danych klientów, przykładowych umów/ofert z prawdziwymi danymi, numerów kont, kluczy API. Dane testowe – wyłącznie fikcyjne („Klient Testowy”, `example.com`). Generowanie DOCX z szablonu umowy: szablon z danymi firmy trzymać poza repo (np. w folderze danych serwera) albo upewnić się, że nie zawiera danych wrażliwych.
10. Przed commitem: `npm run typecheck && npm run lint && npm test && cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`.

## 6. Otwarte kwestie

* Decyzje użytkownika z planu (rozdz. 10): punkt odniesienia zamówień, dni robocze czy kalendarzowe (ustawienie `liczenie_dni`, domyślnie „robocze”), lista kategorii z czasami dostaw, format numeracji umów (przyjęto `NN/RRRR`).
* Kopie zapasowe bazy (nocna kopia `VACUUM INTO` na drugi dysk / do chmury) – do zrobienia w fazie „Sieć”.
* Przechowywanie plików: zrobione dla zdjęć (Faza 2, `pliki.rs`); do dołożenia sprzątanie plików bez odwołań i kopia zapasowa folderu `pliki/` razem z bazą.
* Uprawnienia użytkowników (np. koszt własny widoczny tylko dla właściciela) – obecnie jeden wspólny klucz dla wszystkich stanowisk; API wyceny zwraca marżę każdemu stanowisku, a interfejs chowa ją w rozwijanym „panelu wewnętrznym”.
* Podpisywanie instalatora certyfikatem (bez niego Windows SmartScreen ostrzega przy instalacji) i automatyczne aktualizacje (`tauri-plugin-updater`).
* Szyfrowanie ruchu w sieci (TLS) – patrz rozdz. 2.

## 7. Generator Ofert (Faza 2) – backend

| Plik | Zawartość |
| --- | --- |
| `specyfikacje.rs` | model drzewa, CRUD, kolejność, kopiowanie, dziedziczenie materiału korpusu, głęboka kopia specyfikacji (`kopiuj_specyfikacje_w`), blokada zapisu starszych wersji (`sprawdz_zapis`) |
| `wycena.rs` | **czysta funkcja** `wylicz(&ParametryWyceny, &[SekcjaDoWyceny]) -> Wycena` + testy |
| `oferty.rs` | oferty, wersje, statusy, lista z sumą brutto, porównanie wersji |
| `roznice.rs` | porównanie dwóch specyfikacji po `ident` (do użycia też dla oferta → umowa i aneksów) |
| `szablony.rs` | szablony mebli, wstawianie, import `slowniki.json` i `szablony-mebli.json` |
| `pliki.rs` | zdjęcia w `<folder danych>/pliki/RRRR/MM/<sha256>.<jpg/png>` (deduplikacja po sha256) |
| `eksport/` | model dokumentu (`mod.rs`) → `docx.rs` (docx-rs) i `pdf.rs` (genpdf, czcionka Inter OFL w `zasoby/czcionki/`) |
| `firma.rs` | dane wykonawcy z tabeli `ustawienia` (`firma_*`), domyślna ważność i termin oferty |
| `formaty.rs` | kwoty „1 234,56 zł”, procenty, daty, ilości – jak w `src/lib/formaty.ts` |

**Wycena** (decyzje):
* Cena wpisywana na jednym poziomie wybranym w wersji: `czesc` (suma cen części), `sekcja` (cena sekcji) albo `calosc` (`cena_calosc_gr`). Ceny z innych poziomów są ignorowane (zostają w bazie).
* Tryb cen `brutto` (domyślny – oferty dla klientów indywidualnych): wpisane kwoty są brutto, netto = brutto·100/(100+VAT). Tryb `netto`: VAT doliczany.
* Rabat za całość (procent w setnych, `rabat_calosc_bp`) liczony od sumy za meble **przed** rabatem za termin; oba rabaty odejmowane od sumy za meble, wynik ≥ 0 (z ostrzeżeniem).
* Sekcje `typ = 'agd'`: suma = Σ `cena_gr × ilość` wierszy, bez rabatów, ze stawką VAT sekcji (domyślnie 23%) albo oferty; wiersze „po stronie klienta” i „poza wyceną” nie są liczone.
* Zaokrąglanie: połówki od zera, liczone na liczbach całkowitych (grosze, ilość w tysięcznych).
* Marża = meble netto po rabatach − koszt własny (AGD nie wchodzi). Ostrzeżenia (brak ceny, rabat > suma) wracają w `ostrzezenia`.

**Wersje:** nowa wersja kopiuje specyfikację (z zachowaniem `ident`), zdjęcia (odwołania do tych samych plików) i wszystkie parametry; data oferty = dziś. Zmieniać można tylko najnowszą wersję – starsze są zamrożone (API zwraca 422).

**Materiał korpusu:** wiersze „Półki”, „Cokół”, „Plecy” (`RODZAJE_Z_MATERIALEM_KORPUSU`) dodane bez materiału dostają materiał pierwszego „Korpusu” części i flagę `material_z_korpusu`; zmiana korpusu aktualizuje wiersze z flagą; ręczna zmiana materiału flagę zdejmuje. „Kopiuj materiał w dół” kopiuje materiał wiersza do kolejnych wierszy tej samej części i podgrupy, które są puste albo mają flagę.

**Eksport:** układ OFERTA → Inwestor | Wykonawca → data, termin realizacji, inwestycja, zdanie o ważności cen → numerowane sekcje, w każdej części tabela Rodzaj | Materiał | Opis | Ilość (+ podgrupy Frezowanie/Wyposażenie; AGD z kolumną Wartość), „Suma za meble” pod sekcją → podsumowanie (suma, rabaty, AGD, SUMA brutto, stawka VAT) → klauzule → zdjęcia. Status wiersza dopisywany do opisu („(do potwierdzenia)”). Koszt własny i marża nigdy nie trafiają do dokumentu. Dane wykonawcy: *Oferty → Dane firmy w ofertach* (`PUT /ustawienia/firma`). Znane ograniczenia: PDF ma ~2,5 MB (genpdf osadza pełne czcionki), nagłówek tabeli może zostać sam na końcu strony.

### Import słowników i szablonów

Pliki użytkownika (np. `program-dane/slowniki.json`, `szablony-mebli.json`) importuje się w module *Słowniki*. Import jest addytywny (bez duplikatów: porównanie wartości bez wielkości liter + producent + dekor). Fikcyjne przykłady formatu: `crates/robinwood-serwer/tests/przyklady/`.

`slowniki.json` – rozpoznawane klucze (format „analityczny”, wyprowadzony z dotychczasowych dokumentów):

| Klucz | → słownik |
| --- | --- |
| `rodzaje_elementow[].nazwa` (+ `grupa_dominujaca: "wyposazenie"` → podgrupa) | `rodzaj_elementu` |
| `materialy[].kategoria` | nowa kategoria materiału (jeśli brak) |
| `materialy[].typowe_opisy_materialu[].zapis` | `material` (z kategorią) |
| `materialy[].dekory_kolory[]` (`nazwa_najpelniejsza`, `producent`, `kod`) | `dekor` |
| `okucia_wyposazenie.wg_producenta[].systemy[].przykladowe_zapisy[]`, `bez_producenta[].zapis`, `uchwyty_zapisy[].zapis` | `okucie` |
| `oswietlenie_wlaczniki.zapisy[].wartosc` | `oswietlenie` |
| `opis_i_statusy.typowe_wartosci_opisu[].wzorzec` | `opis` |
| `sprzet_agd.rodzaje{}` / `marki{}` (klucze) | `agd` / `agd_marka` |
| `slowniki: { "<kod>": ["wartość", {"wartosc", "producent", "dekor", "kategoria", "podgrupa"}] }` | format prosty – dowolny istniejący słownik |

`szablony-mebli.json` – `szablony[]` w formacie analitycznym (`typ_zabudowy`, `przykladowe_tytuly_sekcji`, `typowe_czesci[].czesc`, `wiersze_materialowe[]`, `wyposazenie[]`; wiersze z `udzial_sekcji` < 0,3 pomijane, półki/cokół/plecy bez materiału – dziedziczą z korpusu) albo wewnętrznym (`kod`, `nazwa`, `tytul_sekcji`, `czesci[]`, `wiersze[]`). Szablon o tym samym kodzie jest nadpisywany.

## 8. Wspólny edytor specyfikacji – jak użyć w Generatorze Umów

`src/moduly/specyfikacja/` (eksport w `index.ts`):

```tsx
import { EdytorSpecyfikacji } from "../specyfikacja";

<EdytorSpecyfikacji
  specyfikacja={spec}                 // z GET /specyfikacje/{id} albo z odpowiedzi umowy
  onZmiana={(s) => ustawSpec(s)}       // nowe drzewo po każdej zmianie (serwer zwraca całość)
  tylkoDoOdczytu={umowa.status !== "szkic"}   // opcjonalnie: dodatkowa blokada
  panel={({ wybor, sekcja, czesc }) => <PanelUmowy … />}  // opcjonalna trzecia kolumna (np. płatności)
/>
```

* Edytor sam pobiera podpowiedzi (`/slowniki-podpowiedzi`) i szablony; rozmawia wyłącznie z API specyfikacji, więc działa dla każdej specyfikacji (umowa, aneks).
* Umowa z oferty: po stronie serwera skopiuj specyfikację wersji oferty funkcją `specyfikacje::kopiuj_specyfikacje_w(&mut tx, wersja.specyfikacja_id)` – zachowa `ident`, więc `roznice::porownaj_specyfikacje(&oferta, &umowa)` pokaże różnice oferta ↔ umowa (i umowa ↔ aneks).
* Blokada zapisu: `specyfikacje::sprawdz_zapis` blokuje dziś starsze wersje oferty; dla podpisanej umowy dopisz tam warunek (np. `umowy.status != 'szkic'`) – edytor pokaże wtedy tryb tylko do odczytu po `tylko_do_odczytu`.
* Wycena umowy może użyć `wycena::wylicz` (ta sama struktura sekcji/części) i `oferty::wycen` jako wzorca mapowania.
* Eksport umowy: model `eksport::DokumentOferty` i renderery są przykładem; tabelę specyfikacji (sekcja → część → wiersze) warto wydzielić, gdy powstanie dokument umowy.

Klawiatura w tabeli: Tab/Shift+Tab – komórki (Tab w ostatniej komórce ostatniego wiersza dodaje wiersz), Enter – zapis i nowy wiersz poniżej, Ctrl+↑/↓ – wiersz wyżej/niżej, Ctrl+D – kopiuj materiał w dół, Ctrl+Delete – usuń wiersz, Esc – cofnij zmiany w wierszu. W drzewie: F2 – zmień nazwę, Alt+↑/↓ – przesuń, Delete – usuń, ←/→ – zwiń/rozwiń. Wiersz zapisuje się po opuszczeniu go (fokus poza wierszem) albo Enterem.

## 9. Wygląd: tokeny i komponenty (`src/ui/`)

* `tokeny.css` – zmienne CSS: neutralna ciepła szarość (`--tlo-*`, `--linia*`, `--tekst*`), ciemny pasek boczny, akcent `--akcent: #2D5E50`, stany ostrzeżeń `--stan-spokojnie/zolty/pomaranczowy/czerwony` (+ `-tlo`), kolory zadań kalendarza `--zadanie-*` (zarezerwowane), typografia Inter (`@fontsource-variable/inter`, offline) z zastępczym Segoe UI: H1 24, H2 18, H3 15, tekst 14, tabela 13, etykieta 12, mikro 11; promienie `--r-4/6/8/12`; odstępy `--o-2 … --o-48`; szerokości układu (`--szer-drzewa` 300, `--szer-panelu` 380; poniżej 1500 px węższe).
* Komponenty (`src/ui/index.ts`): `Przycisk` (główny/drugorzędny/tekstowy/niebezpieczny, mały, sama ikona z etykietą dostępności), `Pole` (etykieta, podpowiedź, błąd z ikoną), `EtykietaStatusu` (ikona + tekst, stany), `WierszTabeli` (zaznaczenie, klikalny z klawiatury), `ElementDrzewa` (poziom, zwijanie, akcje przy najechaniu), `Segment` (radiogroup ze strzałkami), `Okno` (dialog, Esc).
* Statusy oferty: `src/moduly/oferty/statusy.ts`; statusy wiersza: ikony w `IKONY_STATUSU` (edytor specyfikacji).
* Ekran oferty: trzy kolumny (drzewo ~300 px | tabela | panel ~380 px), w nagłówku PDF / DOCX / Wyślij e-mail (nieaktywny) / Utwórz umowę (nieaktywny). Docelowo 1920×1080, sprawdzone także przy 1366×768 (body `min-width: 1280px`).
