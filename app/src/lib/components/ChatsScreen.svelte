<script lang="ts">
  import Settings from "@lucide/svelte/icons/settings";
  import MessageCircle from "@lucide/svelte/icons/message-circle";
  import LayoutGrid from "@lucide/svelte/icons/layout-grid";
  import { spaces } from "$lib/stores/spaces.svelte";
  import SpacesPane from "./SpacesPane.svelte";
  import { onMount } from "svelte";
  import { flip } from "svelte/animate";
  import { fade } from "svelte/transition";
  import {
    clearNotifications,
    getSettings,
    isAndroid,
    launchRoom,
    startSearchBackfill,
    onOpenRoom,
    setupPush,
    setArchived,
    setMarkedUnread,
    setMuted,
    setPinned,
    type Account,
    type RoomSummary,
  } from "$lib/backend";
  import { chat } from "$lib/stores/chat.svelte";
  import { drafts } from "$lib/stores/drafts.svelte";
  import { MessageSearch } from "$lib/stores/search.svelte";
  import SearchResults from "./SearchResults.svelte";
  import { presence } from "$lib/stores/presence.svelte";
  import { call } from "$lib/stores/call.svelte";
  import CallView from "./CallView.svelte";
  import IncomingCall from "./IncomingCall.svelte";
  import { chatList } from "$lib/stores/chatList.svelte";
  import { encryption } from "$lib/stores/encryption.svelte";
  import Avatar from "./Avatar.svelte";
  import ChatRow from "./ChatRow.svelte";
  import ChatView from "./ChatView.svelte";
  import { onBack } from "$lib/back.svelte";
  import { themeById } from "$lib/themes";
  import EncryptionSheet from "./EncryptionSheet.svelte";
  import SettingsPage from "./SettingsPage.svelte";

  let { account, onSignOut }: { account: Account; onSignOut: () => void } = $props();

  type Filter = "all" | "unread" | "people" | "groups" | "pinned" | "archived";
  const filters: { id: Filter; label: string }[] = [
    { id: "all", label: "All" },
    { id: "unread", label: "Unread" },
    { id: "people", label: "People" },
    { id: "groups", label: "Groups" },
    { id: "pinned", label: "Pinned" },
    { id: "archived", label: "Archived" },
  ];

  let filter = $state<Filter>("all");
  let query = $state("");
  /** Bottom bar: chats, or spaces and their chats. */
  let tab = $state<"chats" | "spaces">("chats");
  /** Messages matching the search box, under the matching chats. */
  const messageSearch = new MessageSearch();
  $effect(() => {
    messageSearch.query = query;
    messageSearch.changed();
  });

  function openHit(roomId: string, eventId: string) {
    chat.jump(roomId, eventId);
    selectedId = roomId;
  }
  let selectedId = $state<string | null>(null);
  let menuOpen = $state(false);
  let settingsOpen = $state(false);
  /** Right-click menu on a chat row. */
  let rowMenu = $state<{ room: RoomSummary; x: number; y: number } | null>(null);
  let actionError = $state<string | null>(null);
  /** The open chat's shared theme colours. */
  const theme = $derived(themeById(chat.style?.theme));

  // Android back: a full-screen call shrinks to its pill first.
  $effect(() => {
    if (call.active && !call.minimized) return onBack(() => (call.minimized = true));
  });
  // Then menus and sheets, then the open chat.
  $effect(() => {
    if (selected) return onBack(closeChat);
  });
  $effect(() => {
    if (settingsOpen) return onBack(() => (settingsOpen = false));
  });
  $effect(() => {
    if (encryption.sheetOpen) return onBack(() => (encryption.sheetOpen = false));
  });
  $effect(() => {
    if (menuOpen) return onBack(() => (menuOpen = false));
  });
  $effect(() => {
    if (rowMenu) return onBack(() => (rowMenu = null));
  });

  const myName = $derived(account.display_name ?? account.user_id.slice(1).split(":")[0]);
  const selected = $derived(chatList.rooms.find((r) => r.id === selectedId) ?? null);

  // Archived chats only appear under "Archived" (and in search).
  const matches: Record<Filter, (r: RoomSummary) => boolean> = {
    all: (r) => !r.is_archived,
    unread: (r) => !r.is_archived && (r.unread_messages > 0 || r.marked_unread || r.is_invite),
    people: (r) => !r.is_archived && r.is_dm,
    groups: (r) => !r.is_archived && !r.is_dm,
    pinned: (r) => r.is_favourite,
    archived: (r) => r.is_archived,
  };

  const visible = $derived.by(() => {
    const q = query.trim().toLocaleLowerCase();
    // Search covers every chat; otherwise space chats stay in their space unless shown here.
    const list = q
      ? chatList.rooms.filter((r) => r.name.toLocaleLowerCase().includes(q))
      : chatList.rooms.filter((r) => matches[filter](r) && spaces.inChats(r));
    // Pinned chats first; otherwise keep the core's order (most recent activity).
    return [...list.filter((r) => r.is_favourite), ...list.filter((r) => !r.is_favourite)];
  });

  const isUnread = (r: RoomSummary) => !r.is_muted && (r.unread_messages > 0 || r.marked_unread || r.is_invite);
  const chatsUnread = $derived(chatList.rooms.filter((r) => !r.is_archived && spaces.inChats(r) && isUnread(r)).length);
  const spacesUnread = $derived(
    chatList.rooms.filter((r) => spaces.spacesOf(r.id).length && !spaces.inChats(r) && isUnread(r)).length,
  );

  function openRowMenu(event: MouseEvent, room: RoomSummary) {
    event.preventDefault();
    rowMenu = { room, x: Math.min(event.clientX, innerWidth - 220), y: Math.min(event.clientY, innerHeight - 200) };
  }

  async function rowAction(
    action: (room: RoomSummary) => Promise<void>,
    change: (room: RoomSummary) => Partial<RoomSummary>,
  ) {
    if (!rowMenu) return;
    const room = rowMenu.room;
    rowMenu = null;
    actionError = null;
    try {
      await action(room);
      chatList.patch(room.id, change(room));
    } catch (e) {
      actionError = String(e);
    }
  }

  // Green dots on 1:1 chats: ask about the people of the first ones every couple of minutes.
  const dmUsers = $derived(
    chatList.rooms
      .filter((r) => r.is_dm && r.dm_user)
      .slice(0, 40)
      .map((r) => r.dm_user as string)
      .join(" "),
  );
  $effect(() => {
    const ids = dmUsers.split(" ").filter(Boolean);
    void presence.refresh(ids);
    const timer = setInterval(() => presence.refresh(ids), 2 * 60 * 1000);
    return () => clearInterval(timer);
  });

  onMount(() => {
    chatList.start();
    encryption.start().catch((e) => console.warn("encryption status unavailable", e));
    presence.start();
    call.listen();
    spaces.start();
    drafts.start();
    startSearchBackfill().catch(() => {});
    messageSearch.start();
    // Tapped notifications open their chat (cold start, or while running).
    launchRoom().then((id) => id && (selectedId = id), () => {});
    const stopOpenRoom = onOpenRoom((id) => (selectedId = id));
    // Re-register push each start: the token may have changed since.
    if (isAndroid)
      getSettings()
        .then((s) => setupPush(s.notifications))
        .catch((e) => console.warn("push setup failed", e));
    return () => {
      void stopOpenRoom.then((stop) => stop());
      chatList.reset();
      chat.close();
      encryption.reset();
    };
  });

  // Opening a chat clears its notification.
  $effect(() => {
    if (selectedId && isAndroid) clearNotifications(selectedId).catch(() => {});
  });

  function closeChat() {
    selectedId = null;
    chat.close();
  }
</script>

<div class="shell" class:has-selection={selected}>
  <aside class="list-pane">
    <header>
      <button class="me" onclick={() => (menuOpen = !menuOpen)} aria-label="Account">
        <Avatar name={myName} size={36} />
      </button>
      <h1>{tab === "chats" ? "Chats" : "Spaces"}</h1>
      {#if menuOpen}
        <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
        <div class="outside" onclick={() => (menuOpen = false)}></div>
        
        <div class="menu" transition:fade={{ duration: 120 }}>
          <div class="menu-who">
            <strong>{myName}</strong>
            <span>{account.user_id}</span>
          </div>
          <button class="plain" onclick={() => ((settingsOpen = true), (menuOpen = false))}><Settings size={16} /> Settings</button>
          <button onclick={onSignOut}>Sign out</button>
        </div>
      {/if}
    </header>

    {#if encryption.locked}
      <button class="unlock-banner" onclick={() => (encryption.sheetOpen = true)} transition:fade>
        <span>🔒</span>
        <span><strong>Unlock your messages</strong><small>Verify this device to read encrypted history</small></span>
      </button>
    {/if}

    {#if tab === "spaces"}
      <div class="rows">
        <SpacesPane {selectedId} onSelect={(id) => (selectedId = id)} />
      </div>
    {:else}
    <input class="search" type="search" placeholder="Search" bind:value={query} />

    <nav class="chips">
      {#each filters as f (f.id)}
        <button class:active={filter === f.id} onclick={() => (filter = f.id)}>{f.label}</button>
      {/each}
    </nav>

    {#if actionError}<p class="state error small">{actionError}</p>{/if}

    <div class="rows">
      {#if chatList.error}
        <p class="state error">{chatList.error}</p>
      {:else if !chatList.loaded}
        {#each Array(8) as _, i (i)}
          <div class="skeleton" style:animation-delay="{i * 60}ms"></div>
        {/each}
      {:else if visible.length === 0 && !messageSearch.active}
        <p class="state">{query ? "No chats match your search" : "Nothing here yet"}</p>
      {:else}
        {#each visible as room (room.id)}
          <div animate:flip={{ duration: 220 }}>
            <ChatRow
              {room}
              selected={room.id === selectedId}
              onclick={() => (selectedId = room.id)}
              oncontextmenu={(e) => openRowMenu(e, room)}
            />
          </div>
        {/each}
        {#if messageSearch.active}
          <h4 class="results-title">
            Messages
            {#if messageSearch.status?.indexing}<small>· reading older history ({messageSearch.status.chats_done}/{messageSearch.status.chats} chats)</small>{/if}
          </h4>
          {#if messageSearch.hits.length}
            <SearchResults hits={messageSearch.hits} onPick={(h) => openHit(h.room_id, h.event_id)} onMore={() => messageSearch.more()} />
          {:else if !messageSearch.loading}
            <p class="state small">No messages found</p>
          {/if}
        {/if}
      {/if}
    </div>
    {/if}

    <nav class="tabbar">
      <button class:active={tab === "chats"} onclick={() => (tab = "chats")}>
        <span class="tab-icon"><MessageCircle size={22} />{#if chatsUnread}<span class="dot">{chatsUnread}</span>{/if}</span>
        Chats
      </button>
      <button class:active={tab === "spaces"} onclick={() => (tab = "spaces")}>
        <span class="tab-icon"><LayoutGrid size={22} />{#if spacesUnread}<span class="dot">{spacesUnread}</span>{/if}</span>
        Spaces
      </button>
    </nav>
  </aside>

  <section class="chat-pane" style:--accent-a={theme.a} style:--accent-b={theme.b}>
    {#if selected}
      <ChatView room={selected} onBack={closeChat} />
    {:else}
      <div class="placeholder">Select a chat to start messaging</div>
    {/if}
  </section>
</div>

{#if encryption.sheetOpen}
  <EncryptionSheet />
{/if}

{#if settingsOpen}
  <SettingsPage onClose={() => (settingsOpen = false)} />
{/if}

{#if rowMenu}
  {@const r = rowMenu.room}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="menu-catcher" onclick={() => (rowMenu = null)} oncontextmenu={(e) => (e.preventDefault(), (rowMenu = null))}></div>
  <div class="row-menu" style:left="{rowMenu.x}px" style:top="{rowMenu.y}px" transition:fade={{ duration: 100 }}>
    <button onclick={() => rowAction((c) => setPinned(c.id, !c.is_favourite), (c) => ({ is_favourite: !c.is_favourite, is_archived: false }))}>
      📌 {r.is_favourite ? "Unpin" : "Pin"}
    </button>
    <button onclick={() => rowAction((c) => setMuted(c.id, !c.is_muted), (c) => ({ is_muted: !c.is_muted }))}>
      {r.is_muted ? "🔔 Unmute" : "🔕 Mute"}
    </button>
    <button
      onclick={() =>
        rowAction(
          (c) => setMarkedUnread(c.id, !(c.marked_unread || c.unread_messages > 0)),
          (c) => ({ marked_unread: !(c.marked_unread || c.unread_messages > 0) }),
        )}
    >
      {r.marked_unread || r.unread_messages > 0 ? "✓ Mark as read" : "● Mark as unread"}
    </button>
    <button onclick={() => rowAction((c) => setArchived(c.id, !c.is_archived), (c) => ({ is_archived: !c.is_archived, is_favourite: false }))}>
      🗄️ {r.is_archived ? "Unarchive" : "Archive"}
    </button>
  </div>
{/if}

<CallView />
<IncomingCall />

<style>
  .shell {
    display: grid;
    /* minmax(0, …): a bare 1fr won't shrink below its widest row (long previews). */
    grid-template-columns: minmax(300px, 380px) minmax(0, 1fr);
    height: 100vh;
    height: 100dvh;
  }
  .list-pane {
    min-width: 0;
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
  .results-title {
    margin: 14px 12px 4px;
    color: var(--muted);
    font-size: 13px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .results-title small {
    font-weight: 400;
    text-transform: none;
    letter-spacing: 0;
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
  .chips::-webkit-scrollbar {
    display: none; /* Android WebView ignores scrollbar-width */
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
  .tabbar {
    display: grid;
    grid-template-columns: 1fr 1fr;
    border-top: 1px solid var(--border);
    background: inherit;
  }
  .tabbar button {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
    padding: 8px 0 6px;
    border: none;
    background: none;
    color: var(--muted);
    font-size: 12px;
    cursor: pointer;
  }
  .tabbar button.active {
    color: var(--text);
    font-weight: 600;
  }
  .tabbar button.active .tab-icon {
    background: color-mix(in srgb, var(--accent-a) 22%, transparent);
    color: var(--accent-a);
  }
  .tab-icon {
    position: relative;
    display: grid;
    width: 56px;
    height: 30px;
    place-items: center;
    border-radius: 999px;
    transition: background 0.2s var(--ease);
  }
  .dot {
    position: absolute;
    top: -2px;
    right: 6px;
    min-width: 18px;
    padding: 1px 5px;
    border-radius: 999px;
    background: var(--accent);
    color: white;
    font-size: 11px;
    font-weight: 600;
    line-height: 16px;
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
    /* Re-derived here: --accent on :root was computed from the default colours. */
    --accent: linear-gradient(135deg, var(--accent-a), var(--accent-b));
    position: relative;
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }
  .unlock-banner {
    display: flex;
    align-items: center;
    gap: 12px;
    margin: 4px 16px 10px;
    padding: 10px 14px;
    border: 1px solid color-mix(in srgb, var(--accent-a) 45%, transparent);
    border-radius: var(--radius);
    background: color-mix(in srgb, var(--accent-a) 14%, var(--surface));
    text-align: left;
    cursor: pointer;
  }
  .unlock-banner span:last-child {
    display: flex;
    flex-direction: column;
  }
  .unlock-banner small {
    color: var(--muted);
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
      grid-template-columns: minmax(0, 1fr);
    }
    .chat-pane {
      display: none;
    }
    .shell.has-selection .list-pane {
      display: none;
    }
    /* Full screen: same colour as the system bars around it (MainActivity paints them --bg). */
    .list-pane {
      border-right: none;
      background: var(--bg);
    }
    .shell.has-selection .chat-pane {
      display: flex;
    }
  }
  .menu button.plain {
    display: flex;
    align-items: center;
    gap: 8px;
    color: inherit;
  }
  .state.small {
    padding: 4px 16px;
    font-size: 0.85rem;
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
    padding: 9px 10px;
    border: none;
    border-radius: 8px;
    background: none;
    text-align: left;
    cursor: pointer;
  }
  .row-menu button:hover {
    background: var(--surface-hover);
  }
  /* Tap anywhere else to close the open menu. */
  .outside {
    position: fixed;
    inset: 0;
    z-index: 5;
  }
</style>
