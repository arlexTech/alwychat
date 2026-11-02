<script lang="ts">
  import { tr } from "$lib/i18n/index.svelte";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import Mic from "@lucide/svelte/icons/mic";
  import MicOff from "@lucide/svelte/icons/mic-off";
  import Video from "@lucide/svelte/icons/video";
  import VideoOff from "@lucide/svelte/icons/video-off";
  import MonitorUp from "@lucide/svelte/icons/monitor-up";
  import PhoneOff from "@lucide/svelte/icons/phone-off";
  import Headphones from "@lucide/svelte/icons/headphones";
  import HeadphoneOff from "@lucide/svelte/icons/headphone-off";
  import SlidersHorizontal from "@lucide/svelte/icons/sliders-horizontal";
  import MessageSquare from "@lucide/svelte/icons/message-square";
  import VolumeX from "@lucide/svelte/icons/volume-x";
  import { fade, fly } from "svelte/transition";
  import type { Track } from "livekit-client";
  import { isAndroid } from "$lib/backend";
  import { call } from "$lib/stores/call.svelte";
  import Maximize2 from "@lucide/svelte/icons/maximize-2";
  import Minimize2 from "@lucide/svelte/icons/minimize-2";
  import X from "@lucide/svelte/icons/x";
  import { onBack } from "$lib/back.svelte";
  import Avatar from "./Avatar.svelte";
  import { keyName } from "$lib/keys";
  import NativeVideo from "./NativeVideo.svelte";

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
      ? tr("Connecting…")
      : call.status === "reconnecting"
        ? tr("Reconnecting…")
        : call.calling
          ? tr("Calling…")
          : null,
  );
  type Tile = (typeof call.tiles)[number];
  type Spot = "screen" | "camera" | "person";
  const hasScreen = (t: Tile) => !!(t.screen || t.screenFrames);
  const hasCamera = (t: Tile) => !!(t.camera || t.cameraFrames);
  /** Clicked tile (Discord: click someone to focus them); null = automatic. */
  let focus = $state<string | null>(null);
  /** The big spot: the focused person (their screen, else camera, else avatar), or the first
   * screen share. */
  const featured = $derived.by((): { tile: Tile; what: Spot } | null => {
    const focused = focus ? call.tiles.find((t) => t.id === focus) : undefined;
    const tile = focused ?? call.tiles.find(hasScreen);
    if (!tile) return null;
    return { tile, what: hasScreen(tile) ? "screen" : hasCamera(tile) ? "camera" : "person" };
  });
  function toggleFocus(t: Tile) {
    if (longPressed) return void (longPressed = false);
    focus = focus === t.id ? null : t.id;
  }

  // Touch: tap focuses, long-press opens the person menu (as a sheet).
  let pressTimer: ReturnType<typeof setTimeout> | undefined;
  let longPressed = false;
  function pressStart(e: PointerEvent, t: Tile) {
    if (e.pointerType !== "touch") return;
    longPressed = false;
    pressTimer = setTimeout(() => {
      longPressed = true;
      navigator.vibrate?.(10);
      call.openMenu(person(t), e);
    }, 450);
  }
  const pressEnd = () => clearTimeout(pressTimer);

  // Full screen for the big spot: the system's when the webview has it, else the whole window.
  let spot = $state<HTMLElement>();
  let immersive = $state(false);
  async function toggleFullscreen() {
    if (document.fullscreenElement || immersive) {
      immersive = false;
      if (document.fullscreenElement) await document.exitFullscreen().catch(() => {});
      return;
    }
    try {
      if (!spot?.requestFullscreen) throw new Error("no fullscreen");
      await spot.requestFullscreen();
    } catch {
      immersive = true;
    }
  }
  $effect(() => {
    if (immersive) return onBack(() => (immersive = false));
  });
  const canShareScreen = $derived(!isAndroid && (call.native || !!navigator.mediaDevices?.getDisplayMedia));

  const person = (t: (typeof call.tiles)[number]) => ({
    userId: t.userId,
    name: t.name,
    avatar: t.avatar,
    isMe: t.isMe,
    live: true,
  });
  let optionsOpen = $state(false);

  // Push-to-talk on a keyboard: hold Space (outside text fields).
  const typing = (e: KeyboardEvent) => e.target instanceof HTMLElement && !!e.target.closest("input, textarea, [contenteditable]");
  function onKey(e: KeyboardEvent, down: boolean) {
    if (!call.active || !call.prefs.pushToTalk || e.code !== call.prefs.pttKey || typing(e)) return;
    e.preventDefault();
    if (!e.repeat) void call.talk(down);
  }
</script>

<svelte:window onkeydown={(e) => onKey(e, true)} onkeyup={(e) => onKey(e, false)} onblur={() => call.talk(false)} />

{#if call.notice && !call.active}
  <div class="notice" transition:fly={{ y: -16, duration: 180 }}>{call.notice}</div>
{/if}
{#if call.active && call.minimized}
  <button class="pill" onclick={() => (call.minimized = false)} transition:fly={{ y: -16, duration: 180 }}>
    <span class="dot"></span>
    <span class="pill-name">{call.roomName}</span>
    <span class="pill-count">{tr("{length} in call", { length: call.tiles.length })}</span>
  </button>
{:else if call.active}
  <div class="call" class:side={call.sideChat} transition:fade={{ duration: 160 }}>
    <header>
      <button class="icon" onclick={() => (call.minimized = true)} aria-label={tr("Back to chats")}><ChevronDown size={24} /></button>
      <div class="title">
        <strong>{call.roomName}</strong>
        <small>{statusText ?? tr("{length} in call", { length: call.tiles.length })}</small>
      </div>
      <button class="icon chat" class:open={call.sideChat} onclick={() => call.toggleChat()} aria-label={tr("Chat")} title={tr("Chat")} aria-pressed={call.sideChat}>
        <MessageSquare size={20} />
      </button>
      <button class="icon options" class:open={optionsOpen} onclick={() => (optionsOpen = !optionsOpen)} aria-label={tr("Call options")} aria-expanded={optionsOpen}>
        <SlidersHorizontal size={20} />
      </button>
    </header>
    {#if optionsOpen}
      <div class="options-panel" transition:fly={{ y: -8, duration: 150 }}>
        <label>
          <span><strong>{tr("Noise suppression")}</strong><small>{tr("Filters background noise from your mic")}</small></span>
          <input type="checkbox" role="switch" checked={call.prefs.noiseSuppression} onchange={(e) => call.setNoiseSuppression(e.currentTarget.checked)} />
        </label>
        <label>
          <span><strong>{tr("Push to talk")}</strong><small>{tr("Your mic is open only while you hold the talk button{value}", { value: isAndroid ? "" : ` or ${keyName(call.prefs.pttKey)}` })}</small></span>
          <input type="checkbox" role="switch" checked={call.prefs.pushToTalk} onchange={(e) => call.setPushToTalk(e.currentTarget.checked)} />
        </label>
      </div>
    {/if}

    {#if featured}
      {@const f = featured.tile}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="featured" class:immersive bind:this={spot} ondblclick={toggleFullscreen}>
        <!-- svelte-ignore a11y_media_has_caption -->
        {#if featured.what === "screen"}
          {#if f.screen}
            <video use:video={f.screen} autoplay playsinline muted></video>
          {:else if f.screenFrames}
            <NativeVideo frames={f.screenFrames} contain />
          {/if}
        {:else if featured.what === "camera"}
          {#if f.camera}
            <video class="contain" use:video={f.camera} autoplay playsinline muted class:mirror={f.isMe}></video>
          {:else if f.cameraFrames}
            <NativeVideo frames={f.cameraFrames} mirror={f.isMe} contain />
          {/if}
        {:else}
          <div class="face big" class:speaking={f.speaking}>
            <div class="ring"><Avatar name={f.name} mxc={f.avatar} size={140} /></div>
          </div>
        {/if}
        <span class="label">
          {featured.what === "screen" ? (f.isMe ? tr("Your screen") : tr("{name}'s screen", { name: f.name })) : f.isMe ? tr("You") : f.name}
        </span>
        <div class="spot-actions">
          <button onclick={toggleFullscreen} aria-label={tr("Full screen")} title={tr("Full screen")}>
            {#if immersive}<Minimize2 size={18} />{:else}<Maximize2 size={18} />{/if}
          </button>
          {#if focus}
            <button onclick={() => (focus = null)} aria-label={tr("Stop focusing")} title={tr("Stop focusing")}><X size={18} /></button>
          {/if}
        </div>
      </div>
    {/if}

    <div class="grid" class:solo={call.tiles.length === 1} class:strip={featured}>
      {#each call.tiles as t (t.id)}
        <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
        <div
          class="tile"
          class:speaking={t.speaking}
          class:focused={focus === t.id}
          onclick={() => toggleFocus(t)}
          onpointerdown={(e) => pressStart(e, t)}
          onpointerup={pressEnd}
          onpointercancel={pressEnd}
          onpointerleave={pressEnd}
          oncontextmenu={(e) => (e.preventDefault(), (e as unknown as PointerEvent).pointerType !== "touch" && call.openMenu(person(t), e))}
          transition:fade={{ duration: 150 }}
        >
          {#if t.cameraFrames}
            <NativeVideo frames={t.cameraFrames} mirror={t.isMe} />
          {:else if t.camera}
            <!-- svelte-ignore a11y_media_has_caption -->
            <video use:video={t.camera} autoplay playsinline muted class:mirror={t.isMe}></video>
          {:else}
            <div class="face">
              <div class="ring"><Avatar name={t.name} mxc={t.avatar} size={featured ? 56 : 88} /></div>
            </div>
          {/if}
          <span class="name">
            {#if t.isMe && call.deafened}<HeadphoneOff size={13} />{:else if !t.micOn}<MicOff size={13} />{/if}{t.isMe ? tr("You") : t.name}
            {#if !t.isMe && call.isMutedForMe(t.userId)}<small><VolumeX size={13} /></small>{:else if !t.isMe && call.volumeOf(t.userId) < 1}<small>{Math.round(call.volumeOf(t.userId) * 100)}%</small>{/if}
          </span>
        </div>
      {/each}
    </div>

    {#if call.error}<p class="error" transition:fade>{call.error}</p>{/if}

    {#if call.prefs.pushToTalk}
      <button
        class="talk"
        class:on={call.talking}
        disabled={call.deafened}
        onpointerdown={(e) => (e.currentTarget.setPointerCapture(e.pointerId), call.talk(true))}
        onpointerup={() => call.talk(false)}
        onpointercancel={() => call.talk(false)}
        oncontextmenu={(e) => e.preventDefault()}
      >
        <Mic size={20} />{call.talking ? tr("Talking…") : tr("Hold to talk")}
      </button>
    {/if}

    <div class="controls">
      <button class="ctl" class:off={!call.micOn} onclick={() => call.toggleMic()} aria-label={call.micOn ? tr("Mute") : tr("Unmute")}>
        {#if call.micOn}<Mic size={24} />{:else}<MicOff size={24} />{/if}
      </button>
      <button class="ctl" class:off={call.deafened} onclick={() => call.toggleDeafen()} aria-label={call.deafened ? tr("Undeafen") : tr("Deafen")}>
        {#if call.deafened}<HeadphoneOff size={24} />{:else}<Headphones size={24} />{/if}
      </button>
      <button class="ctl" class:on={call.camOn} onclick={() => call.toggleCamera()} aria-label={call.camOn ? tr("Turn camera off") : tr("Turn camera on")}>
        {#if call.camOn}<Video size={24} />{:else}<VideoOff size={24} />{/if}
      </button>
      {#if canShareScreen}
        <button class="ctl" class:on={call.screenOn} onclick={() => call.toggleScreen()} aria-label={tr("Share screen")}><MonitorUp size={24} /></button>
      {/if}
      <button class="ctl hangup" onclick={() => call.leave()} aria-label={tr("Leave call")}><PhoneOff size={24} /></button>
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
  /* The call's chat docked on the right (ChatsScreen), same width. */
  .call.side {
    right: var(--side-chat);
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
  .featured:fullscreen,
  .featured.immersive {
    border-radius: 0;
  }
  .featured.immersive {
    position: fixed;
    inset: 0;
    z-index: 80;
  }
  .featured :global(canvas),
  .featured video.contain {
    object-fit: contain;
  }
  .featured .face.big {
    display: grid;
    width: 100%;
    height: 100%;
    place-items: center;
    background: var(--surface-2);
  }
  .spot-actions {
    position: absolute;
    top: 10px;
    right: 10px;
    display: flex;
    gap: 6px;
    opacity: 0;
    transition: opacity 0.15s;
  }
  .featured:hover .spot-actions,
  .featured:focus-within .spot-actions,
  .featured.immersive .spot-actions {
    opacity: 1;
  }
  @media (pointer: coarse) {
    .spot-actions {
      opacity: 1;
    }
  }
  .spot-actions button {
    display: grid;
    width: 36px;
    height: 36px;
    place-items: center;
    border: none;
    border-radius: 50%;
    background: rgb(0 0 0 / 0.55);
    color: #fff;
    cursor: pointer;
  }
  .tile.focused {
    outline: 2px solid var(--accent-a);
    outline-offset: -2px;
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
  .options {
    margin-left: auto;
  }
  .chat {
    margin-left: auto;
  }
  .chat + .options {
    margin-left: 0;
  }
  .chat.open,
  .options.open {
    background: var(--accent);
    color: #fff;
  }
  .options-panel {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 6px 14px;
    border-radius: 16px;
    background: var(--surface);
  }
  .options-panel label {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 0;
    cursor: pointer;
  }
  .options-panel span {
    display: flex;
    flex: 1;
    flex-direction: column;
  }
  .options-panel small {
    color: var(--muted);
    font-size: 0.8rem;
  }
  .tile {
    cursor: pointer;
  }
  .name small {
    display: inline-flex;
    margin-left: 4px;
    opacity: 0.8;
  }
  .talk {
    display: flex;
    align-self: center;
    align-items: center;
    gap: 10px;
    padding: 16px 34px;
    border: none;
    border-radius: 999px;
    background: var(--surface-2);
    color: inherit;
    font-size: 1rem;
    font-weight: 600;
    touch-action: none;
    user-select: none;
    -webkit-user-select: none;
    cursor: pointer;
    transition: background 0.12s var(--ease), transform 0.12s var(--ease);
  }
  .talk.on {
    background: #31d158;
    color: #fff;
    transform: scale(1.05);
  }
  .talk:disabled {
    opacity: 0.4;
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
