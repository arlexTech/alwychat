<script lang="ts">
  import type { RoomSummary } from "$lib/backend";
  import { shortTime } from "$lib/format";
  import Avatar from "./Avatar.svelte";

  let { room, selected = false, onclick }: { room: RoomSummary; selected?: boolean; onclick: () => void } =
    $props();

  const unread = $derived(room.unread_messages > 0 || room.marked_unread || room.is_invite);
  const preview = $derived.by(() => {
    const l = room.latest;
    if (!l) return room.is_invite ? "Invitation" : "";
    const who = l.is_own ? "You: " : l.sender_name ? `${l.sender_name}: ` : "";
    return who + l.text;
  });
</script>

<button class="row" class:selected class:unread {onclick}>
  <Avatar name={room.name} mxc={room.avatar} size={52} />
  <div class="body">
    <div class="line">
      <span class="name">{room.name}</span>
      <span class="time">{shortTime(room.latest?.timestamp)}</span>
    </div>
    <div class="line">
      <span class="preview" class:sending={room.latest?.is_sending}>{preview}</span>
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
</style>
