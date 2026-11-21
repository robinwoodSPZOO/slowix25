import { describe, expect, it } from "vitest";
import przyklad from "../../../docs/przyklady/kategorie-materialow.csv?raw";
import { parsujKategorieCsv, podzielCsv } from "./importCsv";

describe("podzielCsv", () => {
  it("cudzysłowy, średnik i nowa linia w polu, CRLF, BOM, puste wiersze", () => {
    const t = '\uFEFFa;b\r\n"x;y";"z ""cyt"""\r\n\r\n"wiele\nlinii";2\n';
    expect(podzielCsv(t)).toEqual([
      { pola: ["a", "b"], wiersz: 1 },
      { pola: ["x;y", 'z "cyt"'], wiersz: 2 },
      { pola: ["wiele\nlinii", "2"], wiersz: 4 },
    ]);
  });
});

describe("parsujKategorieCsv", () => {
  it("poprawne i pominięte wiersze z powodem", () => {
    const t = [
      "kategoria;przykladowe_materialy;liczba_dokumentow;dostawca;czas_dostawy_min_dni;czas_dostawy_max_dni",
      "Płyta meblowa;płyta testowa 18 mm;12;Hurtownia Testowa;5;10",
      "Okucia;;3;;;",
      ";bez nazwy;1;;1;2",
      "Fronty testowe;;2;;dużo;10",
      "Szkło testowe;;1;;7;",
      "Blaty testowe;;1;;9;3",
      "Uchwyty;;1;;2,0;4",
    ].join("\n");
    const w = parsujKategorieCsv(t);
    expect(w.blad).toBeNull();
    expect(w.wiersze.map((x) => [x.wiersz, x.kategoria, x.dostawca, x.czas_dostawy_min_dni, x.czas_dostawy_max_dni])).toEqual([
      [2, "Płyta meblowa", "Hurtownia Testowa", 5, 10],
      [3, "Okucia", null, null, null],
      [8, "Uchwyty", null, 2, 4],
    ]);
    expect(w.pominiete.map((p) => p.wiersz)).toEqual([4, 5, 6, 7]);
    expect(w.pominiete[1]!.powod).toContain("nie jest liczbą");
  });
  it("kolumny w innej kolejności, wielkie litery", () => {
    const w = parsujKategorieCsv("Dostawca;KATEGORIA;czas_dostawy_max_dni;czas_dostawy_min_dni\nHurt;Płyta;10;5");
    expect(w.wiersze[0]).toMatchObject({ kategoria: "Płyta", dostawca: "Hurt", czas_dostawy_min_dni: 5, czas_dostawy_max_dni: 10 });
  });
  it("błędy całego pliku", () => {
    expect(parsujKategorieCsv("").blad).toContain("pusty");
    expect(parsujKategorieCsv("nazwa;dostawca\nx;y").blad).toContain("kategoria");
    expect(parsujKategorieCsv("kategoria,dostawca\nx,y").blad).toContain("średnik");
  });
  it("przykładowy plik z repozytorium (dane fikcyjne)", () => {
    const w = parsujKategorieCsv(przyklad);
    expect(w.blad).toBeNull();
    expect(w.wiersze.length).toBeGreaterThan(5);
    expect(w.pominiete.length).toBeGreaterThan(0); // przykład celowo zawiera błędny wiersz
  });
});
