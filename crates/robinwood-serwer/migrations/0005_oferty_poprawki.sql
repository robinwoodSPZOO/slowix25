-- ROBINWOOD – poprawki Generatora Ofert po recenzji (numer 0005: 0003 i 0004 to silnik terminów).
--
-- 1. Licznik wersji rekordu (`wersja`) i znacznik `zmieniono` – wykrywanie równoczesnej edycji
--    tego samego elementu na dwóch stanowiskach (API zwraca 409 przy konflikcie pola).
-- 2. Migawka danych inwestora i wykonawcy w wersji oferty (`migawka`, JSON) zapisywana przy
--    wysłaniu – dokument wysłanej wersji nie zmienia się po edycji klienta lub danych firmy.
-- 3. Pomniejszona kopia zdjęcia do dokumentów i podglądu (`pliki.podglad_sciezka`) i wymiary.
-- 4. Sekcje AGD bez stawki VAT dostają 23% (wcześniej brały stawkę oferty).

ALTER TABLE sekcje_specyfikacji ADD COLUMN wersja INTEGER NOT NULL DEFAULT 1;
ALTER TABLE sekcje_specyfikacji ADD COLUMN zmieniono TEXT;
ALTER TABLE czesci_specyfikacji ADD COLUMN wersja INTEGER NOT NULL DEFAULT 1;
ALTER TABLE czesci_specyfikacji ADD COLUMN zmieniono TEXT;
ALTER TABLE wiersze_specyfikacji ADD COLUMN wersja INTEGER NOT NULL DEFAULT 1;
ALTER TABLE wiersze_specyfikacji ADD COLUMN zmieniono TEXT;
ALTER TABLE wersje_oferty ADD COLUMN wersja INTEGER NOT NULL DEFAULT 1;

-- JSON: {"inwestor": {"nazwa", "adres_ulica", "adres_kod", "adres_miasto", "nip", "email", "telefon"},
--        "wykonawca": {"nazwa", "adres", "nip", "telefon", "email"}}
ALTER TABLE wersje_oferty ADD COLUMN migawka TEXT;
ALTER TABLE wersje_oferty ADD COLUMN wyslano TEXT;

ALTER TABLE pliki ADD COLUMN podglad_sciezka TEXT;
ALTER TABLE pliki ADD COLUMN szerokosc INTEGER;
ALTER TABLE pliki ADD COLUMN wysokosc INTEGER;
CREATE INDEX ix_pliki_sha ON pliki(sha256);
CREATE INDEX ix_zdjecia_plik ON zdjecia_specyfikacji(plik_id);

UPDATE sekcje_specyfikacji SET stawka_vat = 23 WHERE typ = 'agd' AND stawka_vat IS NULL;

-- Istniejące oferty, które wyszły ze szkicu: migawka najnowszej wersji z bieżących danych.
UPDATE wersje_oferty
SET migawka = (
        SELECT json_object(
            'inwestor', json_object(
                'nazwa', k.nazwa, 'adres_ulica', k.adres_ulica, 'adres_kod', k.adres_kod,
                'adres_miasto', k.adres_miasto, 'nip', k.nip, 'email', k.email, 'telefon', k.telefon),
            'wykonawca', json_object(
                'nazwa', COALESCE((SELECT wartosc FROM ustawienia WHERE klucz = 'firma_nazwa'), ''),
                'adres', COALESCE((SELECT wartosc FROM ustawienia WHERE klucz = 'firma_adres'), ''),
                'nip', COALESCE((SELECT wartosc FROM ustawienia WHERE klucz = 'firma_nip'), ''),
                'telefon', COALESCE((SELECT wartosc FROM ustawienia WHERE klucz = 'firma_telefon'), ''),
                'email', COALESCE((SELECT wartosc FROM ustawienia WHERE klucz = 'firma_email'), '')))
        FROM oferty o JOIN klienci k ON k.id = o.klient_id
        WHERE o.id = wersje_oferty.oferta_id),
    wyslano = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
WHERE oferta_id IN (SELECT id FROM oferty WHERE status <> 'szkic')
  AND nr_wersji = (SELECT MAX(x.nr_wersji) FROM wersje_oferty x WHERE x.oferta_id = wersje_oferty.oferta_id);
