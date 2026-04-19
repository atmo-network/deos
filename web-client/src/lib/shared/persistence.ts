export function readStoredString(key: string): string | null {
  if (typeof window === "undefined") {
    return null;
  }
  try {
    const value = window.localStorage.getItem(key);
    return typeof value === "string" ? value : null;
  } catch {
    return null;
  }
}

export function writeStoredString(key: string, value: string): void {
  if (typeof window === "undefined") {
    return;
  }
  try {
    window.localStorage.setItem(key, value);
  } catch {
    // Ignore browser storage failures to keep client UX functional
  }
}

export function readStoredJson<T>(key: string): T | null {
  const raw = readStoredString(key);
  if (raw === null) {
    return null;
  }
  try {
    return JSON.parse(raw) as T;
  } catch {
    return null;
  }
}

export function writeStoredJson(key: string, value: unknown): void {
  if (typeof window === "undefined") {
    return;
  }
  try {
    window.localStorage.setItem(key, JSON.stringify(value));
  } catch {
    // Ignore browser storage failures to keep client UX functional
  }
}
