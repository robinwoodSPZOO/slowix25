import { describe, expect, it } from "vitest";
import type { Czesc, Podpowiedzi, Specyfikacja, Wiersz } from "../../api/typy";
import {
  czyZmieniony,
  daneZWiersza,
  formatujIlosc,
  parsujIlosc,
  podgrupaDlaRodzaju,
  podpowiedziDla,
  poprawWybor,
  przesun,
  wierszePogrupowane,
} from "./logika";

const wiersz = (id: number, podgrupa: Wiersz["podgrupa"], rodzaj: string): Wiersz => ({
  id,
  czesc_id: 1,
  ident: null,
  podgrupa,
  rodzaj,
  material: null,
  opis: null,
  ilosc: null,
  jednostka: null,
  status: "ustalone",
  termin_potwierdzenia: null,
  kategoria_materialu_id: null,
  pozycja_slownika_id: null,
  cena_gr: null,
  material_z_korpusu: false,
  kolejnosc: id,
});

const poz = (slownik_kod: string, wartosc: string, podgrupa: "glowna" | "wyposazenie" | null = null) => ({
  id: 0,
  slownik_kod,
  wartosc,
  producent: null,
  dekor: null,
  kategoria_materialu_id: null,
  kolejnosc: 0,
  podgrupa,
});

const podpowiedzi: Podpowiedzi = {
  rodzaj_elementu: [poz("rodzaj_elementu", "Korpus"), poz("rodzaj_elementu", "Uchwyty", "wyposazenie")],
  material: [poz("material", "Płyta A"), poz("material", "płyta a")],
  plyta: [poz("plyta", "Płyta B")],
  okucie: [poz("okucie", "Zawias X")],
  agd: [poz("agd", "Piekarnik")],
  dekor: [poz("dekor", "D1 Dąb")],
};

describe("edytor specyfikacji – logika", () => {
  it("ilość: polski przecinek, puste i błędne", () => {
    expect(parsujIlosc("2,5")).toBe(2.5);
    expect(parsujIlosc(" 3 ")).toBe(3);
    expect(parsujIlosc("")).toBeNull();
    expect(parsujIlosc("abc")).toBeUndefined();
    expect(parsujIlosc("-1")).toBeUndefined();
    expect(formatujIlosc(2.5)).toBe("2,5");
    expect(formatujIlosc(null)).toBe("");
  });

  it("wykrywa zmiany szkicu (pusty tekst = null)", () => {
    const w = wiersz(1, "glowna", "Korpus");
    expect(czyZmieniony(w, daneZWiersza(w))).toBe(false);
    expect(czyZmieniony(w, { ...daneZWiersza(w), material: "" })).toBe(false);
    expect(czyZmieniony(w, { ...daneZWiersza(w), material: "Płyta" })).toBe(true);
  });

  it("grupuje wiersze: elementy, frezowanie, wyposażenie", () => {
    const c: Czesc = {
      id: 1,
      sekcja_id: 1,
      ident: null,
      nazwa: "Słupki",
      cena_gr: null,
      kolejnosc: 0,
      wiersze: [wiersz(1, "wyposazenie", "Uchwyty"), wiersz(2, "glowna", "Korpus")],
    };
    const g = wierszePogrupowane(c);
    expect(g.map((x) => x.podgrupa)).toEqual(["glowna", "wyposazenie"]);
    expect(g[0]!.wiersze[0]!.rodzaj).toBe("Korpus");
  });

  it("podpowiedzi zależą od podgrupy i typu sekcji, bez duplikatów", () => {
    expect(podpowiedziDla("rodzaj", "glowna", false, podpowiedzi)).toEqual(["Korpus"]);
    expect(podpowiedziDla("rodzaj", "wyposazenie", false, podpowiedzi)).toEqual(["Uchwyty"]);
    expect(podpowiedziDla("rodzaj", "glowna", true, podpowiedzi)).toEqual(["Piekarnik"]);
    expect(podpowiedziDla("material", "glowna", false, podpowiedzi)).toEqual(["Płyta A", "Płyta B"]);
    expect(podpowiedziDla("material", "wyposazenie", false, podpowiedzi)).toEqual(["Zawias X"]);
    expect(podpowiedziDla("opis", "glowna", false, podpowiedzi)).toEqual(["D1 Dąb"]);
    expect(podgrupaDlaRodzaju("uchwyty", podpowiedzi)).toBe("wyposazenie");
    expect(podgrupaDlaRodzaju("Nieznany", podpowiedzi)).toBeNull();
  });

  it("przesuwanie i wybór po zmianie drzewa", () => {
    expect(przesun([1, 2, 3], 2, -1)).toEqual([2, 1, 3]);
    expect(przesun([1, 2, 3], 3, 1)).toBeNull();
    const spec: Specyfikacja = {
      id: 1,
      zrodlo_id: null,
      zdjecia: [],
      wlasciciel: null,
      tylko_do_odczytu: false,
      sekcje: [
        { id: 5, specyfikacja_id: 1, ident: null, numer: "01", nazwa: "A", typ: "meble", wariant: null, cena_gr: null, stawka_vat: null, kolejnosc: 0, czesci: [] },
        {
          id: 6,
          specyfikacja_id: 1,
          ident: null,
          numer: "02",
          nazwa: "B",
          typ: "meble",
          wariant: null,
          cena_gr: null,
          stawka_vat: null,
          kolejnosc: 1,
          czesci: [{ id: 9, sekcja_id: 6, ident: null, nazwa: "c", cena_gr: null, kolejnosc: 0, wiersze: [] }],
        },
      ],
    };
    expect(poprawWybor(spec, null)).toEqual({ typ: "czesc", id: 9 });
    expect(poprawWybor(spec, { typ: "czesc", id: 123 })).toEqual({ typ: "czesc", id: 9 });
    expect(poprawWybor(spec, { typ: "sekcja", id: 5 })).toEqual({ typ: "sekcja", id: 5 });
  });
});
