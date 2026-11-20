import {
  closeChat,
  loadNewer,
  loadOlder,
  openChat,
  peekChat,
  type ChatStyle,
  type Pins,
  type RoomSummary,
  type TimelineEntry,
} from "$lib/backend";
import { applyDiffs } from "./list";
import { chatList } from "./chatList.svelte";
import { soundboard } from "./soundboard.svelte";
import { peek, remember } from "$lib/cache";
import { eventMediaUrl } from "$lib/media";

/** Older messages are fetched until at least this many entries are loaded. */
const MIN_ENTRIES = 30;

// What a chat showed when we left it, drawn the moment it's opened again (also after a
// restart) while the core rebuilds its timeline; the live one then takes over in place.
const SNAPSHOT_ENTRIES = 30;
const SNAPSHOT_ROOMS = 24;
/** Chats at the top of the list kept ready (latest messages loaded) before they're opened. */
const READY_TOP = 8;
const SNAPSHOT_KEY = "alwychat.snapshots";

function loadSnapshots(): Map<string, TimelineEntry[]> {
  try {
    const raw = JSON.parse(localStorage.getItem(SNAPSHOT_KEY) ?? "[]");
    const snaps = new Map<string, TimelineEntry[]>(Array.isArray(raw) ? raw : []);
    // A "new messages" line from the last run would be in the wrong place by now.
    for (const [id, entries] of snaps) snaps.set(id, entries.filter((e) => e.type !== "read_marker"));
    return snaps;
  } catch {
    return new Map();
  }
}

/** What tells a snapshot entry and the same one in the live timeline are one row. */
const rowId = (e: TimelineEntry) => (e.type === "message" ? e.item_id : e.key.replace(/^snap:/, ""));

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
    this.#saveSnapshots(now);
  }

  #saveSnapshots(now = false) {
    const snaps = (this.#snapshots ??= loadSnapshots());
    while (snaps.size > SNAPSHOT_ROOMS) snaps.delete(snaps.keys().next().value!);
    clearTimeout(this.#saveTimer);
    const write = () => {
      this.#unsaved = false;
      try {
        localStorage.setItem(SNAPSHOT_KEY, JSON.stringify([...snaps]));
      } catch {
        // Full storage: the in-memory copy still works for this run.
      }
    };
    // Written when the app is idle, not while the next chat opens (at once when hidden:
    // timers stop in the background).
    this.#unsaved = true;
    if (now) write();
    else this.#saveTimer = setTimeout(write, 1500);
  }
  #unsaved = false;

  // Chats are made ready BEFORE they're opened: the core hands over their latest messages
  // (and keeps their timeline), so a click shows the chat as it is now, with nothing left to
  // arrive. Done for the top of the list, for any chat whose list row changes (a new
  // message), and again when a row is pointed at or pressed.
  #seen = new Map<string, string>();
  #queue: string[] = [];
  #warming = false;
  #warmed = new Map<string, number>();

  /** Follow the chat list: `rooms` is every chat, `top` the first ones on screen. */
  watch(rooms: RoomSummary[], top: string[]) {
    for (const r of rooms) {
      const mark = `${r.latest?.timestamp ?? 0}|${r.latest?.text ?? ""}|${r.unread_messages}`;
      const before = this.#seen.get(r.id);
      this.#seen.set(r.id, mark);
      // Only what changed while we watched (a chat seen for the first time is just the list
      // filling in; if it's new and on top, the line below takes it).
      if (before !== undefined && before !== mark) this.warm(r.id);
    }
    for (const id of top.slice(0, READY_TOP)) if (!this.#warmed.has(id)) this.warm(id);
  }

  /** Load these chats now, all at once, until each has its message from `at` (chat ID →
   *  time): the new message waits on the chat list for this. */
  prepare(chats: Map<string, number>) {
    return Promise.all(
      [...chats].map(async ([id, at]) => {
        // The list can hear of a message a moment before the chat's timeline has it.
        for (let i = 0; i < 10; i++) {
          if ((await this.#peek(id)) >= at) return;
          await new Promise((r) => setTimeout(r, 80));
        }
      }),
    );
  }

  /** Get `roomId` ready to open. `now`: it's about to be opened (pointed at, pressed). */
  warm(roomId: string, now = false) {
    if (this.roomId === roomId && this.#live) return;
    if (now) {
      // Just done: nothing new to fetch.
      if (Date.now() - (this.#warmed.get(roomId) ?? 0) < 800) return;
      void this.#peek(roomId);
      return;
    }
    if (!this.#queue.includes(roomId)) this.#queue.push(roomId);
    // Catching up after a long sleep: the latest changes matter most.
    if (this.#queue.length > 12) this.#queue.shift();
    if (this.#warming) return;
    this.#warming = true;
    // One at a time, with a breath in between: never in the way of what the user is doing.
    const next = async () => {
      const id = this.#queue.shift();
      if (id === undefined) return void (this.#warming = false);
      // Loaded a moment ago (a new message held on the list for it): nothing newer yet.
      if (Date.now() - (this.#warmed.get(id) ?? 0) > 2000) await this.#peek(id);
      setTimeout(next, 60);
    };
    setTimeout(next, 150);
  }

  /** Resolves to the time of the newest message it got (Infinity: nothing to wait for). */
  async #peek(roomId: string): Promise<number> {
    if (this.roomId === roomId && this.#live) return Infinity;
    this.#warmed.set(roomId, Date.now());
    try {
      const entries = (await peekChat(roomId)).filter((e) => !(e.type === "message" && !e.item_id.startsWith("$")));
      if (!entries.length) return 0;
      const snaps = (this.#snapshots ??= loadSnapshots());
      // The open chat is newer than any peek once its live timeline answered.
      if (this.roomId === roomId && this.#live) return Infinity;
      snaps.delete(roomId);
      snaps.set(roomId, entries);
      this.#saveSnapshots();
      // Its newest photos too, so they're there when it opens (never a view-once one).
      let photos = 0;
      for (let i = entries.length - 1; i >= 0 && photos < 4; i--) {
        const e = entries[i];
        if (e.type !== "message") continue;
        const c = e.content;
        if ((c.kind === "image" && !c.view_once) || c.kind === "sticker" || (c.kind === "video" && c.has_thumbnail && !c.view_once)) {
          new Image().src = eventMediaUrl(c.media, true);
          photos++;
        }
      }
      // Opened meanwhile and still showing the older snapshot: show this one.
      if (this.roomId === roomId && !this.focused) {
        this.entries = entries.map((e) => ({ ...e, key: snapKey(e) }));
        this.loaded = true;
      }
      return Math.max(0, ...entries.map((e) => (e.type === "message" ? e.timestamp : 0)));
    } catch {
      // Not ready (offline, a chat we left): opening it loads it the usual way.
      this.#warmed.delete(roomId);
      return Infinity;
    }
  }

  /** Signed out: nothing of the account's chats stays on the device. */
  forget() {
    clearTimeout(this.#saveTimer);
    this.#unsaved = false;
    this.#snapshots = new Map();
    // (Closing the open chat right after must not keep it again.)
    this.entries = [];
    this.loaded = false;
    this.#seen.clear();
    this.#warmed.clear();
    this.#queue = [];
    try {
      localStorage.removeItem(SNAPSHOT_KEY);
    } catch {
      // Nothing kept then.
    }
  }

  /** Save the open chat's look now (the app is going to the background). */
  persist() {
    this.#keepSnapshot(true);
    if (this.#unsaved) this.#saveSnapshots(true);
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
  jump(roomId: string, eventId: string): Promise<void> {
    if (this.roomId === roomId) return this.#show(roomId, eventId);
    this.#pendingFocus = { roomId, eventId };
    return Promise.resolve();
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
    // Messages by their event, the rest (day lines, "new messages") by the core's key, which
    // a snapshot peeked from the kept timeline shares with it.
    const snapped = new Set(snapshot?.map(rowId));
    const renamed = new WeakMap<TimelineEntry, TimelineEntry>();
    const shared = (e: TimelineEntry): TimelineEntry => {
      if (!snapped.has(rowId(e))) return e;
      let copy = renamed.get(e);
      if (!copy) renamed.set(e, (copy = { ...e, key: snapKey(e) }));
      return copy;
    };
    let live: TimelineEntry[] | null = null;
    // The first live batch can come after `openChat` returns (the snapshot shows meanwhile):
    // filling a short chat with older pages waits for it, else nothing would load them (a chat
    // too short to scroll never asks).
    let firstBatch = () => {};
    const arrived = new Promise<void>((resolve) => (firstBatch = resolve));
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
          firstBatch();
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
          // Also sent when the chat's sounds change: an open soundboard follows.
          if (soundboard.roomId === roomId) void soundboard.load(roomId);
        },
        (pins) => {
          if (current()) this.pins = pins;
        },
        focus,
      );
      if (!focus) await Promise.race([arrived, new Promise((r) => setTimeout(r, 8000))]);
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
chatList.prepare = (chats) => chat.prepare(chats);
if (typeof document !== "undefined")
  document.addEventListener("visibilitychange", () => document.visibilityState === "hidden" && chat.persist());

// Dev builds only: lets the desktop test harness (WebDriver) read the open chat.
if (import.meta.env.DEV) (globalThis as Record<string, unknown>).__chat = chat;
