<script lang="ts">
  import Settings from "@lucide/svelte/icons/settings";
  import MessageCircle from "@lucide/svelte/icons/message-circle";
  import LayoutGrid from "@lucide/svelte/icons/layout-grid";
  import { inFolder, isUnread, spaces } from "$lib/stores/spaces.svelte";
  import FolderPage from "./FolderPage.svelte";
  import FolderPlus from "@lucide/svelte/icons/folder-plus";
  import FolderMinus from "@lucide/svelte/icons/folder-minus";
  import Pin from "@lucide/svelte/icons/pin";
  import PinOff from "@lucide/svelte/icons/pin-off";
  import Bell from "@lucide/svelte/icons/bell";
  import BellOff from "@lucide/svelte/icons/bell-off";
  import MailOpen from "@lucide/svelte/icons/mail-open";
  import Mail from "@lucide/svelte/icons/mail";
  import Archive from "@lucide/svelte/icons/archive";
  import ArchiveRestore from "@lucide/svelte/icons/archive-restore";
  import Plus from "@lucide/svelte/icons/plus";
  import SpacesPane from "./SpacesPane.svelte";
  import { onMount } from "svelte";
  import { flip } from "svelte/animate";
  import { fade, fly } from "svelte/transition";
  import {
    clearNotifications,
    getSettings,
    isAndroid,
    launchRoom,
    startSearchBackfill,
    onOpenRoom,
    setupPush,
    systemStatus,
    setArchived,
    setMarkedUnread,
    setMuted,
    setPinned,
    type Account,
    type Folder,
    type RoomSummary,
  } from "$lib/backend";
  import { chat } from "$lib/stores/chat.svelte";
  import { drafts } from "$lib/stores/drafts.svelte";
  import { MessageSearch } from "$lib/stores/search.svelte";
  import SearchResults from "./SearchResults.svelte";
  import { presence } from "$lib/stores/presence.svelte";
  import { profiles } from "$lib/stores/profiles.svelte";
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
  import ChatScene from "./ChatScene.svelte";
  import { mediaUrl } from "$lib/media";
  import EncryptionSheet from "./EncryptionSheet.svelte";
  import SettingsPage from "./SettingsPage.svelte";
  import SquarePen from "@lucide/svelte/icons/square-pen";
  import InvitePane from "./InvitePane.svelte";
  import NewChatPage from "./NewChatPage.svelte";
  import { me } from "$lib/stores/me.svelte";
  import { nav } from "$lib/stores/nav.svelte";
  import { share } from "$lib/stores/share.svelte";
  import { connection } from "$lib/stores/connection.svelte";
  import { liveLocation } from "$lib/stores/liveLocation.svelte";
  import WifiOff from "@lucide/svelte/icons/wifi-off";
  import LoaderCircle from "@lucide/svelte/icons/loader-circle";
  import { pickedMediaUrl } from "$lib/media";
  import ForwardPage from "./ForwardPage.svelte";
  import FileIcon from "@lucide/svelte/icons/file";

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

  /** A built-in filter, or `folder:<id>`. */
  let filter = $state<string>("all");
  const activeFolder = $derived(
    filter.startsWith("folder:") ? (spaces.prefs.folders.find((f) => `folder:${f.id}` === filter) ?? null) : null,
  );
  // A folder deleted (here or on another device) falls back to All.
  $effect(() => {
    if (filter.startsWith("folder:") && spaces.loaded && !activeFolder) filter = "all";
  });
  /** The folder editor: a folder, or "new". */
  let folderEdit = $state<Folder | "new" | null>(null);
  $effect(() => {
    if (folderEdit) return onBack(() => (folderEdit = null));
  });
  /** Long-press on a folder chip edits it (touch); right-click does on desktop. */
  let chipTimer: ReturnType<typeof setTimeout> | undefined;
  let chipLongPressed = false;
  function chipDown(f: Folder) {
    chipLongPressed = false;
    chipTimer = setTimeout(() => ((chipLongPressed = true), (folderEdit = f)), 500);
  }
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
  let settingsOpen = $state(false);
  let newChatOpen = $state(false);
  $effect(() => {
    if (newChatOpen) return onBack(() => (newChatOpen = false));
  });
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
    if (rowMenu) return onBack(() => (rowMenu = null));
  });

  const myName = $derived(me.profile?.name ?? account.display_name ?? account.user_id.slice(1).split(":")[0]);
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
      : activeFolder
        ? chatList.rooms.filter((r) => inFolder(activeFolder, r))
        : chatList.rooms.filter((r) => matches[filter as Filter](r) && spaces.inChats(r));
    // Pinned chats first; otherwise keep the core's order (most recent activity).
    return [...list.filter((r) => r.is_favourite), ...list.filter((r) => !r.is_favourite)];
  });

  const folderUnread = (f: Folder) => chatList.rooms.filter((r) => inFolder(f, r) && isUnread(r)).length;
  const chatsUnread = $derived(chatList.rooms.filter((r) => !r.is_archived && spaces.inChats(r) && isUnread(r)).length);
  const spacesUnread = $derived(
    chatList.rooms.filter((r) => spaces.spacesOf(r.id).length && !spaces.inChats(r) && isUnread(r)).length,
  );

  function folderToggle(folderId: string, roomId: string, inside: boolean) {
    rowMenu = null;
    spaces.setInFolder(folderId, roomId, inside).catch((e) => (actionError = String(e)));
  }

  function openRowMenu(event: MouseEvent, room: RoomSummary) {
    event.preventDefault();
    rowMenu = { room, x: Math.min(event.clientX, innerWidth - 220), y: Math.min(event.clientY, innerHeight - 200 - 40 * spaces.prefs.folders.length) };
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
    void profiles.refresh(ids);
    const timer = setInterval(() => (presence.refresh(ids), profiles.refresh(ids)), 2 * 60 * 1000);
    return () => clearInterval(timer);
  });

  onMount(() => {
    chatList.start();
    void me.refresh();
    share.start();
    connection.start();
    liveLocation.start();
    nav.attach(
      (id) => ((selectedId = id), (newChatOpen = false), (settingsOpen = false)),
      () => ((tab = "spaces"), (selectedId = null), (newChatOpen = false)),
    );
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
    // Re-register push each start: the token may have changed since. A new install asks
    // once for permission to show notifications (Android 13+), right after signing in;
    // after that, only the switch in Settings asks again.
    if (isAndroid)
      getSettings()
        .then(async (s) => {
          if (s.notifications && !localStorage.getItem("alwychat.askedNotifications")) {
            localStorage.setItem("alwychat.askedNotifications", "1");
            const status = await systemStatus().catch(() => null);
            if (status && !status.notifications) await systemStatus("notifications").catch(() => {});
          }
          await setupPush(s.notifications);
        })
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
      <button class="me" onclick={() => (settingsOpen = true)} aria-label="Settings" title="Settings">
        <Avatar name={myName} mxc={me.profile?.avatar ?? null} size={36} />
      </button>
      <h1>{tab === "chats" ? "Chats" : "Spaces"}</h1>
      <button class="new-chat" onclick={() => (newChatOpen = true)} aria-label="New chat" title="New chat"><SquarePen size={21} /></button>
    </header>

    {#if connection.shown}
      <div class="connection" class:offline={connection.shown !== "connecting"} transition:fly={{ y: -8, duration: 160 }}>
        {#if connection.shown === "connecting"}<LoaderCircle size={15} class="spin" /> Connecting…
        {:else if connection.shown === "offline"}<WifiOff size={15} /> Waiting for network…
        {:else}<WifiOff size={15} /> Can't reach the server, retrying…{/if}
      </div>
    {/if}
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
      {#each spaces.prefs.folders as f (f.id)}
        {@const unread = folderUnread(f)}
        <button
          class:active={filter === `folder:${f.id}`}
          onclick={() => !chipLongPressed && (filter = `folder:${f.id}`)}
          oncontextmenu={(e) => (e.preventDefault(), clearTimeout(chipTimer), (folderEdit = f))}
          onpointerdown={() => chipDown(f)}
          onpointerup={() => clearTimeout(chipTimer)}
          onpointerleave={() => clearTimeout(chipTimer)}
          title="Right-click or long-press to edit"
        >
          {f.name}{#if unread}<span class="chip-count">{unread}</span>{/if}
        </button>
      {/each}
      <button class="add-folder" onclick={() => (folderEdit = "new")} aria-label="New folder" title="New folder">
        <Plus size={16} />
      </button>
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

  <section
    class="chat-pane"
    class:scenic={!!selected && !selected.is_invite && (!!theme.scene || !!chat.style?.wallpaper)}
    style:--accent-a={theme.a}
    style:--accent-b={theme.b}
    style:--incoming={theme.incoming ?? (chat.style?.wallpaper ? "rgb(20 22 30 / 0.82)" : null)}
  >
    {#if selected && !selected.is_invite && chat.style?.wallpaper}
      <div class="wallpaper" style:background-image="url({mediaUrl(chat.style.wallpaper, 1200)})"></div>
    {:else if selected && !selected.is_invite && theme.scene}
      {#key theme.scene}<div class="scene-wrap" in:fade={{ duration: 300 }}><ChatScene scene={theme.scene} /></div>{/key}
    {/if}
    {#if selected?.is_invite}
      <InvitePane
        room={selected}
        onBack={closeChat}
        onAccepted={() => {
          if (selected?.is_space) nav.openSpace(selected.id);
        }}
        onDeclined={closeChat}
      />
    {:else if selected}
      <ChatView room={selected} onBack={closeChat} />
    {:else}
      <div class="placeholder">Select a chat to start messaging</div>
    {/if}
  </section>
</div>

{#if encryption.sheetOpen}
  <EncryptionSheet />
{/if}

{#if share.incoming}
  {@const s = share.incoming}
  <ForwardPage
    title="Share to…"
    onClose={() => share.cancel()}
    onPick={(r) => {
      share.deliver(r.id);
      nav.openChat(r.id);
    }}
  >
    <div class="share-preview">
      {#each s.files.slice(0, 4) as f (f.path)}
        {#if f.mime.startsWith("image/")}<img src={pickedMediaUrl(f.path)} alt="" />{:else}<span class="share-file"><FileIcon size={16} />{f.name}</span>{/if}
      {/each}
      {#if s.files.length > 4}<span class="share-file">+{s.files.length - 4}</span>{/if}
      {#if s.text}<p>{s.text}</p>{/if}
    </div>
  </ForwardPage>
{/if}

{#if newChatOpen}
  <NewChatPage
    onClose={() => (newChatOpen = false)}
    onOpen={(id, isSpace) => (isSpace ? nav.openSpace(id) : nav.openChat(id))}
  />
{/if}

{#if settingsOpen}
  <SettingsPage userId={account.user_id} onClose={() => (settingsOpen = false)} {onSignOut} />
{/if}

{#if rowMenu}
  {@const r = rowMenu.room}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="menu-catcher" onclick={() => (rowMenu = null)} oncontextmenu={(e) => (e.preventDefault(), (rowMenu = null))}></div>
  <div class="row-menu" style:left="{rowMenu.x}px" style:top="{rowMenu.y}px" transition:fade={{ duration: 100 }}>
    <button onclick={() => rowAction((c) => setPinned(c.id, !c.is_favourite), (c) => ({ is_favourite: !c.is_favourite, is_archived: false }))}>
      {#if r.is_favourite}<PinOff size={16} />Unpin{:else}<Pin size={16} />Pin{/if}
    </button>
    <button onclick={() => rowAction((c) => setMuted(c.id, !c.is_muted), (c) => ({ is_muted: !c.is_muted }))}>
      {#if r.is_muted}<Bell size={16} />Unmute{:else}<BellOff size={16} />Mute{/if}
    </button>
    <button
      onclick={() =>
        rowAction(
          (c) => setMarkedUnread(c.id, !(c.marked_unread || c.unread_messages > 0)),
          (c) => ({ marked_unread: !(c.marked_unread || c.unread_messages > 0) }),
        )}
    >
      {#if r.marked_unread || r.unread_messages > 0}<MailOpen size={16} />Mark as read{:else}<Mail size={16} />Mark as unread{/if}
    </button>
    <button onclick={() => rowAction((c) => setArchived(c.id, !c.is_archived), (c) => ({ is_archived: !c.is_archived, is_favourite: false }))}>
      {#if r.is_archived}<ArchiveRestore size={16} />Unarchive{:else}<Archive size={16} />Archive{/if}
    </button>
    {#each spaces.prefs.folders as f (f.id)}
      {@const inside = inFolder({ ...f, unread_only: false, hide_muted: false }, r)}
      <button onclick={() => folderToggle(f.id, r.id, !inside)}>
        {#if inside}<FolderMinus size={16} />Remove from {f.name}{:else}<FolderPlus size={16} />Add to {f.name}{/if}
      </button>
    {/each}
  </div>
{/if}

{#if folderEdit}
  <FolderPage folder={folderEdit === "new" ? null : folderEdit} onClose={() => (folderEdit = null)} />
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
    flex: 1;
    margin: 0;
    font-size: 1.6rem;
    font-weight: 700;
  }
  .share-preview {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-bottom: 12px;
  }
  .share-preview img {
    width: 64px;
    height: 64px;
    border-radius: 10px;
    object-fit: cover;
  }
  .share-file {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 6px 10px;
    border-radius: 10px;
    background: var(--surface);
    font-size: 0.9rem;
  }
  .share-preview p {
    width: 100%;
    margin: 0;
    padding: 10px 12px;
    border-radius: 12px;
    background: var(--surface);
    overflow-wrap: anywhere;
    white-space: pre-wrap;
  }
  .connection {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    margin: 0 12px 6px;
    padding: 6px 12px;
    border-radius: 999px;
    background: var(--surface);
    color: var(--muted);
    font-size: 0.85rem;
  }
  .connection.offline {
    color: #ffb020;
  }
  .connection :global(.spin) {
    animation: spin 1s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  /* A theme with scenery or a photo: the chat sits on it, always dark. */
  .scene-wrap,
  .wallpaper {
    position: absolute;
    inset: 0;
    z-index: 0;
  }
  .wallpaper {
    background-position: center;
    background-size: cover;
  }
  .wallpaper::after {
    content: "";
    position: absolute;
    inset: 0;
    background: linear-gradient(rgb(0 0 0 / 0.25), rgb(0 0 0 / 0.55));
  }
  .chat-pane.scenic {
    --text: #f2f4f8;
    --muted: rgb(235 240 255 / 0.72);
    --surface-2: rgb(10 14 28 / 0.55);
    --border: rgb(255 255 255 / 0.1);
    color: var(--text);
    color-scheme: dark;
  }
  .chat-pane.scenic > :global(header),
  .chat-pane.scenic > :global(.scroller),
  .chat-pane.scenic > :global(form),
  .chat-pane.scenic > :global(.pin-banner),
  .chat-pane.scenic > :global(.live-bar),
  .chat-pane.scenic > :global(.call-banner),
  .chat-pane.scenic > :global(.typing) {
    position: relative;
    z-index: 1;
  }
  .chat-pane.scenic > :global(header) {
    border-bottom-color: transparent;
    background: rgb(0 0 0 / 0.18);
    backdrop-filter: blur(12px);
  }
  .chat-pane.scenic > :global(form) {
    border-top-color: transparent;
  }
  .new-chat {
    display: grid;
    width: 40px;
    height: 40px;
    place-items: center;
    border: none;
    border-radius: 50%;
    background: var(--surface);
    color: var(--text);
    cursor: pointer;
  }
  .new-chat:hover {
    background: var(--surface-hover);
  }
  .me {
    padding: 0;
    border: none;
    background: none;
    cursor: pointer;
    border-radius: 50%;
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
  .chip-count {
    margin-left: 6px;
    padding: 0 6px;
    border-radius: 999px;
    background: var(--surface-2);
    font-size: 0.75rem;
  }
  .chips button.active .chip-count {
    background: rgb(255 255 255 / 0.25);
  }
  .chips .add-folder {
    display: grid;
    place-items: center;
    padding: 6px 9px;
    color: var(--muted);
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
    /* Your bubbles: one gradient over the whole screen (Messenger), top to bottom. */
    --bubble: linear-gradient(to bottom, var(--accent-a), var(--accent-b));
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
    display: flex;
    align-items: center;
    gap: 10px;
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
</style>
