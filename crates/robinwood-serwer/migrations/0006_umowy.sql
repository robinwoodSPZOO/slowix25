-- Faza 3: Generator Umów.
--
-- Tabela `umowy` istnieje od 0001. Status „anulowana” nie mieści się w CHECK kolumny `status`,
-- a SQLite nie pozwala zmienić CHECK bez przebudowy tabeli (przebudowa w transakcji migracji
-- z włączonymi kluczami obcymi skasowałaby kaskadowo płatności i aneksy). Dlatego anulowanie
-- zapisujemy w osobnej kolumnie `anulowano` (data) – API pokazuje wtedy status „anulowana”,
-- a `status` pamięta etap, na którym umowę anulowano.

ALTER TABLE umowy ADD COLUMN anulowano TEXT;
ALTER TABLE umowy ADD COLUMN anulowanie_powod TEXT;
-- licznik zmian (zmiany częściowe z wykrywaniem konfliktów)
ALTER TABLE umowy ADD COLUMN wersja INTEGER NOT NULL DEFAULT 1;
-- migawka danych stron (JSON) zapisywana przy wyjściu ze szkicu; dokument jej używa
ALTER TABLE umowy ADD COLUMN migawka TEXT;
ALTER TABLE umowy ADD COLUMN wyslano TEXT;
ALTER TABLE umowy ADD COLUMN data_podpisania TEXT;
-- produkcja (wewnętrznie, do kalendarza)
ALTER TABLE umowy ADD COLUMN czas_warsztat_dni INTEGER;
ALTER TABLE umowy ADD COLUMN czas_montazu_dni INTEGER;
-- specyfikacja z chwili podpisania (po podpisaniu aneksu `specyfikacja_id` wskazuje
-- specyfikację aneksu, która od tej chwili obowiązuje)
ALTER TABLE umowy ADD COLUMN specyfikacja_pierwotna_id INTEGER REFERENCES specyfikacje(id) ON DELETE SET NULL;
ALTER TABLE umowy ADD COLUMN uwagi TEXT;

CREATE INDEX ix_umowy_status ON umowy(status);
CREATE INDEX ix_umowy_rok ON umowy(rok, nr_kolejny);

-- Harmonogram etapowy (opcjonalny załącznik nr 2): etapy prac z terminem i kwotą.
CREATE TABLE etapy_umowy (
    id         INTEGER PRIMARY KEY,
    umowa_id   INTEGER NOT NULL REFERENCES umowy(id) ON DELETE CASCADE,
    lp         INTEGER NOT NULL,
    nazwa      TEXT NOT NULL,
    termin     TEXT,
    kwota_gr   INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX ix_etapy_umowy ON etapy_umowy(umowa_id, lp);

ALTER TABLE aneksy ADD COLUMN wersja INTEGER NOT NULL DEFAULT 1;
ALTER TABLE aneksy ADD COLUMN utworzono TEXT;
ALTER TABLE aneksy ADD COLUMN zmieniono TEXT;
ALTER TABLE aneksy ADD COLUMN data_podpisania TEXT;
-- wartość i termin umowy przed aneksem (do dokumentu i listy zmian)
ALTER TABLE aneksy ADD COLUMN poprzednia_wartosc_gr INTEGER;
ALTER TABLE aneksy ADD COLUMN poprzedni_termin_montazu TEXT;

ALTER TABLE protokoly_odbioru ADD COLUMN utworzono TEXT;

-- Ustawienia umów (decyzje do potwierdzenia przez użytkownika – edytowalne w programie).
INSERT INTO ustawienia (klucz, wartosc, opis) VALUES
    ('umowa_format_numeru', '{NN}/{RRRR}', 'Format numeru umowy: {N}, {NN}, {NNN} – numer kolejny w roku, {RRRR}/{RR} – rok'),
    ('umowa_miejscowosc', '', 'Domyślna miejscowość zawarcia umowy'),
    ('umowa_dni_na_informacje', '14', 'Domyślna liczba dni kalendarzowych na przekazanie brakujących informacji'),
    ('umowa_termin_zadatku_dni', '3', 'Zadatek płatny w ciągu N dni kalendarzowych od zawarcia umowy'),
    ('umowa_zadatek_procent', '30', 'Domyślny zadatek w procentach wartości umowy (podpowiedź)'),
    ('umowa_wariant_a_dni_przed_montazem', '3', 'Wariant A: pozostała część najpóźniej N dni roboczych przed rozpoczęciem montażu'),
    ('umowa_wariant_b_dni_przed_montazem', '3', 'Wariant B: kwota przed montażem najpóźniej N dni roboczych przed rozpoczęciem montażu'),
    ('umowa_wariant_b_dni_po_odbiorze', '3', 'Wariant B: pozostałość w terminie N dni roboczych od prawidłowego oddania przedmiotu umowy'),
    ('umowa_czas_warsztat_dni', '10', 'Domyślny czas pracy na warsztacie (dni)'),
    ('umowa_czas_montazu_dni', '2', 'Domyślny czas montażu (dni)'),
    ('firma_bank', '', 'Nazwa banku wykonawcy (dokument umowy)'),
    ('firma_rachunek', '', 'Numer rachunku wykonawcy (dokument umowy)'),
    ('firma_swift', '', 'Kod SWIFT/BIC banku wykonawcy')
ON CONFLICT(klucz) DO NOTHING;
