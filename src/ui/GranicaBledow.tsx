// Granica błędów: błąd renderowania w module nie zostawia pustego białego okna – pokazuje
// komunikat po polsku i przycisk „Odśwież” (ponowne zamontowanie widoku).

import { Component, type ErrorInfo, type ReactNode } from "react";
import { RefreshCw, TriangleAlert } from "lucide-react";

interface Props {
  children: ReactNode;
  /** Nazwa widoku do komunikatu, np. „Kalendarz”. */
  nazwa?: string;
  /** Dodatkowe działanie przy „Odśwież” (np. wyczyszczenie uszkodzonej pamięci). */
  onOdswiez?: () => void;
  /** Klucz resetu – zmiana (np. ścieżki) czyści błąd. */
  reset?: unknown;
  kompaktowo?: boolean;
}

interface Stan {
  blad: Error | null;
  reset: unknown;
  licznik: number;
}

export class GranicaBledow extends Component<Props, Stan> {
  state: Stan = { blad: null, reset: this.props.reset, licznik: 0 };

  static getDerivedStateFromError(blad: Error): Partial<Stan> {
    return { blad };
  }

  static getDerivedStateFromProps(p: Props, s: Stan): Partial<Stan> | null {
    return p.reset !== s.reset ? { reset: p.reset, blad: null } : null;
  }

  componentDidCatch(blad: Error, info: ErrorInfo) {
    console.error(`Błąd widoku ${this.props.nazwa ?? ""}:`, blad, info.componentStack);
  }

  odswiez = () => {
    this.props.onOdswiez?.();
    this.setState((s) => ({ blad: null, licznik: s.licznik + 1 }));
  };

  render() {
    if (this.state.blad) {
      return (
        <div className={`granica-bledow${this.props.kompaktowo ? " granica-bledow--kompaktowo" : ""}`} role="alert">
          <TriangleAlert size={20} aria-hidden />
          <div>
            <strong>{this.props.nazwa ? `${this.props.nazwa}: ` : ""}coś poszło nie tak przy wyświetlaniu.</strong>
            <p>Dane na serwerze są bezpieczne. Spróbuj odświeżyć widok; jeśli błąd wraca, uruchom program ponownie.</p>
            <p className="granica-bledow__szczegoly">{this.state.blad.message}</p>
            <button type="button" className="przycisk przycisk--glowny" onClick={this.odswiez}>
              <RefreshCw size={16} aria-hidden /> Odśwież
            </button>
          </div>
        </div>
      );
    }
    return <div key={this.state.licznik} style={{ display: "contents" }}>{this.props.children}</div>;
  }
}
