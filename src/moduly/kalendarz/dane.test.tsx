// @vitest-environment jsdom
// W1/W2: odświeżanie po zdarzeniach WebSocket i po ponownym połączeniu gniazda.

import { act, cleanup, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Zdarzenie } from "../../api/typy";
import { KontekstPolaczenia, type WartoscKontekstu } from "../../kontekst/Polaczenie";
import { OPOZNIENIE_ODSWIEZANIA_MS, useOdswiezaniePoZdarzeniu } from "./dane";

function Sonda({ f }: { f: () => void }) {
  useOdswiezaniePoZdarzeniu(f);
  return null;
}

function wartosc(z: Partial<WartoscKontekstu>): WartoscKontekstu {
  return { polaczenie: null, api: null, ostatnieZdarzenie: null, odswiez: () => {}, naZywo: true, wznowienia: 0, ...z };
}

const zd = (zasob: string, id = 1): Zdarzenie => ({ zasob, id, operacja: "zmiana" });

beforeEach(() => vi.useFakeTimers());
afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

describe("useOdswiezaniePoZdarzeniu", () => {
  it("zdarzenie kalendarza → jedno odświeżenie po 300 ms (seria scalona)", () => {
    const f = vi.fn();
    const { rerender } = render(<KontekstPolaczenia.Provider value={wartosc({})}><Sonda f={f} /></KontekstPolaczenia.Provider>);
    rerender(<KontekstPolaczenia.Provider value={wartosc({ ostatnieZdarzenie: zd("zadania_kalendarza", 1) })}><Sonda f={f} /></KontekstPolaczenia.Provider>);
    act(() => vi.advanceTimersByTime(100));
    rerender(<KontekstPolaczenia.Provider value={wartosc({ ostatnieZdarzenie: zd("zlecenia", 2) })}><Sonda f={f} /></KontekstPolaczenia.Provider>);
    act(() => vi.advanceTimersByTime(OPOZNIENIE_ODSWIEZANIA_MS + 10));
    expect(f).toHaveBeenCalledTimes(1);
  });

  it("W2: zdarzenie innego rodzaju do 300 ms po zdarzeniu kalendarza nie kasuje odświeżenia", () => {
    const f = vi.fn();
    const { rerender } = render(<KontekstPolaczenia.Provider value={wartosc({})}><Sonda f={f} /></KontekstPolaczenia.Provider>);
    rerender(<KontekstPolaczenia.Provider value={wartosc({ ostatnieZdarzenie: zd("zadania_kalendarza") })}><Sonda f={f} /></KontekstPolaczenia.Provider>);
    act(() => vi.advanceTimersByTime(100));
    rerender(<KontekstPolaczenia.Provider value={wartosc({ ostatnieZdarzenie: zd("oferty") })}><Sonda f={f} /></KontekstPolaczenia.Provider>);
    act(() => vi.advanceTimersByTime(OPOZNIENIE_ODSWIEZANIA_MS));
    expect(f).toHaveBeenCalledTimes(1);
  });

  it("zdarzenie spoza listy samo niczego nie odświeża", () => {
    const f = vi.fn();
    const { rerender } = render(<KontekstPolaczenia.Provider value={wartosc({})}><Sonda f={f} /></KontekstPolaczenia.Provider>);
    rerender(<KontekstPolaczenia.Provider value={wartosc({ ostatnieZdarzenie: zd("oferty") })}><Sonda f={f} /></KontekstPolaczenia.Provider>);
    act(() => vi.advanceTimersByTime(1000));
    expect(f).not.toHaveBeenCalled();
  });

  it("W1: po każdym ponownym połączeniu gniazda – pobranie od nowa", () => {
    const f = vi.fn();
    const { rerender } = render(<KontekstPolaczenia.Provider value={wartosc({})}><Sonda f={f} /></KontekstPolaczenia.Provider>);
    rerender(<KontekstPolaczenia.Provider value={wartosc({ naZywo: false })}><Sonda f={f} /></KontekstPolaczenia.Provider>);
    act(() => vi.advanceTimersByTime(1000));
    expect(f).not.toHaveBeenCalled();
    rerender(<KontekstPolaczenia.Provider value={wartosc({ naZywo: true, wznowienia: 1 })}><Sonda f={f} /></KontekstPolaczenia.Provider>);
    act(() => vi.advanceTimersByTime(OPOZNIENIE_ODSWIEZANIA_MS + 10));
    expect(f).toHaveBeenCalledTimes(1);
    rerender(<KontekstPolaczenia.Provider value={wartosc({ naZywo: false, wznowienia: 1 })}><Sonda f={f} /></KontekstPolaczenia.Provider>);
    rerender(<KontekstPolaczenia.Provider value={wartosc({ naZywo: true, wznowienia: 2 })}><Sonda f={f} /></KontekstPolaczenia.Provider>);
    act(() => vi.advanceTimersByTime(OPOZNIENIE_ODSWIEZANIA_MS + 10));
    expect(f).toHaveBeenCalledTimes(2);
  });
});
