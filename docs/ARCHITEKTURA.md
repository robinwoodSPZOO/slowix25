# Architektura programu ROBINWOOD

Dokument dla kolejnych agentów i programistów. Stan: gałąź `integracja-1` = Faza 0 (szkielet) + Faza 2 (Generator Ofert z poprawkami po recenzji, rozdz. 7–9) + Faza 4a (silnik terminów, rozdz. 10). Wspólne konwencje błędów, transakcji i migracji: rozdz. 3–5.

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

**Migracje** (jednoznaczna, rosnąca numeracja dla nowej bazy; test `tests/integracja.rs::migracje_na_pustej_bazie_i_spojny_schemat` sprawdza kolejność 0001…000N bez luk, `PRAGMA foreign_key_check` i poprawność kluczy obcych):

| Nr | Plik | Gałąź | Zawartość |
| --- | --- | --- | --- |
| 0001 | `0001_schemat_poczatkowy.sql` | Faza 0 | cały schemat (nie zmieniać) |
| 0002 | `0002_oferty_specyfikacja.sql` | Faza 2 | `ident`, ceny AGD, szablony mebli, słowniki, dane firmy |
| 0003 | `0003_silnik_terminow.sql` | Faza 4a | `zadania_kalendarza.klucz`, migawka terminów w `zamowienia_materialow`, ustawienia odstępów |
| 0004 | `0004_silnik_terminow_poprawki.sql` | Faza 4a | `zlecenia.harmonogram_nieaktualny_powod`, `dziennik_zmian.opis`, indeksy dat |
| 0005 | `0005_oferty_poprawki.sql` | Faza 2 (poprawki) | `wersja`/`zmieniono` rekordów specyfikacji i wersji oferty, `wersje_oferty.migawka`/`wyslano`, podglądy plików, AGD 23% |

Kolejne migracje: `0006_…` itd. Program nie ma jeszcze wdrożeń, więc bazy deweloperskie sprzed integracji należy odtworzyć.

Kluczowa decyzja: **specyfikacja jest osobnym obiektem**, do którego wskazuje wersja oferty, umowa i aneks. Przejście oferta → umowa = skopiowanie specyfikacji (`specyfikacje.zrodlo_id` pamięta źródło), co pozwala pokazać różnice.

## 4. API

Prefiks `/api/v1`. **Błędy: zawsze JSON `{ "blad": "komunikat po polsku" }`** (401/404/409/413/422/500/503), także dla błędów spoza kodu tras (nieznana trasa, zła metoda, za duża treść, błędny JSON) – patrz rozdz. 5, zasady 11–12. Dodatkowe pola: `kod` (`konflikt`, `bledy_pozycji`, `zajety`), `konflikty` + `aktualny` (409), `bledy: [{pozycja, pole, powod}]` (422 importu).

Kody 409: `konflikt` (równoczesna edycja tego samego pola – zmiany częściowe, rozdz. 4 niżej) i `do_potwierdzenia` (zamówienie materiału zablokowanego wierszami „do potwierdzenia” bez `mimo_blokady`, rozdz. 10.4). 503 `zajety` – baza chwilowo zajęta (ponów).

| Metoda i ścieżka | Opis |
| --- | --- |
| `GET /status` | nazwa serwera, wersja, `wersja_api` (bez klucza) |
| `GET /klienci?szukaj=` | lista (nazwa, e-mail, telefon) |
| `POST /klienci` | dodanie → 201 |
| `GET/PUT/DELETE /klienci/{id}` | odczyt, pełna zmiana, usunięcie miękkie → 204 |
| `GET /slowniki`, `GET /slowniki/{kod}` | słowniki i ich pozycje |
| `GET /kategorie-materialow` | kategorie z widełkami dostaw |
| `GET /zdarzenia?klucz=` | WebSocket: `{"zasob":"klienci","id":1,"operacja":"zmiana"}`; `zasob:"*"` = odśwież wszystko |
| `POST /klienci/{id}/przywroc` | cofnięcie usunięcia (miękkiego) klienta |
| Kalendarz, zlecenia, materiały, czasy dostaw | patrz rozdz. 10 „Silnik terminów” |

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
| `PUT/DELETE /zdjecia/{id}`, `GET /pliki/{id}?podglad=1` | podpis, odpięcie; plik albo pomniejszony podgląd JPEG (klucz tylko w nagłówku – interfejs pobiera obrazki przez `fetch` i `blob:`) |
| `GET /oferty?status=&klient_id=&szukaj=&limit=&od=`, `POST /oferty` | lista (najnowsza wersja + suma brutto lub `blad_wyceny`, `klient_usuniety`; stała liczba zapytań, domyślnie 200 pozycji), nowa oferta z v1 |
| `GET/PUT/DELETE /oferty/{id}`, `PUT /oferty/{id}/status` | szczegóły, zmiana inwestora, usunięcie, status |
| `POST /oferty/{id}/wersje` `{z_wersji_id?}` | nowa wersja = kopia (specyfikacja + parametry) |
| `GET /oferty/{id}/porownanie?z=&do=` | różnice między wersjami |
| `GET/PUT /wersje-oferty/{id}` | nagłówek, parametry wyceny, **wycena**, specyfikacja |
| `GET /wersje-oferty/{id}/eksport/{pdf,docx}?zdjecia=0/1` | dokument oferty |
| `GET /szablony-mebli`, `GET /slowniki-podpowiedzi` | szablony; wszystkie słowniki jednym zapytaniem |
| `POST /import/slowniki`, `POST /import/szablony-mebli` | import plików użytkownika (JSON w treści) |
| `GET/PUT /ustawienia/firma` | dane wykonawcy i domyślne wartości oferty |

Wszystkie zmiany specyfikacji zwracają `{id, specyfikacja}` – całe aktualne drzewo – i rozsyłają zdarzenie `specyfikacje` (+ zasób właściciela: `oferty` / `umowy`, pole `wlasciciel.zasob`).

**Zmiany częściowe (od poprawek Fazy 2):** `PUT /sekcje|czesci|wiersze/{id}` i `PUT /wersje-oferty/{id}` przyjmują TYLKO zmienione pola (`null` = wyczyść) oraz opcjonalnie `poprzednie` – wartości widziane przez stanowisko przed edycją. Gdy ktoś inny zmienił w międzyczasie to samo pole, serwer zwraca **409** `{blad, kod:"konflikt", konflikty:[{pole, poprzednia, twoja, aktualna}], aktualny}`; zmiany innych pól nie są konfliktem. Nieznane pola → 422. Rekordy mają licznik `wersja` i znacznik `zmieniono`. Kolejność (`…/kolejnosc`) wymaga pełnej listy bez powtórzeń (dodatkowe id z innego rodzica = przeniesienie).

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
6. Kolory typów zadań: tokeny `--zadanie-montaz/-warsztat/-zamowienie/-rysunki/-pomiar` (+ pomocnicze `-platnosc/-klient/-inne`) w `src/ui/tokeny.css` (montaż #0072B2, warsztat #8A5A2B, zamówienie #00896A, rysunki #B8578F, pomiar #2B8FCB). Są zarezerwowane – nie używać ich do niczego innego. `src/lib/kolory.ts` mapuje `zadania_kalendarza.typ` na te zmienne.
7. Wygląd: tokeny i podstawowe komponenty w `src/ui/` (rozdz. 9). Nowe ekrany składamy z nich; statusy i stany zawsze ikoną + tekstem (lucide-react).
8. Interfejs i komunikaty po polsku.
9. **Repozytorium jest publiczne**: żadnych danych klientów, przykładowych umów/ofert z prawdziwymi danymi, numerów kont, kluczy API. Dane testowe – wyłącznie fikcyjne („Klient Testowy”, `example.com`). Generowanie DOCX z szablonu umowy: szablon z danymi firmy trzymać poza repo (np. w folderze danych serwera) albo upewnić się, że nie zawiera danych wrażliwych.
10. Przed commitem: `npm run typecheck && npm run lint && npm test && cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`.
11. **Transakcje zapisu**: każdą transakcję, która cokolwiek zapisuje, otwieraj przez `baza::transakcja_zapisu(&pula)` (`BEGIN IMMEDIATE`). Zwykłe `pula.begin()` (DEFERRED: odczyt → zapis) w trybie WAL kończy się natychmiastowym „database is locked” przy równoczesnych zapisach z kilku stanowisk. Pilnuje tego test `tests/spojnosc_kodu.rs`; test równoległości na pliku bazy: `tests/api_oferty_poprawki.rs::rownolegle_zapisy_bez_database_is_locked`.
12. **Wejście i błędy**: w trasach używaj ekstraktorów `crate::wejscie::{Cialo, Sciezka, Parametry, Tresc}` (błędy JSON, ścieżki, parametrów i limitu rozmiaru → `{"blad"}` po polsku), a błędy zwracaj jako `BladApi`. Warstwa `bledy::ujednolic_bledy` zamienia każdą inną odpowiedź błędu na JSON i dodaje `X-Content-Type-Options: nosniff`. Limity danych: `limity.rs`.

## 6. Otwarte kwestie

* Decyzje użytkownika z planu (rozdz. 10): punkt odniesienia zamówień, dni robocze czy kalendarzowe (ustawienie `liczenie_dni`, domyślnie „robocze”), lista kategorii z czasami dostaw, format numeracji umów (przyjęto `NN/RRRR`).
* Kopie zapasowe bazy (nocna kopia `VACUUM INTO` na drugi dysk / do chmury) – do zrobienia w fazie „Sieć”.
* Przechowywanie plików: zdjęcia z podglądami i sprzątaniem plików bez odwołań (`pliki.rs`); do dołożenia kopia zapasowa folderu `pliki/` razem z bazą.
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
| `pliki.rs` | zdjęcia w `<folder danych>/pliki/RRRR/MM/<sha256>.<jpg/png>` + podgląd `pliki/podglad/<sha>.jpg`; limity ≤ 12 000 px na bok i ≤ 50 Mpx (sprawdzane z nagłówka przed dekodowaniem), pełne dekodowanie przy przesłaniu (uszkodzony plik → 422), zapis atomowy bez sierot, deduplikacja po sha256, sprzątanie plików bez odwołań po usunięciu oferty/zdjęcia |
| `zmiany.rs`, `limity.rs`, `wejscie.rs` | zmiany częściowe i konflikty; limity danych; ekstraktory z błędami JSON po polsku |
| `eksport/` | model dokumentu (`mod.rs`) → `docx.rs` (docx-rs) i `pdf.rs` (genpdf, czcionka Inter OFL w `zasoby/czcionki/`) |
| `firma.rs` | dane wykonawcy z tabeli `ustawienia` (`firma_*`), domyślna ważność i termin oferty |
| `formaty.rs` | kwoty „1 234,56 zł”, procenty, daty, ilości – jak w `src/lib/formaty.ts` |

**Wycena** (decyzje):
* Cena wpisywana na jednym poziomie wybranym w wersji: `czesc` (suma cen części), `sekcja` (cena sekcji) albo `calosc` (`cena_calosc_gr`). Ceny z innych poziomów są ignorowane (zostają w bazie).
* Tryb cen `brutto` (domyślny – oferty dla klientów indywidualnych): wpisane kwoty są brutto, netto = brutto·100/(100+VAT). Tryb `netto`: VAT doliczany.
* Rabat za całość (procent w setnych, `rabat_calosc_bp`) liczony od sumy za meble **przed** rabatem za termin; oba rabaty odejmowane od sumy za meble, wynik ≥ 0 (z ostrzeżeniem).
* Sekcje `typ = 'agd'`: suma = Σ `cena_gr × ilość` wierszy, bez rabatów, ze stawką VAT sekcji – serwer ustawia 23% dla AGD bez stawki (także przy zmianie typu); w panelu wyceny można wybrać 8/23%. Wiersze „po stronie klienta” i „poza wyceną” nie są liczone.
* Limity wejścia (`limity.rs`, te same w `src/lib/formaty.ts`): kwoty ≤ 1 000 000 000,00 zł, ilość ≤ 1 000 000, długości tekstów. Wycena liczy z kontrolą przepełnienia; błędy blokujące dokument (przepełnienie, **rabaty większe niż suma**) trafiają do `wycena.bledy` – eksport zwraca wtedy 422.
* Zaokrąglanie: połówki od zera, liczone na liczbach całkowitych (grosze, ilość w tysięcznych).
* Marża = meble netto po rabatach − koszt własny (AGD nie wchodzi). Ostrzeżenia (brak ceny, rabat > suma) wracają w `ostrzezenia`.

**Wersje:** nowa wersja kopiuje specyfikację (z zachowaniem `ident`), zdjęcia (odwołania do tych samych plików) i parametry; data oferty = dziś, ważność cen = dziś + `oferta_waznosc_dni`; oferta wraca do statusu `szkic`. Zmieniać można tylko najnowszą wersję **w szkicu**: przy wyjściu ze szkicu (wysłana, zaakceptowana, odrzucona, wygasła) wersja dostaje **migawkę** danych inwestora i wykonawcy (`wersje_oferty.migawka`, `wyslano`), której używa eksport, i staje się tylko do odczytu (422 przy próbie zmiany; `wersja.tylko_do_odczytu`, `powod_blokady`); powrót do `szkic` jest zabroniony – zmiany przez „Nowa wersja”. Oferty klienta usuniętego (miękko) są oznaczone i nie można tworzyć ich nowych wersji, dopóki klient nie zostanie przywrócony.

**Właściciel specyfikacji** (`specyfikacje::wlasciciel`): wersja oferty, umowa albo aneks – każdy z własną regułą blokady (umowa/aneks: status ≠ `szkic`) i zasobem zdarzenia. Generator Umów dopisuje tam tylko warunki, jeśli będą inne. Zmiana statusu wiersza z/na „do potwierdzenia” (także dodanie/usunięcie takiego wiersza, sekcji, części) wywołuje w tej samej transakcji punkt zaczepienia `po_zmianie_statusu_wiersza` (używany przez kalendarz).

**Materiał korpusu:** wiersze „Półki”, „Cokół”, „Plecy” (`RODZAJE_Z_MATERIALEM_KORPUSU`) dodane bez materiału dostają materiał pierwszego „Korpusu” części i flagę `material_z_korpusu`; zmiana korpusu aktualizuje wiersze z flagą; ręczna zmiana materiału flagę zdejmuje. „Kopiuj materiał w dół” kopiuje materiał wiersza do kolejnych wierszy tej samej części i podgrupy, które są puste albo mają flagę.

**Eksport:** układ OFERTA → Inwestor | Wykonawca → data, termin realizacji, inwestycja, zdanie o ważności cen → numerowane sekcje, w każdej części tabela Rodzaj | Materiał | Opis | Ilość (+ podgrupy Frezowanie/Wyposażenie; AGD z kolumną Wartość), „Suma za meble” pod sekcją → podsumowanie (suma, rabaty, AGD, SUMA brutto, stawka VAT) → klauzule → zdjęcia. Status wiersza dopisywany do opisu („(do potwierdzenia)”). Koszt własny i marża nigdy nie trafiają do dokumentu. Dane wykonawcy: *Oferty → Dane firmy w ofertach* (`PUT /ustawienia/firma`). PDF: własny element tabeli (`eksport/pdf.rs`) – wiersz nie dzieli się między strony, nagłówek tabeli i tytuł części („(cd.)”) powtarzają się na każdej stronie, nagłówek podgrupy trzyma się z pierwszym wierszem; czcionki to **podzbiór Inter** (fontTools, łacina A/B), bez profilu ICC – typowa oferta ~0,3–0,4 MB. Rozważono genpdfi (fork z podzbiorem czcionek w locie) – statyczny podzbiór wystarcza bez zmiany biblioteki. DOCX: czcionka Calibri (zastępczo Arial; LibreOffice – Carlito). Obrazy w dokumentach – z podglądu JPEG ≤ 1600 px, dekodowane w `spawn_blocking`.

### Import słowników i szablonów

Pliki użytkownika (np. `program-dane/slowniki.json`, `szablony-mebli.json`) importuje się w module *Słowniki*. Import jest addytywny (bez duplikatów: porównanie wartości bez wielkości liter + producent + dekor); istniejące klucze są wczytywane raz do pamięci, nowe pozycje wstawiane partiami (4000 pozycji – ułamek sekundy); limit 8 MB i 20 000 pozycji na plik. **Szablony:** plik jest sprawdzany w całości przed zapisem i przy błędach odrzucany (422 z listą `pozycja / pole / powód`, widoczną w module Słowniki) – częściowy import szablonu nadpisałby wbudowany szablon o tym samym kodzie z „zgubionymi” wierszami. Fikcyjne przykłady formatu: `crates/robinwood-serwer/tests/przyklady/`.

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
* Blokada zapisu: `specyfikacje::wlasciciel` zna już wersje ofert, **umowy i aneksy** (umowa/aneks poza statusem `szkic` → tylko do odczytu z powodem w `wlasciciel.powod_blokady`); edytor pokazuje powód i opcjonalną akcję (`akcjaBlokady`, np. „Utwórz aneks”). Zdarzenia po zmianie specyfikacji idą do zasobu właściciela (`umowy`) oraz `zlecenia`/`zadania_kalendarza`.
* Zapis szkiców przy zamykaniu okna: edytor rejestruje się w `src/lib/szkice.ts` – nic więcej nie trzeba robić; inne formularze (np. umowy) mogą zarejestrować własne `oproznij`.
* Zdjęcia: `PanelZdjec` i `useObrazek` z `src/moduly/specyfikacja` (obrazki przez `api.pobierzPlik` → `blob:`).
* Wycena umowy może użyć `wycena::wylicz` (ta sama struktura sekcji/części) i `oferty::wycen` jako wzorca mapowania.
* Eksport umowy: model `eksport::DokumentOferty` i renderery są przykładem; tabelę specyfikacji (sekcja → część → wiersze) warto wydzielić, gdy powstanie dokument umowy.

Klawiatura w tabeli: Tab/Shift+Tab – komórki (Tab w ostatniej komórce ostatniego wiersza dodaje wiersz), Enter – zapis i nowy wiersz poniżej, Ctrl+↑/↓ – **przejście** do wiersza wyżej/niżej, Alt+↑/↓ – **przesunięcie** wiersza, Ctrl+D – kopiuj materiał w dół, Ctrl+Delete – usuń wiersz (pasek „Cofnij” przez 8 s), Esc – odrzuć niezapisane zmiany wiersza. Wiersze można też przeciągać za uchwyt i przenosić do innej części („Przenieś do…”); części – do innej sekcji (drzewo). W drzewie: F2 – zmień nazwę, Alt+↑/↓ – przesuń, Delete – usuń, ←/→ – zwiń/rozwiń.

**Zapis i szkice:** wiersz zapisuje się ~1 s po ostatnim naciśnięciu klawisza, po opuszczeniu wiersza, Enterem, przy ukryciu/zamknięciu karty (`visibilitychange`/`pagehide`, `fetch keepalive`) i w Tauri przed schowaniem okna i „Zakończ” (zdarzenia `robinwood://przed-zamknieciem` / `przed-zakonczeniem` → interfejs opróżnia szkice → `ukryj_glowne_okno` / `zakoncz_aplikacje`; zapas 4–5 s). Szkice mają kopię w localStorage (odtwarzana po starcie). Wskaźnik nad tabelą: „Zapisywanie… / Zapisano / Niezapisane (n)”. Konflikt 409 → okno „Zachowaj moją / Przyjmij cudzą”. Błędne wartości (ilość, kwota, rabat %) zostają w polu z komunikatem i nie są wysyłane. Rejestr szkiców: `src/lib/szkice.ts` (inne edytory, np. umowy, rejestrują tam swoje `oproznij`).

## 9. Wygląd: tokeny i komponenty (`src/ui/`)

* `tokeny.css` – zmienne CSS: neutralna ciepła szarość (`--tlo-*`, `--linia*`, `--tekst*`), ciemny pasek boczny, akcent `--akcent: #2D5E50`, stany ostrzeżeń `--stan-spokojnie/zolty/pomaranczowy/czerwony` (+ `-tlo`), kolory zadań kalendarza `--zadanie-*` (zarezerwowane), typografia Inter (`@fontsource-variable/inter`, offline) z zastępczym Segoe UI: H1 24, H2 18, H3 15, tekst 14, tabela 13, etykieta 12, mikro 11; promienie `--r-4/6/8/12`; odstępy `--o-2 … --o-48`; szerokości układu (`--szer-drzewa` 300, `--szer-panelu` 380; poniżej 1500 px węższe).
* Komponenty (`src/ui/index.ts`): `Przycisk` (główny/drugorzędny/tekstowy/niebezpieczny, mały, sama ikona z etykietą dostępności), `Pole` (etykieta, podpowiedź, błąd z ikoną), `EtykietaStatusu` (ikona + tekst, stany), `WierszTabeli` (zaznaczenie, klikalny z klawiatury), `ElementDrzewa` (poziom, zwijanie, akcje przy najechaniu), `Segment` (radiogroup ze strzałkami), `Okno` (dialog, Esc).
* Statusy oferty: `src/moduly/oferty/statusy.ts`; statusy wiersza: ikony w `IKONY_STATUSU` (edytor specyfikacji).
* Ekran oferty: trzy kolumny (drzewo | tabela | panel); szerokości drzewa i panelu zależą od szerokości edytora (**container queries**, `clamp(…cqi…)`), a nie od szerokości okna – przy skalowaniu Windows 125% (1536×864 CSS) tabela zachowuje Opis ≥ 200 px i pełne nazwy statusów. Drzewo i panel można zwinąć (przyciski nad tabelą, pamiętane w localStorage); poniżej 1440 px szerokości okna panel startuje zwinięty. Sprawdzone zrzutami przy 1920×1080, 1536×864 (125%), 1366×768 i 1280×720. Wspólne zdjęcia: `src/moduly/specyfikacja/PanelZdjec.tsx`, okno konfliktu `OknoKonfliktu.tsx`.

## 10. Silnik terminów (Faza 4a)

Kod: `crates/robinwood-serwer/src/kalendarz/` (czysta logika, bez bazy i HTTP, testy jednostkowe),
`src/terminarz.rs` (baza), `src/api_kalendarz.rs` (trasy), migracje `0003_silnik_terminow.sql` i
`0004_silnik_terminow_poprawki.sql` (patrz rozdz. 3 „Migracje”). Transakcje zapisu – `baza::transakcja_zapisu`
(rozdz. 5, zasada 11); przeliczenie z punktem zapisu (`SAVEPOINT`) wewnątrz tej transakcji.

### 10.1. Reguły (liczone wstecz od terminu montażu z umowy)

| Krok | Wzór (`wstecz(D, N)` – patrz 10.2) |
| --- | --- |
| Montaż | od `montaz_od`, trwa `czas_montazu_dni` (≥ 1); ostatni dzień = N-ty dzień od startu włącznie |
| Składanie na warsztacie | `warsztat_od = wstecz(montaz_od, czas_warsztat_dni)`, `warsztat_do = wstecz(montaz_od, 1)` (czas 0 → brak zadania, ostrzeżenie) |
| Data wymaganej dostawy | `W = wstecz(warsztat_od, odstep_materialy)` (domyślnie 7) |
| Zamówienie materiału | sweet spot `= wstecz(W, max)`, planowa `= wstecz(W, średnia)`, krytyczna `= wstecz(W, min)` |
| Rysunki wykonawcze | `wstecz(najwcześniejsza planowa w zleceniu, odstep_rysunki)`; bez materiałów z widełkami – od `W` (ostrzeżenie `brak_materialow`) |
| Pomiar końcowy | `wstecz(rysunki, odstep_pomiar)` |
| Potwierdzenie terminu montażu klientowi | `wstecz(montaz_od, odstep_potwierdzenie_montazu_dni)` (7), typ `klient` |
| Pomieszczenie gotowe | `wstecz(montaz_od, odstep_pomieszczenie_gotowe_dni)` (3), typ `klient` |
| Brakujące informacje od klienta | `umowy.data_zawarcia + umowy.dni_na_brakujace_informacje` (liczone naprzód, patrz 10.2), typ `klient` |
| Płatności | `platnosci_planowane.termin` umowy zlecenia, typ `platnosc` |
| Potwierdzenie pozycji „do potwierdzenia” | data planowa zamówienia, typ `klient` (tylko gdy zamówienie jest zablokowane) |

* **Średnia z widełek zaokrąglana w dół**: `(min + max) / 2` całkowite (5–10 → 7, 18–24 → 21). Daje późniejszą
  datę planową; margines bezpieczeństwa zapewnia sweet spot.
* **Źródło widełek**: para kategoria + dostawca (`czasy_dostaw`; dostawca materiału, a gdy pusty – domyślny
  dostawca kategorii), inaczej domyślne widełki kategorii. **Widełki kategorii 0–0 oznaczają „nie ustawiono”**
  (tak są zasiane w migracji 0001) → materiał bez terminów + ostrzeżenie `brak_widelek`; dla materiału „z
  magazynu” (0 dni) trzeba założyć parę kategoria + dostawca 0–0. `min > max` → ostrzeżenie `nieprawidlowe_widelki`.
* Nadpisania odstępów w zleceniu (`zlecenia.odstep_*_dni`) mają pierwszeństwo przed ustawieniami.
* **Zamrażanie**: zadanie wykonane (`wykonano` ≠ NULL) i zadanie ręczne (`wyliczone = 0`, `klucz` NULL) nie są
  zmieniane przy przeliczeniu. Niewykonane zadania wyliczone dostają nowe daty (zachowują `id`), zadania materiałów
  usuniętych ze zlecenia znikają (wykonane zostają jako historia). Zapisywane i liczone (`zmienione`, dziennik) są
  tylko **rzeczywiste** zmiany – przeliczenie bez różnic daje `dodane = zmienione = usuniete = 0`.
  Identyfikacja zadania: `zadania_kalendarza.klucz`
  (`montaz`, `warsztat`, `rysunki`, `pomiar`, `zamowienie:<id materiału>`, `potwierdzenie:<id materiału>`,
  `potwierdzenie_montazu`, `pomieszczenie_gotowe`, `brakujace_informacje`, `platnosc:<id>`).
* **Migawka w `zamowienia_materialow`**: dla materiałów niezamówionych aktualizowane są wszystkie terminy i widełki.
  Dla zamówionych daty sweet spot / planowa / krytyczna i widełki są zamrożone, ale **`data_wymaganej_dostawy` zawsze
  wynika z aktualnego harmonogramu** – od niej liczy się `dostawa_zagrozona` (oczekiwana dostawa = data zamówienia +
  średni czas, później niż wymagana). Przykład: zamówione 27.12.2026, średnia 21 dni (oczekiwana 17.01.2027), montaż
  przesunięty z 8.02 na 1.02.2027 → wymagana 11.01, `dostawa_zagrozona: true`.
* **Stare zadania `wyliczone = 1` bez klucza** (sprzed migracji 0003 albo wstawione przez inne moduły): przy
  pierwszym przeliczeniu są dopasowywane do wyliczonych zadań – po typie (`montaz`, `warsztat`, `rysunki`,
  `pomiar`), po materiale (`zamowienie`, `klient` z `zamowienie_id`) albo po typie i tytule – i dostają klucz
  (zachowują `id` i stan wykonania, nie powstaje duplikat). Niedopasowane stają się ręcznymi (`wyliczone = 0`).
  Wiersze bez klucza można zmieniać i usuwać jak ręczne (zmiana ustawia `wyliczone = 0`). Każda taka zmiana trafia do
  dziennika z opisem. Uzasadnienie: dopasowanie zachowuje historię wykonania i odnośniki, a zamiana na ręczne nie
  kasuje niczego bez wiedzy użytkownika.
* **Kiedy przelicza się automatycznie**: po zmianie zlecenia lub jego materiałów (jeśli zlecenie ma już harmonogram),
  po cofnięciu wykonania oraz po zmianie ustawień, dni wolnych, widełek kategorii i czasów dostaw (wszystkie zlecenia z
  harmonogramem). **Zlecenia w etapach po montażu (`odbior`, `gwarancja`, `zakonczone`) nie są przeliczane
  automatycznie** (ani przy zmianie ustawień, ani przy zmianie samego zlecenia); jawne `POST /zlecenia/{id}/harmonogram`
  działa zawsze. Pierwsze wygenerowanie – jawnie `POST /zlecenia/{id}/harmonogram`.
* **Jedna transakcja**: zapis danych i wywołane nim przeliczenie (jednego albo wszystkich zleceń) idą w jednej
  transakcji. Błąd bazy wycofuje całość (500 – nic nie zapisano). Przeliczenie nieudane z powodu danych (np. `PUT
  /zlecenia/{id}` bez `montaz_od`) wycofuje tylko częściowe zmiany zadań (SAVEPOINT): zapis danych się udaje (200), stare
  zadania zostają, a zlecenie dostaje **znacznik `harmonogram_nieaktualny`** z powodem
  (`zlecenia.harmonogram_nieaktualny_powod`). Udane przeliczenie zdejmuje znacznik. Ustawienie i zdjęcie znacznika
  trafia do dziennika.
* **Blokada „do potwierdzenia”**: wiersz specyfikacji umowy zlecenia (`umowy.specyfikacja_id`) ze statusem
  `do_potwierdzenia` i `kategoria_materialu_id` = kategoria materiału blokuje jego zamówienie: `zablokowane: true`,
  `potwierdzic_do` = data planowa, `wiersze_do_potwierdzenia` = id wierszy. Kolumny
  `wiersze_specyfikacji.termin_potwierdzenia` silnik nie zapisuje (należy do modułu specyfikacji).
  **Blokada miękka (decyzja tymczasowa)**: pierwsze zamówienie takiego materiału (`PUT …/wykonanie`, albo podanie
  `data_zamowienia` w `POST /zlecenia/{id}/materialy` / `PUT /zamowienia-materialow/{id}`) bez pola
  `"mimo_blokady": true` zwraca **409** `{"blad", "kod": "do_potwierdzenia", "wiersze_do_potwierdzenia": [id…]}` i
  niczego nie zapisuje; z tym polem zamówienie jest przyjmowane, a w `dziennik_zmian.opis` materiału zapisuje się
  „zamówiono mimo blokady – wiersze specyfikacji do potwierdzenia: …”.
* **Połączenie ze specyfikacją (zrobione w `integracja-1`)**: blokady są czytane na bieżąco, a zadania
  `potwierdzenie:<id materiału>` powstają/znikają przy przeliczeniu. Moduł specyfikacji (`specyfikacje.rs`,
  `po_zmianie_statusu_wiersza`) wywołuje `terminarz::przelicz_po_zmianie_specyfikacji` **w tej samej transakcji
  zapisu**, gdy wiersz przechodzi z/na `do_potwierdzenia`, gdy zmienia się kategoria materiału takiego wiersza oraz przy
  dodaniu/usunięciu wiersza, części albo sekcji z takimi wierszami. Po zatwierdzeniu API rozsyła zdarzenia `zlecenia`
  i `zadania_kalendarza` (id 0) dla specyfikacji umów i aneksów. Test: `tests/integracja.rs`. Ręcznie:
  `POST /zlecenia/{id}/przelicz`.

### 10.2. Konwencja liczenia dni

Ustawienie `liczenie_dni`: `robocze` (domyślnie) albo `kalendarzowe`; dotyczy czasów dostaw, warsztatu, montażu,
odstępów i progów ostrzeżeń.

* **Dzień wolny** = sobota, niedziela, święto ustawowo wolne (Nowy Rok, Trzech Króli, Wielkanoc, Poniedziałek
  Wielkanocny, 1 i 3 maja, Zielone Świątki, Boże Ciało, 15 sierpnia, 1 i 11 listopada, **Wigilia 24.12 od 2025 r.**,
  25 i 26 grudnia) albo dzień z tabeli `dni_wolne` (każdy rodzaj: urlop, przerwa dostawców…).
* **Okres N dni od dnia X obejmuje dzień X, nie obejmuje dnia końcowego** – przedział `[X, D)`. Zamówienie złożone
  w dniu X przy dostawie N dni jest na miejscu w dniu D; dzień zamówienia jest pierwszym dniem czasu dostawy.
* `wstecz(D, N)` – termin „najpóźniej do”: dzień X taki, że `[X, D)` ma N dni.
  Tryb roboczy: N razy cofnięcie na poprzedni dzień roboczy (wynik zawsze roboczy).
  Tryb kalendarzowy: `D − N`, a jeśli to dzień wolny – **najbliższy wcześniejszy dzień roboczy**.
* `naprzod(D, N)` (dostawa, przesunięcie montażu): pierwszy dzień po N dniach okresu; w trybie roboczym przesuwany na
  najbliższy dzień roboczy (5 dni roboczych od poniedziałku → następny poniedziałek). Tryb kalendarzowy: `D + N`.
* Terminy klienta liczone naprzód od zdarzenia (brakujące informacje): tryb roboczy – N-ty dzień roboczy po dacie
  umowy; tryb kalendarzowy – `data + N`, a gdy wypada w dzień wolny – następny dzień roboczy (jak art. 115 KC).
* `dni_do_terminu` w stanach to zawsze dni **kalendarzowe** od „dziś” (do wyświetlenia „za 3 dni”, „2 dni po terminie”).
* Montaż w trybie kalendarzowym może kończyć się w dzień wolny (nie cofamy końca okresu); termin rozpoczęcia montażu
  w dzień wolny daje ostrzeżenie `montaz_w_dzien_wolny`.
* Daty wejściowe API muszą mieścić się w latach **1990–2100** (`kalendarz::ROK_MIN`/`ROK_MAX`), inaczej 422.

Przykład kontrolny (tryb kalendarzowy, montaż pon. 8.02.2027, warsztat 14 dni, 7-dniowe odstępy): warsztat
25.01–5.02, dostawa do 18.01, fronty MDF 18–24: sweet spot **23.12.2026** (18.01 − 24 = 25.12 święto → 24.12 Wigilia
→ 23.12), planowa 28.12, krytyczna 31.12; płyta 5–10: 8.01 / 11.01 / 13.01; rysunki 21.12; pomiar 14.12.2026.

### 10.3. Stany ostrzeżeń (względem `dzis`)

`stan.stan` ∈ `wykonane` < `spokojnie` < `zolty` < `pomaranczowy` < `czerwony` (kolejność = pilność; grupy w „do
zamówienia” mają stan najpilniejszej pozycji).

Niezamówiony materiał: `dzis ≤ planowa` → `spokojnie` (opis rozróżnia „przed oknem” i „w oknie”), `dzis > planowa` →
`zolty`, `dzis ≥ wstecz(krytyczna, prog_pomaranczowy_dni)` (domyślnie 2) → `pomaranczowy`, `dzis > krytyczna` →
`czerwony` z `przesuniecie_montazu_dni` = dni (w trybie liczenia) między datą wymaganej dostawy a dostawą przy
zamówieniu dziś ze średnim czasem, oraz `nowy_termin_montazu` = `naprzod(montaz_od, przesunięcie)`, a gdy wypada w dzień
wolny – **najbliższy następny dzień roboczy** (w trybie kalendarzowym; w roboczym `naprzod` daje już dzień roboczy).
Przykład: MDF z przykładu kontrolnego, dziś 2.01.2027 → przesunięcie 5 dni, 8.02 + 5 = sob. 13.02 → **15.02.2027**
(`przesuniecie_montazu_dni` zostaje 5 – minimalne przesunięcie). Wygrywa najpilniejszy spełniony warunek (w dniu
planowej: spokojnie; w dniu krytycznej: pomarańczowy; dzień po krytycznej: czerwony).

Pozostałe zadania (termin = `data_do`, a gdy brak – `data_od`): `dzis > termin` → `czerwony`,
`dzis ≥ wstecz(termin, prog_zbliza_sie_dni)` (domyślnie 3, włącznie z dniem terminu) → `zolty`, wcześniej `spokojnie`.

### 10.4. API (prefiks `/api/v1`, klucz jak w rozdz. 2)

Daty `RRRR-MM-DD` (lata 1990–2100). Parametr `dzis` (opcjonalny, domyślnie dzisiejsza data serwera) służy do
wyliczenia stanów i do kontroli „data z przyszłości” w `wykonanie` – widżet i testy mogą podać własny.

**Błędy** (zawsze JSON `{"blad": "…"}` – wspólna konwencja serwera, rozdz. 4):

| Kod | Kiedy |
| --- | --- |
| 401 | brak / zły klucz (wszystkie trasy poza `/status`) |
| 404 | brak rekordu (zlecenie, zadanie, materiał, dzień wolny…) |
| 409 | zamówienie materiału zablokowanego wierszami „do potwierdzenia” bez `mimo_blokady: true`; dodatkowo `"kod": "do_potwierdzenia"`, `"wiersze_do_potwierdzenia": [id…]` |
| 422 | błąd danych z czytelnym komunikatem (np. „zlecenie 3: uzupełnij termin rozpoczęcia montażu (montaz_od), aby policzyć harmonogram”); także **nieznane pole** w treści, **nieznany parametr** zapytania, niepoprawny JSON / typ / ścieżka (`/zlecenia/abc`), rok spoza 1990–2100, `wykonano`/daty zamówienia i dostawy z przyszłości, usunięcie zamówionego materiału, wyczyszczenie `data_zamowienia` w `PUT` materiału |
| 500 | błąd bazy – transakcja wycofana, **nic nie zapisano** |

Wszystkie struktury wejściowe odrzucają nieznane pola (np. literówka `czas_warsztatu_dni` → 422 z nazwą pola).
Parametr `klucz` jest tolerowany w zapytaniach, ale **uwierzytelnia tylko WebSocket** `/zdarzenia` – trasy HTTP wymagają nagłówka `Authorization` (rozdz. 2).

**Znacznik nieaktualnego harmonogramu** (patrz 10.1) – pola `harmonogram_nieaktualny: bool` i
`harmonogram_nieaktualny_powod: string | null` są w: `Zlecenie` (wszystkie trasy zleceń), każdym `Zadanie`
(`/kalendarz/zadania…`, `tydzien`, `do-zamowienia`, `zadania_zapisane`, odpowiedź `wykonanie`), a w `kolizje` – pole
`harmonogram_nieaktualny` (któreś ze zleceń). `tydzien` i `do-zamowienia` mają listę `nieaktualne_harmonogramy:
[{zlecenie_id, zlecenie_nazwa, powod}]` (bez etapów po montażu). Odpowiedzi tras, które przeliczają wszystkie zlecenia
(`PUT /kalendarz/ustawienia`, `POST /kalendarz/przelicz`, `PUT /kalendarz/dni-wolne/{data}`, `POST/PUT /czasy-dostaw`)
zawierają `przeliczone_zlecenia: [id]` i `nieprzeliczone_zlecenia: [{zlecenie_id, zlecenie_nazwa, powod}]`.

| Metoda i ścieżka | Opis |
| --- | --- |
| `GET/POST /zlecenia`, `GET/PUT /zlecenia/{id}` | zlecenia (`klient_id`, `umowa_id`, `nazwa`, `etap`, `montaz_od`, `czas_warsztat_dni`, `czas_montazu_dni`, `odstep_*_dni`; PUT = pełna zmiana) + `harmonogram_nieaktualny`, `harmonogram_nieaktualny_powod`; PUT zwraca też `harmonogram_przeliczony` (`false` + znacznik, gdy przeliczenie się nie udało) |
| `POST /zlecenia/{id}/przelicz` | przeliczenie istniejącego harmonogramu (np. po zmianie statusu wiersza specyfikacji); nie tworzy harmonogramu; odpowiedź jak `PUT /zlecenia/{id}` |
| `GET/POST /zlecenia/{id}/materialy` | materiały zlecenia (`kategoria_id`, `dostawca_id?`, `opis?`, `data_zamowienia?`, `data_dostawy?`, `mimo_blokady?`) z migawką terminów |
| `PUT /zamowienia-materialow/{id}` | **zmiana częściowa**: pole pominięte – bez zmian; `null` czyści `dostawca_id`, `opis`, `data_dostawy`. Zmiana opisu/kategorii/dostawcy nie rusza dat zamówienia i dostawy ani wykonania zadania. Podanie `data_zamowienia` (data) zamawia (blokada miękka, `mimo_blokady?`); `data_zamowienia: null` dla zamówionego → 422 (użyj `cofnij-zamowienie`) |
| `POST /zamowienia-materialow/{id}/cofnij-zamowienie` | jawne cofnięcie zamówienia: czyści daty zamówienia i dostawy, status `do_zamowienia`, zadanie „zamówić” znów niewykonane, przeliczenie; → `Material`; 422, gdy niezamówiony |
| `DELETE /zamowienia-materialow/{id}` | usunięcie materiału **niezamówionego** (204; materiał i jego niewykonane zadania w dzienniku, wykonane zadania – np. potwierdzenie – zostają jako historia); zamówiony/dostarczony → 422 „najpierw cofnij zamówienie” |
| `GET /zlecenia/{id}/harmonogram` | podgląd harmonogramu bez zapisu (`Harmonogram`) |
| `POST /zlecenia/{id}/harmonogram?dzis=` | wygenerowanie/przeliczenie i zapis (także w etapie po montażu); `{harmonogram, dodane, zmienione, usuniete, zamrozone, zadania_zapisane}` – liczby rzeczywistych zmian; 422 przy błędzie danych (istniejący harmonogram dostaje wtedy znacznik) |
| `POST /kalendarz/przelicz` | przeliczenie wszystkich zleceń z harmonogramem (bez etapów po montażu) → `{przeliczone_zlecenia, nieprzeliczone_zlecenia}` |
| `GET /kalendarz/zadania?od=&do=&typ=&zlecenie_id=&stan=&tylko_niewykonane=&dzis=` | zadania nachodzące na zakres; `typ` i `stan` – jedna wartość albo kilka po przecinku |
| `GET /kalendarz/zadania/{id}?dzis=` | jedno zadanie |
| `POST /kalendarz/zadania`, `PUT/DELETE /kalendarz/zadania/{id}` | zadania ręczne (`zlecenie_id?`, `typ`, `tytul`, `data_od`, `data_do?`, `uwagi?`); wyliczonych z kluczem nie można zmieniać ani usuwać (422); stare wyliczone bez klucza – można (stają się ręczne) |
| `PUT /kalendarz/zadania/{id}/wykonanie?dzis=` | `{wykonano: "2026-12-10" \| null, data_zamowienia?, data_dostawy?, uwagi?, mimo_blokady?}`. **`wykonano` wymagane** (brak pola → 422; data z przyszłości względem `dzis` → 422). Pola pominięte – **bez zmian**: `data_dostawy` pominięta zostaje (status `dostarczone` się nie cofa), `null` – czyści (→ `zamowione`); `data_zamowienia` pominięta – bez zmian (przy pierwszym zamówieniu = `wykonano`), `null` – = `wykonano`; `uwagi` pominięte – bez zmian, `null`/`""` – czyści. Pierwsze zamówienie zablokowanego materiału bez `mimo_blokady: true` → 409. `wykonano: null` cofa wykonanie (dla zamówienia także zamówienie materiału) i w tej samej transakcji przelicza zlecenie (nieudane → 200 + `harmonogram_nieaktualny`) |
| `GET /kalendarz/tydzien?dzis=` | `{od, do, dzis, zadania, nieaktualne_harmonogramy}` – niewykonane z terminem/oknem w tygodniu (pon.–niedz.) + wszystkie zaległe (czerwone) |
| `GET /kalendarz/do-zamowienia?dzis=&horyzont=` | `{dzis, grupy: [{dostawca_id, dostawca_nazwa, stan, pozycje: [Zadanie]}], bez_terminow: [Material], nieaktualne_harmonogramy}`; grupy alfabetycznie, „bez dostawcy” na końcu, pozycje wg daty planowej |
| `GET /kalendarz/kolizje?od=&do=` | `[{typ, zlecenie_a, zlecenie_b, od, do, zlecenie_a_nazwa, zlecenie_b_nazwa, harmonogram_nieaktualny}]` – nakładanie się warsztatu lub montażu |
| `GET /kalendarz/statystyka-dostaw` | rzeczywiste czasy dostaw par kategoria + dostawca: `statystyka {liczba, min, max, srednia, sugerowane_widelki}`, `aktualne_widelki`, `sugerowana_korekta` (bez automatycznego nadpisywania) |
| `GET/PUT /kalendarz/ustawienia` | `UstawieniaTerminow`; PUT częściowy, w jednej transakcji z przeliczeniem; zwraca też `przeliczone_zlecenia` i `nieprzeliczone_zlecenia` |
| `GET /kalendarz/dni-wolne?od=&do=`, `PUT/DELETE /kalendarz/dni-wolne/{data}` | dni wolne użytkownika (`{rodzaj: swieto\|urlop\|przerwa_dostawcow\|inne, opis}`); PUT zwraca `DzienWolny` + `przeliczone_zlecenia`, `nieprzeliczone_zlecenia` |
| `GET /kalendarz/swieta?rok=` | wyliczone święta ustawowe `[{data, nazwa}]`; `rok` 1990–2100, inaczej 422 |
| `GET/POST /dostawcy`, `PUT /dostawcy/{id}` | dostawcy |
| `PUT /kategorie-materialow/{id}/widelki` | `{dni_dostawy_min, dni_dostawy_max, domyslny_dostawca_id?}` → 204 (nieudane przeliczenia widać po znaczniku zleceń) |
| `GET/POST /czasy-dostaw`, `PUT/DELETE /czasy-dostaw/{id}` | widełki pary `{kategoria_id, dostawca_id, dni_min, dni_max}`; POST/PUT zwracają rekord + `przeliczone_zlecenia`, `nieprzeliczone_zlecenia` |

**Zdarzenia WebSocket**: `zlecenia`, `zamowienia_materialow`, `dostawcy`, `kategorie_materialow`, `czasy_dostaw`,
`ustawienia`, `dni_wolne` (id rekordu albo 0) oraz `zadania_kalendarza` – tu **`id` = id zlecenia**, którego zadania
się zmieniły (0 = zadanie bez zlecenia). UI kalendarza i widżet po każdym z tych zdarzeń odświeża widoczny zakres.

Przykład – `GET /kalendarz/zadania?typ=zamowienie&tylko_niewykonane=true&dzis=2027-01-22`:

```json
[{
  "id": 4, "zlecenie_id": 1, "zlecenie_nazwa": "Kuchnia testowa", "zamowienie_id": 2,
  "typ": "zamowienie", "klucz": "zamowienie:2", "tytul": "Zamówić: Płyta meblowa (Kuchnia testowa)",
  "data_od": "2027-01-15", "data_do": "2027-01-20",
  "data_sweet_spot": "2027-01-15", "data_planowa": "2027-01-18", "data_krytyczna": "2027-01-20",
  "termin": "2027-01-18", "wyliczone": true, "wykonano": null, "uwagi": null,
  "stan": {
    "stan": "czerwony",
    "opis": "Po terminie krytycznym (2027-01-20). Zamówione dziś dotrze ok. 2027-01-29 – montaż przesunie się o 4 dni (na 2027-02-19)",
    "dni_do_terminu": -2, "przesuniecie_montazu_dni": 4, "nowy_termin_montazu": "2027-02-19"
  },
  "zamowienie": {
    "kategoria_id": 1, "kategoria_nazwa": "Płyta meblowa", "dostawca_id": null, "dostawca_nazwa": null,
    "opis": null, "status": "do_zamowienia", "data_wymaganej_dostawy": "2027-01-25",
    "dni_min": 5, "dni_max": 10, "dni_srednia": 7, "data_zamowienia": null, "data_dostawy": null,
    "oczekiwana_dostawa": null, "dostawa_zagrozona": false,
    "zablokowane": false, "potwierdzic_do": null, "wiersze_do_potwierdzenia": []
  },
  "harmonogram_nieaktualny": false, "harmonogram_nieaktualny_powod": null
}]
```

Przykład – fragment `POST /zlecenia/1/harmonogram` (pole `harmonogram`):

```json
{
  "zlecenie_id": 1, "liczenie_dni": "kalendarzowe",
  "montaz_od": "2027-02-08", "montaz_do": "2027-02-10",
  "warsztat_od": "2027-01-25", "warsztat_do": "2027-02-05",
  "data_wymaganej_dostawy": "2027-01-18", "rysunki_do": "2026-12-21", "pomiar_do": "2026-12-14",
  "materialy": [{
    "material_id": 1, "kategoria_id": 4, "kategoria_nazwa": "Fronty MDF frezowane", "dostawca_id": 1,
    "widelki": {"min": 18, "max": 24}, "dni_srednia": 21, "zrodlo_widelek": "kategoria_dostawca",
    "data_wymaganej_dostawy": "2027-01-18", "data_sweet_spot": "2026-12-23",
    "data_planowa": "2026-12-28", "data_krytyczna": "2026-12-31",
    "zablokowane": false, "potwierdzic_do": null, "zamowione": false
  }],
  "zadania": [{"klucz": "pomiar", "typ": "pomiar", "tytul": "Pomiar końcowy: Kuchnia testowa",
               "data_od": "2026-12-14", "data_do": null, "zamowienie_id": null, "wykonano": null, "…": "…"}],
  "ostrzezenia": [{"kod": "brak_widelek", "komunikat": "Tapicerka: brak czasu dostawy (widełek) – …", "material_id": 3}]
}
```

Kody ostrzeżeń: `brak_widelek`, `nieprawidlowe_widelki`, `do_potwierdzenia`, `brak_materialow`,
`brak_czasu_warsztatu`, `montaz_w_dzien_wolny`, `harmonogram_przed_umowa`.

### 10.5. Wskazówki dla UI kalendarza i widżetu

* Kolor zadania = `typ` (`src/lib/kolory.ts` → tokeny `--zadanie-*`), pilność = `stan.stan` (obwódka/znacznik); `stan.opis` jest gotowym
  tekstem po polsku.
* Zamówienie ma okno `data_od` (sweet spot) – `data_do` (krytyczna) i znacznik `data_planowa` (= `termin`); w widoku
  miesiąca rysować pasek okna z wyróżnionym dniem planowym.
* Widżet: `GET /kalendarz/tydzien` + `GET /kalendarz/do-zamowienia`; powiadomienia Windows np. przy przejściu zadania
  na `pomaranczowy`/`czerwony`.
* Czerwone zamówienie: pokazać `nowy_termin_montazu` i zaproponować aneks/kontakt z klientem.
* `zamowienie.zablokowane` → pokazać „czeka na potwierdzenie klienta do {potwierdzic_do}” i link do wierszy
  specyfikacji (`wiersze_do_potwierdzenia`).
* Ekran ustawień: widełki kategorii (0–0 = brak), pary kategoria + dostawca, odstępy i progi, dni wolne, podgląd świąt;
  przy statystyce `sugerowana_korekta: true` – przycisk „przyjmij sugerowane widełki” (wysyła `PUT /czasy-dostaw/{id}`).
* `harmonogram_nieaktualny: true` (zlecenie, zadanie, kolizja) albo niepusta lista `nieaktualne_harmonogramy` /
  `nieprzeliczone_zlecenia` → pokazać ostrzeżenie „harmonogram nieaktualny: {powod}” (terminy mogą być błędne) z
  odnośnikiem do zlecenia; znacznik znika po udanym przeliczeniu.
* 409 z `kod: "do_potwierdzenia"` przy zamawianiu → okno „Klient nie potwierdził pozycji … – zamówić mimo to?”;
  „tak” = ponowne wysłanie z `"mimo_blokady": true`.
* Usunięcie zamówionego materiału: UI najpierw proponuje „Cofnij zamówienie” (`POST …/cofnij-zamowienie`).
* Edycja materiału wysyła tylko zmienione pola (PUT częściowy).
* Typy TypeScript i metody klienta (`src/api/typy.ts`, `src/api/klient.ts`) dopisuje agent UI na podstawie tego rozdziału.

### 10.6. Do decyzji użytkownika (obecne zachowanie bez zmian)

* Wigilia (24.12, od 2025 r.) liczona jako dzień wolny.
* Średnia z widełek zaokrąglana w dół (późniejsza data planowa).
* Montaż rozpoczynający się w dzień wolny – tylko ostrzeżenie `montaz_w_dzien_wolny`, termin nie jest przesuwany.
* Progi ostrzeżeń w trybie kalendarzowym cofane na dzień roboczy (np. krytyczna pon. 8.03 − 2 = sob. 6.03 → próg
  pt. 5.03).
* Definicja kolizji warsztatu (nakładanie się okresów składania dwóch zleceń, bez uwzględnienia pojemności warsztatu;
  pomijany tylko etap `zakonczone`).
* Termin rysunków liczony od najwcześniejszej daty planowej wszystkich materiałów, także już zamówionych.
