-- ROBINWOOD – silnik terminów kalendarza (Faza 4a)
--
-- * zadania_kalendarza.klucz – stały identyfikator zadania wyliczonego w obrębie zlecenia
--   ('montaz', 'warsztat', 'rysunki', 'pomiar', 'zamowienie:<id>', 'potwierdzenie:<id>',
--   'potwierdzenie_montazu', 'pomieszczenie_gotowe', 'brakujace_informacje', 'platnosc:<id>');
--   pozwala przeliczać harmonogram bez gubienia zadań wykonanych (zamrożonych) i ręcznych (klucz NULL),
-- * zamowienia_materialow: data wymaganej dostawy i widełki użyte do wyliczenia (migawka),
-- * nowe ustawienia progów ostrzeżeń i odstępów zadań pomocniczych.

ALTER TABLE zadania_kalendarza ADD COLUMN klucz TEXT;
CREATE UNIQUE INDEX ux_zadania_zlecenie_klucz ON zadania_kalendarza(zlecenie_id, klucz) WHERE klucz IS NOT NULL;
CREATE INDEX ix_zadania_zamowienie ON zadania_kalendarza(zamowienie_id);

ALTER TABLE zamowienia_materialow ADD COLUMN data_wymaganej_dostawy TEXT;
ALTER TABLE zamowienia_materialow ADD COLUMN dni_min INTEGER;
ALTER TABLE zamowienia_materialow ADD COLUMN dni_max INTEGER;
ALTER TABLE zamowienia_materialow ADD COLUMN dni_srednia INTEGER;
CREATE INDEX ix_zamowienia_kategoria_dostawca ON zamowienia_materialow(kategoria_id, dostawca_id);

INSERT INTO ustawienia (klucz, wartosc, opis) VALUES
    ('prog_pomaranczowy_dni', '2', 'Niezamówiony materiał staje się pomarańczowy N dni przed datą krytyczną'),
    ('prog_zbliza_sie_dni', '3', 'Pomiar, rysunki i terminy klienta stają się żółte N dni przed terminem'),
    ('odstep_potwierdzenie_montazu_dni', '7', 'Potwierdzenie terminu montażu klientowi N dni przed montażem'),
    ('odstep_pomieszczenie_gotowe_dni', '3', 'Pomieszczenie gotowe do montażu N dni przed montażem');
