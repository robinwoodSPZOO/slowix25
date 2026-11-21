import type { ButtonHTMLAttributes } from "react";
import type { LucideIcon } from "lucide-react";

export type WariantPrzycisku = "glowny" | "drugorzedny" | "tekstowy" | "niebezpieczny";

export interface PrzyciskProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  wariant?: WariantPrzycisku;
  maly?: boolean;
  ikona?: LucideIcon;
  /** Sam przycisk-ikona: `etykieta` trafia wtedy do aria-label i podpowiedzi. */
  tylkoIkona?: boolean;
  etykieta?: string;
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
  ...reszta
}: PrzyciskProps) {
  const klasy = [
    "przycisk",
    `przycisk--${wariant}`,
    maly ? "przycisk--maly" : "",
    tylkoIkona ? "przycisk--ikona" : "",
    className ?? "",
  ]
    .filter(Boolean)
    .join(" ");
  return (
    <button
      type={type}
      className={klasy}
      aria-label={tylkoIkona ? etykieta : undefined}
      title={tylkoIkona ? etykieta : reszta.title}
      {...reszta}
    >
      {Ikona && <Ikona size={maly ? 14 : 16} aria-hidden />}
      {!tylkoIkona && (children ?? etykieta)}
    </button>
  );
}
