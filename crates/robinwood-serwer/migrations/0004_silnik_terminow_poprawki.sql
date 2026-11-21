-- ROBINWOOD – poprawki silnika terminów po recenzji (Faza 4a)
--
-- * zlecenia.harmonogram_nieaktualny_powod – ustawiane, gdy automatyczne przeliczenie
--   harmonogramu się nie udało (np. usunięto termin montażu); NULL = harmonogram aktualny,
-- * dziennik_zmian.opis – dodatkowy opis wpisu (np. „zamówiono mimo blokady …”),
-- * indeksy pod zapytania z zakresem dat: niewykonane zadania (widok tygodnia, „do zamówienia”)
--   i koniec okresu zadania (filtr `od`).

ALTER TABLE zlecenia ADD COLUMN harmonogram_nieaktualny_powod TEXT;
ALTER TABLE dziennik_zmian ADD COLUMN opis TEXT;
CREATE INDEX ix_zadania_niewykonane ON zadania_kalendarza(data_od) WHERE wykonano IS NULL;
CREATE INDEX ix_zadania_koniec ON zadania_kalendarza(COALESCE(data_do, data_od));
