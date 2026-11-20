import { subscribeChatList, type RoomSummary } from "$lib/backend";
import { applyDiffs } from "./list";
import type { ListDiff } from "$lib/backend";

/** How long a new message may wait for its chat to be loaded before it shows anyway. */
const HOLD_MS = 1000;

/** Chats that get a new message from someone else in these diffs → that message's time. */
function newMessages(rooms: RoomSummary[], diffs: ListDiff<RoomSummary>[]): Map<string, number> {
  const ids = new Map<string, number>();
  for (const d of diffs) {
    const carried = "values" in d ? d.values : "value" in d ? [d.value] : [];
    for (const r of carried) {
      if (r.is_invite || r.is_space || !r.latest || r.latest.is_own) continue;
      const before = rooms.find((o) => o.id === r.id)?.latest;
      if (!before) continue; // a chat appearing for the first time: just the list filling in
      const at = r.latest.timestamp ?? 0;
      if (at > (before.timestamp ?? 0)) ids.set(r.id, Math.max(at, ids.get(r.id) ?? 0));
    }
  }
  return ids;
}

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

  /** Diffs waiting until the chats they bring a new message to are loaded. */
  #held: ListDiff<RoomSummary>[] | null = null;
  /** Loads these chats (set by the open-chat store): a new message only shows on the list
   *  once a tap on it opens the chat with that message already there. */
  prepare: ((chats: Map<string, number>) => Promise<unknown>) | null = null;

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
          this.loaded = true;
          this.#keep();
        } else {
          this.#show(diffs);
        }
      });
    } catch (e) {
      this.error = String(e);
    }
  }

  #show(diffs: ListDiff<RoomSummary>[]) {
    // Later changes wait behind held ones, so the order of changes never shifts.
    if (this.#held) return void this.#held.push(...diffs);
    const fresh = this.prepare ? newMessages(this.rooms, diffs) : new Map<string, number>();
    if (!fresh.size) {
      applyDiffs(this.rooms, diffs);
      this.loaded = true;
      this.#keep();
      return;
    }
    const held = (this.#held = [...diffs]);
    const release = () => {
      // (Already shown by the timer, and maybe a newer hold since: not that one.)
      if (this.#held !== held) return;
      this.#held = null;
      applyDiffs(this.rooms, held);
      this.loaded = true;
      this.#keep();
    };
    const timer = setTimeout(release, HOLD_MS);
    this.prepare!(fresh)
      .catch(() => {})
      .finally(() => {
        clearTimeout(timer);
        release();
      });
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
    this.#held = null;
  }
}

export const chatList = new ChatList();
if (typeof document !== "undefined")
  document.addEventListener("visibilitychange", () => document.visibilityState === "hidden" && chatList.persist());
