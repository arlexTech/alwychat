import {
  getChatListPrefs,
  setChatListPrefs,
  subscribeSpaces,
  type ChatListPrefs,
  type RoomSummary,
  type SpaceInfo,
} from "$lib/backend";

/** Joined spaces, which chats are in them, and whether those also show in "Chats". */
class Spaces {
  list = $state<SpaceInfo[]>([]);
  prefs = $state<ChatListPrefs>({ show_space_chats: false, spaces: {}, rooms: {} });
  loaded = $state(false);
  /** Chat → the top-level spaces it's in. */
  #spacesOf = $derived.by(() => {
    const map = new Map<string, string[]>();
    for (const space of this.list) {
      for (const room of space.rooms) map.set(room, [...(map.get(room) ?? []), space.id]);
    }
    return map;
  });
  #started = false;

  start() {
    if (this.#started) return;
    this.#started = true;
    subscribeSpaces((list) => {
      this.list = list;
      this.loaded = true;
    }).catch(() => (this.#started = false));
    getChatListPrefs().then((p) => (this.prefs = p), () => {});
  }

  spacesOf(roomId: string): string[] {
    return this.#spacesOf.get(roomId) ?? [];
  }

  /** Whether a chat is listed under "Chats". People (1:1 chats) always are. */
  inChats(room: RoomSummary): boolean {
    const spaces = this.spacesOf(room.id);
    if (room.is_dm || !spaces.length) return true;
    const own = this.prefs.rooms[room.id];
    if (own !== undefined) return own;
    const bySpace = spaces.map((s) => this.prefs.spaces[s]).filter((v) => v !== undefined);
    if (bySpace.length) return bySpace.some(Boolean);
    return this.prefs.show_space_chats;
  }

  /** The space-level choice, or the general one when the space has none. */
  spaceShown(spaceId: string): boolean {
    return this.prefs.spaces[spaceId] ?? this.prefs.show_space_chats;
  }

  async update(change: (prefs: ChatListPrefs) => void) {
    const previous = $state.snapshot(this.prefs);
    const next = $state.snapshot(this.prefs);
    change(next);
    this.prefs = next;
    try {
      await setChatListPrefs(next);
    } catch (e) {
      this.prefs = previous;
      throw e;
    }
  }

  setGlobal(show: boolean) {
    return this.update((p) => {
      p.show_space_chats = show;
    });
  }

  setSpace(spaceId: string, show: boolean) {
    return this.update((p) => {
      p.spaces[spaceId] = show;
    });
  }

  /** null: follow the space / general setting. */
  setRoom(roomId: string, show: boolean | null) {
    return this.update((p) => {
      if (show === null) delete p.rooms[roomId];
      else p.rooms[roomId] = show;
    });
  }
}

export const spaces = new Spaces();
