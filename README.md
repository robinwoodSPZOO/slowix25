# ROBINWOOD – program do ofert, umów, klientów i kalendarza

Aplikacja na Windows dla firmy ROBINWOOD (meble na wymiar). Docelowo obejmuje:

- **Generator Ofert** – specyfikacja mebli (sekcja → część → wiersze Rodzaj | Materiał | Opis | Ilość → Wyposażenie), wycena, rabaty, VAT 8%/23%, zdjęcia, wersje, PDF/DOCX,
- **Generator Umów** – umowa z oferty lub od zera, skrócony formularz, płatności A/B, aneksy, wpłaty, protokół odbioru,
- **Bazę klientów** – karta klienta, korespondencja Gmail, import eksportu czatu WhatsApp, wysyłka e-maili,
- **Kalendarz** – terminy liczone wstecz od montażu, widełki czasów dostaw (sweet spot / planowa / krytyczna), kolory zadań, widżet na pulpicie, powiadomienia Windows, autostart.

Jedna baza danych leży na komputerze-**serwerze** w sieci firmowej (Wi-Fi/LAN), pozostałe komputery (**klienci**) łączą się z nim. Nie ma chmury.

> **Stan: Faza 0 – szkielet.** Działa: tryb serwer/klient, baza z pełnym schematem i migracjami, API (klienci – pełny CRUD, słowniki, kategorie materiałów, zdarzenia WebSocket), nawigacja po modułach, lista i karta klienta, ustawienia trybu pracy, wykrywanie serwera w sieci (mDNS), ikona w zasobniku, autostart, powiadomienie testowe, okno-widżet (bez danych). Pozostałe moduły to zaślepki. Wygląd jest tymczasowy – ekrany będą projektowane w Figmie.

> **Repozytorium jest publiczne.** Nie umieszczaj w nim danych klientów (nazwisk, adresów, numerów umów, kwot, treści umów i ofert), numerów kont ani haseł/kluczy. Dane trzyma wyłącznie baza na serwerze.

## Instalacja (użytkownik)

1. Pobierz instalator `ROBINWOOD_x.y.z_x64-setup.exe` z artefaktów GitHub Actions (zakładka *Actions* → najnowszy przebieg *CI* → *robinwood-instalator-windows*).
2. Zainstaluj na każdym komputerze (instalacja dla bieżącego użytkownika, bez uprawnień administratora).
3. **Na komputerze, który ma być serwerem** (stale włączonym): *Ustawienia* → tryb **Serwer** → *Zapisz i uruchom ponownie*. Program wygeneruje **klucz dla stanowisk** – zapisz go. Przy pierwszym uruchomieniu Windows zapyta o zezwolenie w zaporze – zezwól dla **sieci prywatnej**.
4. **Na pozostałych komputerach**: *Ustawienia* → tryb **Klient** → *Znajdź serwer w sieci* (albo wpisz adres, np. `192.168.1.10:47800`) → wklej klucz → *Zapisz i uruchom ponownie*.

Zamknięcie okna chowa program do zasobnika; całkowite wyjście: prawy przycisk na ikonie → *Zakończ*.

## Uruchomienie dla programisty

Wymagania: Node.js 22+, Rust (stable), na Windows – WebView2 (jest w Windows 10/11) i Build Tools for Visual Studio (C++). Na Linuksie biblioteki z [wymagań Tauri](https://v2.tauri.app/start/prerequisites/) (`libwebkit2gtk-4.1-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`, `libxdo-dev`, `libssl-dev`).

```bash
npm install
npm run tauri dev          # aplikacja okienkowa z przeładowaniem na żywo
```

Sam serwer (bez okna, np. do pracy nad interfejsem w przeglądarce albo na mini PC):

```bash
ROBINWOOD_KLUCZ=dev ROBINWOOD_BAZA=dane/robinwood.db npm run serwer
# w drugim terminalu:
VITE_API_URL=http://127.0.0.1:47800 VITE_API_KLUCZ=dev npm run dev   # http://localhost:1420
```

Diagnostyka wykrywania w sieci: `cargo run -p robinwood-serwer -- --szukaj`.

## Testy i kontrola jakości

```bash
npm run typecheck && npm run lint && npm test          # frontend
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace                                 # baza, migracje, API
```

## Budowa instalatora

```bash
npx tauri build --bundles nsis
# wynik: target/release/bundle/nsis/ROBINWOOD_<wersja>_x64-setup.exe
```

Instalator Windows buduje też automatycznie GitHub Actions (`.github/workflows/ci.yml`, job *Instalator Windows*) i udostępnia go jako artefakt przebiegu.

## Struktura

```
src/                     interfejs (React + TypeScript)
  api/                   klient HTTP do API i typy danych
  moduly/                moduły: pulpit, klienci, oferty, umowy, kalendarz, korespondencja, słowniki, ustawienia
  moduly/rejestr.tsx     jedno miejsce rejestracji modułów (menu + trasy)
  widzet/                okno-widżet na pulpit
src-tauri/               aplikacja okienkowa (Tauri 2): okna, zasobnik, autostart, powiadomienia, tryb pracy
crates/robinwood-serwer/ baza SQLite, migracje, API HTTP/WebSocket (axum), mDNS – bez zależności od Tauri
docs/ARCHITEKTURA.md     architektura, schemat danych i konwencje dla kolejnych faz
```

Szczegóły: [docs/ARCHITEKTURA.md](docs/ARCHITEKTURA.md).
