// Obrazek z serwera jako adres blob: (pobrany z kluczem w nagłówku – klucz nie trafia do <img src>).

import { useEffect, useState } from "react";
import type { KlientApi } from "../../api/klient";

export function useObrazek(api: KlientApi | null, plikId: number, podglad = true): string | null {
  const [adres, ustawAdres] = useState<string | null>(null);
  useEffect(() => {
    if (!api) return;
    let url: string | null = null;
    let aktywny = true;
    api.pobierzPlik(plikId, podglad).then(
      (b) => {
        if (!aktywny) return;
        url = URL.createObjectURL(b);
        ustawAdres(url);
      },
      () => aktywny && ustawAdres(null),
    );
    return () => {
      aktywny = false;
      if (url) URL.revokeObjectURL(url);
    };
  }, [api, plikId, podglad]);
  return adres;
}
