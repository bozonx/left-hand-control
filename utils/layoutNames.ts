const INVALID_LAYOUT_NAME_CHARS = /[\\/:*?"<>|]/;

export function normalizeLayoutName(name: string): string {
  return name.trim();
}

export function validateLayoutName(name: string): string | null {
  const normalized = normalizeLayoutName(name);
  if (!normalized) return "empty";
  if (normalized === "." || normalized === "..") return "reserved";
  if (normalized.startsWith(".")) return "leadingDot";
  if (INVALID_LAYOUT_NAME_CHARS.test(normalized)) return "invalidChars";
  if ([...normalized].some((ch) => ch < " " || ch === "\u007f")) {
    return "controlChars";
  }
  return null;
}
