// Niezapisane zmiany (szkice) edytorów: rejestr funkcji „opróżniających” (zapis na serwer)
// i kopia awaryjna w localStorage.
//
// * Edytor rejestruje funkcję `oproznij` – wywoływaną przy ukryciu karty (`visibilitychange`),
//   zamykaniu strony (`pagehide`) oraz w Tauri przed schowaniem okna i przed „Zakończ”.
// * Kopia w localStorage chroni przed utratą danych, gdy zapis się nie uda (brak sieci,
//   twarde zamknięcie); edytor odtwarza ją po ponownym otwarciu.

type Oproznij = () => Promise<void>;

const rejestr = new Set<Oproznij>();
let nasluch = false;

function zainstalujNasluch() {
  if (nasluch || typeof window === "undefined") return;
  nasluch = true;
  document.addEventListener("visibilitychange", () => {
    if (document.visibilityState === "hidden") void oproznijWszystkieSzkice();
  });
  window.addEventListener("pagehide", () => void oproznijWszystkieSzkice());
}

/** Rejestruje funkcję zapisującą szkice; zwraca funkcję wyrejestrowującą. */
export function zarejestrujOproznianie(f: Oproznij): () => void {
  zainstalujNasluch();
  rejestr.add(f);
  return () => {
    rejestr.delete(f);
  };
}

/** Zapisuje wszystkie szkice (błędy pojedynczych edytorów nie przerywają pozostałych). */
export async function oproznijWszystkieSzkice(): Promise<void> {
  await Promise.allSettled([...rejestr].map((f) => f()));
}

const PREFIKS = "robinwood:szkic:";

export function zapiszKopie(klucz: string, dane: unknown): void {
  try {
    if (dane === null || dane === undefined || (typeof dane === "object" && Object.keys(dane).length === 0)) {
      localStorage.removeItem(PREFIKS + klucz);
    } else {
      localStorage.setItem(PREFIKS + klucz, JSON.stringify(dane));
    }
  } catch {
    /* brak localStorage (tryb prywatny, limit) – kopia awaryjna niedostępna */
  }
}

export function wczytajKopie<T>(klucz: string): T | null {
  try {
    const t = localStorage.getItem(PREFIKS + klucz);
    return t ? (JSON.parse(t) as T) : null;
  } catch {
    return null;
  }
}
