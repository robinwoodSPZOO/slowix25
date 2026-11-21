import type { HTMLAttributes, KeyboardEvent, ReactNode } from "react";

export interface WierszTabeliProps extends HTMLAttributes<HTMLTableRowElement> {
  zaznaczony?: boolean;
  /** Wiersz klikalny – obsługuje też Enter/Spację z klawiatury. */
  onWybierz?: () => void;
  children: ReactNode;
}

export function WierszTabeli({ zaznaczony, onWybierz, className, children, ...reszta }: WierszTabeliProps) {
  const klawisz = (e: KeyboardEvent<HTMLTableRowElement>) => {
    if (onWybierz && (e.key === "Enter" || e.key === " ") && e.target === e.currentTarget) {
      e.preventDefault();
      onWybierz();
    }
  };
  return (
    <tr
      className={[
        "wiersz-tabeli",
        onWybierz ? "wiersz-tabeli--klikalny" : "",
        zaznaczony ? "wiersz-tabeli--zaznaczony" : "",
        className ?? "",
      ]
        .filter(Boolean)
        .join(" ")}
      aria-selected={zaznaczony}
      tabIndex={onWybierz ? 0 : undefined}
      onClick={onWybierz}
      onKeyDown={klawisz}
      {...reszta}
    >
      {children}
    </tr>
  );
}
