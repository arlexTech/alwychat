import { searchMessages, searchStatus, type KindFilter, type SearchHit, type SearchStatus } from "$lib/backend";

/** Results come this long after the last keystroke. */
const DEBOUNCE_MS = 180;
/** While older history is being indexed, results are refreshed this often. */
const REFRESH_MS = 4_000;
const PAGE = 40;

/** One message search (all chats, or one): typing, filters, paging. */
export class MessageSearch {
  query = $state("");
  kind = $state<KindFilter | null>(null);
  hits = $state<SearchHit[]>([]);
  loading = $state(false);
  /** No more results to page through. */
  done = $state(false);
  status = $state<SearchStatus | null>(null);
  error = $state<string | null>(null);

  #roomId: string | null;
  #timer: ReturnType<typeof setTimeout> | undefined;
  #refresh: ReturnType<typeof setInterval> | undefined;
  #generation = 0;

  constructor(roomId: string | null = null) {
    this.#roomId = roomId;
  }

  get active() {
    return this.query.trim().length >= 2;
  }

  /** Call when `query` or `kind` changed. */
  changed() {
    clearTimeout(this.#timer);
    this.#timer = setTimeout(() => this.#run(), DEBOUNCE_MS);
  }

  async #run() {
    const generation = ++this.#generation;
    this.error = null;
    if (!this.active) {
      this.hits = [];
      this.done = true;
      return;
    }
    this.loading = true;
    try {
      const hits = await searchMessages(this.query, { roomId: this.#roomId, kind: this.kind });
      if (generation !== this.#generation) return;
      this.hits = hits;
      this.done = hits.length < PAGE;
    } catch (e) {
      if (generation === this.#generation) this.error = String(e);
    } finally {
      if (generation === this.#generation) this.loading = false;
    }
    void this.#updateStatus();
  }

  /** The next page (older results). */
  async more() {
    if (this.done || this.loading || !this.hits.length) return;
    const generation = this.#generation;
    this.loading = true;
    try {
      const before = this.hits[this.hits.length - 1].timestamp;
      const hits = await searchMessages(this.query, { roomId: this.#roomId, kind: this.kind, before });
      if (generation !== this.#generation) return;
      const seen = new Set(this.hits.map((h) => h.event_id));
      this.hits.push(...hits.filter((h) => !seen.has(h.event_id)));
      this.done = hits.length < PAGE;
    } catch (e) {
      this.error = String(e);
    } finally {
      this.loading = false;
    }
  }

  async #updateStatus() {
    try {
      this.status = await searchStatus(this.#roomId);
    } catch {
      this.status = null;
    }
  }

  /** Keep the index status fresh, and results too while history is still arriving. */
  start() {
    void this.#updateStatus();
    this.#refresh = setInterval(async () => {
      const before = this.status?.messages ?? 0;
      await this.#updateStatus();
      const grew = (this.status?.messages ?? 0) > before;
      // Only the first page is refreshed: don't reshuffle what the user scrolled through.
      if (grew && this.active && this.hits.length <= PAGE && !this.loading) void this.#run();
    }, REFRESH_MS);
  }

  stop() {
    clearTimeout(this.#timer);
    clearInterval(this.#refresh);
  }
}
