// The sync connection, for the bar above the chat list. Short blips aren't shown.
import { resumeSync, subscribeConnection, type ConnectionStatus } from "$lib/backend";

/** Only show the bar if a non-online state lasts this long (a normal start stays silent). */
const SHOW_AFTER_MS = 1500;

class Connection {
  status = $state<ConnectionStatus>("connecting");
  /** What the bar shows (null: hidden). */
  shown = $state<ConnectionStatus | null>(null);
  #timer: ReturnType<typeof setTimeout> | undefined;
  #started = false;

  start() {
    if (this.#started) return;
    this.#started = true;
    this.#set("connecting");
    void subscribeConnection((s) => this.#set(s)).catch(() => {});
    document.addEventListener("visibilitychange", () => {
      if (document.visibilityState === "visible") void resumeSync().catch(() => {});
    });
  }

  #set(status: ConnectionStatus) {
    this.status = status;
    clearTimeout(this.#timer);
    if (status === "online") this.shown = null;
    else if (this.shown) this.shown = status;
    else this.#timer = setTimeout(() => (this.shown = this.status === "online" ? null : this.status), SHOW_AFTER_MS);
  }
}

export const connection = new Connection();
