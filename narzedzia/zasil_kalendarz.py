#!/usr/bin/env python3
"""Zasila bazę ROBINWOOD FIKCYJNYMI danymi do sprawdzania kalendarza (Faza 4b).

Tworzy zlecenia w różnych stanach względem dzisiejszej daty serwera: czerwone (z nowym terminem
montażu), pomarańczowe, żółte, spokojne, zablokowane „do potwierdzenia” (umowa ze specyfikacją),
kolizję warsztatu, nieaktualny harmonogram, zamówienie z zagrożoną dostawą, dni wolne użytkownika,
historię dostaw (statystyka z sugerowaną korektą widełek) i zadanie ręczne. Opcja --duzo dodaje
40 zleceń (~500 zadań) do sprawdzenia wydajności.

Użycie (serwer uruchomiony na pustej bazie):
    ROBINWOOD_KLUCZ=dev ROBINWOOD_BAZA=dane/test.db npm run serwer
    python3 narzedzia/zasil_kalendarz.py --adres http://127.0.0.1:47800 --klucz dev --baza dane/test.db [--duzo]

`--baza` (plik SQLite serwera) jest potrzebna tylko do wstawienia umowy ze specyfikacją – API umów
powstaje w osobnej fazie. Wszystkie nazwy są fikcyjne („Klient Testowy”, example.com).
"""

import argparse
import datetime as dt
import json
import sqlite3
import sys
import urllib.error
import urllib.request


class Api:
    def __init__(self, adres, klucz):
        self.adres = adres.rstrip("/") + "/api/v1"
        self.klucz = klucz

    def __call__(self, metoda, sciezka, dane=None, oczekiwane=(200, 201, 204)):
        req = urllib.request.Request(
            self.adres + sciezka,
            method=metoda,
            data=None if dane is None else json.dumps(dane).encode(),
            headers={"Authorization": f"Bearer {self.klucz}", "Content-Type": "application/json"},
        )
        try:
            with urllib.request.urlopen(req) as o:
                tekst = o.read().decode()
                return json.loads(tekst) if tekst else None
        except urllib.error.HTTPError as e:
            tekst = e.read().decode()
            if e.code in oczekiwane:
                return json.loads(tekst) if tekst else None
            raise SystemExit(f"{metoda} {sciezka} → {e.code}: {tekst}")


def iso(d):
    return d.isoformat()


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--adres", default="http://127.0.0.1:47800")
    p.add_argument("--klucz", default="dev")
    p.add_argument("--baza", help="plik bazy serwera (do umowy z pozycjami „do potwierdzenia”)")
    p.add_argument("--duzo", action="store_true", help="dodatkowo 40 zleceń (~500 zadań)")
    a = p.parse_args()
    api = Api(a.adres, a.klucz)
    dzis = dt.date.today()

    if api("GET", "/zlecenia"):
        sys.exit("Baza ma już zlecenia – uruchom skrypt na pustej bazie.")

    # --- Słowniki: dostawcy, widełki, czasy dostaw ---
    kat = {k["nazwa"]: k["id"] for k in api("GET", "/kategorie-materialow")}
    dost = {}
    for n in ["Hurtownia Płyt Testowa", "Lakiernia Przykładowa", "Okucia Testowe sp. z o.o.", "Blaty Przykład"]:
        dost[n] = api("POST", "/dostawcy", {"nazwa": n, "email": "zamowienia@example.com"})["id"]
    api("PUT", f"/kategorie-materialow/{kat['Płyta meblowa']}/widelki", {"dni_dostawy_min": 5, "dni_dostawy_max": 10, "domyslny_dostawca_id": dost["Hurtownia Płyt Testowa"]})
    api("PUT", f"/kategorie-materialow/{kat['Płyta HDF (plecy)']}/widelki", {"dni_dostawy_min": 5, "dni_dostawy_max": 10})
    api("PUT", f"/kategorie-materialow/{kat['Okucia']}/widelki", {"dni_dostawy_min": 3, "dni_dostawy_max": 5, "domyslny_dostawca_id": dost["Okucia Testowe sp. z o.o."]})
    api("POST", "/czasy-dostaw", {"kategoria_id": kat["Fronty MDF lakierowane"], "dostawca_id": dost["Lakiernia Przykładowa"], "dni_min": 15, "dni_max": 21})
    api("POST", "/czasy-dostaw", {"kategoria_id": kat["Fronty MDF frezowane"], "dostawca_id": dost["Lakiernia Przykładowa"], "dni_min": 18, "dni_max": 24})
    api("POST", "/czasy-dostaw", {"kategoria_id": kat["Blaty laminowane"], "dostawca_id": dost["Blaty Przykład"], "dni_min": 7, "dni_max": 14})

    # Dni wolne użytkownika – przed zleceniami, bo przesuwają terminy (stany dobierane niżej).
    pierwszy_pon_grudnia = next(dt.date(dzis.year, 12, d) for d in range(1, 8) if dt.date(dzis.year, 12, d).weekday() == 0)
    api("PUT", f"/kalendarz/dni-wolne/{iso(pierwszy_pon_grudnia + dt.timedelta(days=4))}", {"rodzaj": "inne", "opis": "Inwentaryzacja (test)"})
    api("POST", "/kalendarz/dni-wolne/zakres", {"od": f"{dzis.year}-12-28", "do": f"{dzis.year}-12-31", "rodzaj": "urlop", "opis": "Przerwa świąteczna (test)"})

    klienci = [api("POST", "/klienci", {"nazwa": f"Klient Testowy {i}", "email": f"klient{i}@example.com"})["id"] for i in range(1, 11)]

    def zlecenie(nazwa, klient, montaz, warsztat=10, czas_montazu=2, materialy=(), etap="zamowienia", umowa=None):
        d = {"klient_id": klient, "nazwa": nazwa, "etap": etap, "montaz_od": iso(montaz), "czas_warsztat_dni": warsztat, "czas_montazu_dni": czas_montazu}
        if umowa:
            d["umowa_id"] = umowa
        z = api("POST", "/zlecenia", d)
        for m in materialy:
            api("POST", f"/zlecenia/{z['id']}/materialy", m)
        api("POST", f"/zlecenia/{z['id']}/harmonogram")
        return z

    def zadania(zid):
        return api("GET", f"/kalendarz/zadania?zlecenie_id={zid}")

    def dobierz(nazwa, klient, materialy, cel, kategoria, od_dni=5, do_dni=80):
        """Szuka terminu montażu, przy którym zamówienie materiału `kategoria` ma stan `cel`."""
        z = None
        for n in range(od_dni, do_dni):
            m = dzis + dt.timedelta(days=n)
            if m.weekday() >= 5:
                continue
            if z is None:
                z = zlecenie(nazwa, klient, m, materialy=materialy)
            else:
                api("PUT", f"/zlecenia/{z['id']}", {"klient_id": klient, "nazwa": nazwa, "etap": "zamowienia", "montaz_od": iso(m), "czas_warsztat_dni": 10, "czas_montazu_dni": 2})
            for t in zadania(z["id"]):
                if t["typ"] == "zamowienie" and t["zamowienie"]["kategoria_id"] == kategoria and t["stan"]["stan"] == cel:
                    return z
        raise SystemExit(f"nie udało się dobrać terminu dla stanu {cel}")

    P = {"kategoria_id": kat["Płyta meblowa"]}
    MDF = {"kategoria_id": kat["Fronty MDF lakierowane"], "dostawca_id": dost["Lakiernia Przykładowa"]}
    FREZ = {"kategoria_id": kat["Fronty MDF frezowane"], "dostawca_id": dost["Lakiernia Przykładowa"]}
    BLAT = {"kategoria_id": kat["Blaty laminowane"], "dostawca_id": dost["Blaty Przykład"]}
    OK = {"kategoria_id": kat["Okucia"]}

    # 1. Czerwone: montaż za 3 tygodnie, fronty frezowane (18–24 dni) – za późno.
    czerwone = zlecenie("Kuchnia z wyspą – test czerwony", klienci[0], dzis + dt.timedelta(days=21), materialy=[P, FREZ, OK])
    # 2–3. Pomarańczowe i żółte – termin montażu dobrany silnikiem.
    dobierz("Garderoba – test pomarańczowy", klienci[1], [P, OK], "pomaranczowy", kat["Płyta meblowa"])
    dobierz("Szafa w zabudowie – test żółty", klienci[2], [MDF], "zolty", kat["Fronty MDF lakierowane"])
    # 4. Spokojne, z wykonanym pomiarem i kategorią bez widełek (ostrzeżenie brak_widelek).
    spokojne = zlecenie("Salon z biblioteczką – test spokojny", klienci[3], dzis + dt.timedelta(days=75), materialy=[P, MDF, BLAT, {"kategoria_id": kat["Tapicerka"]}], etap="pomiar")
    pomiar = next(t for t in zadania(spokojne["id"]) if t["typ"] == "pomiar")
    api("PUT", f"/kalendarz/zadania/{pomiar['id']}/wykonanie", {"wykonano": iso(dzis - dt.timedelta(days=1))}, oczekiwane=(200, 422))
    # 5–6. Kolizja warsztatu dwóch zleceń.
    zlecenie("Łazienka – test kolizji A", klienci[4], dzis + dt.timedelta(days=40), materialy=[P])
    zlecenie("Pralnia – test kolizji B", klienci[5], dzis + dt.timedelta(days=42), materialy=[P])
    # 7. Nieaktualny harmonogram (zlecenie bez terminu montażu po zmianie).
    n = zlecenie("Biuro domowe – test nieaktualny", klienci[6], dzis + dt.timedelta(days=50), materialy=[P, OK])
    api("PUT", f"/zlecenia/{n['id']}", {"klient_id": klienci[6], "nazwa": n["nazwa"], "etap": "rysunki", "czas_warsztat_dni": 10, "czas_montazu_dni": 2})
    # 8. Zamówione, czekamy na dostawę – dostawa zagrożona; drugi materiał dostarczony.
    z8 = zlecenie("Kuchnia narożna – test dostawy", klienci[7], dzis + dt.timedelta(days=24), materialy=[MDF, P], etap="warsztat")
    for t in zadania(z8["id"]):
        if t["typ"] != "zamowienie":
            continue
        if t["zamowienie"]["kategoria_id"] == kat["Fronty MDF lakierowane"]:
            api("PUT", f"/kalendarz/zadania/{t['id']}/wykonanie", {"wykonano": iso(dzis - dt.timedelta(days=2))})
        else:
            api("PUT", f"/kalendarz/zadania/{t['id']}/wykonanie", {"wykonano": iso(dzis - dt.timedelta(days=9)), "data_dostawy": iso(dzis - dt.timedelta(days=1))})
    # 9. Historia dostaw – rzeczywiste czasy dłuższe niż widełki (sugerowana korekta).
    for i, (zam, dni_dost) in enumerate([(60, 26), (90, 28), (120, 24)]):
        zz = api("POST", "/zlecenia", {"klient_id": klienci[8], "nazwa": f"Zlecenie archiwalne {i + 1} – test", "etap": "zakonczone", "montaz_od": iso(dzis - dt.timedelta(days=zam - 40)), "czas_warsztat_dni": 10, "czas_montazu_dni": 2})
        api("POST", f"/zlecenia/{zz['id']}/materialy", {**MDF, "data_zamowienia": iso(dzis - dt.timedelta(days=zam)), "data_dostawy": iso(dzis - dt.timedelta(days=zam - dni_dost))})
    # 10. Zablokowane „do potwierdzenia” – umowa ze specyfikacją (bezpośrednio w bazie).
    if a.baza:
        db = sqlite3.connect(a.baza, timeout=10)
        db.execute("PRAGMA foreign_keys = ON")
        spec = db.execute("INSERT INTO specyfikacje DEFAULT VALUES").lastrowid
        sek = db.execute("INSERT INTO sekcje_specyfikacji (specyfikacja_id, numer, nazwa) VALUES (?, '01', 'Zabudowa kuchni')", (spec,)).lastrowid
        cz = db.execute("INSERT INTO czesci_specyfikacji (sekcja_id, nazwa) VALUES (?, 'Szafki górne')", (sek,)).lastrowid
        db.execute("INSERT INTO wiersze_specyfikacji (czesc_id, rodzaj, material, opis, status, kategoria_materialu_id) VALUES (?, 'Fronty', 'MDF lakier – kolor do wyboru', 'klient wybiera odcień', 'do_potwierdzenia', ?)", (cz, kat["Fronty MDF lakierowane"]))
        db.execute("INSERT INTO wiersze_specyfikacji (czesc_id, rodzaj, material, status, kategoria_materialu_id) VALUES (?, 'Korpus', 'Płyta testowa biała 18 mm', 'ustalone', ?)", (cz, kat["Płyta meblowa"]))
        umowa = db.execute(
            "INSERT INTO umowy (rok, nr_kolejny, numer, klient_id, specyfikacja_id, data_zawarcia, status, dni_na_brakujace_informacje) VALUES (?, 1, ?, ?, ?, ?, 'podpisana', 14)",
            (dzis.year, f"99/{dzis.year}", klienci[9], spec, iso(dzis - dt.timedelta(days=5))),
        ).lastrowid
        db.commit()
        db.close()
        zlecenie("Kuchnia z kolumną – test do potwierdzenia", klienci[9], dzis + dt.timedelta(days=38), materialy=[P, MDF], etap="umowa", umowa=umowa)
    else:
        print("(pominięto zlecenie zablokowane – podaj --baza)")

    # 11. Zadanie ręczne.
    api("POST", "/kalendarz/zadania", {"typ": "inne", "tytul": "Odbiór próbek frontów od lakiernika (test)", "data_od": iso(dzis + dt.timedelta(days=2)), "zlecenie_id": czerwone["id"]})

    # 12. Duża liczba zleceń (wydajność).
    if a.duzo:
        for i in range(40):
            zlecenie(f"Zlecenie testowe nr {i + 1}", klienci[i % 10], dzis + dt.timedelta(days=20 + (i * 53) % 70), warsztat=5 + i % 10, materialy=[P, OK, MDF if i % 2 else BLAT])

    wszystkie = api("GET", f"/kalendarz/zadania?od={iso(dzis - dt.timedelta(days=60))}&do={iso(dzis + dt.timedelta(days=120))}")
    stany = {}
    for t in wszystkie:
        stany[t["stan"]["stan"]] = stany.get(t["stan"]["stan"], 0) + 1
    print(f"Zlecenia: {len(api('GET', '/zlecenia'))}, zadania: {len(wszystkie)}, stany: {stany}")


if __name__ == "__main__":
    main()
