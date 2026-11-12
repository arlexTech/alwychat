import { peek, remember } from "$lib/cache";
import {
  getChatListPrefs,
  setChatListPrefs,
  subscribeSpaces,
  type ChatListPrefs,
  type Folder,
  type RoomSummary,
  type SpaceInfo,
} from "$lib/backend";

/** Joined spaces, which chats are in them, and whether those also show in "Chats". */
class Spaces {
  // Last run's list until the live one arrives (the Spaces tab never opens empty).
  list = $state<SpaceInfo[]>(peek<SpaceInfo[]>("spaces", true) ?? []);
  prefs = $state<ChatListPrefs>({ show_space_chats: false, spaces: {}, rooms: {}, folders: [] });
  loaded = $state(!!peek<SpaceInfo[]>("spaces", true));
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
      this.list = remember("spaces", list, true);
      this.loaded = true;
    }).catch(() => (this.#started = false));
    this.#reload();
    // Changed on another device: picked up when we come back to the app.
    document.addEventListener("visibilitychange", () => document.visibilityState === "visible" && this.#reload());
  }

  #reload() {
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

  /**
   * Shown right away, then applied to the latest saved copy, so a change made on another
   * device since we loaded isn't overwritten.
   */
  update(change: (prefs: ChatListPrefs) => void) {
    // One at a time: each applies its change to what the previous one saved.
    const run = this.#queue.then(() => this.#apply(change));
    this.#queue = run.catch(() => {});
    return run;
  }
  #queue: Promise<void> = Promise.resolve();

  async #apply(change: (prefs: ChatListPrefs) => void) {
    const previous = $state.snapshot(this.prefs);
    const optimistic = $state.snapshot(this.prefs);
    change(optimistic);
    this.prefs = optimistic;
    try {
      const next = await getChatListPrefs();
      change(next);
      await setChatListPrefs(next);
      this.prefs = next;
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

  /** Add or replace a folder (matched by id). */
  saveFolder(folder: Folder) {
    return this.update((p) => {
      const i = p.folders.findIndex((f) => f.id === folder.id);
      if (i < 0) p.folders.push(folder);
      else p.folders[i] = folder;
    });
  }

  deleteFolder(id: string) {
    return this.update((p) => {
      p.folders = p.folders.filter((f) => f.id !== id);
    });
  }

  moveFolder(id: string, by: -1 | 1) {
    return this.update((p) => {
      const i = p.folders.findIndex((f) => f.id === id);
      const j = i + by;
      if (i < 0 || j < 0 || j >= p.folders.length) return;
      [p.folders[i], p.folders[j]] = [p.folders[j], p.folders[i]];
    });
  }

  /** Put a chat in a folder by hand, or take it out (also when it's there by type). */
  setInFolder(folderId: string, roomId: string, inside: boolean) {
    return this.update((p) => {
      const f = p.folders.find((f) => f.id === folderId);
      if (!f) return;
      f.rooms = f.rooms.filter((r) => r !== roomId);
      f.exclude = f.exclude.filter((r) => r !== roomId);
      if (inside) f.rooms.push(roomId);
      else f.exclude.push(roomId);
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

export const isUnread = (r: RoomSummary) =>
  !r.is_muted && (r.unread_messages > 0 || r.marked_unread || r.is_invite);

/** Whether a chat is in a folder. Archived chats only when picked by hand. */
export function inFolder(folder: Folder, room: RoomSummary): boolean {
  if (folder.exclude.includes(room.id)) return false;
  const picked = folder.rooms.includes(room.id);
  const byType =
    !room.is_archived && spaces.inChats(room) && ((folder.people && room.is_dm) || (folder.groups && !room.is_dm));
  if (!picked && !byType) return false;
  if (folder.hide_muted && room.is_muted) return false;
  return !folder.unread_only || isUnread(room);
}

export const spaces = new Spaces();
