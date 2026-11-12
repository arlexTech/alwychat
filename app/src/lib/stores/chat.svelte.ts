import { closeChat, loadNewer, loadOlder, openChat, type ChatStyle, type Pins, type TimelineEntry } from "$lib/backend";
import { applyDiffs } from "./list";
import { peek, remember } from "$lib/cache";

/** Older messages are fetched until at least this many entries are loaded. */
const MIN_ENTRIES = 30;

// What a chat showed when we left it, drawn the moment it's opened again (also after a
// restart) while the core rebuilds its timeline; the live one then takes over in place.
const SNAPSHOT_ENTRIES = 30;
const SNAPSHOT_ROOMS = 15;
const SNAPSHOT_KEY = "alwychat.snapshots";

function loadSnapshots(): Map<string, TimelineEntry[]> {
  try {
    const raw = JSON.parse(localStorage.getItem(SNAPSHOT_KEY) ?? "[]");
    return new Map(Array.isArray(raw) ? raw : []);
  } catch {
    return new Map();
  }
}

/** Row keys shared by a snapshot message and the same message in the live timeline, so the
 *  swap redraws nothing (prefixed: they never clash with the core's own keys). */
const snapKey = (e: TimelineEntry) =>
  e.type === "message" ? `snap:${e.item_id}` : e.key.startsWith("snap:") ? e.key : `snap:${e.key}`;

/** The chat on screen: its live timeline, kept in sync with the core. */
class Chat {
  roomId = $state<string | null>(null);
  // Plain objects, replaced as a whole on each batch: thousands of deeply reactive entries
  // made every update (and every read while drawing) slow in long chats.
  entries = $state.raw<TimelineEntry[]>([]);
  loaded = $state(false);
  atStart = $state(false);
  loadingOlder = $state(false);
  error = $state<string | null>(null);
  /** Names of people typing right now. */
  typing = $state<string[]>([]);
  /** Theme, quick reaction and nicknames; null until loaded (the last one seen is reused). */
  style = $state<ChatStyle | null>(null);
  #styles = new Map<string, ChatStyle>();
  #snapshots: Map<string, TimelineEntry[]> | null = null;
  /** The live timeline answered (until then a snapshot may be on screen). */
  #live = false;
  #saveTimer: ReturnType<typeof setTimeout> | undefined;

  /** Keep what this chat shows now for its next opening. */
  #keepSnapshot(now = false) {
    const roomId = this.roomId;
    if (!roomId || this.focused || !this.loaded || !this.entries.length) return;
    const snaps = (this.#snapshots ??= loadSnapshots());
    snaps.delete(roomId);
    snaps.set(
      roomId,
      // Sent messages only (not ones still sending), and no "new messages" line.
      this.entries
        .slice(-SNAPSHOT_ENTRIES)
        .filter((e) => e.type !== "read_marker" && !(e.type === "message" && !e.item_id.startsWith("$"))),
    );
    while (snaps.size > SNAPSHOT_ROOMS) snaps.delete(snaps.keys().next().value!);
    clearTimeout(this.#saveTimer);
    const write = () => {
      try {
        localStorage.setItem(SNAPSHOT_KEY, JSON.stringify([...snaps]));
      } catch {
        // Full storage: the in-memory copy still works for this run.
      }
    };
    // Written when the app is idle, not while the next chat opens (at once when hidden:
    // timers stop in the background).
    if (now) write();
    else this.#saveTimer = setTimeout(write, 1500);
  }

  /** Save the open chat's look now (the app is going to the background). */
  persist() {
    this.#keepSnapshot(true);
  }
  /** Pinned messages (newest pin first). */
  pins = $state<Pins>({ messages: [], can_pin: false });

  /** The message the timeline is centred on (opened from search); null = live, latest. */
  focused = $state<string | null>(null);
  /** A message found in the live timeline that the view should scroll to (then cleared). */
  revealed = $state<string | null>(null);
  /** The newest message is loaded (always, unless focused on an older one). */
  atEnd = $state(true);
  loadingNewer = $state(false);
  /** A search result to show once this chat opens. */
  #pendingFocus: { roomId: string; eventId: string } | null = null;
  /** Bumped on every (re)load, so late updates of a replaced timeline are dropped. */
  #generation = 0;

  async open(roomId: string) {
    const focus = this.#pendingFocus?.roomId === roomId ? this.#pendingFocus.eventId : null;
    this.#pendingFocus = null;
    if (this.roomId === roomId && !focus) return;
    await this.#load(roomId, null);
    if (focus) await this.#show(roomId, focus);
  }

  /** Show `eventId` in its chat: right away if it's open, else when the chat opens. */
  jump(roomId: string, eventId: string) {
    if (this.roomId === roomId) void this.#show(roomId, eventId);
    else this.#pendingFocus = { roomId, eventId };
  }

  /** Recent messages stay in the live timeline; older ones open a timeline around them. */
  async #show(roomId: string, eventId: string) {
    if (this.focused) await this.#load(roomId, null);
    if (await this.reveal(eventId, 3)) {
      this.focused = null;
      this.revealed = eventId;
    } else if (this.roomId === roomId) {
      await this.#load(roomId, eventId);
    }
  }

  /** Back to the latest messages (live) after viewing a search result. */
  async latest() {
    if (this.roomId && this.focused) await this.#load(this.roomId, null);
  }

  async #load(roomId: string, focus: string | null) {
    const generation = ++this.#generation;
    const current = () => this.#generation === generation;
    const sameRoom = this.roomId === roomId;
    if (!sameRoom) this.#keepSnapshot();
    this.roomId = roomId;
    this.focused = focus;
    this.atEnd = !focus;
    // The snapshot (if any) until the first live batch.
    const snapshot = focus ? undefined : (this.#snapshots ??= loadSnapshots()).get(roomId);
    this.entries = snapshot?.map((e) => ({ ...e, key: snapKey(e) })) ?? [];
    this.loaded = !!snapshot?.length;
    const snapped = new Set(snapshot?.flatMap((e) => (e.type === "message" ? [e.item_id] : [])));
    const renamed = new WeakMap<TimelineEntry, TimelineEntry>();
    const shared = (e: TimelineEntry): TimelineEntry => {
      if (e.type !== "message" || !snapped.has(e.item_id)) return e;
      let copy = renamed.get(e);
      if (!copy) renamed.set(e, (copy = { ...e, key: snapKey(e) }));
      return copy;
    };
    let live: TimelineEntry[] | null = null;
    this.#live = false;
    this.atStart = false;
    this.error = null;
    if (!sameRoom) {
      this.typing = [];
      // Last known look right away (also after a restart), so the chat doesn't flash the
      // default theme.
      this.style = this.#styles.get(roomId) ?? peek<ChatStyle>(`style:${roomId}`, true) ?? null;
      this.pins = { messages: [], can_pin: false };
    }
    try {
      await openChat(
        roomId,
        (diffs) => {
          // Late batches from the previous timeline are dropped.
          if (!current()) return;
          // Diffs apply to the live list, never to the snapshot shown meanwhile.
          live = live ? live.slice() : [];
          this.#live = true;
          applyDiffs(live, diffs);
          this.entries = snapped.size ? live.map(shared) : live;
          this.loaded = true;
        },
        (names) => {
          if (current()) this.typing = names;
        },
        (style) => {
          this.#styles.set(roomId, style);
          remember(`style:${roomId}`, style, true);
          if (current()) this.style = style;
        },
        (pins) => {
          if (current()) this.pins = pins;
        },
        focus,
      );
      while (current() && !focus && !this.atStart && ((live as TimelineEntry[] | null)?.length ?? 0) < MIN_ENTRIES) {
        if (!(await this.older())) break;
      }
    } catch (e) {
      if (current()) this.error = String(e);
    }
  }

  /** Fetch newer messages of a focused timeline. */
  async newer() {
    const roomId = this.roomId;
    if (!roomId || !this.focused || this.atEnd || this.loadingNewer) return;
    this.loadingNewer = true;
    const generation = this.#generation;
    try {
      const atEnd = await loadNewer(roomId);
      if (this.#generation === generation) this.atEnd = atEnd;
    } catch (e) {
      if (this.#generation === generation) this.error = String(e);
    } finally {
      this.loadingNewer = false;
    }
  }

  /** Fetch one page of older messages. Returns false if nothing was requested. */
  /** Older messages couldn't be loaded (after retries): the chat offers Retry. */
  olderFailed = $state(false);

  async older(): Promise<boolean> {
    const roomId = this.roomId;
    // Older messages come from the live timeline, not the snapshot shown before it.
    if (!roomId || this.atStart || this.loadingOlder || !this.#live) return false;
    this.loadingOlder = true;
    this.olderFailed = false;
    try {
      // A network hiccup shouldn't stop the history: a few quiet retries first.
      for (const wait of [0, 2000, 5000, 10000]) {
        if (wait) await new Promise((r) => setTimeout(r, wait));
        if (this.roomId !== roomId) return false;
        try {
          const atStart = await loadOlder(roomId);
          if (this.roomId === roomId) this.atStart = atStart;
          return true;
        } catch (e) {
          console.warn("older messages", e);
        }
      }
      if (this.roomId === roomId) this.olderFailed = true;
      return false;
    } finally {
      this.loadingOlder = false;
    }
  }

  /** Load older messages until `eventId` is in the timeline (gives up after a few pages). */
  async reveal(eventId: string, pages = 15): Promise<boolean> {
    const has = () => this.entries.some((e) => e.type === "message" && e.item_id === eventId);
    for (let page = 0; !has() && page < pages; page++) {
      if (!(await this.older())) break;
    }
    return has();
  }

  close() {
    if (!this.roomId) return;
    this.#keepSnapshot();
    this.#generation++;
    this.focused = null;
    this.roomId = null;
    this.entries = [];
    this.typing = [];
    this.style = null;
    closeChat().catch(() => {});
  }
}

export const chat = new Chat();
if (typeof document !== "undefined")
  document.addEventListener("visibilitychange", () => document.visibilityState === "hidden" && chat.persist());

// Dev builds only: lets the desktop test harness (WebDriver) read the open chat.
if (import.meta.env.DEV) (globalThis as Record<string, unknown>).__chat = chat;
