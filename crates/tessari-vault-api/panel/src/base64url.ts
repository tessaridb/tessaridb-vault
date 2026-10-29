// Unpadded base64url, the encoding the vault's API uses for every byte string it carries.

/** `bytes` as unpadded base64url. */
export function encode(bytes: Uint8Array): string {
  return btoa(Array.from(bytes, (byte) => String.fromCharCode(byte)).join(""))
    .replaceAll("+", "-")
    .replaceAll("/", "_")
    .replace(/=+$/, "");
}

/** The bytes `text` encodes, or null when it is not unpadded base64url. */
export function decode(text: string): Uint8Array<ArrayBuffer> | null {
  if (!/^[A-Za-z0-9_-]*$/.test(text) || text.length % 4 === 1) {
    return null;
  }
  const standard = text.replaceAll("-", "+").replaceAll("_", "/");
  return Uint8Array.from(atob(standard.padEnd(Math.ceil(standard.length / 4) * 4, "=")), (character) => character.charCodeAt(0));
}
