import { closeChat, loadNewer, loadOlder, openChat, type ChatStyle, type Pins, type TimelineEntry } from "$lib/backend";
import { applyDiffs } from "./list";

/** Older messages are fetched until at least this many entries are loaded. */
const MIN_ENTRIES = 30;

/** The chat on screen: its live timeline, kept in sync with the core. */
class Chat {
  roomId = $state<string | null>(null);
  entries = $state<TimelineEntry[]>([]);
  loaded = $state(false);
  atStart = $state(false);
  loadingOlder = $state(false);
  error = $state<string | null>(null);
  /** Names of people typing right now. */
  typing = $state<string[]>([]);
  /** Theme, quick reaction and nicknames; null until loaded (the last one seen is reused). */
  style = $state<ChatStyle | null>(null);
  #styles = new Map<string, ChatStyle>();
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
    this.roomId = roomId;
    this.focused = focus;
    this.atEnd = !focus;
    this.entries = [];
    this.loaded = false;
    this.atStart = false;
    this.error = null;
    if (!sameRoom) {
      this.typing = [];
      // Last known look right away, so the chat doesn't flash the default theme.
      this.style = this.#styles.get(roomId) ?? null;
      this.pins = { messages: [], can_pin: false };
    }
    try {
      await openChat(
        roomId,
        (diffs) => {
          // Late batches from the previous timeline are dropped.
          if (!current()) return;
          applyDiffs(this.entries, diffs);
          this.loaded = true;
        },
        (names) => {
          if (current()) this.typing = names;
        },
        (style) => {
          this.#styles.set(roomId, style);
          if (current()) this.style = style;
        },
        (pins) => {
          if (current()) this.pins = pins;
        },
        focus,
      );
      while (current() && !focus && !this.atStart && this.entries.length < MIN_ENTRIES) {
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
    if (!roomId || this.atStart || this.loadingOlder) return false;
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
