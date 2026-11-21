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
`src/terminarz.rs` (baza), `src/api_kalendarz.rs` (trasy), migracja `0003_silnik_terminow.sql`.

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
  usuniętych ze zlecenia znikają (wykonane zostają jako historia). Terminy w `zamowienia_materialow` aktualizuje się
  tylko dla materiałów jeszcze niezamówionych. Identyfikacja zadania: `zadania_kalendarza.klucz`
  (`montaz`, `warsztat`, `rysunki`, `pomiar`, `zamowienie:<id materiału>`, `potwierdzenie:<id materiału>`,
  `potwierdzenie_montazu`, `pomieszczenie_gotowe`, `brakujace_informacje`, `platnosc:<id>`).
* **Kiedy przelicza się automatycznie**: po zmianie zlecenia lub jego materiałów (jeśli zlecenie ma już harmonogram)
  oraz po zmianie ustawień, dni wolnych, widełek kategorii i czasów dostaw (wszystkie zlecenia z harmonogramem poza
  etapem `zakonczone`). Pierwsze wygenerowanie – jawnie `POST /zlecenia/{id}/harmonogram`.
* **Blokada „do potwierdzenia”**: wiersz specyfikacji umowy zlecenia (`umowy.specyfikacja_id`) ze statusem
  `do_potwierdzenia` i `kategoria_materialu_id` = kategoria materiału blokuje jego zamówienie: `zablokowane: true`,
  `potwierdzic_do` = data planowa, `wiersze_do_potwierdzenia` = id wierszy. Kolumny
  `wiersze_specyfikacji.termin_potwierdzenia` silnik nie zapisuje (należy do modułu specyfikacji).

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

Przykład kontrolny (tryb kalendarzowy, montaż pon. 8.02.2027, warsztat 14 dni, 7-dniowe odstępy): warsztat
25.01–5.02, dostawa do 18.01, fronty MDF 18–24: sweet spot **23.12.2026** (18.01 − 24 = 25.12 święto → 24.12 Wigilia
→ 23.12), planowa 28.12, krytyczna 31.12; płyta 5–10: 8.01 / 11.01 / 13.01; rysunki 21.12; pomiar 14.12.2026.

### 7.3. Stany ostrzeżeń (względem `dzis`)

`stan.stan` ∈ `wykonane` < `spokojnie` < `zolty` < `pomaranczowy` < `czerwony` (kolejność = pilność; grupy w „do
zamówienia” mają stan najpilniejszej pozycji).

Niezamówiony materiał: `dzis ≤ planowa` → `spokojnie` (opis rozróżnia „przed oknem” i „w oknie”), `dzis > planowa` →
`zolty`, `dzis ≥ wstecz(krytyczna, prog_pomaranczowy_dni)` (domyślnie 2) → `pomaranczowy`, `dzis > krytyczna` →
`czerwony` z `przesuniecie_montazu_dni` = dni (w trybie liczenia) między datą wymaganej dostawy a dostawą przy
zamówieniu dziś ze średnim czasem, oraz `nowy_termin_montazu`. Wygrywa najpilniejszy spełniony warunek (w dniu
planowej: spokojnie; w dniu krytycznej: pomarańczowy; dzień po krytycznej: czerwony).

Pozostałe zadania (termin = `data_do`, a gdy brak – `data_od`): `dzis > termin` → `czerwony`,
`dzis ≥ wstecz(termin, prog_zbliza_sie_dni)` (domyślnie 3, włącznie z dniem terminu) → `zolty`, wcześniej `spokojnie`.

### 7.4. API (prefiks `/api/v1`, klucz jak w rozdz. 2)

Daty `RRRR-MM-DD`. Parametr `dzis` (opcjonalny, domyślnie dzisiejsza data serwera) służy do wyliczenia stanów – widżet
i testy mogą podać własny. Błędy jak w rozdz. 4 (422 z czytelnym komunikatem, np. „zlecenie 3: uzupełnij termin
rozpoczęcia montażu (montaz_od), aby policzyć harmonogram”).

| Metoda i ścieżka | Opis |
| --- | --- |
| `GET/POST /zlecenia`, `GET/PUT /zlecenia/{id}` | zlecenia (`klient_id`, `umowa_id`, `nazwa`, `etap`, `montaz_od`, `czas_warsztat_dni`, `czas_montazu_dni`, `odstep_*_dni`); PUT zwraca też `harmonogram_przeliczony` |
| `GET/POST /zlecenia/{id}/materialy` | materiały zlecenia (`kategoria_id`, `dostawca_id?`, `opis?`, `data_zamowienia?`, `data_dostawy?`) z migawką terminów |
| `PUT/DELETE /zamowienia-materialow/{id}` | zmiana / usunięcie materiału |
| `GET /zlecenia/{id}/harmonogram` | podgląd harmonogramu bez zapisu (`Harmonogram`) |
| `POST /zlecenia/{id}/harmonogram?dzis=` | wygenerowanie/przeliczenie i zapis; `{harmonogram, dodane, zmienione, usuniete, zamrozone, zadania_zapisane}` |
| `POST /kalendarz/przelicz` | przeliczenie wszystkich zleceń z harmonogramem → `{przeliczone_zlecenia: [...]}` |
| `GET /kalendarz/zadania?od=&do=&typ=&zlecenie_id=&stan=&tylko_niewykonane=&dzis=` | zadania nachodzące na zakres; `typ` i `stan` – jedna wartość albo kilka po przecinku |
| `GET /kalendarz/zadania/{id}?dzis=` | jedno zadanie |
| `POST /kalendarz/zadania`, `PUT/DELETE /kalendarz/zadania/{id}` | zadania ręczne (`zlecenie_id?`, `typ`, `tytul`, `data_od`, `data_do?`, `uwagi?`); wyliczonych nie można zmieniać ani usuwać (422) |
| `PUT /kalendarz/zadania/{id}/wykonanie` | `{wykonano: "2026-12-10" \| null, data_zamowienia?, data_dostawy?, uwagi?}` – dla zamówień ustawia też materiał (`data_zamowienia` domyślnie = `wykonano`, status `zamowione`/`dostarczone`); `wykonano: null` cofa wykonanie i przelicza zlecenie |
| `GET /kalendarz/tydzien?dzis=` | `{od, do, dzis, zadania}` – niewykonane z terminem/oknem w tygodniu (pon.–niedz.) + wszystkie zaległe (czerwone) |
| `GET /kalendarz/do-zamowienia?dzis=&horyzont=` | `{dzis, grupy: [{dostawca_id, dostawca_nazwa, stan, pozycje: [Zadanie]}], bez_terminow: [Material]}`; grupy alfabetycznie, „bez dostawcy” na końcu, pozycje wg daty planowej |
| `GET /kalendarz/kolizje?od=&do=` | `[{typ, zlecenie_a, zlecenie_b, od, do, zlecenie_a_nazwa, zlecenie_b_nazwa}]` – nakładanie się warsztatu lub montażu |
| `GET /kalendarz/statystyka-dostaw` | rzeczywiste czasy dostaw par kategoria + dostawca: `statystyka {liczba, min, max, srednia, sugerowane_widelki}`, `aktualne_widelki`, `sugerowana_korekta` (bez automatycznego nadpisywania) |
| `GET/PUT /kalendarz/ustawienia` | `UstawieniaTerminow`; PUT częściowy, zwraca też `przeliczone_zlecenia` |
| `GET /kalendarz/dni-wolne?od=&do=`, `PUT/DELETE /kalendarz/dni-wolne/{data}` | dni wolne użytkownika (`{rodzaj: swieto\|urlop\|przerwa_dostawcow\|inne, opis}`) |
| `GET /kalendarz/swieta?rok=` | wyliczone święta ustawowe `[{data, nazwa}]` |
| `GET/POST /dostawcy`, `PUT /dostawcy/{id}` | dostawcy |
| `PUT /kategorie-materialow/{id}/widelki` | `{dni_dostawy_min, dni_dostawy_max, domyslny_dostawca_id?}` → 204 |
| `GET/POST /czasy-dostaw`, `PUT/DELETE /czasy-dostaw/{id}` | widełki pary `{kategoria_id, dostawca_id, dni_min, dni_max}` |

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
  }
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
* Typy TypeScript i metody klienta (`src/api/typy.ts`, `src/api/klient.ts`) dopisuje agent UI na podstawie tego rozdziału.
