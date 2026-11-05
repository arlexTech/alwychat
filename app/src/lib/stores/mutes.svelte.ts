// Timed mutes and muted spaces (core mutes.rs). The mute itself is the server's push rule; when
// it ends, and which chats a muted space covers, is AlwyChat's own data, applied by the core
// here every minute while the app runs (and on each push on Android).
import { applyMutes, getMutes, muteRoom, muteSpace, unmuteRoom, unmuteSpace, type Mutes } from "$lib/backend";

class MuteStore {
  data = $state<Mutes>({ rooms: {}, spaces: {} });
  #started = false;

  /** End mutes whose time is up now and every minute, and when the app comes back. */
  start() {
    if (this.#started) return;
    this.#started = true;
    void this.apply();
    setInterval(() => void this.apply(), 60_000);
    document.addEventListener("visibilitychange", () => {
      if (document.visibilityState === "visible") void this.apply();
    });
  }

  async apply() {
    try {
      this.data = await applyMutes();
    } catch (e) {
      console.warn("mutes", e);
    }
  }

  async #refresh() {
    this.data = await getMutes().catch(() => this.data);
  }

  /** When a chat's own mute ends (undefined: not muted for a while). */
  roomUntil(roomId: string): number | undefined {
    return this.data.rooms[roomId];
  }

  /** The space's mute: undefined = not muted; until null = until turned back on. */
  space(spaceId: string): { until: number | null } | undefined {
    return this.data.spaces[spaceId];
  }

  async muteRoom(roomId: string, until: number | null) {
    await muteRoom(roomId, until);
    await this.#refresh();
  }

  async unmuteRoom(roomId: string) {
    await unmuteRoom(roomId);
    await this.#refresh();
  }

  async muteSpace(spaceId: string, until: number | null) {
    await muteSpace(spaceId, until);
    await this.#refresh();
  }

  async unmuteSpace(spaceId: string) {
    await unmuteSpace(spaceId);
    await this.#refresh();
  }
}

export const mutes = new MuteStore();
