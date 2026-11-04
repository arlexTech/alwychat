<script lang="ts">
  import { tr, trn } from "$lib/i18n/index.svelte";
  import { onDestroy } from "svelte";
  import { fly } from "svelte/transition";
  import ChevronLeft from "@lucide/svelte/icons/chevron-left";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import Settings from "@lucide/svelte/icons/settings";
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
  import Pencil from "@lucide/svelte/icons/pencil";
  import { call } from "$lib/stores/call.svelte";
  import { chatList } from "$lib/stores/chatList.svelte";
  import { spaces } from "$lib/stores/spaces.svelte";
  import Avatar from "./Avatar.svelte";
  import { spaceBanner } from "$lib/backend";
  import { mediaUrl } from "$lib/media";
  import ChatRow from "./ChatRow.svelte";
  import Page from "./Page.svelte";
  import VoiceChannel from "./VoiceChannel.svelte";
  import SpaceSettingsPage from "./SpaceSettingsPage.svelte";
  import CreateRoomPage from "./CreateRoomPage.svelte";
  import { nav } from "$lib/stores/nav.svelte";

  let { selectedId, onSelect }: { selectedId: string | null; onSelect: (roomId: string) => void } = $props();

  type Ref = { id: string; name: string; avatar: string | null };
  /** Spaces opened, outermost first (sub-spaces push onto it). */
  let path = $state<Ref[]>([]);
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
    if (id) spaceBanner(id).then((b) => current?.id === id && (banner = b)).catch(() => {});
    else banner = null;
  });
  let newSpace = $state(false);
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
    children = [];
    error = null;
    if (!id) return void closeSpace().catch(() => {});
    loading = true;
    openSpace(id, (list) => {
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
    canAdd = false;
    if (id) canAddToSpace(id).then((ok) => currentId === id && (canAdd = ok)).catch(() => {});
  });

  const roomsById = $derived(new Map(chatList.rooms.map((r) => [r.id, r])));
  const unreadIn = (ids: string[]) =>
    ids.filter((id) => {
      const r = roomsById.get(id);
      return r && (r.unread_messages > 0 || r.marked_unread);
    }).length;

  // Joined chats in the chat list's order (most recent first); then sub-spaces; then the rest.
  const joinedRooms = $derived.by(() => {
    const ids = new Set(children.filter((c) => !c.is_space && c.joined).map((c) => c.id));
    return chatList.rooms.filter((r) => ids.has(r.id));
  });
  const joinedChats = $derived(joinedRooms.filter((r) => !r.is_voice));
  // Voice channels keep the space's order (not the most recent first), joined or not.
  const voiceChannels = $derived(
    children
      .filter((c) => c.is_voice)
      .map((c) => ({ child: c, room: roomsById.get(c.id) ?? null }))
      .filter((v) => v.room || !v.child.joined),
  );
  const subSpaces = $derived(children.filter((c) => c.is_space));
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
  let categoriesTimer: ReturnType<typeof setTimeout> | undefined;
  /** Reload them (after the space's children change, or a change of ours). */
  function loadCategories(delay = 300) {
    const id = currentId;
    clearTimeout(categoriesTimer);
    if (!id) return void (categories = []);
    categoriesTimer = setTimeout(() => {
      spaceCategories(id)
        .then((list) => currentId === id && (categories = list))
        .catch((e) => console.warn("categories", e));
    }, delay);
  }
  $effect(() => {
    void children;
    loadCategories();
  });
  const COLLAPSED_KEY = "alwychat.collapsedCategories";
  let collapsed = $state<string[]>(JSON.parse(localStorage.getItem(COLLAPSED_KEY) ?? "[]"));
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

  async function enterVoice(child: SpaceChild, room: RoomSummary | null) {
    error = null;
    if (room) return void call.join(room, false, false);
    joining = child.id;
    try {
      await joinRoom(child.id, child.via);
      await call.join(await arrival(child.id), false, false);
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

{#if !current}
  <div class="list">
    <button class="space new" onclick={() => (newSpace = true)}>
      <span class="plus"><Plus size={22} /></span>
      <span class="text"><strong>{tr("New space")}</strong><small>{tr("Group chats and voice channels, like a server")}</small></span>
    </button>
    {#if spaces.loaded && spaces.list.length === 0}
      <p class="state">{tr("You haven't joined any spaces yet.")}<br />{tr("Spaces group chats together, like Discord servers.")}</p>
    {/if}
    {#each spaces.list as space (space.id)}
      {@const unread = unreadIn(space.rooms)}
      <button class="space" onclick={() => (path = [{ id: space.id, name: space.name, avatar: space.avatar }])}>
        <Avatar name={space.name} mxc={space.avatar} size={48} square />
        <span class="text">
          <strong>{space.name}</strong>
          <small>{trn(space.rooms.length, "{n} chat", "{n} chats")}{spaces.spaceShown(space.id) ? tr(" · also in Chats") : ""}</small>
        </span>
        {#if unread}<span class="badge">{unread}</span>{/if}
        <ChevronRight size={18} />
      </button>
    {/each}
  </div>
{:else}
  <div class="space-view" in:fly={{ x: 24, duration: 180 }}>
    {#if banner}<img class="space-banner" src={mediaUrl(banner, 400)} alt="" />{/if}
    <header>
      <button class="icon" onclick={() => (path = path.slice(0, -1))} aria-label={tr("Back")}><ChevronLeft size={22} /></button>
      <Avatar name={current.name} mxc={current.avatar} size={32} square />
      <strong class="title">{current.name}</strong>
      <button class="icon" onclick={() => (settingsFor = current)} aria-label={tr("Space settings")}><Settings size={20} /></button>
    </header>
    {#if error}<p class="state error">{error}</p>{/if}
    {#each joinedChats as room (room.id)}
      <ChatRow {room} selected={room.id === selectedId} onclick={() => onSelect(room.id)} />
    {/each}
    {#if voiceChannels.length || canAdd}
      <h4 class="with-action">
        {tr("Voice channels")}
        {#if canAdd}
          <button class="icon small" onclick={() => (newChannel = { name: "", busy: false, error: null })} aria-label={tr("New voice channel")} title={tr("New voice channel")}><Plus size={18} /></button>
        {/if}
      </h4>
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
            <ChatRow {room} selected={room.id === selectedId} onclick={() => onSelect(room.id)} />
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
      {#each Array(4) as _, i (i)}<div class="skeleton"></div>{/each}
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
  .new .plus {
    display: grid;
    width: 48px;
    height: 48px;
    place-items: center;
    border: 2px dashed var(--border);
    border-radius: 14px;
    color: var(--muted);
    box-sizing: border-box;
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
    font-size: 12px;
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
  h4 {
    margin: 16px 12px 6px;
    color: var(--muted);
    font-size: 13px;
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
    font-size: 11px;
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
  .with-action {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-right: 4px;
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
  .skeleton {
    height: 64px;
    margin: 4px 8px;
    border-radius: 14px;
    background: var(--surface-2);
  }
</style>
