// Names and photos of a chat's members, loaded once per chat and refreshed on demand (who
// reacted, tooltips). Unknown IDs fall back to the user ID's local part.
import { SvelteMap } from "svelte/reactivity";
import { roomMembers, type Member } from "$lib/backend";

class Members {
  #rooms = new SvelteMap<string, Map<string, Member>>();
  #loading = new Set<string>();

  async load(roomId: string, refresh = false) {
    if (this.#loading.has(roomId) || (this.#rooms.has(roomId) && !refresh)) return;
    this.#loading.add(roomId);
    try {
      const list = await roomMembers(roomId);
      this.#rooms.set(roomId, new Map(list.map((m) => [m.user_id, m])));
    } catch {
      // Names fall back to user IDs.
    } finally {
      this.#loading.delete(roomId);
    }
  }

  get(roomId: string, userId: string): { name: string; avatar: string | null; isMe: boolean } {
    const m = this.#rooms.get(roomId)?.get(userId);
    return {
      name: m?.name ?? userId.replace(/^@/, "").split(":")[0],
      avatar: m?.avatar ?? null,
      isMe: m?.is_me ?? false,
    };
  }
}

export const members = new Members();
