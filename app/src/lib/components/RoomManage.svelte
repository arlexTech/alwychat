<script lang="ts">
  import Bell from "@lucide/svelte/icons/bell";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import Flag from "@lucide/svelte/icons/flag";
  import Link from "@lucide/svelte/icons/link";
  import Lock from "@lucide/svelte/icons/lock";
  import LogOut from "@lucide/svelte/icons/log-out";
  import Pencil from "@lucide/svelte/icons/pencil";
  import ShieldCheck from "@lucide/svelte/icons/shield-check";
  import UserPlus from "@lucide/svelte/icons/user-plus";
  import Users from "@lucide/svelte/icons/users";
  import { onBack } from "$lib/back.svelte";
  import {
    enableRoomEncryption,
    leaveRoom,
    reportRoom,
    setHistoryVisibility,
    setJoinRule,
    setNotifyMode,
    setRoomAvatar,
    setRoomName,
    setRoomTopic,
    spaceRoomsJoined,
    type RoomDetails,
  } from "$lib/backend";
  import { chatList } from "$lib/stores/chatList.svelte";
  import { nav } from "$lib/stores/nav.svelte";
  import { spaces } from "$lib/stores/spaces.svelte";
  import { can } from "$lib/roles";
  import AvatarEdit from "./AvatarEdit.svelte";
  import MembersPage from "./MembersPage.svelte";
  import RolesPage from "./RolesPage.svelte";

  /**
   * The settings every chat and space has: photo, name, topic (header), then members, invites,
   * notifications, who can join / read history, encryption, roles, link, leave, report.
   * `part`: the header, or the rest (so a page can put its own sections in between).
   */
  let {
    room,
    part,
    reload,
    onLeft,
  }: {
    room: RoomDetails;
    /** "body": everything below the header; or one group of it. */
    part: "header" | "body" | "people" | "notify" | "privacy" | "leave";
    reload: () => void;
    onLeft?: () => void;
  } = $props();

  const may = (key: string) => can(room.levels, room.my_power, key);
  let editing = $state<{ name: string; topic: string } | null>(null);
  let page = $state<"members" | "invite" | "roles" | null>(null);
  let confirm = $state<"leave" | "encrypt" | "report" | null>(null);
  let leaveAlso = $state<{ id: string; name: string; on: boolean }[]>([]);
  let reason = $state("");
  let busy = $state(false);
  let error = $state<string | null>(null);
  let notice = $state<string | null>(null);

  $effect(() => {
    if (page) return onBack(() => (page = null));
  });

  async function act(what: () => Promise<unknown>, done?: string) {
    busy = true;
    error = null;
    notice = null;
    try {
      await what();
      if (done) notice = done;
      reload();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function saveText() {
    const e = editing!;
    await act(async () => {
      if (e.name.trim() && e.name.trim() !== room.name) await setRoomName(room.id, e.name);
      if (e.topic.trim() !== (room.topic ?? "")) await setRoomTopic(room.id, e.topic);
    });
    if (!error) editing = null;
  }

  // Spaces this chat could be open to: its parents we know of, or the one it already allows.
  const accessSpaces = $derived(room.allow_spaces.length ? room.allow_spaces : room.parent_spaces);
  const spaceName = (id: string) => spaces.list.find((s) => s.id === id)?.name ?? "the space";

  async function startLeave() {
    leaveAlso = [];
    confirm = "leave";
    if (room.is_space) {
      const ids = await spaceRoomsJoined(room.id).catch(() => []);
      leaveAlso = ids.map((id) => ({ id, name: chatList.rooms.find((r) => r.id === id)?.name ?? id, on: false }));
    }
  }

  async function leave() {
    await act(async () => {
      for (const r of leaveAlso.filter((r) => r.on)) await leaveRoom(r.id);
      await leaveRoom(room.id);
    });
    if (!error) {
      confirm = null;
      onLeft?.();
    }
  }

  const link = $derived(
    `https://matrix.to/#/${encodeURIComponent(room.alias ?? room.id)}${room.alias ? "" : `?via=${room.id.split(":")[1]}`}`,
  );
  const historyLabels: Record<string, string> = {
    shared: "Members (all of it)",
    invited: "Members, since they were invited",
    joined: "Members, since they joined",
    world_readable: "Anyone",
  };
  const kind = $derived(room.is_space ? "space" : room.is_dm ? "chat" : "group");
</script>

{#if part === "header"}
  <div class="header">
    <AvatarEdit
      name={room.name}
      mxc={room.avatar}
      square={room.is_space}
      disabled={room.is_dm || !may("m.room.avatar")}
      onPicked={(data) => act(() => setRoomAvatar(room.id, data))}
    />
    {#if editing}
      <input class="m-input" bind:value={editing.name} placeholder="Name" maxlength="80" disabled={!may("m.room.name")} />
      <textarea class="m-input" bind:value={editing.topic} placeholder="Topic" maxlength="500" disabled={!may("m.room.topic")}></textarea>
      <div class="m-buttons">
        <button class="m-btn plain" onclick={() => (editing = null)}>Cancel</button>
        <button class="m-btn" disabled={busy} onclick={saveText}>Save</button>
      </div>
    {:else}
      <h2>{room.name}</h2>
      {#if room.topic}<p class="topic">{room.topic}</p>{/if}
      <p class="meta">
        {#if room.encrypted}<Lock size={13} /> Encrypted ·{/if}
        {room.members} {room.members === 1 ? "member" : "members"}
      </p>
      {#if !room.is_dm && (may("m.room.name") || may("m.room.topic"))}
        <button class="edit" onclick={() => (editing = { name: room.name, topic: room.topic ?? "" })}><Pencil size={14} /> Edit name and topic</button>
      {/if}
    {/if}
  </div>
{:else}
  {@const all = part === "body"}
  {#if (all || part === "people") && !room.is_dm}
    {#if all}<h3 class="m-section">People</h3>{/if}
    <button class="m-row" onclick={() => (page = "members")}>
      <span class="m-icon"><Users size={20} /></span><span class="m-text"><strong>Members</strong></span>
      <span class="m-end">{room.members}</span><ChevronRight size={18} />
    </button>
    {#if may("invite")}
      <button class="m-row" onclick={() => (page = "invite")}>
        <span class="m-icon"><UserPlus size={20} /></span><span class="m-text"><strong>Invite people</strong></span>
      </button>
    {/if}
    {#if room.join_rule === "public" || room.alias}
      <button class="m-row" onclick={() => navigator.clipboard?.writeText(link).then(() => (notice = "Link copied"))}>
        <span class="m-icon"><Link size={20} /></span><span class="m-text"><strong>Copy link</strong><small>{link}</small></span>
      </button>
    {/if}
  {/if}

  {#if (all || part === "notify") && !room.is_space}
    {#if all}<h3 class="m-section">Notifications</h3>{/if}
    <div class="m-seg">
      <button class:on={!room.notify_custom} onclick={() => act(() => setNotifyMode(room.id, "default"))}>Default</button>
      <button class:on={room.notify_custom && room.notify === "all"} onclick={() => act(() => setNotifyMode(room.id, "all"))}>All</button>
      <button class:on={room.notify_custom && room.notify === "mentions"} onclick={() => act(() => setNotifyMode(room.id, "mentions"))}>Mentions</button>
      <button class:on={room.notify_custom && room.notify === "mute"} onclick={() => act(() => setNotifyMode(room.id, "mute"))}>Off</button>
    </div>
    <p class="m-hint"><Bell size={12} />
      {room.notify === "all" ? "Every message notifies you." : room.notify === "mentions" ? "Only mentions and your keywords notify you." : "No notifications or unread badge."}
      {#if !room.notify_custom}(Your default for {room.is_dm ? "1:1 chats" : "groups"}.){/if}
    </p>
  {/if}

  {#if (all || part === "privacy") && !room.is_dm}
    <h3 class="m-section">Privacy</h3>
    <div class="m-row">
      <span class="m-text"><span>Who can join</span></span>
      <select
        class="m-select"
        disabled={busy || !may("m.room.join_rules")}
        value={room.join_rule}
        onchange={(e) => act(() => setJoinRule(room.id, e.currentTarget.value, accessSpaces))}
      >
        <option value="invite">Invited only</option>
        {#if accessSpaces.length}<option value="restricted">Members of {spaceName(accessSpaces[0])}</option>{/if}
        <option value="knock">Ask to join</option>
        <option value="public">Anyone with the link</option>
        {#if !["invite", "restricted", "knock", "public"].includes(room.join_rule)}<option value={room.join_rule}>{room.join_rule}</option>{/if}
      </select>
    </div>
    {#if !room.is_space}
      <div class="m-row">
        <span class="m-text"><span>Who can read the history</span></span>
        <select
          class="m-select"
          disabled={busy || !may("m.room.history_visibility")}
          value={room.history}
          onchange={(e) => act(() => setHistoryVisibility(room.id, e.currentTarget.value))}
        >
          {#each Object.entries(historyLabels) as [value, label] (value)}<option {value}>{label}</option>{/each}
        </select>
      </div>
    {/if}
  {/if}

  {#if (all || part === "privacy") && !room.is_space}
    <h3 class="m-section">Security</h3>
    {#if room.encrypted}
      <div class="m-row">
        <span class="m-icon"><ShieldCheck size={20} /></span>
        <span class="m-text"><strong>End-to-end encrypted</strong><small>Messages can only be read by the people in this {kind}.</small></span>
      </div>
    {:else}
      <button class="m-row" disabled={!may("m.room.encryption")} onclick={() => (confirm = "encrypt")}>
        <span class="m-icon"><Lock size={20} /></span>
        <span class="m-text"><strong>Turn on encryption</strong><small>Not encrypted. Once on, it can't be turned off.</small></span>
      </button>
    {/if}
  {/if}

  {#if (all || part === "privacy") && !room.is_dm}
    <button class="m-row" onclick={() => (page = "roles")}>
      <span class="m-icon"><ShieldCheck size={20} /></span>
      <span class="m-text"><strong>Roles & permissions</strong><small>{may("m.room.power_levels") ? "Who can invite, remove, change settings…" : "See who can do what"}</small></span>
      <ChevronRight size={18} />
    </button>
  {/if}

  {#if all || part === "leave"}
  {#if all}<h3 class="m-section">Leave</h3>{/if}
  <button class="m-row danger" onclick={startLeave}>
    <span class="m-icon"><LogOut size={20} /></span><span class="m-text"><strong>Leave {kind}</strong></span>
  </button>
  <button class="m-row danger" onclick={() => (confirm = "report")}>
    <span class="m-icon"><Flag size={20} /></span><span class="m-text"><strong>Report {kind}</strong><small>To your server's admins</small></span>
  </button>
  {/if}

  {#if confirm}
    <div class="confirm">
      {#if confirm === "leave"}
        <p><strong>Leave {room.name}?</strong></p>
        <p class="m-hint">
          {room.join_rule === "public" ? "You can join again later." : "You'll need a new invite to come back."}
        </p>
        {#if leaveAlso.length}
          <p class="m-hint">Also leave its chats:</p>
          {#each leaveAlso as r (r.id)}
            <label class="m-row"><span class="m-text"><span>{r.name}</span></span><input type="checkbox" bind:checked={r.on} /></label>
          {/each}
        {/if}
      {:else if confirm === "encrypt"}
        <p><strong>Turn on encryption?</strong></p>
        <p class="m-hint">It can't be turned off. Bots, bridges and people who join later may not be able to read older messages.</p>
      {:else}
        <p><strong>Report {room.name}?</strong></p>
        <input class="m-input" bind:value={reason} placeholder="What's wrong?" />
      {/if}
      <div class="m-buttons">
        <button class="m-btn plain" onclick={() => (confirm = null)}>Cancel</button>
        {#if confirm === "leave"}
          <button class="m-btn danger" disabled={busy} onclick={leave}>Leave</button>
        {:else if confirm === "encrypt"}
          <button class="m-btn" disabled={busy} onclick={() => act(() => enableRoomEncryption(room.id), "Encryption is on").then(() => (confirm = null))}>Turn on</button>
        {:else}
          <button class="m-btn danger" disabled={busy || !reason.trim()} onclick={() => act(() => reportRoom(room.id, reason), "Reported, thanks").then(() => ((confirm = null), (reason = "")))}>Report</button>
        {/if}
      </div>
    </div>
  {/if}
{/if}

{#if notice}<p class="m-hint">{notice}</p>{/if}
{#if error}<p class="m-error">{error}</p>{/if}

{#if page === "members" || page === "invite"}
  <MembersPage {room} startInviting={page === "invite"} onClose={() => (page = null)} onOpenChat={(id) => ((page = null), nav.openChat(id))} onChanged={reload} />
{:else if page === "roles"}
  <RolesPage {room} onClose={() => (page = null)} onChanged={reload} />
{/if}

<style>
  .header {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    margin: 8px 0 10px;
    text-align: center;
  }
  .header .m-input {
    margin-top: 6px;
  }
  .header .m-buttons {
    width: 100%;
  }
  h2 {
    margin: 10px 0 0;
  }
  .topic {
    margin: 0;
    color: var(--muted);
    white-space: pre-wrap;
  }
  .meta {
    display: flex;
    align-items: center;
    gap: 5px;
    margin: 0;
    color: var(--muted);
    font-size: 0.9rem;
  }
  .edit {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin-top: 4px;
    padding: 6px 12px;
    border: none;
    border-radius: 999px;
    background: var(--surface);
    color: var(--text);
    font: inherit;
    font-size: 0.9rem;
    cursor: pointer;
  }
  .m-hint :global(svg) {
    vertical-align: -1px;
  }
  .confirm {
    margin: 10px 0;
    padding: 14px;
    border-radius: 14px;
    background: var(--surface);
  }
  .confirm p {
    margin: 0 0 6px;
  }
</style>
