// Show what we had last time at once, then refresh it quietly ("stale while revalidate"):
// pages and panes start from `peek` and replace it when `fresh` answers, so opening something
// seen before never shows an empty screen or a spinner.
//
// Kept in memory for the run; `persist: true` entries also go to localStorage (small data
// only: space contents, not messages), so they're there right after a restart too.

const PREFIX = "alwychat.cache.";
/** Most persisted entries kept (oldest dropped). */
const MAX_PERSISTED = 60;

const memory = new Map<string, unknown>();

export function peek<T>(key: string, persist = false): T | undefined {
  if (memory.has(key)) return memory.get(key) as T;
  if (!persist) return undefined;
  try {
    const raw = localStorage.getItem(PREFIX + key);
    if (raw == null) return undefined;
    const value = JSON.parse(raw) as T;
    memory.set(key, value);
    return value;
  } catch {
    return undefined;
  }
}

export function remember<T>(key: string, value: T, persist = false): T {
  memory.set(key, value);
  if (persist) {
    try {
      localStorage.setItem(PREFIX + key, JSON.stringify(value));
      trim(key);
    } catch {
      // Full or blocked storage: memory still has it.
    }
  }
  return value;
}

/** Load it now and keep the answer for the next `peek`. */
export async function fresh<T>(key: string, load: () => Promise<T>, persist = false): Promise<T> {
  return remember(key, await load(), persist);
}

/** Warm the cache without waiting (a page about to be opened). Errors are ignored. */
export function prefetch<T>(key: string, load: () => Promise<T>, persist = false) {
  void fresh(key, load, persist).catch(() => {});
}

/** Persisted keys, least recently written first. */
const INDEX = PREFIX + "index";
function trim(key: string) {
  try {
    const keys = (JSON.parse(localStorage.getItem(INDEX) ?? "[]") as string[]).filter((k) => k !== key);
    keys.push(key);
    for (const old of keys.splice(0, Math.max(0, keys.length - MAX_PERSISTED))) localStorage.removeItem(PREFIX + old);
    localStorage.setItem(INDEX, JSON.stringify(keys));
  } catch {
    // Damaged index: start a new one.
    localStorage.setItem(INDEX, JSON.stringify([key]));
  }
}
