import { SvelteMap } from "svelte/reactivity";
import { getDrafts, onDraft, setDraft, type Draft } from "$lib/backend";

/** Typing pauses this long before the draft is saved to the server. */
const SAVE_DELAY_MS = 1_500;
/** Sync echoes of our own saves arrive within this. */
const ECHO_WINDOW_MS = 10_000;

/** Unsent text per chat, shown in the chat list and restored in the composer. */
class Drafts {
  #drafts = new SvelteMap<string, Draft>();
  #timers = new Map<string, ReturnType<typeof setTimeout>>();
  #started = false;

  async start() {
    if (this.#started) return;
    this.#started = true;
    onDraft(({ room_id, draft }) => this.#merge(room_id, draft));
    // The app may be killed in the background: save what's pending first.
    document.addEventListener("visibilitychange", () => {
      if (document.hidden) for (const roomId of [...this.#timers.keys()]) this.flush(roomId);
    });
    try {
      for (const [roomId, draft] of Object.entries(await getDrafts())) this.#merge(roomId, draft);
    } catch {
      // Not signed in yet or no store: drafts show up as they sync.
    }
  }

  get(roomId: string): Draft | undefined {
    return this.#drafts.get(roomId);
  }

  /** The composer changed: kept here at once, saved after a pause. */
  set(roomId: string, text: string, replyTo: string | null) {
    const current = this.#drafts.get(roomId);
    if ((current?.text ?? "") === text && (current?.reply_to ?? null) === replyTo) return;
    if (text.trim()) this.#drafts.set(roomId, { text, reply_to: replyTo, updated: Date.now() });
    else this.#drafts.delete(roomId);
    clearTimeout(this.#timers.get(roomId));
    this.#timers.set(
      roomId,
      setTimeout(() => this.flush(roomId), SAVE_DELAY_MS),
    );
  }

  /** Save now (leaving the chat, sending). */
  flush(roomId: string) {
    const timer = this.#timers.get(roomId);
    if (timer === undefined) return;
    clearTimeout(timer);
    this.#timers.delete(roomId);
    const draft = this.#drafts.get(roomId) ?? null;
    if (!draft) this.#clearedAt.set(roomId, Date.now());
    setDraft(roomId, draft).catch(() => {});
  }

  /** When we last cleared each draft, so a late echo of the old text doesn't bring it back. */
  #clearedAt = new Map<string, number>();

  /** From sync. Edits not saved yet win; otherwise the newest version does. */
  #merge(roomId: string, draft: Draft | null) {
    if (this.#timers.has(roomId)) return;
    const current = this.#drafts.get(roomId);
    if (!draft) {
      // An empty draft has no date: ignore it right after we typed (it's an older echo).
      if (!current || current.updated < Date.now() - ECHO_WINDOW_MS) this.#drafts.delete(roomId);
    } else if (draft.updated > (current?.updated ?? this.#clearedAt.get(roomId) ?? 0)) {
      this.#drafts.set(roomId, draft);
    }
  }
}

export const drafts = new Drafts();
