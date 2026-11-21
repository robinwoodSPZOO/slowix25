-- ROBINWOOD – Faza 2: Generator Ofert (specyfikacja, wersje, szablony, zdjęcia, dane firmy)
--
-- Uzupełnia schemat 0001 o pola potrzebne edytorowi specyfikacji i ofertom.
-- Wszystkie dane przykładowe w tej migracji są ogólne (nazwy elementów mebli, klauzule) –
-- żadnych danych klientów ani firmy.

-- ---------------------------------------------------------------------------
-- Specyfikacja
-- ---------------------------------------------------------------------------

-- „ident” to stały identyfikator elementu przenoszony przy kopiowaniu specyfikacji
-- (nowa wersja oferty, oferta → umowa, aneks). Po nim porównujemy wersje.
ALTER TABLE sekcje_specyfikacji ADD COLUMN ident TEXT;
ALTER TABLE czesci_specyfikacji ADD COLUMN ident TEXT;
ALTER TABLE wiersze_specyfikacji ADD COLUMN ident TEXT;

-- Cena jednostkowa wiersza – używana w sekcji AGD (suma = cena × ilość).
ALTER TABLE wiersze_specyfikacji ADD COLUMN cena_gr INTEGER;

-- 1 = materiał wypełniony automatycznie (z korpusu albo „kopiuj materiał w dół”) i jeszcze
-- niezmieniony ręcznie; taki wiersz dalej podąża za zmianą materiału korpusu.
ALTER TABLE wiersze_specyfikacji ADD COLUMN material_z_korpusu INTEGER NOT NULL DEFAULT 0
    CHECK (material_z_korpusu IN (0, 1));

CREATE INDEX ix_czesci_kolejnosc ON czesci_specyfikacji(sekcja_id, kolejnosc);
CREATE INDEX ix_wiersze_kolejnosc ON wiersze_specyfikacji(czesc_id, kolejnosc);
CREATE INDEX ix_zdjecia_spec ON zdjecia_specyfikacji(specyfikacja_id);

-- ---------------------------------------------------------------------------
-- Wersje oferty
-- ---------------------------------------------------------------------------

-- Adres inwestycji wpisywany tekstem (inwestycje z bazy – opcjonalnie przez oferty.inwestycja_id).
ALTER TABLE wersje_oferty ADD COLUMN adres_inwestycji TEXT;
-- Czy ceny w ofercie wpisuje się brutto (domyślnie, jak w ofertach dla klientów indywidualnych)
-- czy netto (VAT doliczany).
ALTER TABLE wersje_oferty ADD COLUMN tryb_cen TEXT NOT NULL DEFAULT 'brutto'
    CHECK (tryb_cen IN ('brutto', 'netto'));
ALTER TABLE wersje_oferty ADD COLUMN zmieniono TEXT;
-- Uwaga: kolumna wersje_oferty.klauzule przechowuje od tej wersji JSON z listą TREŚCI klauzul
-- (kopia w chwili tworzenia wersji), a nie identyfikatorów – zmiana słownika nie zmienia
-- wysłanych ofert.

-- ---------------------------------------------------------------------------
-- Słowniki: podgrupa podpowiedzi i źródło (import)
-- ---------------------------------------------------------------------------

ALTER TABLE pozycje_slownika ADD COLUMN podgrupa TEXT
    CHECK (podgrupa IN ('glowna', 'wyposazenie', 'frezowanie'));
ALTER TABLE pozycje_slownika ADD COLUMN zrodlo TEXT;   -- NULL = ręcznie / migracja, 'import' = z pliku

INSERT INTO slowniki (kod, nazwa) VALUES
    ('material', 'Materiały (kolumna Materiał)'),
    ('dekor', 'Dekory i kolory'),
    ('opis', 'Typowe opisy (kolumna Opis)'),
    ('agd', 'Sprzęt AGD – rodzaje'),
    ('agd_marka', 'Sprzęt AGD – marki'),
    ('jednostka', 'Jednostki ilości');

INSERT INTO pozycje_slownika (slownik_kod, wartosc, kolejnosc) VALUES
    ('jednostka', 'szt.', 1), ('jednostka', 'kpl.', 2), ('jednostka', 'mb', 3), ('jednostka', 'm²', 4);

INSERT INTO pozycje_slownika (slownik_kod, wartosc, kolejnosc, podgrupa) VALUES
    ('rodzaj_elementu', 'Szuflady', 20, 'wyposazenie'),
    ('rodzaj_elementu', 'Systemy otwierania', 21, 'wyposazenie'),
    ('rodzaj_elementu', 'Uchwyty', 22, 'wyposazenie'),
    ('rodzaj_elementu', 'Oświetlenie', 23, 'wyposazenie'),
    ('rodzaj_elementu', 'Drążek / wieszak', 24, 'wyposazenie'),
    ('rodzaj_elementu', 'Kosz cargo', 25, 'wyposazenie');

INSERT INTO pozycje_slownika (slownik_kod, wartosc, kolejnosc) VALUES
    ('material', 'Płyta meblowa laminowana', 1),
    ('material', 'Płyta HDF', 2),
    ('material', 'MDF lakierowany', 3),
    ('material', 'MDF frezowany + lakier', 4),
    ('material', 'Blat laminowany', 5),
    ('material', 'HPL kompaktowy', 6),
    ('opis', 'Kolor: do potwierdzenia', 1),
    ('opis', 'Wg projektu', 2),
    ('opis', 'Do ustalenia w dniu montażu', 3),
    ('agd', 'Piekarnik', 1), ('agd', 'Płyta indukcyjna', 2), ('agd', 'Okap', 3),
    ('agd', 'Zmywarka', 4), ('agd', 'Lodówka', 5), ('agd', 'Zlew', 6), ('agd', 'Bateria', 7);

-- Stałe klauzule końcowe oferty (ogólne – do zmiany w module Słowniki).
INSERT INTO pozycje_slownika (slownik_kod, wartosc, kolejnosc) VALUES
    ('klauzula', 'Oferta obejmuje wykonanie, transport i montaż mebli opisanych w specyfikacji.', 1),
    ('klauzula', 'Sprzęt AGD, oświetlenie i wyposażenie nieujęte w specyfikacji nie są częścią oferty.', 2),
    ('klauzula', 'Ostateczne wymiary zostaną potwierdzone pomiarem końcowym przed wykonaniem rysunków.', 3),
    ('klauzula', 'Pozycje oznaczone „do potwierdzenia” wymagają decyzji Inwestora przed zamówieniem materiałów.', 4);

-- ---------------------------------------------------------------------------
-- Szablony mebli (typ zabudowy → typowe części i wiersze)
-- ---------------------------------------------------------------------------

-- dane: JSON {"czesci": ["Słupki", ...],
--              "wiersze": [{"podgrupa": "glowna", "rodzaj": "Korpus", "material": "...",
--                           "opis": "...", "kategoria": "...", "udzial": 0.98}, ...]}
CREATE TABLE szablony_mebli (
    id            INTEGER PRIMARY KEY,
    kod           TEXT NOT NULL UNIQUE,
    nazwa         TEXT NOT NULL,
    tytul_sekcji  TEXT NOT NULL,
    dane          TEXT NOT NULL,
    zrodlo        TEXT,                 -- NULL = wbudowany, 'import' = z pliku użytkownika
    kolejnosc     INTEGER NOT NULL DEFAULT 0,
    aktywny       INTEGER NOT NULL DEFAULT 1 CHECK (aktywny IN (0, 1))
);

INSERT INTO szablony_mebli (kod, nazwa, tytul_sekcji, kolejnosc, dane) VALUES
('kuchnia', 'Kuchnia', 'Zabudowa Kuchni', 1,
 '{"czesci":["Szafki pod blatem","Szafki wiszące","Słupki","Blat"],"wiersze":[
   {"podgrupa":"glowna","rodzaj":"Korpus","material":"Płyta meblowa laminowana","opis":"Kolor: do potwierdzenia","kategoria":"Płyta meblowa"},
   {"podgrupa":"glowna","rodzaj":"Fronty","material":"","opis":"Kolor: do potwierdzenia","kategoria":"Fronty MDF lakierowane"},
   {"podgrupa":"glowna","rodzaj":"Półki","material":"","opis":""},
   {"podgrupa":"glowna","rodzaj":"Cokół","material":"","opis":""},
   {"podgrupa":"glowna","rodzaj":"Plecy","material":"Płyta HDF","opis":"","kategoria":"Płyta HDF (plecy)"},
   {"podgrupa":"wyposazenie","rodzaj":"Szuflady","material":"","opis":""},
   {"podgrupa":"wyposazenie","rodzaj":"Systemy otwierania","material":"","opis":""},
   {"podgrupa":"wyposazenie","rodzaj":"Uchwyty","material":"","opis":"Do ustalenia"}]}'),
('szafa', 'Szafa / wnęka', 'Szafa we wnęce', 2,
 '{"czesci":["Szafa","Nadstawka"],"wiersze":[
   {"podgrupa":"glowna","rodzaj":"Korpus","material":"Płyta meblowa laminowana","opis":"Kolor: do potwierdzenia","kategoria":"Płyta meblowa"},
   {"podgrupa":"glowna","rodzaj":"Fronty","material":"","opis":"Kolor: do potwierdzenia"},
   {"podgrupa":"glowna","rodzaj":"Półki","material":"","opis":""},
   {"podgrupa":"glowna","rodzaj":"Bok wykończeniowy","material":"","opis":""},
   {"podgrupa":"wyposazenie","rodzaj":"Drążek / wieszak","material":"","opis":""},
   {"podgrupa":"wyposazenie","rodzaj":"Systemy otwierania","material":"","opis":""}]}'),
('garderoba', 'Garderoba', 'Garderoba', 3,
 '{"czesci":["Regały","Szafa z drzwiami"],"wiersze":[
   {"podgrupa":"glowna","rodzaj":"Korpus","material":"Płyta meblowa laminowana","opis":"","kategoria":"Płyta meblowa"},
   {"podgrupa":"glowna","rodzaj":"Półki","material":"","opis":""},
   {"podgrupa":"wyposazenie","rodzaj":"Drążek / wieszak","material":"","opis":""},
   {"podgrupa":"wyposazenie","rodzaj":"Szuflady","material":"","opis":""},
   {"podgrupa":"wyposazenie","rodzaj":"Oświetlenie","material":"","opis":""}]}'),
('lazienka', 'Łazienka / WC', 'Zabudowa Łazienki', 4,
 '{"czesci":["Szafka pod umywalkę","Zabudowa stelaża WC","Słupek"],"wiersze":[
   {"podgrupa":"glowna","rodzaj":"Korpus","material":"Płyta meblowa laminowana","opis":"Wersja wodoodporna","kategoria":"Płyta meblowa"},
   {"podgrupa":"glowna","rodzaj":"Fronty","material":"","opis":"Kolor: do potwierdzenia"},
   {"podgrupa":"glowna","rodzaj":"Blat","material":"HPL kompaktowy","opis":"","kategoria":"Blaty HPL kompaktowe"},
   {"podgrupa":"wyposazenie","rodzaj":"Systemy otwierania","material":"","opis":""}]}'),
('salon', 'RTV / salon', 'Zabudowa RTV', 5,
 '{"czesci":["Szafka RTV","Półki wiszące"],"wiersze":[
   {"podgrupa":"glowna","rodzaj":"Korpus","material":"Płyta meblowa laminowana","opis":"","kategoria":"Płyta meblowa"},
   {"podgrupa":"glowna","rodzaj":"Fronty","material":"","opis":""},
   {"podgrupa":"glowna","rodzaj":"Ażur","material":"","opis":""},
   {"podgrupa":"wyposazenie","rodzaj":"Oświetlenie","material":"","opis":""}]}'),
('przedpokoj', 'Wiatrołap / przedpokój', 'Zabudowa Przedpokoju', 6,
 '{"czesci":["Szafa","Siedzisko","Lustro"],"wiersze":[
   {"podgrupa":"glowna","rodzaj":"Korpus","material":"Płyta meblowa laminowana","opis":"","kategoria":"Płyta meblowa"},
   {"podgrupa":"glowna","rodzaj":"Fronty","material":"","opis":""},
   {"podgrupa":"glowna","rodzaj":"Siedzisko tapicerowane","material":"","opis":"","kategoria":"Tapicerka"},
   {"podgrupa":"wyposazenie","rodzaj":"Drążek / wieszak","material":"","opis":""}]}');

-- ---------------------------------------------------------------------------
-- Ustawienia: dane wykonawcy do nagłówka oferty i domyślne wartości oferty
-- ---------------------------------------------------------------------------

INSERT INTO ustawienia (klucz, wartosc, opis) VALUES
    ('firma_nazwa', '', 'Nazwa wykonawcy w nagłówku oferty i umowy'),
    ('firma_adres', '', 'Adres wykonawcy (może mieć kilka linii)'),
    ('firma_nip', '', 'NIP wykonawcy'),
    ('firma_telefon', '', 'Telefon wykonawcy'),
    ('firma_email', '', 'E-mail wykonawcy'),
    ('oferta_waznosc_dni', '30', 'Domyślna liczba dni ważności cen w nowej ofercie'),
    ('oferta_termin_realizacji', 'do uzgodnienia', 'Domyślny termin realizacji w nowej ofercie');
