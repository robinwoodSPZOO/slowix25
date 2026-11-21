/** Zapisuje plik pobrany z API (Blob) – w oknie Tauri trafia do folderu Pobrane. */
export function zapiszPlik(plik: Blob, nazwa: string) {
  const url = URL.createObjectURL(plik);
  const a = document.createElement("a");
  a.href = url;
  a.download = nazwa;
  document.body.appendChild(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(url), 10_000);
}
