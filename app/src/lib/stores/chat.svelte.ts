import { closeChat, loadOlder, openChat, type ChatStyle, type TimelineEntry } from "$lib/backend";
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

  async open(roomId: string) {
    if (this.roomId === roomId) return;
    this.roomId = roomId;
    this.entries = [];
    this.loaded = false;
    this.atStart = false;
    this.error = null;
    this.typing = [];
    // Last known look right away, so the chat doesn't flash the default theme.
    this.style = this.#styles.get(roomId) ?? null;
    try {
      await openChat(
        roomId,
        (diffs) => {
          // Late batches from the previous chat are dropped.
          if (this.roomId !== roomId) return;
          applyDiffs(this.entries, diffs);
          this.loaded = true;
        },
        (names) => {
          if (this.roomId === roomId) this.typing = names;
        },
        (style) => {
          this.#styles.set(roomId, style);
          if (this.roomId === roomId) this.style = style;
        },
      );
      while (this.roomId === roomId && !this.atStart && this.entries.length < MIN_ENTRIES) {
        if (!(await this.older())) break;
      }
    } catch (e) {
      if (this.roomId === roomId) this.error = String(e);
    }
  }

  /** Fetch one page of older messages. Returns false if nothing was requested. */
  async older(): Promise<boolean> {
    const roomId = this.roomId;
    if (!roomId || this.atStart || this.loadingOlder) return false;
    this.loadingOlder = true;
    try {
      const atStart = await loadOlder(roomId);
      if (this.roomId === roomId) this.atStart = atStart;
      return true;
    } catch (e) {
      if (this.roomId === roomId) this.error = String(e);
      return false;
    } finally {
      this.loadingOlder = false;
    }
  }

  close() {
    if (!this.roomId) return;
    this.roomId = null;
    this.entries = [];
    this.typing = [];
    this.style = null;
    closeChat().catch(() => {});
  }
}

export const chat = new Chat();
