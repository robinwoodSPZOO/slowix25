// Fabryka fikcyjnych zadań do testów (dane wyłącznie testowe).
import type { ZadanieKalendarza } from "../../api/typy";

let nastepneId = 1;

export function zadanie(z: Partial<ZadanieKalendarza> & { data_od: string }): ZadanieKalendarza {
  const id = z.id ?? nastepneId++;
  return {
    id,
    zlecenie_id: 1,
    zlecenie_nazwa: "Kuchnia testowa",
    zamowienie_id: null,
    typ: "pomiar",
    klucz: null,
    tytul: `Zadanie testowe ${id}`,
    data_do: null,
    data_sweet_spot: null,
    data_planowa: null,
    data_krytyczna: null,
    termin: z.data_do ?? z.data_od,
    wyliczone: true,
    wykonano: null,
    uwagi: null,
    stan: { stan: "spokojnie", opis: "Spokojnie", dni_do_terminu: 5, przesuniecie_montazu_dni: null, nowy_termin_montazu: null },
    zamowienie: null,
    harmonogram_nieaktualny: false,
    harmonogram_nieaktualny_powod: null,
    ...z,
  };
}

export function zamowienie(sweet: string, planowa: string, krytyczna: string, z: Partial<ZadanieKalendarza> = {}): ZadanieKalendarza {
  return zadanie({
    typ: "zamowienie",
    data_od: sweet,
    data_do: krytyczna,
    data_sweet_spot: sweet,
    data_planowa: planowa,
    data_krytyczna: krytyczna,
    termin: planowa,
    zamowienie: {
      kategoria_id: 1, kategoria_nazwa: "Płyta meblowa", dostawca_id: null, dostawca_nazwa: null, opis: null,
      status: "do_zamowienia", data_wymaganej_dostawy: null, dni_min: 5, dni_max: 10, dni_srednia: 7,
      data_zamowienia: null, data_dostawy: null, oczekiwana_dostawa: null, dostawa_zagrozona: false,
      zablokowane: false, potwierdzic_do: null, wiersze_do_potwierdzenia: [],
    },
    ...z,
  });
}
