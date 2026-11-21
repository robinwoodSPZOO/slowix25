import type { ButtonHTMLAttributes, MouseEvent } from "react";
import type { LucideIcon } from "lucide-react";
import { useAkcja } from "../lib/akcje";

export type WariantPrzycisku = "glowny" | "drugorzedny" | "tekstowy" | "niebezpieczny";

export interface PrzyciskProps extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, "onClick"> {
  wariant?: WariantPrzycisku;
  maly?: boolean;
  ikona?: LucideIcon;
  /** Sam przycisk-ikona: `etykieta` trafia wtedy do aria-label i podpowiedzi. */
  tylkoIkona?: boolean;
  etykieta?: string;
  /**
   * Gdy zwróci obietnicę (np. `onClick={() => api.zapisz()}`), przycisk jest zablokowany do jej
   * zakończenia – podwójne kliknięcie nie wyśle żądania dwa razy (`src/lib/akcje.ts`).
   */
  onClick?: (e: MouseEvent<HTMLButtonElement>) => unknown;
}

/** Przycisk: główny / drugorzędny / tekstowy / niebezpieczny. */
export function Przycisk({
  wariant = "drugorzedny",
  maly,
  ikona: Ikona,
  tylkoIkona,
  etykieta,
  className,
  children,
  type = "button",
  onClick,
  disabled,
  ...reszta
}: PrzyciskProps) {
  const { trwa, uruchomJesliObietnica } = useAkcja();
  const klasy = [
    "przycisk",
    `przycisk--${wariant}`,
    maly ? "przycisk--maly" : "",
    tylkoIkona ? "przycisk--ikona" : "",
    trwa ? "przycisk--trwa" : "",
    className ?? "",
  ]
    .filter(Boolean)
    .join(" ");
  // Synchroniczna blokada: drugie kliknięcie w trakcie żądania jest ignorowane. W trakcie żądania
  // przycisk ma aria-disabled (nie `disabled`), żeby nie tracić fokusu klawiatury.
  const klik = onClick ? (e: MouseEvent<HTMLButtonElement>) => uruchomJesliObietnica(() => onClick(e)) : undefined;
  return (
    <button
      type={type}
      className={klasy}
      aria-label={tylkoIkona ? etykieta : undefined}
      title={tylkoIkona ? etykieta : reszta.title}
      disabled={disabled}
      aria-disabled={trwa || undefined}
      aria-busy={trwa || undefined}
      onClick={klik}
      {...reszta}
    >
      {Ikona && <Ikona size={maly ? 14 : 16} aria-hidden />}
      {!tylkoIkona && (children ?? etykieta)}
    </button>
  );
}
