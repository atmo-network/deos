/*
Domain: Browser session persistence
Owns: Defensive localStorage read/write helpers for client-side system state.
Excludes: Domain storage schemas, wallet semantics, and UI presentation.
Zone: System infrastructure; may use browser APIs but must not import product slices.
*/
export function readStoredString(key: string): string | null {
  if (typeof window === 'undefined') {
    return null;
  }
  try {
    const value = window.localStorage.getItem(key);
    return typeof value === 'string' ? value : null;
  } catch {
    return null;
  }
}

export function writeStoredString(key: string, value: string): void {
  if (typeof window === 'undefined') {
    return;
  }
  try {
    window.localStorage.setItem(key, value);
  } catch {
    // Ignore browser storage failures to keep client UX functional
  }
}

export function readStoredJson(key: string): unknown {
  const raw = readStoredString(key);
  if (raw === null) {
    return null;
  }
  try {
    return JSON.parse(raw);
  } catch {
    return null;
  }
}

export function writeStoredJson(key: string, value: unknown): void {
  if (typeof window === 'undefined') {
    return;
  }
  try {
    window.localStorage.setItem(key, JSON.stringify(value));
  } catch {
    // Ignore browser storage failures to keep client UX functional
  }
}
