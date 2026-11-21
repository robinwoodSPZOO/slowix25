import type { LucideIcon } from "lucide-react";

export type Stan = "spokojnie" | "zolty" | "pomaranczowy" | "czerwony" | "neutralny" | "akcent";

/** Status lub stan: zawsze ikona + tekst (nie sam kolor). */
export function EtykietaStatusu({ stan, ikona: Ikona, tekst, title }: { stan: Stan; ikona: LucideIcon; tekst: string; title?: string }) {
  return (
    <span className={`status status--${stan}`} title={title}>
      <Ikona size={13} aria-hidden />
      {tekst}
    </span>
  );
}
