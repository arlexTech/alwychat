<script lang="ts">
  import MessageSquare from "@lucide/svelte/icons/message-square";
  import Mic from "@lucide/svelte/icons/mic";
  import MicOff from "@lucide/svelte/icons/mic-off";
  import Headphones from "@lucide/svelte/icons/headphones";
  import HeadphoneOff from "@lucide/svelte/icons/headphone-off";
  import PhoneOff from "@lucide/svelte/icons/phone-off";
  import Maximize2 from "@lucide/svelte/icons/maximize-2";
  import Volume2 from "@lucide/svelte/icons/volume-2";
  import type { RoomSummary } from "$lib/backend";
  import { call } from "$lib/stores/call.svelte";
  import Avatar from "./Avatar.svelte";

  /**
   * A voice channel in a space (Discord style): tapping it joins the call, the people in it are
   * listed underneath, live. `room` is null for a channel we haven't joined yet (`name` then).
   */
  let {
    room,
    name = "",
    busy = false,
    onJoin,
    onChat,
  }: {
    room: RoomSummary | null;
    name?: string;
    busy?: boolean;
    onJoin: () => void;
    onChat?: () => void;
  } = $props();

  const connected = $derived(!!room && call.roomId === room.id);
  type Person = {
    id: string;
    userId: string;
    isMe: boolean;
    name: string;
    avatar: string | null;
    speaking: boolean;
    muted: boolean;
  };
  // While we're in it, LiveKit knows first (and who speaks); otherwise the room's call members.
  const people = $derived.by((): Person[] => {
    if (!room) return [];
    if (connected && call.tiles.length) {
      return call.tiles.map((t) => ({
        id: t.id,
        userId: t.userId,
        isMe: t.isMe,
        name: t.isMe ? `${t.name} (you)` : t.name,
        avatar: t.avatar,
        speaking: t.speaking,
        muted: !t.micOn,
      }));
    }
    return room.in_call.map((m) => ({ id: m.user_id, userId: m.user_id, isMe: false, name: m.name, avatar: m.avatar, speaking: false, muted: false }));
  });
  /** Whose volume slider is open (tap someone while connected). */
  let volumeFor = $state<string | null>(null);
</script>

<div class="channel" class:connected data-room={room?.id}>
  <div class="head">
    <button class="join" onclick={onJoin} disabled={busy} aria-label="Join voice channel {room?.name ?? name}">
      <span class="kind"><Volume2 size={18} /></span>
      <span class="name">{room?.name ?? name}</span>
      {#if busy}<small>Joining…</small>{:else if connected}<small class="live">{call.status === "connected" ? "Connected" : "Connecting…"}</small>{/if}
    </button>
    {#if room && onChat}
      <button class="chat" onclick={onChat} aria-label="Open the channel's chat" title="Chat">
        <MessageSquare size={17} />
        {#if room.unread_messages > 0}<span class="dot"></span>{/if}
      </button>
    {/if}
  </div>
  {#if people.length}
    <ul>
      {#each people as p (p.id)}
        <li class:speaking={p.speaking}>
          <button
            class="person"
            disabled={!connected || p.isMe}
            onclick={() => (volumeFor = volumeFor === p.userId ? null : p.userId)}
            aria-label={connected && !p.isMe ? `Volume of ${p.name}` : p.name}
          >
            <span class="avatar"><Avatar name={p.name} mxc={p.avatar} size={24} /></span>
            <span class="who">{p.name}</span>
            {#if connected && !p.isMe && call.volumeOf(p.userId) < 1}<small>{Math.round(call.volumeOf(p.userId) * 100)}%</small>{/if}
            {#if p.isMe && call.deafened}<HeadphoneOff size={13} />{:else if p.muted}<MicOff size={13} />{/if}
          </button>
          {#if connected && volumeFor === p.userId}
            <input
              class="volume"
              type="range"
              min="0"
              max="100"
              value={Math.round(call.volumeOf(p.userId) * 100)}
              oninput={(e) => call.setVolume(p.userId, Number(e.currentTarget.value) / 100)}
              aria-label="Volume of {p.name}"
            />
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
  {#if connected}
    <div class="controls">
      <button class:off={!call.micOn} onclick={() => call.toggleMic()} aria-label={call.micOn ? "Mute" : "Unmute"} title={call.micOn ? "Mute" : "Unmute"}>
        {#if call.micOn}<Mic size={18} />{:else}<MicOff size={18} />{/if}
      </button>
      <button class:off={call.deafened} onclick={() => call.toggleDeafen()} aria-label={call.deafened ? "Undeafen" : "Deafen"} title={call.deafened ? "Undeafen" : "Deafen"}>
        {#if call.deafened}<HeadphoneOff size={18} />{:else}<Headphones size={18} />{/if}
      </button>
      <button onclick={() => (call.minimized = false)} aria-label="Open the call" title="Open the call"><Maximize2 size={17} /></button>
      <button class="leave" onclick={() => call.leave()} aria-label="Leave" title="Leave"><PhoneOff size={18} /></button>
    </div>
  {/if}
</div>

<style>
  .channel {
    margin: 2px 4px;
    border-radius: 12px;
    transition: background 0.15s var(--ease);
  }
  .channel.connected {
    background: color-mix(in srgb, #31d158 10%, transparent);
  }
  .head {
    display: flex;
    align-items: center;
  }
  .join {
    display: flex;
    flex: 1;
    align-items: center;
    gap: 10px;
    min-width: 0;
    padding: 10px 10px;
    border: none;
    border-radius: 12px;
    background: none;
    color: var(--text);
    text-align: left;
    cursor: pointer;
  }
  .join:hover {
    background: var(--surface-hover);
  }
  .kind {
    display: grid;
    flex: none;
    width: 36px;
    height: 36px;
    place-items: center;
    border-radius: 10px;
    background: var(--surface-2);
    color: var(--muted);
  }
  .connected .kind {
    background: #31d158;
    color: white;
  }
  .name {
    flex: 1;
    overflow: hidden;
    font-weight: 600;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  small {
    color: var(--muted);
  }
  .live {
    color: #31d158;
  }
  .chat {
    position: relative;
    display: grid;
    flex: none;
    width: 38px;
    height: 38px;
    margin-right: 4px;
    place-items: center;
    border: none;
    border-radius: 50%;
    background: none;
    color: var(--muted);
    cursor: pointer;
  }
  .chat:hover {
    background: var(--surface-hover);
    color: var(--text);
  }
  .dot {
    position: absolute;
    top: 8px;
    right: 8px;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--accent);
  }
  ul {
    margin: 0;
    padding: 0 10px 8px 56px;
    list-style: none;
  }
  li {
    display: flex;
    flex-direction: column;
    color: var(--muted);
    font-size: 0.9rem;
  }
  .person {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
    padding: 3px 0;
    border: none;
    background: none;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .person:disabled {
    cursor: default;
    opacity: 1;
  }
  .person small {
    font-size: 0.75rem;
  }
  .volume {
    margin: 2px 0 6px 32px;
    accent-color: var(--accent);
  }
  .controls {
    display: flex;
    gap: 6px;
    padding: 0 10px 10px 56px;
  }
  .controls button {
    display: grid;
    width: 36px;
    height: 32px;
    place-items: center;
    border: none;
    border-radius: 10px;
    background: var(--surface-2);
    color: var(--text);
    cursor: pointer;
  }
  .controls button.off {
    background: #fff;
    color: #111;
  }
  .controls .leave {
    margin-left: auto;
    background: var(--danger);
    color: #fff;
  }
  li :global(svg) {
    flex: none;
  }
  .avatar {
    display: flex;
    border-radius: 50%;
    box-shadow: 0 0 0 2px transparent;
    transition: box-shadow 0.12s var(--ease);
  }
  .speaking .avatar {
    box-shadow: 0 0 0 2px #31d158;
  }
  .speaking .who {
    color: var(--text);
  }
  .who {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
