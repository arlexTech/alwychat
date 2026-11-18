<script lang="ts">
  import { tr, trn } from "$lib/i18n/index.svelte";
  import { fade, fly } from "svelte/transition";
  import Phone from "@lucide/svelte/icons/phone";
  import PhoneOff from "@lucide/svelte/icons/phone-off";
  import Video from "@lucide/svelte/icons/video";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import BellOff from "@lucide/svelte/icons/bell-off";
  import { call } from "$lib/stores/call.svelte";
  import { ringActive } from "$lib/backend";
  import { untrack } from "svelte";
  import { chatList } from "$lib/stores/chatList.svelte";
  import Avatar from "./Avatar.svelte";

  const ring = $derived(call.incoming);
  const room = $derived(ring ? (chatList.rooms.find((r) => r.id === ring.room_id) ?? null) : null);

  /** Hidden into a banner at the top: the app stays usable while it rings. A new ring shows
   * full screen again. */
  let small = $state(false);
  let shownFor: string | null = null;
  $effect(() => {
    const id = ring?.notification_id ?? null;
    if (id !== shownFor) {
      shownFor = id;
      small = false;
    }
  });

  // Stop ringing once the caller hung up or we answered on another device. Asked of the
  // server (whenever the room's call members change here, and every 3 s): the chat list's copy
  // lags, and right after a call it still showed the previous one ending, which stopped the
  // new ring at once (a call back only showed "1 in the call").
  let checking = false;
  async function check() {
    const current = ring;
    if (!current || checking) return;
    checking = true;
    try {
      const on = await ringActive(current.room_id);
      if (on === false && call.incoming?.notification_id === current.notification_id) call.stopRing();
    } catch {
      // Unknown: keep ringing (it ends at its deadline anyway).
    } finally {
      checking = false;
    }
  }
  $effect(() => {
    if (!ring) return;
    void room?.in_call; // re-check when the members change
    void untrack(check);
  });
  $effect(() => {
    if (!ring) return;
    const timer = setInterval(() => void check(), 3000);
    return () => clearInterval(timer);
  });
</script>

{#if ring && small}
  <div class="banner" role="dialog" aria-label={tr("Incoming call")} transition:fly={{ y: -40, duration: 200 }}>
    <button class="open" onclick={() => (small = false)} aria-label={tr("Show the call")}>
      <Avatar name={ring.caller_name} mxc={ring.caller_avatar} size={40} />
      <span class="text">
        <strong>{ring.caller_name}</strong>
        <small>
          {#if room && !room.is_dm}{tr("Calls you into {name}", { name: room.name })}{#if room.in_call.length} · {trn(room.in_call.length, "{n} in the call", "{n} in the call")}{/if}{:else}{ring.video ? tr("Incoming video call") : tr("Incoming call")}{/if}
        </small>
      </span>
    </button>
    {#if !call.ringSilenced}
      <button class="mini quiet" onclick={() => call.silenceRing()} aria-label={tr("Silence")} title={tr("Silence")}><BellOff size={18} /></button>
    {/if}
    <button class="mini decline" onclick={() => call.decline()} aria-label={tr("Decline")}><PhoneOff size={18} /></button>
    <button class="mini accept" onclick={() => room && call.accept(room)} disabled={!room} aria-label={tr("Answer")}>
      {#if ring.video}<Video size={18} />{:else}<Phone size={18} />{/if}
    </button>
  </div>
{:else if ring}
  <div class="incoming" transition:fade={{ duration: 180 }}>
    <button class="hide" onclick={() => (small = true)} aria-label={tr("Hide")} title={tr("Hide")}><ChevronDown size={24} /></button>
    <div class="who" in:fly={{ y: 20, duration: 260 }}>
      <div class="pulse"><Avatar name={ring.caller_name} mxc={ring.caller_avatar} size={120} /></div>
      <strong>{ring.caller_name}</strong>
      {#if room && !room.is_dm}
        <!-- Rung into a group call or a voice channel. -->
        <span>{tr("Calls you into {name}", { name: room.name })}</span>
        {#if room.in_call.length}
          <!-- Who's already there. -->
          <div class="present" aria-label={tr("In the call")}>
            <span class="label">{tr("In the call")}</span>
            <ul>
              {#each room.in_call.slice(0, 8) as m (m.user_id)}
                <li><Avatar name={m.name} mxc={m.avatar} size={36} /><span>{m.name}</span></li>
              {/each}
            </ul>
            {#if room.in_call.length > 8}<span class="more">{tr("+{n} more", { n: room.in_call.length - 8 })}</span>{/if}
          </div>
        {/if}
      {:else}
        <span>{ring.video ? tr("Incoming video call") : tr("Incoming call")}</span>
      {/if}
    </div>
    <div class="buttons">
      <button class="decline" onclick={() => call.decline()} aria-label={tr("Decline")}>
        <PhoneOff size={28} />
      </button>
      <button class="accept" onclick={() => room && call.accept(room)} disabled={!room} aria-label={tr("Answer")}>
        {#if ring.video}<Video size={28} />{:else}<Phone size={28} />{/if}
      </button>
    </div>
  </div>
{/if}

<style>
  .hide {
    position: absolute;
    top: 16px;
    left: 16px;
    display: grid;
    place-items: center;
    width: 44px;
    height: 44px;
    border: none;
    border-radius: 50%;
    background: color-mix(in srgb, var(--text) 10%, transparent);
    color: var(--text);
    cursor: pointer;
  }
  .banner {
    position: fixed;
    top: 10px;
    left: 50%;
    z-index: 70;
    display: flex;
    align-items: center;
    gap: 8px;
    width: min(480px, calc(100% - 20px));
    padding: 8px 10px 8px 8px;
    transform: translateX(-50%);
    border-radius: 22px;
    background: var(--surface, var(--bg));
    box-shadow: 0 8px 28px rgb(0 0 0 / 0.35);
  }
  .banner .open {
    display: flex;
    flex: 1;
    align-items: center;
    gap: 10px;
    min-width: 0;
    padding: 0;
    border: none;
    background: none;
    color: inherit;
    text-align: left;
    cursor: pointer;
  }
  .banner .text {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .banner .text strong,
  .banner .text small {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .banner .text small {
    color: var(--muted);
  }
  .mini {
    display: grid;
    flex: none;
    place-items: center;
    width: 40px;
    height: 40px;
    border: none;
    border-radius: 50%;
    color: white;
    cursor: pointer;
  }
  .mini.quiet {
    background: color-mix(in srgb, var(--text) 14%, transparent);
    color: var(--text);
  }
  .incoming {
    position: fixed;
    inset: 0;
    z-index: 70;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: space-between;
    padding: 18vh 24px 10vh;
    background: radial-gradient(circle at 50% 30%, color-mix(in srgb, var(--accent-a) 35%, var(--bg)), var(--bg) 75%);
  }
  .who {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
  }
  .who strong {
    margin-top: 16px;
    font-size: 1.6rem;
  }
  .who span {
    color: var(--muted);
  }
  .present {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    margin-top: 18px;
  }
  .present .label,
  .present .more {
    color: var(--muted);
    font-size: 0.85rem;
  }
  .present ul {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: 10px 14px;
    max-width: min(420px, 90vw);
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .present li {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    width: 64px;
  }
  .present li span {
    max-width: 100%;
    overflow: hidden;
    font-size: 0.78rem;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .pulse {
    border-radius: 50%;
    animation: pulse 1.6s ease-out infinite;
  }
  @keyframes pulse {
    from {
      box-shadow: 0 0 0 0 rgb(49 209 88 / 0.5);
    }
    to {
      box-shadow: 0 0 0 36px rgb(49 209 88 / 0);
    }
  }
  .buttons {
    display: flex;
    gap: 72px;
  }
  .buttons button {
    display: grid;
    place-items: center;
    width: 72px;
    height: 72px;
    border: none;
    border-radius: 50%;
    color: white;
    cursor: pointer;
    transition: transform 0.12s var(--ease);
  }
  .buttons button:active {
    transform: scale(0.9);
  }
  .decline {
    background: var(--danger);
  }
  .accept {
    background: #31d158;
  }
</style>
