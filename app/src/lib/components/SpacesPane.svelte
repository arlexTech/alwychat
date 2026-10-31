<script lang="ts">
  import { onDestroy } from "svelte";
  import { fly } from "svelte/transition";
  import ChevronLeft from "@lucide/svelte/icons/chevron-left";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import Settings from "@lucide/svelte/icons/settings";
  import Users from "@lucide/svelte/icons/users";
  import { onBack } from "$lib/back.svelte";
  import { closeSpace, joinRoom, openSpace, type SpaceChild } from "$lib/backend";
  import { chatList } from "$lib/stores/chatList.svelte";
  import { spaces } from "$lib/stores/spaces.svelte";
  import Avatar from "./Avatar.svelte";
  import ChatRow from "./ChatRow.svelte";
  import Page from "./Page.svelte";

  let { selectedId, onSelect }: { selectedId: string | null; onSelect: (roomId: string) => void } = $props();

  type Ref = { id: string; name: string; avatar: string | null };
  /** Spaces opened, outermost first (sub-spaces push onto it). */
  let path = $state<Ref[]>([]);
  const current = $derived(path.at(-1) ?? null);
  let children = $state<SpaceChild[]>([]);
  let loading = $state(false);
  let settingsFor = $state<Ref | null>(null);
  let joining = $state<string | null>(null);
  let error = $state<string | null>(null);

  // Android back: settings page, then up one space.
  $effect(() => {
    if (path.length) return onBack(() => (path = path.slice(0, -1)));
  });
  $effect(() => {
    if (settingsFor) return onBack(() => (settingsFor = null));
  });

  $effect(() => {
    const space = current;
    children = [];
    error = null;
    if (!space) return void closeSpace().catch(() => {});
    loading = true;
    openSpace(space.id, (list) => {
      if (current?.id === space.id) {
        children = list;
        loading = false;
      }
    }).catch((e) => ((error = String(e)), (loading = false)));
  });
  onDestroy(() => void closeSpace().catch(() => {}));

  const roomsById = $derived(new Map(chatList.rooms.map((r) => [r.id, r])));
  const unreadIn = (ids: string[]) =>
    ids.filter((id) => {
      const r = roomsById.get(id);
      return r && (r.unread_messages > 0 || r.marked_unread);
    }).length;

  // Joined chats in the chat list's order (most recent first); then sub-spaces; then the rest.
  const joinedChats = $derived.by(() => {
    const ids = new Set(children.filter((c) => !c.is_space && c.joined).map((c) => c.id));
    return chatList.rooms.filter((r) => ids.has(r.id));
  });
  const subSpaces = $derived(children.filter((c) => c.is_space));
  const joinable = $derived(children.filter((c) => !c.is_space && !c.joined && !roomsById.has(c.id)));

  async function join(child: SpaceChild) {
    joining = child.id;
    error = null;
    try {
      await joinRoom(child.id, child.via);
    } catch (e) {
      error = `Couldn't join: ${e}`;
    } finally {
      joining = null;
    }
  }
</script>

{#if !current}
  <div class="list">
    {#if spaces.loaded && spaces.list.length === 0}
      <p class="state">You haven't joined any spaces yet.<br />Spaces group chats together, like Discord servers.</p>
    {/if}
    {#each spaces.list as space (space.id)}
      {@const unread = unreadIn(space.rooms)}
      <button class="space" onclick={() => (path = [{ id: space.id, name: space.name, avatar: space.avatar }])}>
        <Avatar name={space.name} mxc={space.avatar} size={48} square />
        <span class="text">
          <strong>{space.name}</strong>
          <small>{space.rooms.length} {space.rooms.length === 1 ? "chat" : "chats"}{spaces.spaceShown(space.id) ? " · also in Chats" : ""}</small>
        </span>
        {#if unread}<span class="badge">{unread}</span>{/if}
        <ChevronRight size={18} />
      </button>
    {/each}
  </div>
{:else}
  <div class="space-view" in:fly={{ x: 24, duration: 180 }}>
    <header>
      <button class="icon" onclick={() => (path = path.slice(0, -1))} aria-label="Back"><ChevronLeft size={22} /></button>
      <Avatar name={current.name} mxc={current.avatar} size={32} square />
      <strong class="title">{current.name}</strong>
      <button class="icon" onclick={() => (settingsFor = path[0])} aria-label="Space settings"><Settings size={20} /></button>
    </header>
    {#if error}<p class="state error">{error}</p>{/if}
    {#each joinedChats as room (room.id)}
      <ChatRow {room} selected={room.id === selectedId} onclick={() => onSelect(room.id)} />
    {/each}
    {#each subSpaces as sub (sub.id)}
      <button class="space" onclick={() => (path = [...path, { id: sub.id, name: sub.name, avatar: sub.avatar }])}>
        <Avatar name={sub.name} mxc={sub.avatar} size={44} square />
        <span class="text"><strong>{sub.name}</strong><small>Space{sub.joined ? "" : " · not joined"}</small></span>
        <ChevronRight size={18} />
      </button>
    {/each}
    {#if joinable.length}
      <h4>Other chats in this space</h4>
      {#each joinable as child (child.id)}
        <div class="joinable">
          <Avatar name={child.name} mxc={child.avatar} size={44} />
          <span class="text">
            <strong>{child.name}{#if child.suggested}<em> · suggested</em>{/if}</strong>
            <small><Users size={12} /> {child.members}{child.topic ? ` · ${child.topic}` : ""}</small>
          </span>
          <button class="join" disabled={joining === child.id} onclick={() => join(child)}>
            {joining === child.id ? "Joining…" : "Join"}
          </button>
        </div>
      {/each}
    {/if}
    {#if loading && !children.length}
      {#each Array(4) as _, i (i)}<div class="skeleton"></div>{/each}
    {:else if !joinedChats.length && !subSpaces.length && !joinable.length && !error}
      <p class="state">This space has no chats yet.</p>
    {/if}
  </div>
{/if}

{#if settingsFor}
  {@const space = settingsFor}
  <Page title={space.name} onClose={() => (settingsFor = null)}>
    <label class="toggle">
      <span>
        <strong>Show its chats in Chats</strong>
        <small>Its chats also appear in the Chats tab. Each chat can override this in its settings.</small>
      </span>
      <input
        type="checkbox"
        role="switch"
        checked={spaces.spaceShown(space.id)}
        onchange={(e) => spaces.setSpace(space.id, e.currentTarget.checked).catch(() => {})}
      />
    </label>
  </Page>
{/if}

<style>
  .list,
  .space-view {
    display: flex;
    flex-direction: column;
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
  .toggle {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 12px 4px;
  }
  .toggle span {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 4px;
  }
  .toggle small {
    color: var(--muted);
  }
</style>
