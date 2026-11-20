# Architektura programu ROBINWOOD

Dokument dla kolejnych agentów i programistów. Stan: Faza 0 (szkielet).

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
6. Kolory typów zadań: `src/lib/kolory.ts` (tymczasowe – zostaną zastąpione tokenami z Figmy).
7. Wygląd: obecny CSS (`src/styles.css`) jest tymczasowy. Ekrany wdrażamy według projektów z Figmy; nie inwestować w styl przed ich otrzymaniem.
8. Interfejs i komunikaty po polsku.
9. **Repozytorium jest publiczne**: żadnych danych klientów, przykładowych umów/ofert z prawdziwymi danymi, numerów kont, kluczy API. Dane testowe – wyłącznie fikcyjne („Klient Testowy”, `example.com`). Generowanie DOCX z szablonu umowy: szablon z danymi firmy trzymać poza repo (np. w folderze danych serwera) albo upewnić się, że nie zawiera danych wrażliwych.
10. Przed commitem: `npm run typecheck && npm run lint && npm test && cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`.

## 6. Otwarte kwestie

* Decyzje użytkownika z planu (rozdz. 10): punkt odniesienia zamówień, dni robocze czy kalendarzowe (ustawienie `liczenie_dni`, domyślnie „robocze”), lista kategorii z czasami dostaw, format numeracji umów (przyjęto `NN/RRRR`).
* Kopie zapasowe bazy (nocna kopia `VACUUM INTO` na drugi dysk / do chmury) – do zrobienia w fazie „Sieć”.
* Przechowywanie plików (zdjęcia, PDF) w folderze danych serwera i ich udostępnianie przez API.
* Uprawnienia użytkowników (np. koszt własny widoczny tylko dla właściciela) – obecnie jeden wspólny klucz dla wszystkich stanowisk.
* Podpisywanie instalatora certyfikatem (bez niego Windows SmartScreen ostrzega przy instalacji) i automatyczne aktualizacje (`tauri-plugin-updater`).
* Szyfrowanie ruchu w sieci (TLS) – patrz rozdz. 2.
