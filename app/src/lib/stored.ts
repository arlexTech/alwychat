/** A JSON value kept in localStorage; `fallback` when it's missing or unreadable (a damaged
 * value must never stop the app from starting). */
export function stored<T>(key: string, fallback: T): T {
  try {
    const text = localStorage.getItem(key);
    return text === null ? fallback : ((JSON.parse(text) as T) ?? fallback);
  } catch {
    return fallback;
  }
}

/** `value` if it's a list (of strings unless told otherwise), else an empty one. */
export function asList<T = string>(value: unknown): T[] {
  return Array.isArray(value) ? (value as T[]) : [];
}
