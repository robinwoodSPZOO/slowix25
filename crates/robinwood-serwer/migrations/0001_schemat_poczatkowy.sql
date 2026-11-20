-- ROBINWOOD – schemat początkowy (Faza 0)
--
-- Konwencje:
--   * klucze główne: INTEGER PRIMARY KEY (rowid),
--   * kwoty pieniężne: INTEGER w groszach (sufiks _gr), nigdy REAL,
--   * procenty rabatu: INTEGER w setnych częściach procenta (sufiks _bp, 1250 = 12,50%),
--   * daty: TEXT 'RRRR-MM-DD', znaczniki czasu: TEXT ISO 8601 UTC,
--   * statusy i typy: TEXT z ograniczeniem CHECK (wartości w języku polskim, bez polskich znaków),
--   * usuwanie „miękkie” tam, gdzie trzeba zachować historię (kolumna usunieto).
-- Kolejne zmiany schematu dokładamy WYŁĄCZNIE jako nowe pliki migracji (0002_..., 0003_...).

-- Klucze obce włącza połączenie (SqliteConnectOptions::foreign_keys), nie migracja.

-- ---------------------------------------------------------------------------
-- Ustawienia i kalendarz dni wolnych
-- ---------------------------------------------------------------------------

CREATE TABLE ustawienia (
    klucz    TEXT PRIMARY KEY,
    wartosc  TEXT NOT NULL,
    opis     TEXT
);

INSERT INTO ustawienia (klucz, wartosc, opis) VALUES
    ('odstep_materialy_przed_warsztatem_dni', '7', 'Materiały i akcesoria na warsztacie najpóźniej N dni przed początkiem składania'),
    ('odstep_rysunki_przed_zamowieniem_dni', '7', 'Rysunki wykonawcze najpóźniej N dni przed najwcześniejszym (planowym) zamówieniem'),
    ('odstep_pomiar_przed_rysunkami_dni', '7', 'Pomiar końcowy najpóźniej N dni przed terminem rysunków'),
    ('liczenie_dni', 'robocze', 'Czy odstępy i czasy dostaw liczyć w dniach roboczych (robocze) czy kalendarzowych (kalendarzowe) – decyzja użytkownika do potwierdzenia'),
    ('gwarancja_miesiace', '24', 'Domyślny okres gwarancji od odbioru'),
    ('stawka_vat_domyslna', '23', 'Domyślna stawka VAT dla nowych ofert');

-- Święta stałe liczy program; tu trafiają urlopy, przerwy dostawców i dni wyjątkowe.
CREATE TABLE dni_wolne (
    data    TEXT PRIMARY KEY,
    rodzaj  TEXT NOT NULL CHECK (rodzaj IN ('swieto', 'urlop', 'przerwa_dostawcow', 'inne')),
    opis    TEXT
);

-- ---------------------------------------------------------------------------
-- Klienci, osoby kontaktowe, etykiety, inwestycje
-- ---------------------------------------------------------------------------

CREATE TABLE klienci (
    id                  INTEGER PRIMARY KEY,
    typ                 TEXT NOT NULL DEFAULT 'indywidualny'
                        CHECK (typ IN ('indywidualny', 'firma', 'architekt', 'instytucja')),
    nazwa               TEXT NOT NULL,               -- imię i nazwisko albo nazwa firmy
    nip                 TEXT,
    email               TEXT,
    telefon             TEXT,
    adres_ulica         TEXT,
    adres_kod           TEXT,
    adres_miasto        TEXT,
    polecajacy_id       INTEGER REFERENCES klienci(id) ON DELETE SET NULL, -- architekt/biuro polecające
    zgoda_marketing     INTEGER NOT NULL DEFAULT 0 CHECK (zgoda_marketing IN (0, 1)),
    zgoda_marketing_data TEXT,
    uwagi               TEXT,
    utworzono           TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    zmieniono           TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    usunieto            TEXT
);
CREATE INDEX ix_klienci_nazwa ON klienci(nazwa);
CREATE INDEX ix_klienci_email ON klienci(email);

CREATE TABLE osoby_kontaktowe (
    id             INTEGER PRIMARY KEY,
    klient_id      INTEGER NOT NULL REFERENCES klienci(id) ON DELETE CASCADE,
    imie_nazwisko  TEXT NOT NULL,
    rola           TEXT,
    email          TEXT,
    telefon        TEXT
);
CREATE INDEX ix_osoby_kontaktowe_klient ON osoby_kontaktowe(klient_id);

CREATE TABLE etykiety (
    id     INTEGER PRIMARY KEY,
    nazwa  TEXT NOT NULL UNIQUE,
    kolor  TEXT
);
INSERT INTO etykiety (nazwa) VALUES
    ('oferta bez odpowiedzi'), ('w realizacji'), ('zakończone'), ('stały klient');

CREATE TABLE klienci_etykiety (
    klient_id    INTEGER NOT NULL REFERENCES klienci(id) ON DELETE CASCADE,
    etykieta_id  INTEGER NOT NULL REFERENCES etykiety(id) ON DELETE CASCADE,
    PRIMARY KEY (klient_id, etykieta_id)
);

-- Inwestycja = miejsce montażu (klient może mieć kilka, np. kolejne pomieszczenia).
CREATE TABLE inwestycje (
    id                 INTEGER PRIMARY KEY,
    klient_id          INTEGER NOT NULL REFERENCES klienci(id) ON DELETE RESTRICT,
    nazwa              TEXT NOT NULL,
    adres_ulica        TEXT,
    adres_kod          TEXT,
    adres_miasto       TEXT,
    pietro             TEXT,
    kod_domofonu       TEXT,
    parking            TEXT,
    winda_wniesienie   TEXT,
    uwagi              TEXT,
    utworzono          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);
CREATE INDEX ix_inwestycje_klient ON inwestycje(klient_id);

-- ---------------------------------------------------------------------------
-- Dostawcy, kategorie materiałów, czasy dostaw, słowniki
-- ---------------------------------------------------------------------------

CREATE TABLE dostawcy (
    id        INTEGER PRIMARY KEY,
    nazwa     TEXT NOT NULL UNIQUE,
    email     TEXT,
    telefon   TEXT,
    uwagi     TEXT,
    aktywny   INTEGER NOT NULL DEFAULT 1 CHECK (aktywny IN (0, 1))
);

-- Kategoria materiału decyduje o czasie dostawy w kalendarzu.
CREATE TABLE kategorie_materialow (
    id                    INTEGER PRIMARY KEY,
    nazwa                 TEXT NOT NULL UNIQUE,
    opis                  TEXT,
    domyslny_dostawca_id  INTEGER REFERENCES dostawcy(id) ON DELETE SET NULL,
    dni_dostawy_min       INTEGER NOT NULL DEFAULT 0 CHECK (dni_dostawy_min >= 0),
    dni_dostawy_max       INTEGER NOT NULL DEFAULT 0 CHECK (dni_dostawy_max >= dni_dostawy_min),
    aktywna               INTEGER NOT NULL DEFAULT 1 CHECK (aktywna IN (0, 1))
);

-- Czas dostawy dla pary kategoria + dostawca (nadpisuje widełki kategorii).
-- dni_srednia_rzeczywista uzupełnia program na podstawie faktycznych dostaw.
CREATE TABLE czasy_dostaw (
    id                       INTEGER PRIMARY KEY,
    kategoria_id             INTEGER NOT NULL REFERENCES kategorie_materialow(id) ON DELETE CASCADE,
    dostawca_id              INTEGER NOT NULL REFERENCES dostawcy(id) ON DELETE CASCADE,
    dni_min                  INTEGER NOT NULL CHECK (dni_min >= 0),
    dni_max                  INTEGER NOT NULL CHECK (dni_max >= dni_min),
    dni_srednia_rzeczywista  REAL,
    UNIQUE (kategoria_id, dostawca_id)
);

-- Przykładowe kategorie BEZ czasów dostaw (czasy uzupełnia użytkownik w Ustawieniach).
INSERT INTO kategorie_materialow (nazwa) VALUES
    ('Płyta meblowa'), ('Płyta HDF (plecy)'), ('Fronty MDF lakierowane'), ('Fronty MDF frezowane'),
    ('Fronty akrylowe'), ('Ramki aluminiowe / szkło / lustra'), ('Blaty laminowane'), ('Blaty HPL kompaktowe'),
    ('Okucia'), ('Uchwyty'), ('Oświetlenie LED'), ('Tapicerka'), ('AGD');

-- Słowniki podpowiedzi (rodzaj elementu, płyty, fronty, blaty, okucia, oświetlenie...).
CREATE TABLE slowniki (
    kod    TEXT PRIMARY KEY,
    nazwa  TEXT NOT NULL
);
INSERT INTO slowniki (kod, nazwa) VALUES
    ('rodzaj_elementu', 'Rodzaj elementu'),
    ('plyta', 'Płyty'),
    ('front', 'Fronty specjalne'),
    ('blat', 'Blaty'),
    ('okucie', 'Okucia'),
    ('oswietlenie', 'Oświetlenie'),
    ('klauzula', 'Klauzule końcowe oferty');

CREATE TABLE pozycje_slownika (
    id                      INTEGER PRIMARY KEY,
    slownik_kod             TEXT NOT NULL REFERENCES slowniki(kod) ON DELETE CASCADE,
    wartosc                 TEXT NOT NULL,
    producent               TEXT,
    dekor                   TEXT,
    kategoria_materialu_id  INTEGER REFERENCES kategorie_materialow(id) ON DELETE SET NULL,
    kolejnosc               INTEGER NOT NULL DEFAULT 0,
    aktywna                 INTEGER NOT NULL DEFAULT 1 CHECK (aktywna IN (0, 1)),
    UNIQUE (slownik_kod, wartosc, producent, dekor)
);
CREATE INDEX ix_pozycje_slownika_kod ON pozycje_slownika(slownik_kod);

INSERT INTO pozycje_slownika (slownik_kod, wartosc, kolejnosc) VALUES
    ('rodzaj_elementu', 'Korpus', 1), ('rodzaj_elementu', 'Półki', 2), ('rodzaj_elementu', 'Fronty', 3),
    ('rodzaj_elementu', 'Bok wykończeniowy', 4), ('rodzaj_elementu', 'Blenda', 5),
    ('rodzaj_elementu', 'Cokół', 6), ('rodzaj_elementu', 'Plecy', 7), ('rodzaj_elementu', 'Blat', 8),
    ('rodzaj_elementu', 'Plecy wyspy', 9), ('rodzaj_elementu', 'Panel LED', 10),
    ('rodzaj_elementu', 'Ażur', 11), ('rodzaj_elementu', 'Siedzisko tapicerowane', 12);

-- ---------------------------------------------------------------------------
-- Pliki (zdjęcia, rysunki, PDF) – przechowywane w folderze danych serwera
-- ---------------------------------------------------------------------------

CREATE TABLE pliki (
    id                INTEGER PRIMARY KEY,
    sciezka_wzgledna  TEXT NOT NULL UNIQUE,   -- względem folderu danych serwera
    nazwa             TEXT NOT NULL,
    typ_mime          TEXT,
    rozmiar_bajty     INTEGER,
    sha256            TEXT,
    klient_id         INTEGER REFERENCES klienci(id) ON DELETE SET NULL,
    utworzono         TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

-- ---------------------------------------------------------------------------
-- Specyfikacja: wspólna dla wersji oferty, umowy i aneksu
-- specyfikacja → sekcja (pomieszczenie) → część (moduł) → wiersze
-- ---------------------------------------------------------------------------

CREATE TABLE specyfikacje (
    id             INTEGER PRIMARY KEY,
    zrodlo_id      INTEGER REFERENCES specyfikacje(id) ON DELETE SET NULL, -- z której skopiowano (oferta → umowa)
    utworzono      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE TABLE sekcje_specyfikacji (
    id                INTEGER PRIMARY KEY,
    specyfikacja_id   INTEGER NOT NULL REFERENCES specyfikacje(id) ON DELETE CASCADE,
    numer             TEXT,                   -- np. '01'
    nazwa             TEXT NOT NULL,          -- np. 'Zabudowa Kuchni'
    typ               TEXT NOT NULL DEFAULT 'meble' CHECK (typ IN ('meble', 'agd')),
    wariant           TEXT,                   -- np. 'Wersja 1' (warianty w jednej ofercie)
    cena_gr           INTEGER,                -- cena na poziomie sekcji (jeśli tak wyceniamy)
    stawka_vat        INTEGER CHECK (stawka_vat IN (8, 23)), -- nadpisanie stawki (np. AGD)
    kolejnosc         INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX ix_sekcje_spec ON sekcje_specyfikacji(specyfikacja_id);

CREATE TABLE czesci_specyfikacji (
    id          INTEGER PRIMARY KEY,
    sekcja_id   INTEGER NOT NULL REFERENCES sekcje_specyfikacji(id) ON DELETE CASCADE,
    nazwa       TEXT NOT NULL,                -- np. 'Słupki', 'Zabudowa pod blatem – A'
    cena_gr     INTEGER,                      -- cena modułu (jeśli tak wyceniamy)
    kolejnosc   INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX ix_czesci_sekcja ON czesci_specyfikacji(sekcja_id);

CREATE TABLE wiersze_specyfikacji (
    id                      INTEGER PRIMARY KEY,
    czesc_id                INTEGER NOT NULL REFERENCES czesci_specyfikacji(id) ON DELETE CASCADE,
    podgrupa                TEXT NOT NULL DEFAULT 'glowna'
                            CHECK (podgrupa IN ('glowna', 'wyposazenie', 'frezowanie')),
    rodzaj                  TEXT,             -- kolumna „Rodzaj”
    material                TEXT,             -- kolumna „Materiał”
    opis                    TEXT,             -- kolumna „Opis”
    ilosc                   REAL,             -- kolumna „Ilość” (wartość)
    jednostka               TEXT,             -- 'szt.', 'kpl.', 'm²', 'mb'
    status                  TEXT NOT NULL DEFAULT 'ustalone'
                            CHECK (status IN ('ustalone', 'do_potwierdzenia', 'po_stronie_klienta', 'poza_wycena')),
    termin_potwierdzenia    TEXT,             -- do kiedy klient musi potwierdzić
    kategoria_materialu_id  INTEGER REFERENCES kategorie_materialow(id) ON DELETE SET NULL,
    pozycja_slownika_id     INTEGER REFERENCES pozycje_slownika(id) ON DELETE SET NULL,
    kolejnosc               INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX ix_wiersze_czesc ON wiersze_specyfikacji(czesc_id);
CREATE INDEX ix_wiersze_status ON wiersze_specyfikacji(status);

CREATE TABLE zdjecia_specyfikacji (
    id               INTEGER PRIMARY KEY,
    specyfikacja_id  INTEGER NOT NULL REFERENCES specyfikacje(id) ON DELETE CASCADE,
    sekcja_id        INTEGER REFERENCES sekcje_specyfikacji(id) ON DELETE CASCADE,
    czesc_id         INTEGER REFERENCES czesci_specyfikacji(id) ON DELETE CASCADE,
    plik_id          INTEGER NOT NULL REFERENCES pliki(id) ON DELETE RESTRICT,
    podpis           TEXT,
    kolejnosc        INTEGER NOT NULL DEFAULT 0
);

-- ---------------------------------------------------------------------------
-- Oferty i ich wersje
-- ---------------------------------------------------------------------------

CREATE TABLE oferty (
    id             INTEGER PRIMARY KEY,
    numer          TEXT NOT NULL UNIQUE,
    klient_id      INTEGER NOT NULL REFERENCES klienci(id) ON DELETE RESTRICT,
    inwestycja_id  INTEGER REFERENCES inwestycje(id) ON DELETE SET NULL,
    status         TEXT NOT NULL DEFAULT 'szkic'
                   CHECK (status IN ('szkic', 'wyslana', 'zaakceptowana', 'odrzucona', 'wygasla')),
    data_akceptacji TEXT,
    utworzono      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    zmieniono      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);
CREATE INDEX ix_oferty_klient ON oferty(klient_id);

CREATE TABLE wersje_oferty (
    id                     INTEGER PRIMARY KEY,
    oferta_id              INTEGER NOT NULL REFERENCES oferty(id) ON DELETE CASCADE,
    nr_wersji              INTEGER NOT NULL,
    specyfikacja_id        INTEGER NOT NULL UNIQUE REFERENCES specyfikacje(id) ON DELETE RESTRICT,
    data_oferty            TEXT,
    termin_realizacji      TEXT,              -- tekst z oferty, np. 'do 15 tygodni'
    data_waznosci_cen      TEXT,
    poziom_wyceny          TEXT NOT NULL DEFAULT 'sekcja'
                           CHECK (poziom_wyceny IN ('czesc', 'sekcja', 'calosc')),
    cena_calosc_gr         INTEGER,
    rabat_termin_gr        INTEGER NOT NULL DEFAULT 0,
    rabat_calosc_bp        INTEGER NOT NULL DEFAULT 0,
    stawka_vat             INTEGER NOT NULL DEFAULT 23 CHECK (stawka_vat IN (8, 23)),
    koszt_wlasny_gr        INTEGER,           -- widoczny tylko dla właściciela
    klauzule               TEXT,              -- JSON: lista id pozycji słownika 'klauzula'
    uwagi                  TEXT,
    utworzono              TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    UNIQUE (oferta_id, nr_wersji)
);

-- ---------------------------------------------------------------------------
-- Umowy, płatności, wpłaty, aneksy, protokoły
-- ---------------------------------------------------------------------------

CREATE TABLE umowy (
    id                            INTEGER PRIMARY KEY,
    rok                           INTEGER NOT NULL,
    nr_kolejny                    INTEGER NOT NULL,
    numer                         TEXT NOT NULL UNIQUE,   -- format 'NN/RRRR' (do potwierdzenia)
    klient_id                     INTEGER NOT NULL REFERENCES klienci(id) ON DELETE RESTRICT,
    inwestycja_id                 INTEGER REFERENCES inwestycje(id) ON DELETE SET NULL,
    wersja_oferty_id              INTEGER REFERENCES wersje_oferty(id) ON DELETE SET NULL,
    specyfikacja_id               INTEGER NOT NULL REFERENCES specyfikacje(id) ON DELETE RESTRICT,
    miejscowosc                   TEXT,
    data_zawarcia                 TEXT,
    status                        TEXT NOT NULL DEFAULT 'szkic'
                                  CHECK (status IN ('szkic', 'wyslana', 'podpisana', 'w_realizacji', 'odebrana', 'zamknieta')),
    wartosc_brutto_gr             INTEGER,
    stawka_vat                    INTEGER CHECK (stawka_vat IN (8, 23)),
    zadatek_gr                    INTEGER,
    termin_zadatku                TEXT,
    wariant_platnosci             TEXT CHECK (wariant_platnosci IN ('A', 'B')),
    kwota_przed_montazem_gr       INTEGER,
    montaz_do                     TEXT,          -- rozpoczęcie montażu „do” wskazanej daty
    dni_na_brakujace_informacje   INTEGER,
    vat_rodzaj_nieruchomosci      TEXT,
    vat_powierzchnia_m2           REAL,
    vat_podstawa_powierzchni      TEXT,
    zalaczniki                    TEXT,          -- JSON: lista kodów dołączonych załączników
    gwarancja_do                  TEXT,
    utworzono                     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    zmieniono                     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    UNIQUE (rok, nr_kolejny)
);
CREATE INDEX ix_umowy_klient ON umowy(klient_id);

CREATE TABLE platnosci_planowane (
    id          INTEGER PRIMARY KEY,
    umowa_id    INTEGER NOT NULL REFERENCES umowy(id) ON DELETE CASCADE,
    rodzaj      TEXT NOT NULL CHECK (rodzaj IN ('zadatek', 'przed_montazem', 'koncowa', 'etap', 'inna')),
    opis        TEXT,
    kwota_gr    INTEGER NOT NULL,
    termin      TEXT,
    kolejnosc   INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE wplaty (
    id                      INTEGER PRIMARY KEY,
    umowa_id                INTEGER NOT NULL REFERENCES umowy(id) ON DELETE RESTRICT,
    platnosc_planowana_id   INTEGER REFERENCES platnosci_planowane(id) ON DELETE SET NULL,
    data                    TEXT NOT NULL,
    kwota_gr                INTEGER NOT NULL,
    sposob                  TEXT CHECK (sposob IN ('przelew', 'gotowka', 'karta', 'inny')),
    uwagi                   TEXT,
    utworzono               TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);
CREATE INDEX ix_wplaty_umowa ON wplaty(umowa_id);

CREATE TABLE aneksy (
    id                    INTEGER PRIMARY KEY,
    umowa_id              INTEGER NOT NULL REFERENCES umowy(id) ON DELETE CASCADE,
    nr                    INTEGER NOT NULL,
    data                  TEXT,
    opis_zmian            TEXT NOT NULL,
    zmiana_ceny_gr        INTEGER NOT NULL DEFAULT 0,
    nowy_termin_montazu   TEXT,
    specyfikacja_id       INTEGER REFERENCES specyfikacje(id) ON DELETE SET NULL,
    status                TEXT NOT NULL DEFAULT 'szkic' CHECK (status IN ('szkic', 'wyslany', 'podpisany')),
    UNIQUE (umowa_id, nr)
);

CREATE TABLE protokoly_odbioru (
    id          INTEGER PRIMARY KEY,
    umowa_id    INTEGER NOT NULL REFERENCES umowy(id) ON DELETE CASCADE,
    data        TEXT NOT NULL,
    uwagi       TEXT,
    bez_zastrzezen INTEGER NOT NULL DEFAULT 1 CHECK (bez_zastrzezen IN (0, 1))
);

-- ---------------------------------------------------------------------------
-- Zlecenia, zamówienia materiałów, zadania kalendarza
-- ---------------------------------------------------------------------------

CREATE TABLE zlecenia (
    id                     INTEGER PRIMARY KEY,
    klient_id              INTEGER NOT NULL REFERENCES klienci(id) ON DELETE RESTRICT,
    umowa_id               INTEGER UNIQUE REFERENCES umowy(id) ON DELETE SET NULL,
    nazwa                  TEXT NOT NULL,
    etap                   TEXT NOT NULL DEFAULT 'oferta'
                           CHECK (etap IN ('oferta', 'umowa', 'pomiar', 'rysunki', 'zamowienia', 'warsztat', 'montaz', 'odbior', 'gwarancja', 'zakonczone')),
    montaz_od              TEXT,
    czas_warsztat_dni      INTEGER,
    czas_montazu_dni       INTEGER,
    -- nadpisania odstępów z ustawień (NULL = wartość globalna)
    odstep_materialy_dni   INTEGER,
    odstep_rysunki_dni     INTEGER,
    odstep_pomiar_dni      INTEGER,
    utworzono              TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE TABLE zamowienia_materialow (
    id                INTEGER PRIMARY KEY,
    zlecenie_id       INTEGER NOT NULL REFERENCES zlecenia(id) ON DELETE CASCADE,
    kategoria_id      INTEGER NOT NULL REFERENCES kategorie_materialow(id) ON DELETE RESTRICT,
    dostawca_id       INTEGER REFERENCES dostawcy(id) ON DELETE SET NULL,
    opis              TEXT,
    status            TEXT NOT NULL DEFAULT 'do_zamowienia'
                      CHECK (status IN ('do_zamowienia', 'zamowione', 'potwierdzone', 'dostarczone')),
    data_sweet_spot   TEXT,
    data_planowa      TEXT,
    data_krytyczna    TEXT,
    data_zamowienia   TEXT,     -- faktyczna
    data_dostawy      TEXT      -- faktyczna (do korekty średnich czasów dostaw)
);
CREATE INDEX ix_zamowienia_zlecenie ON zamowienia_materialow(zlecenie_id);

CREATE TABLE zadania_kalendarza (
    id               INTEGER PRIMARY KEY,
    zlecenie_id      INTEGER REFERENCES zlecenia(id) ON DELETE CASCADE,
    zamowienie_id    INTEGER REFERENCES zamowienia_materialow(id) ON DELETE CASCADE,
    typ              TEXT NOT NULL
                     CHECK (typ IN ('montaz', 'warsztat', 'zamowienie', 'rysunki', 'pomiar', 'platnosc', 'klient', 'inne')),
    tytul            TEXT NOT NULL,
    data_od          TEXT NOT NULL,
    data_do          TEXT,
    data_sweet_spot  TEXT,
    data_planowa     TEXT,
    data_krytyczna   TEXT,
    wyliczone        INTEGER NOT NULL DEFAULT 1 CHECK (wyliczone IN (0, 1)), -- 0 = wpisane ręcznie
    wykonano         TEXT,     -- data wykonania; NULL = do zrobienia
    uwagi            TEXT
);
CREATE INDEX ix_zadania_data ON zadania_kalendarza(data_od);
CREATE INDEX ix_zadania_zlecenie ON zadania_kalendarza(zlecenie_id);

-- ---------------------------------------------------------------------------
-- Korespondencja (Gmail, import eksportu czatu WhatsApp, wpisy ręczne)
-- ---------------------------------------------------------------------------

CREATE TABLE korespondencja (
    id              INTEGER PRIMARY KEY,
    klient_id       INTEGER REFERENCES klienci(id) ON DELETE SET NULL,
    umowa_id        INTEGER REFERENCES umowy(id) ON DELETE SET NULL,
    zrodlo          TEXT NOT NULL CHECK (zrodlo IN ('gmail', 'whatsapp', 'reczna')),
    kierunek        TEXT NOT NULL CHECK (kierunek IN ('przychodzaca', 'wychodzaca')),
    zewnetrzne_id   TEXT,       -- np. id wiadomości Gmail; chroni przed duplikatami
    watek_id        TEXT,
    nadawca         TEXT,
    odbiorcy        TEXT,
    temat           TEXT,
    tresc           TEXT,
    data            TEXT NOT NULL,
    utworzono       TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    UNIQUE (zrodlo, zewnetrzne_id)
);
CREATE INDEX ix_korespondencja_klient ON korespondencja(klient_id, data);

CREATE TABLE zalaczniki_korespondencji (
    korespondencja_id  INTEGER NOT NULL REFERENCES korespondencja(id) ON DELETE CASCADE,
    plik_id            INTEGER NOT NULL REFERENCES pliki(id) ON DELETE RESTRICT,
    PRIMARY KEY (korespondencja_id, plik_id)
);

-- ---------------------------------------------------------------------------
-- Dziennik zmian (audyt i podstawa do powiadamiania stanowisk)
-- ---------------------------------------------------------------------------

CREATE TABLE dziennik_zmian (
    id          INTEGER PRIMARY KEY,
    tabela      TEXT NOT NULL,
    rekord_id   INTEGER NOT NULL,
    operacja    TEXT NOT NULL CHECK (operacja IN ('dodanie', 'zmiana', 'usuniecie')),
    stanowisko  TEXT,
    kiedy       TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);
