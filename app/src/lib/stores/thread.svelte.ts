import { closeThread, loadOlder, openThread, type TimelineEntry } from "$lib/backend";
import { applyDiffs } from "./list";

/**
 * A thread open beside its chat. Mirrors the parts of the chat store the chat view reads, so
 * the same view shows either; the core addresses the thread by its root event ID.
 */
export class ThreadTimeline {
  root: string;
  // Plain objects, replaced as a whole on each batch: thousands of deeply reactive entries
  // made every update (and every read while drawing) slow in long chats.
  entries = $state.raw<TimelineEntry[]>([]);
  loaded = $state(false);
  atStart = $state(false);
  loadingOlder = $state(false);
  error = $state<string | null>(null);
  // Threads always show their latest replies.
  readonly focused = null;
  revealed = $state<string | null>(null);
  #open = true;

  constructor(roomId: string, root: string) {
    this.root = root;
    openThread(roomId, root, (diffs) => {
      if (!this.#open) return;
      const next = this.entries.slice();
      applyDiffs(next, diffs);
      this.entries = next;
      this.loaded = true;
    })
      .then(() => this.older())
      .catch((e) => (this.error = String(e)));
  }

  olderFailed = $state(false);

  async older(): Promise<boolean> {
    if (!this.#open || this.atStart || this.loadingOlder) return false;
    this.loadingOlder = true;
    this.olderFailed = false;
    try {
      for (const wait of [0, 2000, 5000]) {
        if (wait) await new Promise((r) => setTimeout(r, wait));
        try {
          this.atStart = await loadOlder(this.root);
          return true;
        } catch (e) {
          console.warn("older replies", e);
        }
      }
      this.olderFailed = true;
      return false;
    } finally {
      this.loadingOlder = false;
    }
  }

  async reveal(eventId: string, pages = 15): Promise<boolean> {
    const has = () => this.entries.some((e) => e.type === "message" && e.item_id === eventId);
    for (let page = 0; !has() && page < pages; page++) {
      if (!(await this.older())) break;
    }
    return has();
  }

  async newer() {}
  async latest() {}

  close() {
    this.#open = false;
    closeThread().catch(() => {});
  }
}
