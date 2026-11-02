import { getPresence, reportActivity, type Presence } from "$lib/backend";
import { tr, trn } from "$lib/i18n/index.svelte";

/** Our own activity is re-reported this often while the app is in front. */
const REPORT_EVERY_MS = 2 * 60 * 1000;

interface Seen extends Presence {
  /** When we asked (last_active_ago is relative to that). */
  fetchedAt: number;
}

/** "Online" / "Last seen 5 min ago" for the people on screen, and our own activity. */
class PresenceStore {
  #seen = $state<Record<string, Seen>>({});
  /** Ticks every minute so "x min ago" labels move on their own. */
  #now = $state(Date.now());
  #started = false;

  /** Ask the server about these people (results merge in when they arrive). */
  async refresh(userIds: string[]) {
    const ids = [...new Set(userIds)].filter(Boolean);
    if (!ids.length) return;
    try {
      const fetchedAt = Date.now();
      for (const p of await getPresence(ids)) this.#seen[p.user_id] = { ...p, fetchedAt };
    } catch {
      // Presence is a nicety: keep what we had.
    }
  }

  online(userId: string | null | undefined): boolean {
    return !!userId && !!this.#seen[userId]?.online;
  }

  /** "Online", "Last seen 5 min ago", … or null when unknown. */
  label(userId: string | null | undefined): string | null {
    const p = userId ? this.#seen[userId] : undefined;
    if (!p) return null;
    if (p.online) return tr("Online");
    if (p.last_active_ago_ms == null) return null;
    const ago = this.#now - (p.fetchedAt - p.last_active_ago_ms);
    const min = Math.floor(ago / 60_000);
    if (min < 1) return tr("Last seen just now");
    if (min < 60) return tr("Last seen {min} min ago", { min: min });
    const h = Math.floor(min / 60);
    if (h < 24) return tr("Last seen {h} h ago", { h: h });
    const d = Math.floor(h / 24);
    return d === 1 ? tr("Last seen yesterday") : trn(d, "Last seen {n} day ago", "Last seen {n} days ago");
  }

  /** Start reporting our activity (online while visible, away when hidden). Call once. */
  start() {
    if (this.#started) return;
    this.#started = true;
    const report = () => void reportActivity(document.visibilityState === "visible").catch(() => {});
    report();
    document.addEventListener("visibilitychange", report);
    setInterval(() => document.visibilityState === "visible" && report(), REPORT_EVERY_MS);
    setInterval(() => (this.#now = Date.now()), 60_000);
  }

  /** Re-send our state after the "Show when I'm active" setting changed. */
  reportNow() {
    void reportActivity(document.visibilityState === "visible").catch(() => {});
  }
}

export const presence = new PresenceStore();
