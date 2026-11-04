// View-once photos/videos we already opened (synced between our devices by the core).
import { hasBackend, openViewOnce, viewedOnce } from "$lib/backend";

class ViewOnce {
  opened = $state(new Set<string>());
  #loaded = false;

  async load() {
    if (!hasBackend || this.#loaded) return;
    this.#loaded = true;
    const ids = await viewedOnce().catch(() => []);
    this.opened = new Set([...this.opened, ...ids]);
  }

  /** Mark opened at once (the viewer shows it), then tell the core. */
  open(roomId: string, eventId: string) {
    this.opened = new Set([...this.opened, eventId]);
    void openViewOnce(roomId, eventId).catch((e) => console.warn("view once", e));
  }
}

export const viewOnce = new ViewOnce();
