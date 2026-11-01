// "Share" from another app (Android): pick a chat, then the text and files wait in its composer
// for a caption and Send.
import { discardPicked, isAndroid, takeShare, type IncomingShare } from "$lib/backend";

class ShareStore {
  /** Waiting for the user to pick a chat. */
  incoming = $state<IncomingShare | null>(null);
  /** Picked: the chat's composer takes it (ChatView). */
  ready = $state<(IncomingShare & { roomId: string }) | null>(null);
  #started = false;

  /** Check now, and each time the app comes back to the front (a share brings it there). */
  start() {
    if (!isAndroid || this.#started) return;
    this.#started = true;
    void this.check();
    document.addEventListener("visibilitychange", () => {
      if (document.visibilityState === "visible") void this.check();
    });
  }

  async check() {
    const share = await takeShare().catch(() => null);
    if (!share || (!share.text?.trim() && !share.files.length)) return;
    this.cancel();
    this.incoming = share;
  }

  deliver(roomId: string) {
    if (this.incoming) this.ready = { ...this.incoming, roomId };
    this.incoming = null;
  }

  /** The composer of `roomId` takes the share (once). */
  take(roomId: string): IncomingShare | null {
    const ready = this.ready;
    if (ready?.roomId !== roomId) return null;
    this.ready = null;
    return ready;
  }

  cancel() {
    for (const f of [...(this.incoming?.files ?? []), ...(this.ready?.files ?? [])]) discardPicked(f).catch(() => {});
    this.incoming = null;
    this.ready = null;
  }
}

export const share = new ShareStore();
