// Downloading the two halves of an SSH key as text, with no server round trip.

/** The field label as a safe filename, with a conventional fallback. */
export function sshDownloadName(label: string, half: "public" | "private"): string {
  const base = label
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .replace(/-{2,}/g, "-");
  const name = base === "" ? "id-ed25519" : base;
  return half === "public" ? `${name}.pub` : name;
}

/** Saves `contents` under `name`, releasing the temporary object URL immediately after the click. */
export function downloadText(name: string, contents: string): void {
  const url = URL.createObjectURL(new Blob([contents], { type: "text/plain;charset=utf-8" }));
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = name;
  anchor.click();
  URL.revokeObjectURL(url);
}
