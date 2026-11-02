<script lang="ts">
  import { tr } from "$lib/i18n/index.svelte";
  import { fade, fly } from "svelte/transition";
  import Phone from "@lucide/svelte/icons/phone";
  import PhoneOff from "@lucide/svelte/icons/phone-off";
  import Video from "@lucide/svelte/icons/video";
  import { call } from "$lib/stores/call.svelte";
  import { chatList } from "$lib/stores/chatList.svelte";
  import Avatar from "./Avatar.svelte";

  const ring = $derived(call.incoming);
  const room = $derived(ring ? (chatList.rooms.find((r) => r.id === ring.room_id) ?? null) : null);

  // The caller hung up before we answered: their membership is gone from the room.
  let seenInCall = false;
  $effect(() => {
    if (!ring || !room) return;
    if (room.in_call.length) seenInCall = true;
    else if (seenInCall) {
      seenInCall = false;
      call.stopRing();
    }
  });
</script>

{#if ring}
  <div class="incoming" transition:fade={{ duration: 180 }}>
    <div class="who" in:fly={{ y: 20, duration: 260 }}>
      <div class="pulse"><Avatar name={ring.caller_name} mxc={ring.caller_avatar} size={120} /></div>
      <strong>{ring.caller_name}</strong>
      <span>{ring.video ? tr("Incoming video call") : tr("Incoming call")}</span>
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
