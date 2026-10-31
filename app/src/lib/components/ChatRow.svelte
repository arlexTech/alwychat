<script lang="ts">
  import Phone from "@lucide/svelte/icons/phone";
  import type { RoomSummary } from "$lib/backend";
  import { shortTime } from "$lib/format";
  import { drafts } from "$lib/stores/drafts.svelte";
  import { presence } from "$lib/stores/presence.svelte";
  import Avatar from "./Avatar.svelte";

  let {
    room,
    selected = false,
    onclick,
    oncontextmenu,
  }: {
    room: RoomSummary;
    selected?: boolean;
    onclick: () => void;
    oncontextmenu?: (event: MouseEvent) => void;
  } = $props();

  const unread = $derived(room.unread_messages > 0 || room.marked_unread || room.is_invite);
  // Telegram-style: an unsent draft replaces the preview (not on the chat being typed in).
  const draft = $derived(selected ? undefined : drafts.get(room.id));
  const preview = $derived.by(() => {
    const l = room.latest;
    if (!l) return room.is_invite ? "Invitation" : "";
    const who = l.is_own ? "You: " : l.sender_name ? `${l.sender_name}: ` : "";
    return who + l.text;
  });
</script>

<button class="row" class:selected class:unread class:muted={room.is_muted} data-room={room.id} {onclick} {oncontextmenu}>
  <span class="avatar">
    <Avatar name={room.name} mxc={room.avatar} size={52} />
    {#if room.is_dm && presence.online(room.dm_user)}<span class="online" title="Online"></span>{/if}
  </span>
  <div class="body">
    <div class="line">
      <span class="name">{room.name}</span>
      {#if room.in_call.length}<span class="icon in-call" title="{room.in_call.length} in the call"><Phone size={12} /> {room.in_call.length}</span>{/if}
      {#if room.is_muted}<span class="icon" title="Muted">🔕</span>{/if}
      {#if room.is_favourite}<span class="icon" title="Pinned">📌</span>{/if}
      <span class="time">{shortTime(room.latest?.timestamp)}</span>
    </div>
    <div class="line">
      <span class="preview" class:sending={room.latest?.is_sending}
        >{#if draft}<span class="draft">Draft:</span> {draft.text.replace(/\s+/g, " ")}{:else}{preview}{/if}</span
      >
      {#if room.unread_mentions > 0}
        <span class="badge mention">@</span>
      {:else if room.unread_messages > 0}
        <span class="badge">{room.unread_messages > 99 ? "99+" : room.unread_messages}</span>
      {:else if unread}
        <span class="dot"></span>
      {/if}
    </div>
  </div>
</button>

<style>
  .row {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
    padding: 8px 12px;
    border: none;
    border-radius: var(--radius);
    background: transparent;
    text-align: left;
    cursor: pointer;
    transition: background 0.15s var(--ease), transform 0.1s var(--ease);
    content-visibility: auto;
    contain-intrinsic-size: auto 68px;
  }
  .row:hover {
    background: var(--surface-hover);
  }
  .row:active {
    transform: scale(0.985);
  }
  .row.selected {
    background: var(--surface-2);
  }
  .body {
    flex: 1;
    min-width: 0;
  }
  .line {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .line + .line {
    margin-top: 3px;
  }
  .name,
  .preview {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .name {
    font-weight: 500;
  }
  .time {
    font-size: 0.78rem;
    color: var(--muted);
  }
  .preview {
    font-size: 0.9rem;
    color: var(--muted);
  }
  .draft {
    color: var(--danger, #e5484d);
  }
  .preview.sending {
    opacity: 0.6;
  }
  .unread .name,
  .unread .preview {
    font-weight: 650;
    color: var(--text);
  }
  .unread .time {
    color: var(--accent-b);
  }
  .badge {
    min-width: 20px;
    height: 20px;
    padding: 0 6px;
    border-radius: 10px;
    background: var(--accent);
    color: white;
    font-size: 0.72rem;
    font-weight: 700;
    display: grid;
    place-items: center;
  }
  .dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: var(--accent);
  }
  .icon {
    flex: none;
    font-size: 0.75rem;
    opacity: 0.8;
  }
  .muted .badge {
    background: var(--surface-hover);
    color: var(--muted);
  }
  .avatar {
    position: relative;
    flex: none;
    display: flex;
  }
  /* Messenger's green dot: someone in a 1:1 chat is online. */
  .online {
    position: absolute;
    right: 1px;
    bottom: 1px;
    width: 14px;
    height: 14px;
    border: 3px solid var(--surface);
    border-radius: 50%;
    background: #31d158;
  }
  .in-call {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    color: #31d158;
    font-size: 0.8rem;
    white-space: nowrap;
  }
</style>
