import { peek, remember } from "$lib/cache";
import {
  getChatListPrefs,
  setChatListPrefs,
  subscribeSpaces,
  type ChatListPrefs,
  type Folder,
  type RoomSummary,
  type SpaceGroup,
  type SpaceInfo,
} from "$lib/backend";

/** A row of the Spaces tab: a space, or a group of spaces. */
export type SpaceItem = { kind: "space"; space: SpaceInfo } | { kind: "group"; group: SpaceGroup; spaces: SpaceInfo[] };

/** Joined spaces, which chats are in them, and whether those also show in "Chats". */
class Spaces {
  // Last run's list until the live one arrives (the Spaces tab never opens empty).
  list = $state<SpaceInfo[]>(peek<SpaceInfo[]>("spaces", true) ?? []);
  prefs = $state<ChatListPrefs>({ show_space_chats: false, spaces: {}, rooms: {}, folders: [], space_order: [], space_groups: [] });
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
    // A computer's window is rarely hidden: coming back to it counts too.
    window.addEventListener("focus", () => this.#reload());
  }

  #reload() {
    getChatListPrefs().then((p) => (this.prefs = p), () => {});
  }

  /** The Spaces tab in its saved order: groups with their spaces, then spaces nobody placed yet
   * (newly joined) in the server's order. Spaces we left are skipped; empty groups too. */
  layout = $derived.by((): SpaceItem[] => {
    const byId = new Map(this.list.map((s) => [s.id, s]));
    const groups = this.prefs.space_groups ?? [];
    const grouped = new Set(groups.flatMap((g) => g.spaces));
    const out: SpaceItem[] = [];
    const placed = new Set<string>();
    const addGroup = (g: SpaceGroup) => {
      if (placed.has(g.id)) return;
      placed.add(g.id);
      const spaces = g.spaces.flatMap((id) => byId.get(id) ?? []);
      if (spaces.length) out.push({ kind: "group", group: g, spaces });
    };
    for (const id of this.prefs.space_order ?? []) {
      const g = groups.find((x) => x.id === id);
      if (g) addGroup(g);
      else if (byId.has(id) && !grouped.has(id) && !placed.has(id)) {
        placed.add(id);
        out.push({ kind: "space", space: byId.get(id)! });
      }
    }
    for (const g of groups) addGroup(g);
    for (const s of this.list) if (!grouped.has(s.id) && !placed.has(s.id)) out.push({ kind: "space", space: s });
    return out;
  });

  /** The group a space is in, if any. */
  groupOf(spaceId: string): SpaceGroup | undefined {
    return (this.prefs.space_groups ?? []).find((g) => g.spaces.includes(spaceId));
  }

  /** New order of the Spaces tab's rows (space and group IDs). */
  setSpaceOrder(ids: string[]) {
    return this.update((p) => {
      p.space_order = ids;
    });
  }

  /** New order of a group's spaces. */
  setGroupOrder(groupId: string, ids: string[]) {
    return this.update((p) => {
      const g = (p.space_groups ??= []).find((x) => x.id === groupId);
      if (g) g.spaces = ids;
    });
  }

  /** Put a space in a group (null: out of any group, back where the group is). A new group is
   * created when `newName` is given. */
  moveSpace(spaceId: string, groupId: string | null, newName?: string) {
    const visible = this.layout.map((i) => (i.kind === "group" ? i.group.id : i.space.id));
    return this.update((p) => {
      p.space_groups ??= [];
      p.space_order = p.space_order?.length ? p.space_order : visible;
      const from = p.space_groups.find((g) => g.spaces.includes(spaceId));
      if (from) from.spaces = from.spaces.filter((id) => id !== spaceId);
      p.space_order = p.space_order.filter((id) => id !== spaceId);
      let target = groupId ? p.space_groups.find((g) => g.id === groupId) : undefined;
      if (!target && newName !== undefined) {
        target = { id: `group-${Date.now().toString(36)}`, name: newName, spaces: [] };
        p.space_groups.push(target);
        // The new group takes the space's place.
        const at = visible.indexOf(spaceId);
        p.space_order.splice(at < 0 ? p.space_order.length : at, 0, target.id);
      }
      if (target) target.spaces.push(spaceId);
      else {
        // Out of its group: right after the group.
        const at = from ? p.space_order.indexOf(from.id) : -1;
        p.space_order.splice(at < 0 ? p.space_order.length : at + 1, 0, spaceId);
      }
      p.space_groups = p.space_groups.filter((g) => g.spaces.length);
      p.space_order = p.space_order.filter((id) => !id.startsWith("group-") || p.space_groups.some((g) => g.id === id));
    });
  }

  renameGroup(groupId: string, name: string) {
    return this.update((p) => {
      const g = (p.space_groups ??= []).find((x) => x.id === groupId);
      if (g) g.name = name;
    });
  }

  /** Ungroup: its spaces take the group's place. */
  deleteGroup(groupId: string) {
    const visible = this.layout.map((i) => (i.kind === "group" ? i.group.id : i.space.id));
    return this.update((p) => {
      const g = (p.space_groups ??= []).find((x) => x.id === groupId);
      p.space_groups = p.space_groups.filter((x) => x.id !== groupId);
      const order = p.space_order?.length ? p.space_order : visible;
      const at = order.indexOf(groupId);
      p.space_order = order.filter((id) => id !== groupId);
      p.space_order.splice(at < 0 ? p.space_order.length : at, 0, ...(g?.spaces ?? []));
    });
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
