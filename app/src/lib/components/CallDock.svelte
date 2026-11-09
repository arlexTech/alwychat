<script lang="ts">
  import { tr } from "$lib/i18n/index.svelte";
  import Mic from "@lucide/svelte/icons/mic";
  import MicOff from "@lucide/svelte/icons/mic-off";
  import Headphones from "@lucide/svelte/icons/headphones";
  import HeadphoneOff from "@lucide/svelte/icons/headphone-off";
  import Video from "@lucide/svelte/icons/video";
  import VideoOff from "@lucide/svelte/icons/video-off";
  import MonitorUp from "@lucide/svelte/icons/monitor-up";
  import Maximize2 from "@lucide/svelte/icons/maximize-2";
  import PhoneOff from "@lucide/svelte/icons/phone-off";
  import { slide } from "svelte/transition";
  import { call } from "$lib/stores/call.svelte";

  /** The call's controls while the call screen is closed (Discord's voice panel): under the
   * chat list, and on narrow screens (one row) also above the composer of an open chat, where
   * the list isn't visible. */
  let { inChat = false }: { inChat?: boolean } = $props();
  const compact = $derived(!call.docked);
  const statusText = $derived(
    call.status === "connecting"
      ? tr("Connecting…")
      : call.status === "reconnecting"
        ? tr("Reconnecting…")
        : call.calling
          ? tr("Calling…")
          : tr("Connected"),
  );
</script>

{#if call.active && call.minimized && (!inChat || compact)}
  <section class="dock" class:compact aria-label={call.roomName} transition:slide={{ duration: 160 }}>
    <div class="top">
      <button class="info" onclick={() => (call.minimized = false)} title={tr("Open the call")}>
        <strong class:waiting={call.status !== "connected"}>{statusText}</strong>
        <span>{call.roomName}</span>
      </button>
      {#if compact}
        <button class="icon" class:off={!call.micOn} onclick={() => call.toggleMic()} aria-pressed={!call.micOn} aria-label={call.micOn ? tr("Mute") : tr("Unmute")}>
          {#if call.micOn}<Mic size={20} />{:else}<MicOff size={20} />{/if}
        </button>
        <button class="icon" class:off={call.deafened} onclick={() => call.toggleDeafen()} aria-pressed={call.deafened} aria-label={call.deafened ? tr("Undeafen") : tr("Deafen")}>
          {#if call.deafened}<HeadphoneOff size={20} />{:else}<Headphones size={20} />{/if}
        </button>
      {:else}
        <button class="icon" onclick={() => (call.minimized = false)} aria-label={tr("Open the call")} title={tr("Open the call")}><Maximize2 size={17} /></button>
      {/if}
      <button class="icon leave" onclick={() => call.leave()} aria-label={tr("Leave call")} title={tr("Leave call")}><PhoneOff size={18} /></button>
    </div>
    {#if call.error}<p class="error" title={call.error}>{call.error}</p>{/if}
    {#if !compact}<div class="row">
      <button class:off={!call.micOn} onclick={() => call.toggleMic()} aria-pressed={!call.micOn} aria-label={call.micOn ? tr("Mute") : tr("Unmute")} title={call.micOn ? tr("Mute") : tr("Unmute")}>
        {#if call.micOn}<Mic size={19} />{:else}<MicOff size={19} />{/if}
      </button>
      <button class:off={call.deafened} onclick={() => call.toggleDeafen()} aria-pressed={call.deafened} aria-label={call.deafened ? tr("Undeafen") : tr("Deafen")} title={call.deafened ? tr("Undeafen") : tr("Deafen")}>
        {#if call.deafened}<HeadphoneOff size={19} />{:else}<Headphones size={19} />{/if}
      </button>
      <button class:on={call.camOn} onclick={() => call.toggleCamera()} aria-pressed={call.camOn} aria-label={call.camOn ? tr("Turn camera off") : tr("Turn camera on")} title={call.camOn ? tr("Turn camera off") : tr("Turn camera on")}>
        {#if call.camOn}<Video size={19} />{:else}<VideoOff size={19} />{/if}
      </button>
      {#if call.canShareScreen}
        <button class:on={call.screenOn} onclick={() => call.toggleScreen()} aria-pressed={call.screenOn} aria-label={tr("Share screen")} title={tr("Share screen")}><MonitorUp size={19} /></button>
      {/if}
    </div>{/if}
  </section>
{/if}

<style>
  .dock {
    flex: none;
    padding: 8px 10px 10px;
    border-top: 1px solid var(--border);
    background: var(--surface-2);
  }
  .compact {
    padding: 6px 8px;
  }
  .compact .icon {
    width: 40px;
    height: 40px;
    color: var(--text);
  }
  .compact .icon.leave {
    color: var(--danger);
  }
  .icon.off {
    background: var(--danger);
    color: #fff;
  }
  .top {
    display: flex;
    align-items: center;
    gap: 2px;
  }
  .info {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-width: 0;
    padding: 2px 6px;
    border: none;
    border-radius: 8px;
    background: none;
    color: var(--text);
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .info:hover {
    background: var(--surface-hover);
  }
  strong {
    color: #31d158;
    font-size: 0.85rem;
  }
  strong.waiting {
    color: var(--muted);
  }
  .info span,
  .error {
    overflow: hidden;
    color: var(--muted);
    font-size: 0.8rem;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .error {
    margin: 2px 6px 0;
    color: var(--danger);
  }
  .icon {
    display: grid;
    flex: none;
    width: 34px;
    height: 34px;
    place-items: center;
    border: none;
    border-radius: 8px;
    background: none;
    color: var(--muted);
    cursor: pointer;
  }
  .icon:hover {
    background: var(--surface-hover);
    color: var(--text);
  }
  .icon.leave:hover {
    background: var(--danger);
    color: #fff;
  }
  .row {
    display: flex;
    gap: 6px;
    margin-top: 6px;
  }
  .row button {
    display: grid;
    flex: 1;
    height: 36px;
    place-items: center;
    border: none;
    border-radius: 8px;
    background: var(--surface);
    color: var(--text);
    cursor: pointer;
  }
  .row button:hover {
    background: var(--surface-hover);
  }
  .row button.off {
    background: var(--danger);
    color: #fff;
  }
  .row button.on {
    background: #31d158;
    color: #fff;
  }
</style>
