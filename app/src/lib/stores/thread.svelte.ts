import { closeThread, loadOlder, openThread, type TimelineEntry } from "$lib/backend";
import { applyDiffs } from "./list";

/**
 * A thread open beside its chat. Mirrors the parts of the chat store the chat view reads, so
 * the same view shows either; the core addresses the thread by its root event ID.
 */
export class ThreadTimeline {
  root: string;
  entries = $state<TimelineEntry[]>([]);
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
      applyDiffs(this.entries, diffs);
      this.loaded = true;
    })
      .then(() => this.older())
      .catch((e) => (this.error = String(e)));
  }

  async older(): Promise<boolean> {
    if (!this.#open || this.atStart || this.loadingOlder) return false;
    this.loadingOlder = true;
    try {
      this.atStart = await loadOlder(this.root);
      return true;
    } catch (e) {
      this.error = String(e);
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
