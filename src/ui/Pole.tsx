import type { ReactNode } from "react";
import { CircleAlert } from "lucide-react";

export interface PoleProps {
  etykieta: string;
  /** id kontrolki w środku (powiązanie etykiety). */
  id: string;
  pomoc?: string;
  blad?: string | null;
  className?: string;
  children: ReactNode;
}

/** Pole formularza z etykietą, podpowiedzią i stanem błędu (ikona + tekst). */
export function Pole({ etykieta, id, pomoc, blad, className, children }: PoleProps) {
  return (
    <div className={`pole${blad ? " pole--blad" : ""}${className ? ` ${className}` : ""}`}>
      <label className="pole__etykieta" htmlFor={id}>
        {etykieta}
      </label>
      {children}
      {blad ? (
        <span className="pole__blad" role="alert">
          <CircleAlert size={12} aria-hidden />
          {blad}
        </span>
      ) : (
        pomoc && <span className="pole__pomoc">{pomoc}</span>
      )}
    </div>
  );
}
