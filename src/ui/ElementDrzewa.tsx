import type { CSSProperties, KeyboardEvent, ReactNode } from "react";
import { ChevronDown, ChevronRight, type LucideIcon } from "lucide-react";

export interface ElementDrzewaProps {
  etykieta: ReactNode;
  poziom?: number;
  ikona?: LucideIcon;
  zaznaczony?: boolean;
  /** undefined = liść (bez przełącznika). */
  rozwiniety?: boolean;
  licznik?: ReactNode;
  akcje?: ReactNode;
  onWybierz?: () => void;
  onPrzelacz?: () => void;
  onDwuklik?: () => void;
  onKlawisz?: (e: KeyboardEvent<HTMLDivElement>) => void;
}

/** Element drzewa (sekcja / część). Strzałki ←/→ zwijają i rozwijają. */
export function ElementDrzewa(p: ElementDrzewaProps) {
  const Ikona = p.ikona;
  const klawisz = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.target !== e.currentTarget) return;
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      p.onWybierz?.();
    } else if (e.key === "ArrowRight" && p.rozwiniety === false) {
      p.onPrzelacz?.();
    } else if (e.key === "ArrowLeft" && p.rozwiniety === true) {
      p.onPrzelacz?.();
    }
    p.onKlawisz?.(e);
  };
  return (
    <div
      role="treeitem"
      aria-selected={!!p.zaznaczony}
      aria-expanded={p.rozwiniety}
      aria-level={(p.poziom ?? 0) + 1}
      tabIndex={0}
      className={`element-drzewa${p.zaznaczony ? " element-drzewa--zaznaczony" : ""}`}
      style={{ "--poziom": p.poziom ?? 0 } as CSSProperties}
      onClick={p.onWybierz}
      onDoubleClick={p.onDwuklik}
      onKeyDown={klawisz}
    >
      <span
        className="element-drzewa__przelacznik"
        onClick={(e) => {
          if (p.rozwiniety === undefined) return;
          e.stopPropagation();
          p.onPrzelacz?.();
        }}
      >
        {p.rozwiniety === undefined ? null : p.rozwiniety ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
      </span>
      {Ikona && <Ikona size={15} aria-hidden />}
      <span className="element-drzewa__etykieta">{p.etykieta}</span>
      {p.licznik !== undefined && <span className="element-drzewa__licznik">{p.licznik}</span>}
      {p.akcje && (
        <span className="element-drzewa__akcje" onClick={(e) => e.stopPropagation()}>
          {p.akcje}
        </span>
      )}
    </div>
  );
}
