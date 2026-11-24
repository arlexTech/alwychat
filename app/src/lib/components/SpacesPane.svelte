<script lang="ts">
  import { inBody } from "$lib/layout";
  import { tr, trn } from "$lib/i18n/index.svelte";
  import { asList, stored } from "$lib/stored";
  import { onDestroy, untrack } from "svelte";
  import { MediaQuery } from "svelte/reactivity";
  import { fade, fly } from "svelte/transition";
  import ChevronLeft from "@lucide/svelte/icons/chevron-left";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import Settings from "@lucide/svelte/icons/settings";
  import Bell from "@lucide/svelte/icons/bell";
  import BellOff from "@lucide/svelte/icons/bell-off";
  import MuteChooser from "./MuteChooser.svelte";
  import { mutes } from "$lib/stores/mutes.svelte";
  import { laterTime } from "$lib/format";
  import Plus from "@lucide/svelte/icons/plus";
  import Users from "@lucide/svelte/icons/users";
  import { onBack } from "$lib/back.svelte";
  import {
    canAddToSpace,
    closeSpace,
    createVoiceChannel,
    joinRoom,
    openSpace,
    createRoom,
    spaceCategories,
    type RoomSummary,
    type SpaceCategory,
    type SpaceChild,
  } from "$lib/backend";
  import CategoryPage from "./CategoryPage.svelte";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import Folder from "@lucide/svelte/icons/folder";
  import Globe from "@lucide/svelte/icons/globe";
  import Lock from "@lucide/svelte/icons/lock";
  import FolderInput from "@lucide/svelte/icons/folder-input";
  import ArrowUpDown from "@lucide/svelte/icons/arrow-up-down";
  import Ungroup from "@lucide/svelte/icons/ungroup";
  import Check from "@lucide/svelte/icons/check";
  import ReorderList from "./ReorderList.svelte";
  import type { SpaceGroup } from "$lib/backend";
  import Pencil from "@lucide/svelte/icons/pencil";
  import { call } from "$lib/stores/call.svelte";
  import { chatList } from "$lib/stores/chatList.svelte";
  import LoaderCircle from "@lucide/svelte/icons/loader-circle";
  import { isUnread, spaces } from "$lib/stores/spaces.svelte";
  import Avatar from "./Avatar.svelte";
  import { spaceBanner } from "$lib/backend";
  import { mediaUrl } from "$lib/media";
  import ChatRow from "./ChatRow.svelte";
  import Page from "./Page.svelte";
  import VoiceChannel from "./VoiceChannel.svelte";
  import SpaceSettingsPage from "./SpaceSettingsPage.svelte";
  import CreateRoomPage from "./CreateRoomPage.svelte";
  import { nav } from "$lib/stores/nav.svelte";
  import { peek, remember } from "$lib/cache";
  import { sectionOrder } from "$lib/spaceSections";

  let {
    selectedId,
    onSelect,
    onRowMenu,
    onExit,
    home = 0,
    newSpace = $bindable(false),
  }: {
    selectedId: string | null;
    onSelect: (roomId: string) => void;
    /** Back from a space to the list of spaces (its chat closes then). */
    onExit?: () => void;
    /** Changes when the Spaces tab is tapped again: back to the list of spaces. */
    home?: number;
    /** The "New space" page is open (the + in the header opens it). */
    newSpace?: boolean;
    /** Right-click / long-press on a chat: the chat list's menu (pin, mute…). */
    onRowMenu?: (event: MouseEvent, room: RoomSummary) => void;
  } = $props();
  /** "Mute for…" open for this space. */
  let mutingSpace = $state<Ref | null>(null);
  /** Long-press / right-click menu on a space (in the list, or its name when open). */
  let spaceMenu = $state<(Ref & { x: number; y: number }) | null>(null);
  function openSpaceMenu(event: MouseEvent, space: Ref) {
    event.preventDefault();
    const at = inBody(event);
    spaceMenu = { ...space, x: Math.min(at.x, at.width - 240), y: Math.min(at.y, at.height - 140) };
  }
  $effect(() => {
    if (spaceMenu) return onBack(() => (spaceMenu = null));
  });
  /** A menu item: the space is read before the menu closes (the menu's `{@const menu}` follows
   * `spaceMenu`, so reading it after closing gave null: its buttons did nothing). */
  function closeMenuThen(act: (space: Ref) => void) {
    const menu = spaceMenu;
    spaceMenu = null;
    if (menu) act({ id: menu.id, name: menu.name, avatar: menu.avatar });
  }

  // ── Groups of spaces (Discord's server folders) and the Spaces tab's order, synced between
  // our devices (stores/spaces.svelte.ts). Folded groups are remembered per device. ──
  let groupMenu = $state<{ group: SpaceGroup; x: number; y: number } | null>(null);
  function openGroupMenu(event: MouseEvent, group: SpaceGroup) {
    event.preventDefault();
    const at = inBody(event);
    groupMenu = { group, x: Math.min(at.x, at.width - 240), y: Math.min(at.y, at.height - 180) };
  }
  $effect(() => {
    if (groupMenu) return onBack(() => (groupMenu = null));
  });
  function closeGroupMenuThen(act: (group: SpaceGroup) => void) {
    const menu = groupMenu;
    groupMenu = null;
    if (menu) act(menu.group);
  }
  /** "Move to a group…" open for this space. */
  let moving = $state<Ref | null>(null);
  /** A choice on that page: the space is read before the page closes (see closeMenuThen). */
  function closeMovingThen(act: (space: Ref) => void) {
    const space = moving;
    moving = null;
    if (space) act(space);
  }
  /** Naming a group: a new one for `spaceId`, or renaming `groupId`. */
  let naming = $state<{ groupId: string | null; spaceId: string | null; name: string } | null>(null);
  /** Reordering: the whole tab (null) or one group's spaces. */
  let ordering = $state<{ groupId: string | null } | null>(null);
  const orderItems = $derived.by(() => {
    if (!ordering) return [];
    if (ordering.groupId) {
      const item = spaces.layout.find((i) => i.kind === "group" && i.group.id === ordering!.groupId);
      return item?.kind === "group" ? item.spaces.map((sp) => ({ id: sp.id, name: sp.name, avatar: sp.avatar })) : [];
    }
    return spaces.layout.map((i) =>
      i.kind === "group" ? { id: i.group.id, name: i.group.name, avatar: null, icon: Folder } : { id: i.space.id, name: i.space.name, avatar: i.space.avatar },
    );
  });
  function saveName() {
    const form = naming;
    if (!form || !form.name.trim()) return;
    naming = null;
    const name = form.name.trim();
    const done = form.groupId ? spaces.renameGroup(form.groupId, name) : form.spaceId ? spaces.moveSpace(form.spaceId, null, name) : null;
    void done?.catch((e) => (error = String(e)));
  }
  const fail = (e: unknown) => (error = String(e));

  type Ref = { id: string; name: string; avatar: string | null };
  /** Spaces opened, outermost first (sub-spaces push onto it). */
  let path = $state<Ref[]>([]);
  // The Spaces tab tapped again while in a space: back to the list.
  let lastHome = untrack(() => home);
  $effect(() => {
    if (home === lastHome) return;
    lastHome = home;
    path = [];
  });
  // Back to the list of spaces (← , Android back, leaving the space): its chat closes.
  let wasInSpace = false;
  $effect(() => {
    const inSpace = path.length > 0;
    if (wasInSpace && !inSpace) untrack(() => onExit?.());
    wasInSpace = inSpace;
  });
  const opened = $derived(path.at(-1) ?? null);
  // Name and photo follow the live list (a just-created space syncs its name a bit later).
  const current = $derived.by(() => {
    if (!opened) return null;
    const live = spaces.list.find((s) => s.id === opened.id);
    return live ? { id: live.id, name: live.name, avatar: live.avatar } : opened;
  });
  let children = $state<SpaceChild[]>([]);
  let loading = $state(false);
  let settingsFor = $state<Ref | null>(null);

  // The open space's banner, read again when its settings close (it may have changed).
  let banner = $state<string | null>(null);
  $effect(() => {
    const id = current?.id;
    void settingsFor; // also when its settings open or close
    banner = id ? (peek<string | null>(`banner:${id}`, true) ?? null) : null;
    if (id)
      spaceBanner(id)
        .then((b) => (remember(`banner:${id}`, b, true), current?.id === id && (banner = b)))
        .catch(() => {});
  });

  /** A space to open once it shows up (just created or joined). */
  let pending = $state<string | null>(null);
  $effect(() => {
    if (nav.space) {
      pending = nav.space;
      nav.space = null;
    }
  });
  $effect(() => {
    const id = pending;
    const space = id && spaces.list.find((s) => s.id === id);
    if (space) {
      pending = null;
      path = [{ id: space.id, name: space.name, avatar: space.avatar }];
    }
  });
  $effect(() => {
    if (newSpace) return onBack(() => (newSpace = false));
  });
  let joining = $state<string | null>(null);
  let error = $state<string | null>(null);

  // Android back: settings page, then up one space.
  $effect(() => {
    if (path.length) return onBack(() => (path = path.slice(0, -1)));
  });
  $effect(() => {
    if (settingsFor) return onBack(() => (settingsFor = null));
  });

  /** Effects keyed on the space, not on its (live) name. */
  const currentId = $derived(opened?.id ?? null);
  $effect(() => {
    const id = currentId;
    // What the space held last time shows at once; the server's answer replaces it.
    const known = id ? peek<SpaceChild[]>(`space:${id}`, true) : undefined;
    children = known ?? [];
    error = null;
    if (!id) return void closeSpace().catch(() => {});
    loading = !known;
    let first = true;
    openSpace(id, (list) => {
      // The first answer is empty until the server's hierarchy arrives (several seconds for a
      // big or federated space): keep loading, or the known list, meanwhile. A later empty
      // answer is real (the core sends one once the server answered, even for an empty space).
      const placeholder = first && !list.length;
      first = false;
      if (placeholder) return;
      remember(`space:${id}`, list, true);
      if (currentId === id) {
        children = list;
        loading = false;
      }
    }).catch((e) => ((error = String(e)), (loading = false)));
  });
  onDestroy(() => void closeSpace().catch(() => {}));

  let canAdd = $state(false);
  $effect(() => {
    const id = currentId;
    canAdd = id ? (peek<boolean>(`canAdd:${id}`, true) ?? false) : false;
    if (id) canAddToSpace(id).then((ok) => (remember(`canAdd:${id}`, ok, true), currentId === id && (canAdd = ok))).catch(() => {});
  });

  const roomsById = $derived(new Map(chatList.rooms.map((r) => [r.id, r])));
  /** While the space's contents load: the chats in it we already joined (known from the
   * Spaces list, no server needed), so they're there at once. */
  const knownJoined = $derived(
    loading && !children.length
      ? (spaces.list.find((s) => s.id === currentId)?.rooms ?? []).flatMap((id) => roomsById.get(id) ?? [])
      : [],
  );
  const unreadIn = (ids: string[]) =>
    ids.filter((id) => {
      const r = roomsById.get(id);
      // Same rule as the Spaces tab's badge (muted chats don't count).
      return r && isUnread(r);
    }).length;

  // Joined chats in the space's own order (set in its settings, shared with other apps).
  const joinedRooms = $derived(
    children.filter((c) => !c.is_space).flatMap((c) => roomsById.get(c.id) ?? []),
  );
  const joinedChats = $derived(joinedRooms.filter((r) => !r.is_voice));
  // Voice channels keep the space's order (not the most recent first), joined or not.
  const voiceChannels = $derived(
    children
      .filter((c) => c.is_voice)
      .map((c) => ({ child: c, room: roomsById.get(c.id) ?? null }))
      .filter((v) => v.room || !v.child.joined),
  );
  const subSpaces = $derived(children.filter((c) => c.is_space));
  /** Which kind comes first in the space's order decides which section comes first. */
  const sections = $derived(sectionOrder(children));
  const joinable = $derived(children.filter((c) => !c.is_space && !c.is_voice && !c.joined && !roomsById.has(c.id)));

  /** Wait for a just-joined (or created) room to reach the chat list. */
  function arrival(id: string, ms = 15_000): Promise<RoomSummary> {
    return new Promise((resolve, reject) => {
      const started = Date.now();
      const look = () => {
        const room = chatList.rooms.find((r) => r.id === id);
        if (room) resolve(room);
        else if (Date.now() - started > ms) reject(new Error("the channel didn't show up in time"));
        else setTimeout(look, 250);
      };
      look();
    });
  }

  // ── Categories: the space's sub-spaces, shown as sections with their chats ──
  let categories = $state<SpaceCategory[]>([]);
  $effect(() => {
    const id = currentId;
    categories = id ? (peek<SpaceCategory[]>(`categories:${id}`, true) ?? []) : [];
  });
  let categoriesTimer: ReturnType<typeof setTimeout> | undefined;
  /** Reload them (after the space's children change, or a change of ours). */
  function loadCategories(delay = 300) {
    const id = currentId;
    clearTimeout(categoriesTimer);
    if (!id) return void (categories = []);
    categoriesTimer = setTimeout(() => {
      spaceCategories(id)
        .then((list) => (remember(`categories:${id}`, list, true), currentId === id && (categories = list)))
        .catch((e) => console.warn("categories", e));
    }, delay);
  }
  $effect(() => {
    void children;
    loadCategories();
  });
  const COLLAPSED_KEY = "alwychat.collapsedCategories";
  let collapsed = $state<string[]>(asList(stored<unknown>(COLLAPSED_KEY, [])));
  function toggleCategory(id: string) {
    collapsed = collapsed.includes(id) ? collapsed.filter((c) => c !== id) : [...collapsed, id];
    localStorage.setItem(COLLAPSED_KEY, JSON.stringify(collapsed));
  }
  /** A sub-space's section: its data once loaded (the name meanwhile). */
  const categoryOf = (sub: SpaceChild): SpaceCategory => categories.find((c) => c.id === sub.id) ?? { id: sub.id, name: sub.name, joined: sub.joined, children: [] };
  let editingCategory = $state<SpaceCategory | null>(null);
  $effect(() => {
    if (editingCategory) return onBack(() => (editingCategory = null));
  });
  /** The space's own chats, which can move into a category. */
  const looseChats = $derived(children.filter((c) => !c.is_space));

  let newCategory = $state<{ name: string; busy: boolean; error: string | null } | null>(null);
  $effect(() => {
    if (newCategory) return onBack(() => (newCategory = null));
  });
  async function createCategory() {
    const space = current;
    const form = newCategory;
    if (!space || !form || !form.name.trim()) return;
    form.busy = true;
    form.error = null;
    try {
      // A sub-space anyone in the space can see into (what Element and Cinny show as sections).
      await createRoom({ kind: "space", name: form.name.trim(), topic: "", access: "space", encrypted: false, parent: space.id, invite: [] });
      newCategory = null;
      loadCategories(1500);
    } catch (e) {
      form.error = String(e);
    } finally {
      form.busy = false;
    }
  }

  // ── Opening a space opens a chat in it (two panes only: on a phone the chat would cover the
  // space's list): the one used last there, else the first in the space's order. ──
  const twoPanes = new MediaQuery("min-width: 721px");
  const LAST_CHAT_KEY = "alwychat.spaceLastChat";
  let lastChat: Record<string, string> = stored<Record<string, string>>(LAST_CHAT_KEY, {});
  /** A space just opened from the list, waiting for its chats to be known. */
  let autoOpen = $state<{ id: string; until: number } | null>(null);

  function enterSpace(space: Ref) {
    path = [space];
    autoOpen = twoPanes.current ? { id: space.id, until: Date.now() + 10_000 } : null;
  }

  /** The first chat in the space's order; undefined while a section before it is still loading. */
  const firstChat = $derived.by((): string | null | undefined => {
    for (const section of sections) {
      if (section === "chats" && joinedChats[0]) return joinedChats[0].id;
      if (section !== "spaces") continue;
      for (const sub of subSpaces) {
        const cat = categories.find((c) => c.id === sub.id);
        if (!cat) return undefined;
        const chat = cat.children.find((c) => !c.is_voice && !c.is_space && roomsById.has(c.id));
        if (chat) return chat.id;
      }
    }
    // Still loading: a chat of the space we already joined opens at once (its exact order
    // comes with the server's answer).
    if (loading) return knownJoined.find((r) => !r.is_voice)?.id;
    return null;
  });

  $effect(() => {
    const wanted = autoOpen;
    if (!wanted) return;
    if (path.length !== 1 || currentId !== wanted.id || Date.now() > wanted.until) return void (autoOpen = null);
    const rooms = spaces.list.find((s) => s.id === wanted.id)?.rooms ?? [];
    const last = lastChat[wanted.id];
    const target = last && rooms.includes(last) && roomsById.has(last) ? last : firstChat;
    if (target === undefined) return;
    autoOpen = null;
    if (target && target !== untrack(() => selectedId)) onSelect(target);
  });

  // Remember the chat used last in each space.
  $effect(() => {
    const id = selectedId;
    const top = path[0]?.id;
    if (!id || !top || lastChat[top] === id) return;
    if (!untrack(() => spaces.list.find((s) => s.id === top)?.rooms.includes(id))) return;
    lastChat = { ...lastChat, [top]: id };
    try {
      localStorage.setItem(LAST_CHAT_KEY, JSON.stringify(lastChat));
    } catch {
      /* only a convenience */
    }
  });

  async function enterVoice(child: SpaceChild, room: RoomSummary | null) {
    error = null;
    if (room) return void call.join(room, false, false, true);
    joining = child.id;
    try {
      await joinRoom(child.id, child.via);
      await call.join(await arrival(child.id), false, false, true);
    } catch (e) {
      error = tr("Couldn't join: {e}", { e: e });
    } finally {
      joining = null;
    }
  }

  let newChannel = $state<{ name: string; busy: boolean; error: string | null } | null>(null);
  $effect(() => {
    if (newChannel) return onBack(() => (newChannel = null));
  });
  async function createChannel() {
    const space = current;
    const form = newChannel;
    if (!space || !form || !form.name.trim()) return;
    form.busy = true;
    form.error = null;
    try {
      const id = await createVoiceChannel(space.id, form.name);
      await arrival(id).catch(() => {});
      newChannel = null;
    } catch (e) {
      form.error = String(e);
    } finally {
      form.busy = false;
    }
  }

  async function join(child: SpaceChild) {
    joining = child.id;
    error = null;
    try {
      await joinRoom(child.id, child.via);
    } catch (e) {
      error = tr("Couldn't join: {e}", { e: e });
    } finally {
      joining = null;
    }
  }
</script>

{#snippet spaceRow(space: (typeof spaces.list)[number], nested: boolean)}
  {@const unread = unreadIn(space.rooms)}
  <button
    class="space"
    class:nested
    onclick={() => enterSpace({ id: space.id, name: space.name, avatar: space.avatar })}
    oncontextmenu={(e) => openSpaceMenu(e, { id: space.id, name: space.name, avatar: space.avatar })}
  >
    <Avatar name={space.name} mxc={space.avatar} size={nested ? 40 : 48} square />
    <span class="text">
      <strong>{space.name}</strong>
      <small class="space-meta">
        {#if space.public !== undefined}
          {#if space.public}<Globe size={12} />{tr("Public")}{:else}<Lock size={12} />{tr("Private")}{/if}
        {/if}
        {#if space.members !== undefined}<span>· {trn(space.members, "{n} member", "{n} members")}</span>{/if}
        {#if spaces.spaceShown(space.id)}<span>{tr(" · also in Chats")}</span>{/if}
      </small>
    </span>
    {#if mutes.space(space.id)}<span class="muted-bell" title={tr("Muted")}><BellOff size={17} /></span>{/if}
    {#if unread}<span class="badge">{unread}</span>{/if}
    <ChevronRight size={18} />
  </button>
{/snippet}

{#if !current}
  <div class="list">
    {#if spaces.loaded && spaces.list.length === 0}
      <p class="state">{tr("You haven't joined any spaces yet.")}<br />{tr("Spaces group chats together, like Discord servers.")}</p>
    {/if}
    {#each spaces.layout as item (item.kind === "group" ? item.group.id : item.space.id)}
      {#if item.kind === "space"}
        {@render spaceRow(item.space, false)}
      {:else}
        {@const g = item.group}
        {@const open = !collapsed.includes(g.id)}
        {@const unread = item.spaces.reduce((n, sp) => n + unreadIn(sp.rooms), 0)}
        <button class="space group" onclick={() => toggleCategory(g.id)} oncontextmenu={(e) => openGroupMenu(e, g)} aria-expanded={open}>
          <span class="group-icon" aria-hidden="true">
            {#each item.spaces.slice(0, 4) as sp (sp.id)}<Avatar name={sp.name} mxc={sp.avatar} size={22} square />{/each}
          </span>
          <span class="text">
            <strong>{g.name}</strong>
            <small>{trn(item.spaces.length, "{n} space", "{n} spaces")}</small>
          </span>
          {#if unread && !open}<span class="badge">{unread}</span>{/if}
          <span class="fold" class:open><ChevronDown size={18} /></span>
        </button>
        {#if open}
          <div class="group-spaces">
            {#each item.spaces as sp (sp.id)}{@render spaceRow(sp, true)}{/each}
          </div>
        {/if}
      {/if}
    {/each}
  </div>
{:else}
  <div class="space-view" in:fly={{ x: 24, duration: 180 }}>
    {#if banner}<img class="space-banner" src={mediaUrl(banner, 400)} alt="" />{/if}
    <header>
      <button class="icon" onclick={() => (path = path.slice(0, -1))} aria-label={tr("Back")}><ChevronLeft size={22} /></button>
      <Avatar name={current.name} mxc={current.avatar} size={32} square />
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <strong class="title" oncontextmenu={(e) => path.length === 1 && openSpaceMenu(e, current)}>
        {#if path.length === 1 && mutes.space(current.id)}<BellOff size={15} />{/if}{current.name}
      </strong>
      <button class="icon" onclick={() => (settingsFor = current)} aria-label={tr("Space settings")}><Settings size={20} /></button>
    </header>
    {#if error}<p class="state error">{error}</p>{/if}
    <!-- New voice channels: in the space's settings. -->
    {#snippet chatsSection()}
    {#if joinedChats.length}<h4>{tr("Chats")}</h4>{/if}
    {#each joinedChats as room (room.id)}
      <ChatRow {room} selected={room.id === selectedId} onclick={() => onSelect(room.id)} oncontextmenu={onRowMenu && ((e) => onRowMenu(e, room))} />
    {/each}
    {/snippet}
    {#snippet voiceSection()}
    {#if voiceChannels.length}
      <h4>{tr("Voice channels")}</h4>
      {#each voiceChannels as v (v.child.id)}
        <VoiceChannel
          room={v.room}
          name={v.child.name}
          busy={joining === v.child.id}
          onJoin={() => enterVoice(v.child, v.room)}
          onChat={v.room ? () => onSelect(v.child.id) : undefined}
        />
      {/each}
    {/if}
    {/snippet}
    {#snippet categoriesSection()}
    {#each subSpaces as sub (sub.id)}
      {@const cat = categoryOf(sub)}
      {@const open = !collapsed.includes(cat.id)}
      {@const unread = unreadIn(cat.children.map((c) => c.id))}
      <h4 class="category">
        <button class="category-toggle" onclick={() => toggleCategory(cat.id)} aria-expanded={open}>
          <span class="chevron" class:closed={!open}><ChevronDown size={14} /></span>
          <span class="category-name">{cat.name}</span>
          {#if unread && !open}<span class="badge">{unread}</span>{/if}
        </button>
        {#if canAdd}
          <button class="icon small" onclick={() => (editingCategory = cat)} aria-label={tr("Edit category")} title={tr("Edit category")}><Pencil size={15} /></button>
        {/if}
        <button class="icon small" onclick={() => (path = [...path, { id: sub.id, name: sub.name, avatar: sub.avatar }])} aria-label={tr("Open {name}", { name: cat.name })} title={tr("Open {name}", { name: cat.name })}><ChevronRight size={16} /></button>
      </h4>
      {#if open}
        {#each cat.children as child (child.id)}
          {@const room = roomsById.get(child.id) ?? null}
          {#if child.is_voice}
            <VoiceChannel {room} name={child.name} busy={joining === child.id} onJoin={() => enterVoice(child, room)} onChat={room ? () => onSelect(child.id) : undefined} />
          {:else if child.is_space}
            <button class="space" onclick={() => (path = [...path, { id: child.id, name: child.name, avatar: child.avatar }])}>
              <Avatar name={child.name} mxc={child.avatar} size={36} square />
              <span class="text"><strong>{child.name}</strong></span>
              <ChevronRight size={18} />
            </button>
          {:else if room}
            <ChatRow {room} selected={room.id === selectedId} onclick={() => onSelect(room.id)} oncontextmenu={onRowMenu && ((e) => onRowMenu(e, room))} />
          {:else}
            <div class="joinable">
              <Avatar name={child.name} mxc={child.avatar} size={44} />
              <span class="text"><strong>{child.name}</strong><small><Users size={12} /> {child.members}{child.topic ? ` · ${child.topic}` : ""}</small></span>
              <button class="join" disabled={joining === child.id} onclick={() => join(child)}>{joining === child.id ? tr("Joining…") : tr("Join")}</button>
            </div>
          {/if}
        {:else}
          <p class="category-empty">{cat.joined || categories.some((c) => c.id === cat.id) ? tr("No chats in this category yet.") : tr("Loading…")}</p>
        {/each}
      {/if}
    {/each}
    {/snippet}
    <!-- Sections in the space's own order (set in its settings → Change the order). -->
    {#each sections as section (section)}
      {#if section === "chats"}{@render chatsSection()}{:else if section === "voice"}{@render voiceSection()}{:else}{@render categoriesSection()}{/if}
    {/each}
    {#if canAdd}
      <button class="new-category" onclick={() => (newCategory = { name: "", busy: false, error: null })}><Plus size={16} />{tr("New category")}</button>
    {/if}
    {#if joinable.length}
      <h4>{tr("Other chats in this space")}</h4>
      {#each joinable as child (child.id)}
        <div class="joinable">
          <Avatar name={child.name} mxc={child.avatar} size={44} />
          <span class="text">
            <strong>{child.name}{#if child.suggested}<em> {tr("· suggested")}</em>{/if}</strong>
            <small><Users size={12} /> {child.members}{child.topic ? ` · ${child.topic}` : ""}</small>
          </span>
          <button class="join" disabled={joining === child.id} onclick={() => join(child)}>
            {joining === child.id ? tr("Joining…") : tr("Join")}
          </button>
        </div>
      {/each}
    {/if}
    {#if loading && !children.length}
      {#each knownJoined as room (room.id)}
        <ChatRow {room} selected={room.id === selectedId} onclick={() => onSelect(room.id)} oncontextmenu={onRowMenu && ((e) => onRowMenu(e, room))} />
      {/each}
      <p class="loading-line" role="status"><LoaderCircle size={15} />{knownJoined.length ? tr("Loading the rest of the space…") : tr("Loading the space…")}</p>
      {#if !knownJoined.length}
        {#each Array(4) as _, i (i)}<div class="skeleton"></div>{/each}
      {/if}
    {:else if !joinedChats.length && !voiceChannels.length && !subSpaces.length && !joinable.length && !error}
      <p class="state">{tr("This space has no chats yet.")}</p>
    {/if}
  </div>
{/if}

{#if newCategory}
  {@const form = newCategory}
  <Page title={tr("New category")} onClose={() => (newCategory = null)}>
    <form class="new-channel" onsubmit={(e) => (e.preventDefault(), createCategory())}>
      <!-- svelte-ignore a11y_autofocus -->
      <input placeholder={tr("Category name")} maxlength="60" bind:value={form.name} autofocus />
      <p class="hint">{tr("A section of {value}, with its own chats and voice channels. Element and Cinny show it as a sub-space.", { value: current?.name ?? "the space" })}</p>
      {#if form.error}<p class="hint error">{form.error}</p>{/if}
      <button class="create" disabled={!form.name.trim() || form.busy}>{form.busy ? tr("Creating…") : tr("Create")}</button>
    </form>
  </Page>
{/if}
{#if editingCategory && current}
  <CategoryPage spaceId={current.id} category={categories.find((c) => c.id === editingCategory?.id) ?? editingCategory} loose={looseChats} onChanged={() => loadCategories(1500)} onClose={() => (editingCategory = null)} />
{/if}

{#if newChannel}
  {@const form = newChannel}
  <Page title={tr("New voice channel")} onClose={() => (newChannel = null)}>
    <form class="new-channel" onsubmit={(e) => (e.preventDefault(), createChannel())}>
      <!-- svelte-ignore a11y_autofocus -->
      <input placeholder={tr("Channel name")} maxlength="60" bind:value={form.name} autofocus />
      <p class="hint">{tr("Everyone in {value} can join. Tapping the channel joins the call; it also has a text chat. Voice channels aren't end-to-end encrypted, so people using Element can join too.", { value: current?.name ?? "the space" })}</p>
      {#if form.error}<p class="hint error">{form.error}</p>{/if}
      <button class="create" disabled={!form.name.trim() || form.busy}>{form.busy ? tr("Creating…") : tr("Create")}</button>
    </form>
  </Page>
{/if}

{#if spaceMenu}
  {@const menu = spaceMenu}
  {@const muted = mutes.space(menu.id)}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="menu-catcher" onclick={() => (spaceMenu = null)} oncontextmenu={(e) => (e.preventDefault(), (spaceMenu = null))}></div>
  <div class="row-menu" style:left="{menu.x}px" style:top="{menu.y}px" transition:fade={{ duration: 100 }}>
    {#if muted}
      <button onclick={() => closeMenuThen((space) => void mutes.unmuteSpace(space.id).catch((e) => (error = String(e))))}>
        <Bell size={16} />{muted.until ? tr("Unmute space (muted until {time})", { time: laterTime(muted.until) }) : tr("Unmute space")}
      </button>
    {:else}
      <button onclick={() => closeMenuThen((space) => (mutingSpace = space))}><BellOff size={16} />{tr("Mute space…")}</button>
    {/if}
    <button onclick={() => closeMenuThen((space) => (settingsFor = space))}><Settings size={16} />{tr("Space settings")}</button>
    {#if path.length === 0}
      <button onclick={() => closeMenuThen((space) => (moving = space))}><FolderInput size={16} />{tr("Move to a group…")}</button>
      <button onclick={() => closeMenuThen((space) => (ordering = { groupId: spaces.groupOf(space.id)?.id ?? null }))}><ArrowUpDown size={16} />{tr("Reorder spaces")}</button>
    {/if}
  </div>
{/if}
{#if groupMenu}
  {@const menu = groupMenu}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="menu-catcher" onclick={() => (groupMenu = null)} oncontextmenu={(e) => (e.preventDefault(), (groupMenu = null))}></div>
  <div class="row-menu" style:left="{menu.x}px" style:top="{menu.y}px" transition:fade={{ duration: 100 }}>
    <button onclick={() => closeGroupMenuThen((g) => (naming = { groupId: g.id, spaceId: null, name: g.name }))}><Pencil size={16} />{tr("Rename group")}</button>
    <button onclick={() => closeGroupMenuThen((g) => (ordering = { groupId: g.id }))}><ArrowUpDown size={16} />{tr("Reorder its spaces")}</button>
    <button onclick={() => closeGroupMenuThen(() => (ordering = { groupId: null }))}><ArrowUpDown size={16} />{tr("Reorder spaces")}</button>
    <button onclick={() => closeGroupMenuThen((g) => void spaces.deleteGroup(g.id).catch(fail))}><Ungroup size={16} />{tr("Ungroup")}</button>
  </div>
{/if}
{#if moving}
  {@const space = moving}
  {@const inGroup = spaces.groupOf(space.id)}
  <Page title={tr("Move {name} to…", { name: space.name })} onClose={() => (moving = null)}>
    <div class="choices">
      {#each spaces.prefs.space_groups ?? [] as g (g.id)}
        <button
          class="choice"
          disabled={inGroup?.id === g.id}
          onclick={() => closeMovingThen((sp) => void spaces.moveSpace(sp.id, g.id).catch(fail))}
        >
          <Folder size={18} /><span>{g.name}</span>{#if inGroup?.id === g.id}<Check size={16} />{/if}
        </button>
      {/each}
      <button class="choice" onclick={() => closeMovingThen((sp) => (naming = { groupId: null, spaceId: sp.id, name: "" }))}><Plus size={18} /><span>{tr("New group…")}</span></button>
      {#if inGroup}
        <button class="choice" onclick={() => closeMovingThen((sp) => void spaces.moveSpace(sp.id, null).catch(fail))}><Ungroup size={18} /><span>{tr("Not in a group")}</span></button>
      {/if}
    </div>
    <p class="hint">{tr("Groups and their order are the same in all your AlwyChat apps.")}</p>
  </Page>
{/if}
{#if naming}
  {@const form = naming}
  <Page title={form.groupId ? tr("Rename group") : tr("New group")} onClose={() => (naming = null)}>
    <form class="new-channel" onsubmit={(e) => (e.preventDefault(), saveName())}>
      <!-- svelte-ignore a11y_autofocus -->
      <input placeholder={tr("Group name")} maxlength="40" bind:value={form.name} autofocus />
      <button class="create" disabled={!form.name.trim()}>{form.groupId ? tr("Save") : tr("Create")}</button>
    </form>
  </Page>
{/if}
{#if ordering}
  {@const groupId = ordering.groupId}
  <Page title={groupId ? (spaces.prefs.space_groups.find((g) => g.id === groupId)?.name ?? tr("Group")) : tr("Reorder spaces")} onClose={() => (ordering = null)}>
    <ReorderList items={orderItems} square onMove={(ids) => void (groupId ? spaces.setGroupOrder(groupId, ids) : spaces.setSpaceOrder(ids)).catch(fail)} />
    <p class="hint">{tr("Drag by the handle. The order is the same in all your AlwyChat apps.")}</p>
  </Page>
{/if}
{#if mutingSpace}
  {@const space = mutingSpace}
  <MuteChooser
    title={tr("Mute the space {name}", { name: space.name })}
    onPick={(until) => void mutes.muteSpace(space.id, until).catch((e) => (error = String(e)))}
    onClose={() => (mutingSpace = null)}
  />
{/if}
{#if settingsFor}
  {@const space = settingsFor}
  <SpaceSettingsPage
    spaceId={space.id}
    {children}
    onClose={() => (settingsFor = null)}
    onLeft={() => ((settingsFor = null), (path = path.slice(0, -1)))}
    onNewVoiceChannel={() => ((settingsFor = null), (newChannel = { name: "", busy: false, error: null }))}
  />
{/if}

{#if newSpace}
  <CreateRoomPage
    kind="space"
    onClose={() => (newSpace = false)}
    onCreated={(id) => ((newSpace = false), (pending = id))}
  />
{/if}

<style>
  .list,
  .space-view {
    display: flex;
    flex-direction: column;
  }
  .space-banner {
    width: calc(100% - 16px);
    aspect-ratio: 3;
    margin: 4px 8px 0;
    border-radius: 14px;
    object-fit: cover;
  }
  .space {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 12px;
    border: none;
    border-radius: 14px;
    background: none;
    color: var(--text);
    text-align: left;
    cursor: pointer;
  }
  .space:hover {
    background: var(--surface-hover);
  }
  .text {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .text strong {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .text small {
    overflow: hidden;
    color: var(--muted);
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .text small :global(svg) {
    margin-right: 4px;
    vertical-align: -1px;
  }
  em {
    color: var(--muted);
    font-style: normal;
    font-weight: 400;
  }
  .badge {
    min-width: 22px;
    padding: 2px 7px;
    border-radius: 999px;
    background: var(--accent);
    color: white;
    font-size: 0.8rem;
    font-weight: 600;
    text-align: center;
  }
  header {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 4px 4px 10px;
  }
  .title {
    flex: 1;
    overflow: hidden;
    font-size: 1.1rem;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .icon {
    display: grid;
    width: 38px;
    height: 38px;
    place-items: center;
    border: none;
    border-radius: 50%;
    background: none;
    color: var(--text);
    cursor: pointer;
  }
  .icon:hover {
    background: var(--surface-hover);
  }
  .title :global(svg) {
    margin-right: 6px;
    color: var(--muted);
    vertical-align: -2px;
  }
  .muted-bell {
    display: grid;
    flex: none;
    color: var(--muted);
  }
  .space-meta {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .space.nested {
    padding-left: 28px;
  }
  .group-icon {
    display: grid;
    flex: none;
    grid-template-columns: repeat(2, 22px);
    gap: 2px;
    width: 48px;
    height: 48px;
    padding: 1px;
    box-sizing: border-box;
    border-radius: 14px;
    background: var(--surface-2);
    place-content: center;
  }
  .fold {
    display: grid;
    transition: transform 0.15s var(--ease);
    transform: rotate(-90deg);
  }
  .fold.open {
    transform: none;
  }
  .choices {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .choice {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 14px;
    border: none;
    border-radius: 12px;
    background: var(--surface);
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .choice span {
    flex: 1;
  }
  .choice:disabled {
    opacity: 0.6;
    cursor: default;
  }
  .menu-catcher {
    position: fixed;
    inset: 0;
    z-index: 40;
  }
  .row-menu {
    position: fixed;
    z-index: 41;
    display: flex;
    flex-direction: column;
    min-width: 200px;
    padding: 6px;
    border: 1px solid var(--border);
    border-radius: 12px;
    background: var(--surface-2);
    box-shadow: 0 12px 32px rgb(0 0 0 / 0.45);
  }
  .row-menu button {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 9px 10px;
    border: none;
    border-radius: 8px;
    background: none;
    color: var(--text);
    text-align: left;
    cursor: pointer;
  }
  .row-menu button:hover {
    background: var(--surface-hover);
  }
  h4 {
    margin: 16px 12px 6px;
    color: var(--muted);
    font-size: 0.867rem;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }
  .joinable {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 12px;
  }
  .join {
    padding: 7px 16px;
    border: none;
    border-radius: 999px;
    background: var(--accent);
    color: white;
    cursor: pointer;
  }
  h4.category {
    display: flex;
    align-items: center;
    gap: 2px;
    margin: 14px 4px 4px 6px;
  }
  .category-toggle {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 4px;
    min-width: 0;
    padding: 4px 6px;
    border: 0;
    background: none;
    color: var(--muted);
    font: inherit;
    letter-spacing: inherit;
    text-transform: inherit;
    cursor: pointer;
    text-align: left;
  }
  .category-toggle:hover {
    color: var(--text);
  }
  .chevron {
    display: grid;
    transition: transform 0.15s;
  }
  .chevron.closed {
    transform: rotate(-90deg);
  }
  .category-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .category .badge {
    margin-left: auto;
    min-width: 18px;
    padding: 0 6px;
    border-radius: 9px;
    background: var(--accent);
    color: #fff;
    font-size: 0.733rem;
    text-align: center;
    text-transform: none;
  }
  .category-empty {
    margin: 2px 14px 8px;
    color: var(--muted);
    font-size: 0.85rem;
  }
  .new-category {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 12px;
    padding: 8px 12px;
    border: 1px dashed var(--border);
    border-radius: 12px;
    background: none;
    color: var(--muted);
    font: inherit;
    cursor: pointer;
  }
  .icon.small {
    width: 30px;
    height: 30px;
    color: var(--muted);
  }
  .new-channel {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .new-channel input {
    padding: 11px 14px;
    border: 1px solid var(--border);
    border-radius: 12px;
    background: var(--surface);
    color: inherit;
    font: inherit;
    font-size: 1.05rem;
  }
  .hint {
    margin: 0;
    color: var(--muted);
    font-size: 0.9rem;
    line-height: 1.4;
  }
  .hint.error {
    color: var(--danger);
  }
  .create {
    padding: 11px 18px;
    border: none;
    border-radius: 12px;
    background: var(--accent);
    color: white;
    font: inherit;
    font-weight: 600;
    cursor: pointer;
  }
  .create:disabled {
    opacity: 0.45;
  }
  .join:disabled {
    opacity: 0.6;
  }
  .state {
    padding: 24px 16px;
    color: var(--muted);
    text-align: center;
  }
  .state.error {
    color: var(--danger);
  }
  .loading-line {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    margin: 10px 0;
    color: var(--muted);
    font-size: 0.85rem;
  }
  .loading-line :global(svg) {
    animation: spin 1s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .loading-line :global(svg) {
      animation: none;
    }
  }
  .skeleton {
    height: 64px;
    margin: 4px 8px;
    border-radius: 14px;
    background: var(--surface-2);
  }
</style>
