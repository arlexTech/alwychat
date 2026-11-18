import { subscribeChatList, type RoomSummary } from "$lib/backend";
import { applyDiffs } from "./list";

// The top of the list as it was last time, shown the moment the app starts; the core's list
// replaces it as soon as it has rows (a fraction of a second later, more on a slow phone).
const KEPT_PREFIX = "alwychat.chats:";
const KEPT_ROWS = 40;

/** The chat list, kept in sync with the core's live list. */
class ChatList {
  rooms = $state<RoomSummary[]>([]);
  loaded = $state(false);
  error = $state<string | null>(null);
  #key: string | null = null;
  /** The core's list while last time's rows are still on screen. */
  #arriving: RoomSummary[] | null = null;
  #saveTimer: ReturnType<typeof setTimeout> | undefined;

  /** Our own user ID (signed in). */
  userId = $state<string | null>(null);

  async start(userId: string) {
    this.userId = userId;
    this.error = null;
    this.#key = KEPT_PREFIX + userId;
    try {
      // Someone else's rows (another account signed in before) are never shown.
      for (const k of Object.keys(localStorage)) if (k.startsWith(KEPT_PREFIX) && k !== this.#key) localStorage.removeItem(k);
      const kept = JSON.parse(localStorage.getItem(this.#key) ?? "[]");
      if (!this.loaded && Array.isArray(kept) && kept.length) {
        this.rooms = kept;
        this.loaded = true;
        this.#arriving = [];
        // An account whose chats are all gone since: don't keep showing them.
        setTimeout(() => this.#arrived(), 4000);
      }
    } catch {
      // Damaged or blocked storage: the list comes from the core as usual.
    }
    try {
      await subscribeChatList((diffs) => {
        if (this.#arriving) {
          applyDiffs(this.#arriving, diffs);
          if (this.#arriving.length) this.#arrived();
        } else {
          applyDiffs(this.rooms, diffs);
        }
        this.loaded = true;
        this.#keep();
      });
    } catch (e) {
      this.error = String(e);
    }
  }

  #arrived() {
    if (!this.#arriving) return;
    this.rooms = this.#arriving;
    this.#arriving = null;
  }

  #keep() {
    clearTimeout(this.#saveTimer);
    this.#saveTimer = setTimeout(() => this.persist(), 2000);
  }

  /** Save the top of the list for the next start (also called when the app is hidden). */
  persist() {
    clearTimeout(this.#saveTimer);
    if (!this.#key || this.#arriving || !this.loaded) return;
    try {
      // Nobody is "sending" or in a call by the time this is shown again.
      const rows = this.rooms.slice(0, KEPT_ROWS).map((r) => ({
        ...r,
        in_call: [],
        latest: r.latest && { ...r.latest, is_sending: false },
      }));
      localStorage.setItem(this.#key, JSON.stringify(rows));
    } catch {
      // Full storage: the next start just shows the loading rows.
    }
  }

  /** Apply a change we know succeeded before the core re-emits the row (some, like
   *  mute, only show up there on a later update). The next diff overwrites it. */
  patch(roomId: string, change: Partial<RoomSummary>) {
    const i = this.rooms.findIndex((r) => r.id === roomId);
    if (i >= 0) this.rooms[i] = { ...this.rooms[i], ...change };
  }

  /** `signedOut`: the account is gone from this device, and so is its kept list. */
  reset(signedOut = false) {
    clearTimeout(this.#saveTimer);
    if (signedOut && this.#key) {
      try {
        localStorage.removeItem(this.#key);
      } catch {
        // Nothing kept then.
      }
    }
    this.rooms = [];
    this.loaded = false;
    this.#arriving = null;
  }
}

export const chatList = new ChatList();
if (typeof document !== "undefined")
  document.addEventListener("visibilitychange", () => document.visibilityState === "hidden" && chatList.persist());
