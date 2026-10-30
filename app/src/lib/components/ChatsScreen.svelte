<script lang="ts">
  import { onMount } from "svelte";
  import { flip } from "svelte/animate";
  import { fade } from "svelte/transition";
  import type { Account, RoomSummary } from "$lib/backend";
  import { chatList } from "$lib/stores/chatList.svelte";
  import Avatar from "./Avatar.svelte";
  import ChatRow from "./ChatRow.svelte";

  let { account, onSignOut }: { account: Account; onSignOut: () => void } = $props();

  type Filter = "all" | "unread" | "people" | "groups" | "favourites";
  const filters: { id: Filter; label: string }[] = [
    { id: "all", label: "All" },
    { id: "unread", label: "Unread" },
    { id: "people", label: "People" },
    { id: "groups", label: "Groups" },
    { id: "favourites", label: "Favourites" },
  ];

  let filter = $state<Filter>("all");
  let query = $state("");
  let selectedId = $state<string | null>(null);
  let menuOpen = $state(false);

  const myName = $derived(account.display_name ?? account.user_id.slice(1).split(":")[0]);
  const selected = $derived(chatList.rooms.find((r) => r.id === selectedId) ?? null);

  const matches: Record<Filter, (r: RoomSummary) => boolean> = {
    all: () => true,
    unread: (r) => r.unread_messages > 0 || r.marked_unread || r.is_invite,
    people: (r) => r.is_dm,
    groups: (r) => !r.is_dm,
    favourites: (r) => r.is_favourite,
  };

  const visible = $derived.by(() => {
    const q = query.trim().toLocaleLowerCase();
    return chatList.rooms.filter(
      (r) => matches[filter](r) && (!q || r.name.toLocaleLowerCase().includes(q)),
    );
  });

  onMount(() => {
    chatList.start();
    return () => chatList.reset();
  });
</script>

<div class="shell" class:has-selection={selected}>
  <aside class="list-pane">
    <header>
      <button class="me" onclick={() => (menuOpen = !menuOpen)} aria-label="Account">
        <Avatar name={myName} size={36} />
      </button>
      <h1>Chats</h1>
      {#if menuOpen}
        <div class="menu" transition:fade={{ duration: 120 }}>
          <div class="menu-who">
            <strong>{myName}</strong>
            <span>{account.user_id}</span>
          </div>
          <button onclick={onSignOut}>Sign out</button>
        </div>
      {/if}
    </header>

    <input class="search" type="search" placeholder="Search" bind:value={query} />

    <nav class="chips">
      {#each filters as f (f.id)}
        <button class:active={filter === f.id} onclick={() => (filter = f.id)}>{f.label}</button>
      {/each}
    </nav>

    <div class="rows">
      {#if chatList.error}
        <p class="state error">{chatList.error}</p>
      {:else if !chatList.loaded}
        {#each Array(8) as _, i (i)}
          <div class="skeleton" style:animation-delay="{i * 60}ms"></div>
        {/each}
      {:else if visible.length === 0}
        <p class="state">{query ? "No chats match your search" : "Nothing here yet"}</p>
      {:else}
        {#each visible as room (room.id)}
          <div animate:flip={{ duration: 220 }}>
            <ChatRow {room} selected={room.id === selectedId} onclick={() => (selectedId = room.id)} />
          </div>
        {/each}
      {/if}
    </div>
  </aside>

  <section class="chat-pane">
    {#if selected}
      <header class="chat-header">
        <button class="back" onclick={() => (selectedId = null)} aria-label="Back">‹</button>
        <Avatar name={selected.name} mxc={selected.avatar} size={40} />
        <div>
          <strong>{selected.name}</strong>
          {#if selected.is_encrypted}<span class="sub">🔒 End-to-end encrypted</span>{/if}
        </div>
      </header>
      <div class="placeholder">Messages arrive in the next step ✨</div>
    {:else}
      <div class="placeholder">Select a chat to start messaging</div>
    {/if}
  </section>
</div>

<style>
  .shell {
    display: grid;
    grid-template-columns: minmax(300px, 380px) 1fr;
    height: 100vh;
  }
  .list-pane {
    display: flex;
    flex-direction: column;
    min-height: 0;
    border-right: 1px solid var(--border);
    background: var(--surface);
  }
  header {
    position: relative;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 16px 16px 8px;
  }
  h1 {
    margin: 0;
    font-size: 1.6rem;
    font-weight: 700;
  }
  .me {
    padding: 0;
    border: none;
    background: none;
    cursor: pointer;
    border-radius: 50%;
  }
  .menu {
    position: absolute;
    top: 60px;
    left: 16px;
    z-index: 10;
    min-width: 240px;
    padding: 8px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--surface-2);
    box-shadow: 0 12px 32px rgb(0 0 0 / 0.4);
  }
  .menu-who {
    display: flex;
    flex-direction: column;
    padding: 8px 10px 10px;
  }
  .menu-who span {
    font-size: 0.82rem;
    color: var(--muted);
  }
  .menu button {
    width: 100%;
    padding: 10px;
    border: none;
    border-radius: 10px;
    background: none;
    text-align: left;
    color: var(--danger);
    cursor: pointer;
  }
  .menu button:hover {
    background: var(--surface-hover);
  }
  .search {
    margin: 4px 16px 8px;
    padding: 10px 14px;
    border: none;
    border-radius: 999px;
    background: var(--surface-2);
    outline: none;
  }
  .search:focus {
    box-shadow: 0 0 0 2px var(--accent-a);
  }
  .chips {
    display: flex;
    gap: 6px;
    padding: 0 16px 8px;
    overflow-x: auto;
    scrollbar-width: none;
  }
  .chips button {
    flex: none;
    padding: 6px 14px;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: transparent;
    font-size: 0.85rem;
    cursor: pointer;
    transition: background 0.15s var(--ease), border-color 0.15s var(--ease);
  }
  .chips button.active {
    border-color: transparent;
    background: var(--accent);
    color: white;
  }
  .rows {
    flex: 1;
    overflow-y: auto;
    padding: 0 6px 12px;
  }
  .state {
    padding: 32px 16px;
    text-align: center;
    color: var(--muted);
  }
  .state.error {
    color: var(--danger);
  }
  .skeleton {
    height: 60px;
    margin: 8px 10px;
    border-radius: var(--radius);
    background: linear-gradient(90deg, var(--surface-2), var(--surface-hover), var(--surface-2));
    background-size: 200% 100%;
    animation: shimmer 1.2s infinite linear;
  }
  @keyframes shimmer {
    to {
      background-position: -200% 0;
    }
  }
  .chat-pane {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .chat-header {
    padding: 12px 16px;
    border-bottom: 1px solid var(--border);
  }
  .chat-header div {
    display: flex;
    flex-direction: column;
  }
  .sub {
    font-size: 0.8rem;
    color: var(--muted);
  }
  .back {
    display: none;
    border: none;
    background: none;
    font-size: 2rem;
    line-height: 1;
    cursor: pointer;
  }
  .placeholder {
    flex: 1;
    display: grid;
    place-items: center;
    color: var(--muted);
  }

  /* Phone layout: one pane at a time. */
  @media (max-width: 720px) {
    .shell {
      grid-template-columns: 1fr;
    }
    .chat-pane {
      display: none;
    }
    .shell.has-selection .list-pane {
      display: none;
    }
    .shell.has-selection .chat-pane {
      display: flex;
    }
    .back {
      display: block;
    }
  }
</style>
