<script lang="ts">
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import Mic from "@lucide/svelte/icons/mic";
  import MicOff from "@lucide/svelte/icons/mic-off";
  import Video from "@lucide/svelte/icons/video";
  import VideoOff from "@lucide/svelte/icons/video-off";
  import MonitorUp from "@lucide/svelte/icons/monitor-up";
  import PhoneOff from "@lucide/svelte/icons/phone-off";
  import { fade, fly } from "svelte/transition";
  import type { Track } from "livekit-client";
  import { isAndroid } from "$lib/backend";
  import { call } from "$lib/stores/call.svelte";
  import Avatar from "./Avatar.svelte";

  /** Show a LiveKit video track in a <video> element. */
  function video(el: HTMLVideoElement, track: Track) {
    let current = track;
    current.attach(el);
    return {
      update(next: Track) {
        if (next === current) return;
        current.detach(el);
        current = next;
        current.attach(el);
      },
      destroy: () => current.detach(el),
    };
  }

  const statusText = $derived(
    call.status === "connecting"
      ? "Connecting…"
      : call.status === "reconnecting"
        ? "Reconnecting…"
        : call.calling
          ? "Calling…"
          : null,
  );
  /** Screen shares get the big spot. */
  const featured = $derived(call.tiles.find((t) => t.screen) ?? null);
  const canShareScreen = !isAndroid && !!navigator.mediaDevices?.getDisplayMedia;
</script>

{#if call.notice && !call.active}
  <div class="notice" transition:fly={{ y: -16, duration: 180 }}>{call.notice}</div>
{/if}
{#if call.active && call.minimized}
  <button class="pill" onclick={() => (call.minimized = false)} transition:fly={{ y: -16, duration: 180 }}>
    <span class="dot"></span>
    <span class="pill-name">{call.roomName}</span>
    <span class="pill-count">{call.tiles.length} in call</span>
  </button>
{:else if call.active}
  <div class="call" transition:fade={{ duration: 160 }}>
    <header>
      <button class="icon" onclick={() => (call.minimized = true)} aria-label="Back to chats"><ChevronDown size={24} /></button>
      <div class="title">
        <strong>{call.roomName}</strong>
        <small>{statusText ?? `${call.tiles.length} in call`}</small>
      </div>
    </header>

    {#if featured?.screen}
      <div class="featured">
        <!-- svelte-ignore a11y_media_has_caption -->
        <video use:video={featured.screen} autoplay playsinline muted></video>
        <span class="label">{featured.isMe ? "Your screen" : featured.name}</span>
      </div>
    {/if}

    <div class="grid" class:solo={call.tiles.length === 1} class:strip={featured}>
      {#each call.tiles as t (t.id)}
        <div class="tile" class:speaking={t.speaking} transition:fade={{ duration: 150 }}>
          {#if t.camera}
            <!-- svelte-ignore a11y_media_has_caption -->
            <video use:video={t.camera} autoplay playsinline muted class:mirror={t.isMe}></video>
          {:else}
            <div class="face">
              <div class="ring"><Avatar name={t.name} mxc={t.avatar} size={featured ? 56 : 88} /></div>
            </div>
          {/if}
          <span class="name">{#if !t.micOn}<MicOff size={13} />{/if}{t.isMe ? "You" : t.name}</span>
        </div>
      {/each}
    </div>

    {#if call.error}<p class="error" transition:fade>{call.error}</p>{/if}

    <div class="controls">
      <button class="ctl" class:off={!call.micOn} onclick={() => call.toggleMic()} aria-label={call.micOn ? "Mute" : "Unmute"}>
        {#if call.micOn}<Mic size={24} />{:else}<MicOff size={24} />{/if}
      </button>
      <button class="ctl" class:on={call.camOn} onclick={() => call.toggleCamera()} aria-label={call.camOn ? "Turn camera off" : "Turn camera on"}>
        {#if call.camOn}<Video size={24} />{:else}<VideoOff size={24} />{/if}
      </button>
      {#if canShareScreen}
        <button class="ctl" class:on={call.screenOn} onclick={() => call.toggleScreen()} aria-label="Share screen"><MonitorUp size={24} /></button>
      {/if}
      <button class="ctl hangup" onclick={() => call.leave()} aria-label="Leave call"><PhoneOff size={24} /></button>
    </div>
  </div>
{/if}

<style>
  .call {
    position: fixed;
    inset: 0;
    z-index: 60;
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 12px 12px 20px;
    background: radial-gradient(circle at 50% 0%, color-mix(in srgb, var(--accent-a) 25%, var(--bg)), var(--bg) 70%);
  }
  header {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .title {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .title strong {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .title small {
    color: var(--muted);
  }
  .icon {
    width: 40px;
    height: 40px;
    border: none;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: var(--surface-2);
    color: inherit;
    cursor: pointer;
  }
  .featured {
    position: relative;
    flex: 2;
    min-height: 0;
    border-radius: 18px;
    overflow: hidden;
    background: black;
  }
  .featured video {
    width: 100%;
    height: 100%;
    object-fit: contain;
  }
  .grid {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
    grid-auto-rows: minmax(140px, 1fr);
    gap: 10px;
    overflow-y: auto;
  }
  .grid.solo {
    grid-template-columns: 1fr;
  }
  .grid.strip {
    flex: none;
    display: flex;
    overflow-x: auto;
  }
  .grid.strip .tile {
    flex: none;
    width: 120px;
    height: 120px;
  }
  .tile {
    position: relative;
    display: grid;
    place-items: center;
    border-radius: 18px;
    overflow: hidden;
    background: var(--surface);
    transition: box-shadow 0.15s var(--ease);
  }
  .tile.speaking {
    box-shadow: inset 0 0 0 3px #31d158;
  }
  .tile video {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .tile video.mirror {
    transform: scaleX(-1);
  }
  .ring {
    padding: 4px;
    border-radius: 50%;
    transition: box-shadow 0.15s var(--ease);
  }
  .speaking .ring {
    box-shadow: 0 0 0 3px #31d158, 0 0 24px #31d15866;
  }
  .name,
  .label {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    position: absolute;
    left: 10px;
    bottom: 8px;
    max-width: calc(100% - 20px);
    padding: 3px 9px;
    border-radius: 999px;
    background: rgb(0 0 0 / 0.5);
    font-size: 0.8rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .error {
    margin: 0;
    text-align: center;
    color: var(--danger);
  }
  .controls {
    display: flex;
    justify-content: center;
    gap: 18px;
  }
  .ctl {
    width: 60px;
    height: 60px;
    border: none;
    display: grid;
    place-items: center;
    border-radius: 50%;
    background: var(--surface-2);
    color: inherit;
    cursor: pointer;
    transition: transform 0.12s var(--ease), background 0.15s var(--ease);
  }
  .ctl:active {
    transform: scale(0.92);
  }
  .ctl.off {
    background: #fff;
    color: #111;
  }
  .ctl.on {
    background: var(--accent);
  }
  .hangup {
    background: var(--danger);
    color: white;
  }
  .pill {
    position: fixed;
    top: 10px;
    left: 50%;
    z-index: 60;
    translate: -50% 0;
    display: flex;
    align-items: center;
    gap: 8px;
    max-width: calc(100vw - 24px);
    padding: 8px 16px;
    border: none;
    border-radius: 999px;
    background: #1f9d48;
    color: white;
    box-shadow: 0 8px 24px rgb(0 0 0 / 0.4);
    cursor: pointer;
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: white;
    animation: blink 1.2s ease-in-out infinite alternate;
  }
  @keyframes blink {
    to {
      opacity: 0.3;
    }
  }
  .pill-name {
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .pill-count {
    opacity: 0.85;
    font-size: 0.85rem;
    white-space: nowrap;
  }
  .notice {
    position: fixed;
    top: 10px;
    left: 50%;
    z-index: 60;
    translate: -50% 0;
    padding: 8px 16px;
    border-radius: 999px;
    background: var(--surface-2);
    box-shadow: 0 8px 24px rgb(0 0 0 / 0.4);
  }
</style>
