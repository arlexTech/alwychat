import { SvelteMap } from "svelte/reactivity";
import { profileExtras, type ProfileExtras } from "$lib/backend";

/** Profiles are re-read at most this often per person. */
const FRESH_MS = 2 * 60 * 1000;

const ourZone = () => Intl.DateTimeFormat().resolvedOptions().timeZone;

/** Status and time zone of the people on screen (extended profiles), ours included. */
class Profiles {
  #extras = new SvelteMap<string, ProfileExtras>();
  #fetchedAt = new Map<string, number>();
  /** Ticks every minute so local times move on. */
  #now = $state(Date.now());
  #timer: ReturnType<typeof setInterval> | undefined;

  /** Fetch these people's profiles unless read recently (`force`: anyway). */
  async refresh(userIds: string[], force = false) {
    const now = Date.now();
    const ids = [...new Set(userIds)].filter(
      (id) => id && (force || now - (this.#fetchedAt.get(id) ?? 0) > FRESH_MS),
    );
    if (!ids.length) return;
    for (const id of ids) this.#fetchedAt.set(id, now);
    this.#timer ??= setInterval(() => (this.#now = Date.now()), 60_000);
    try {
      const found = await profileExtras(ids);
      for (const id of ids) {
        const e = found.find((f) => f.user_id === id);
        if (e) this.#extras.set(id, e);
        else this.#extras.delete(id);
      }
    } catch {
      // A nicety: keep what we had.
    }
  }

  get(userId: string | null | undefined): ProfileExtras | undefined {
    return userId ? this.#extras.get(userId) : undefined;
  }

  /** "📚 Studying", or null. */
  statusLabel(userId: string | null | undefined): string | null {
    const s = this.get(userId)?.status;
    return s ? [s.emoji, s.text].filter(Boolean).join(" ") : null;
  }

  /** "22:14 their time" when their time zone differs from ours (by offset, now), else null. */
  localTime(userId: string | null | undefined): string | null {
    const zone = this.get(userId)?.time_zone;
    if (!zone) return null;
    const at = new Date(this.#now);
    try {
      const fmt = (timeZone: string) =>
        new Intl.DateTimeFormat(undefined, { timeZone, hour: "2-digit", minute: "2-digit" }).format(at);
      const theirs = fmt(zone);
      return theirs === fmt(ourZone()) ? null : `${theirs} their time`;
    } catch {
      return null; // unknown zone name
    }
  }
}

export const profiles = new Profiles();
export { ourZone };
