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

Prefiks `/api/v1`. Błędy: JSON `{ "blad": "komunikat po polsku" }` z kodem 401/404/409/422/500 (409 – operacja
wymaga jawnego potwierdzenia, patrz 7.4; trasy kalendarza zwracają też błędy deserializacji treści, ścieżki i
parametrów jako 422 w tym formacie).

| Metoda i ścieżka | Opis |
| --- | --- |
| `GET /status` | nazwa serwera, wersja, `wersja_api` (bez klucza) |
| `GET /klienci?szukaj=` | lista (nazwa, e-mail, telefon) |
| `POST /klienci` | dodanie → 201 |
| `GET/PUT/DELETE /klienci/{id}` | odczyt, pełna zmiana, usunięcie miękkie → 204 |
| `GET /slowniki`, `GET /slowniki/{kod}` | słowniki i ich pozycje |
| `GET /kategorie-materialow` | kategorie z widełkami dostaw |
| `GET /zdarzenia?klucz=` | WebSocket: `{"zasob":"klienci","id":1,"operacja":"zmiana"}`; `zasob:"*"` = odśwież wszystko |
| Kalendarz, zlecenia, materiały, czasy dostaw | patrz rozdz. 7 „Silnik terminów” |

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

## 7. Silnik terminów (Faza 4a)

Kod: `crates/robinwood-serwer/src/kalendarz/` (czysta logika, bez bazy i HTTP, testy jednostkowe),
`src/terminarz.rs` (baza), `src/api_kalendarz.rs` (trasy), migracje `0003_silnik_terminow.sql` i
`0004_silnik_terminow_poprawki.sql` (znacznik `zlecenia.harmonogram_nieaktualny_powod`, `dziennik_zmian.opis`,
indeksy zakresów dat). Migracja 0003 została poprawiona w miejscu (`INSERT OR IGNORE` dla ustawień) – nie była
jeszcze nigdzie wdrożona; baza deweloperska z wcześniejszą wersją 0003 wymaga odtworzenia (sqlx sprawdza sumy
kontrolne).

### 7.1. Reguły (liczone wstecz od terminu montażu z umowy)

| Krok | Wzór (`wstecz(D, N)` – patrz 7.2) |
| --- | --- |
| Montaż | od `montaz_od`, trwa `czas_montazu_dni` (≥ 1); ostatni dzień = N-ty dzień od startu włącznie |
| Składanie na warsztacie | `warsztat_od = wstecz(montaz_od, czas_warsztat_dni)`, `warsztat_do = wstecz(montaz_od, 1)` (czas 0 → brak zadania, ostrzeżenie) |
| Data wymaganej dostawy | `W = wstecz(warsztat_od, odstep_materialy)` (domyślnie 7) |
| Zamówienie materiału | sweet spot `= wstecz(W, max)`, planowa `= wstecz(W, średnia)`, krytyczna `= wstecz(W, min)` |
| Rysunki wykonawcze | `wstecz(najwcześniejsza planowa w zleceniu, odstep_rysunki)`; bez materiałów z widełkami – od `W` (ostrzeżenie `brak_materialow`) |
| Pomiar końcowy | `wstecz(rysunki, odstep_pomiar)` |
| Potwierdzenie terminu montażu klientowi | `wstecz(montaz_od, odstep_potwierdzenie_montazu_dni)` (7), typ `klient` |
| Pomieszczenie gotowe | `wstecz(montaz_od, odstep_pomieszczenie_gotowe_dni)` (3), typ `klient` |
| Brakujące informacje od klienta | `umowy.data_zawarcia + umowy.dni_na_brakujace_informacje` (liczone naprzód, patrz 7.2), typ `klient` |
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
* **Dla agenta modułu specyfikacji**: blokady są czytane na bieżąco (`zablokowane` w odczytach zmienia się od razu po
  zmianie statusu wiersza), ale zadania `potwierdzenie:<id materiału>` powstają/znikają dopiero przy przeliczeniu.
  Po zmianie statusu wiersza (`do_potwierdzenia` ↔ inny) albo kategorii materiału wiersza wywołaj w swojej transakcji
  `terminarz::przelicz_po_zmianie_specyfikacji(&mut tx, specyfikacja_id)` (przelicza zlecenia, których umowa wskazuje
  tę specyfikację; zwraca `Vec<(zlecenie_id, Przeliczenie)>`) i po zatwierdzeniu wyślij zdarzenie
  `zadania_kalendarza` z id zlecenia. Z interfejsu: `POST /zlecenia/{id}/przelicz`.

### 7.2. Konwencja liczenia dni

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

### 7.3. Stany ostrzeżeń (względem `dzis`)

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

### 7.4. API (prefiks `/api/v1`, klucz jak w rozdz. 2)

Daty `RRRR-MM-DD` (lata 1990–2100). Parametr `dzis` (opcjonalny, domyślnie dzisiejsza data serwera) służy do
wyliczenia stanów i do kontroli „data z przyszłości” w `wykonanie` – widżet i testy mogą podać własny.

**Błędy** (zawsze JSON `{"blad": "…"}`):

| Kod | Kiedy |
| --- | --- |
| 401 | brak / zły klucz (wszystkie trasy poza `/status`) |
| 404 | brak rekordu (zlecenie, zadanie, materiał, dzień wolny…) |
| 409 | zamówienie materiału zablokowanego wierszami „do potwierdzenia” bez `mimo_blokady: true`; dodatkowo `"kod": "do_potwierdzenia"`, `"wiersze_do_potwierdzenia": [id…]` |
| 422 | błąd danych z czytelnym komunikatem (np. „zlecenie 3: uzupełnij termin rozpoczęcia montażu (montaz_od), aby policzyć harmonogram”); także **nieznane pole** w treści, **nieznany parametr** zapytania, niepoprawny JSON / typ / ścieżka (`/zlecenia/abc`), rok spoza 1990–2100, `wykonano`/daty zamówienia i dostawy z przyszłości, usunięcie zamówionego materiału, wyczyszczenie `data_zamowienia` w `PUT` materiału |
| 500 | błąd bazy – transakcja wycofana, **nic nie zapisano** |

Wszystkie struktury wejściowe odrzucają nieznane pola (np. literówka `czas_warsztatu_dni` → 422 z nazwą pola).
Parametr `klucz` (klucz API) jest dozwolony w każdym zapytaniu.

**Znacznik nieaktualnego harmonogramu** (patrz 7.1) – pola `harmonogram_nieaktualny: bool` i
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

### 7.5. Wskazówki dla UI kalendarza i widżetu

* Kolor zadania = `typ` (`src/lib/kolory.ts`), pilność = `stan.stan` (obwódka/znacznik); `stan.opis` jest gotowym
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

### 7.6. Do decyzji użytkownika (obecne zachowanie bez zmian)

* Wigilia (24.12, od 2025 r.) liczona jako dzień wolny.
* Średnia z widełek zaokrąglana w dół (późniejsza data planowa).
* Montaż rozpoczynający się w dzień wolny – tylko ostrzeżenie `montaz_w_dzien_wolny`, termin nie jest przesuwany.
* Progi ostrzeżeń w trybie kalendarzowym cofane na dzień roboczy (np. krytyczna pon. 8.03 − 2 = sob. 6.03 → próg
  pt. 5.03).
* Definicja kolizji warsztatu (nakładanie się okresów składania dwóch zleceń, bez uwzględnienia pojemności warsztatu;
  pomijany tylko etap `zakonczone`).
* Termin rysunków liczony od najwcześniejszej daty planowej wszystkich materiałów, także już zamówionych.
