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
  import { isAndroid, pipShape } from "$lib/backend";
  import Ear from "@lucide/svelte/icons/ear";
  import Volume2 from "@lucide/svelte/icons/volume-2";
  import { call } from "$lib/stores/call.svelte";
  import Maximize2 from "@lucide/svelte/icons/maximize-2";
  import Minimize2 from "@lucide/svelte/icons/minimize-2";
  import X from "@lucide/svelte/icons/x";
  import Lock from "@lucide/svelte/icons/lock";
  import Eye from "@lucide/svelte/icons/eye";
  import EyeOff from "@lucide/svelte/icons/eye-off";
  import Drum from "@lucide/svelte/icons/drum";
  import SoundboardPanel from "./SoundboardPanel.svelte";
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
  /** What can be shown: each person (camera, else avatar) and, apart, each shared screen, so
   * someone sharing can have either their screen or their face in the big spot. */
  type Item = { key: string; tile: Tile; what: Spot };
  const personItem = (t: Tile): Item => ({ key: t.id, tile: t, what: hasCamera(t) ? "camera" : "person" });
  const items = $derived(
    call.tiles.flatMap((t): Item[] => (hasScreen(t) ? [personItem(t), { key: `${t.id}:screen`, tile: t, what: "screen" }] : [personItem(t)])),
  );
  /** Clicked item key (Discord: click something to focus it); null = automatic. */
  let focus = $state<string | null>(null);
  /** The big spot: the focused item, or the first screen share. */
  const featured = $derived.by((): Item | null => {
    return (focus ? items.find((i) => i.key === focus) : undefined) ?? items.find((i) => i.what === "screen") ?? null;
  });

  // ── Layout (Discord / Teams): others share the stage in a grid sized for the window and
  // their videos' shapes (shown whole, on a blurred copy instead of black bars); our own
  // camera is a small self-view in a corner (drag it to another, or hide it) once anyone else
  // is there. ──
  const me = $derived(call.tiles.find((t) => t.isMe) ?? null);
  const others = $derived(call.tiles.filter((t) => !t.isMe));
  // (With someone in the big spot, everyone, us too, is in the strip below it.)
  const floatMe = $derived(!!me && others.length > 0 && !featured);
  const gridTiles = $derived(floatMe ? others : call.tiles);
  let stageW = $state(0);
  let stageH = $state(0);
  /** Width / height of each tile's video, once known. */
  let ratios = $state<Record<string, number>>({});
  function noteRatio(id: string, w: number, h: number) {
    if (!w || !h) return;
    const r = w / h;
    if (Math.abs((ratios[id] ?? 0) - r) > 0.01) ratios = { ...ratios, [id]: r };
  }
  const GAP = 10;
  // With a big spot: the strip goes on the right when the call is wider than tall (phone in
  // landscape, desktop), below it otherwise; the spot takes its picture's shape.
  let bodyW = $state(0);
  let bodyH = $state(0);
  const sideStrip = $derived(!!featured && bodyW > bodyH * 1.15);
  const spotRatio = $derived(
    featured?.what === "screen" ? (ratios[`${featured.tile.id}:screen`] ?? 16 / 9) : featured?.what === "camera" ? (ratios[featured.tile.id] ?? 16 / 9) : null,
  );
  /** Columns × rows that show the most of everyone (video area, avatars as squares). */
  const layout = $derived.by(() => {
    const n = gridTiles.length;
    const fallback = { cols: Math.ceil(Math.sqrt(n || 1)), w: 0, h: 0 };
    if (!n || !stageW || !stageH) return fallback;
    let best = { ...fallback, score: -1 };
    for (let cols = 1; cols <= n; cols++) {
      const rows = Math.ceil(n / cols);
      const w = (stageW - GAP * (cols - 1)) / cols;
      const h = (stageH - GAP * (rows - 1)) / rows;
      if (w < 90 || h < 90) continue;
      let score = 0;
      for (const t of gridTiles) {
        const r = ratios[t.id];
        if (r && (t.camera || t.cameraFrames)) {
          const vw = Math.min(w, h * r);
          score += vw * (vw / r);
        } else {
          const side = Math.min(w, h);
          score += side * side * 0.5;
        }
      }
      if (score > best.score) best = { cols, w, h, score };
    }
    return best;
  });

  type Corner = "tl" | "tr" | "bl" | "br";
  const SELF_KEY = "alwychat.selfView";
  const savedSelf = (() => {
    try {
      return JSON.parse(localStorage.getItem(SELF_KEY) ?? "{}") as { corner?: Corner; hidden?: boolean };
    } catch {
      return {};
    }
  })();
  let selfCorner = $state<Corner>(savedSelf.corner ?? "br");
  let selfHidden = $state(savedSelf.hidden ?? false);
  $effect(() => localStorage.setItem(SELF_KEY, JSON.stringify({ corner: selfCorner, hidden: selfHidden })));
  /** Dragging the self-view: its offset from where it sits. */
  let drag = $state<{ x: number; y: number; startX: number; startY: number; moved: boolean } | null>(null);
  function selfDown(e: PointerEvent) {
    if ((e.target as HTMLElement).closest("button")) return;
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    drag = { x: 0, y: 0, startX: e.clientX, startY: e.clientY, moved: false };
  }
  function selfMove(e: PointerEvent) {
    if (!drag) return;
    const x = e.clientX - drag.startX;
    const y = e.clientY - drag.startY;
    drag = { ...drag, x, y, moved: drag.moved || Math.hypot(x, y) > 6 };
  }
  function selfUp(e: PointerEvent) {
    if (!drag) return;
    const moved = drag.moved;
    drag = null;
    if (!moved) return;
    // Snap to the corner nearest to where it was let go.
    const stage = (e.currentTarget as HTMLElement).parentElement!.getBoundingClientRect();
    const left = e.clientX < stage.left + stage.width / 2;
    const top = e.clientY < stage.top + stage.height / 2;
    selfCorner = `${top ? "t" : "b"}${left ? "l" : "r"}` as Corner;
  }

  /** Picture-in-picture: whoever was focused, else a screen share, else the other person's
   * camera (the one speaking first), else them, else us (alone in the call). */
  const pipMain = $derived.by((): Item | Omit<Item, "key"> | null => {
    if (featured) return featured;
    const others = call.tiles.filter((t) => !t.isMe);
    const byVoice = [...others].sort((a, b) => Number(b.speaking) - Number(a.speaking));
    const tile = byVoice.find(hasCamera) ?? byVoice[0] ?? call.tiles.find((t) => t.isMe);
    if (!tile) return null;
    return { tile, what: hasCamera(tile) ? "camera" : "person" };
  });
  /** A camera in a corner: while someone shares their screen, their face (or another
   * person's), else ours when it's on and we aren't the main picture. */
  const pipInset = $derived.by(() => {
    const main = pipMain;
    if (!main) return null;
    const cams = call.tiles.filter((t) => hasCamera(t) && !(t.id === main.tile.id && main.what === "camera"));
    if (main.what === "screen") {
      const others = cams.filter((t) => !t.isMe);
      return others.find((t) => t.id === main.tile.id) ?? others.find((t) => t.speaking) ?? others[0] ?? cams[0] ?? null;
    }
    return cams.find((t) => t.isMe) ?? null;
  });
  // Android: a screen gets a wide picture-in-picture window.
  $effect(() => {
    if (isAndroid && call.active) void pipShape(pipMain?.what === "screen").catch(() => {});
  });
  function toggleFocus(key: string) {
    if (longPressed) return void (longPressed = false);
    focus = focus === key ? null : key;
  }
  /** Small tiles: with a big spot, everything else (screens too); otherwise the grid's people. */
  const stripItems = $derived(featured ? items.filter((i) => i.key !== featured.key) : gridTiles.map(personItem));

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

  const person = (t: (typeof call.tiles)[number]) => ({
    userId: t.userId,
    name: t.name,
    avatar: t.avatar,
    isMe: t.isMe,
    live: true,
  });
  let optionsOpen = $state(false);
  let soundsOpen = $state(false);
  // This view outlives calls: the next one starts with panels closed and nothing focused.
  $effect(() => {
    if (call.active) return;
    soundsOpen = optionsOpen = immersive = false;
    focus = null;
  });

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
  <!-- Closed: the controls are in CallDock. -->
{:else if call.active && call.pip}
  <!-- Android picture-in-picture: one picture filling the small window, like phone apps. -->
  {@const main = pipMain}
  <div class="pip-view">
    {#if main}
      {#if main.what === "screen" && main.tile.screen}
        <!-- svelte-ignore a11y_media_has_caption -->
        <video use:video={main.tile.screen} autoplay playsinline muted class="contain"></video>
      {:else if main.what === "screen" && main.tile.screenFrames}
        <NativeVideo frames={main.tile.screenFrames} contain />
      {:else if main.what === "camera" && main.tile.camera}
        <!-- svelte-ignore a11y_media_has_caption -->
        <video use:video={main.tile.camera} autoplay playsinline muted class:mirror={main.tile.isMe}></video>
      {:else if main.what === "camera" && main.tile.cameraFrames}
        <NativeVideo frames={main.tile.cameraFrames} mirror={main.tile.isMe} />
      {:else}
        <div class="pip-face" class:speaking={main.tile.speaking}>
          <Avatar name={main.tile.name} mxc={main.tile.avatar} size={64} />
        </div>
      {/if}
      <span class="pip-name">{#if !main.tile.micOn}<MicOff size={11} />{/if}{main.tile.isMe ? tr("You") : main.tile.name}</span>
    {/if}
    {#if pipInset}
      <div class="pip-inset">
        {#if pipInset.camera}
          <!-- svelte-ignore a11y_media_has_caption -->
          <video use:video={pipInset.camera} autoplay playsinline muted class:mirror={pipInset.isMe}></video>
        {:else if pipInset.cameraFrames}
          <NativeVideo frames={pipInset.cameraFrames} mirror={pipInset.isMe} />
        {/if}
      </div>
    {/if}
  </div>
{:else if call.active}
  <div class="call" class:side={call.sideChat} transition:fade={{ duration: 160 }}>
    <header data-tauri-drag-region="deep">
      <button class="icon" onclick={() => (call.minimized = true)} aria-label={tr("Back to chats")}><ChevronDown size={24} /></button>
      <div class="title">
        <strong>{call.roomName}</strong>
        <small>
          {#if call.encrypted}<span class="lock" title={tr("End-to-end encrypted")}><Lock size={12} /></span>{/if}
          {statusText ?? tr("{length} in call", { length: call.tiles.length })}
        </small>
      </div>
      <button class="icon chat" class:open={call.sideChat} onclick={() => call.toggleChat()} aria-label={tr("Chat")} title={tr("Chat")} aria-pressed={call.sideChat}>
        <MessageSquare size={20} />
      </button>
      <button class="icon sounds" class:open={soundsOpen} onclick={() => ((soundsOpen = !soundsOpen), (optionsOpen = false))} aria-label={tr("Soundboard")} title={tr("Soundboard")} aria-expanded={soundsOpen}>
        <Drum size={20} />
      </button>
      <button class="icon options" class:open={optionsOpen} onclick={() => (optionsOpen = !optionsOpen)} aria-label={tr("Call options")} aria-expanded={optionsOpen}>
        <SlidersHorizontal size={20} />
      </button>
    </header>
    {#if soundsOpen && call.roomId}
      <SoundboardPanel roomId={call.roomId} onClose={() => (soundsOpen = false)} />
    {/if}
    {#if optionsOpen}
      <div class="options-panel" transition:fly={{ y: -8, duration: 150 }}>
        <label>
          <span><strong>{tr("Noise suppression")}</strong><small>{tr("Filters background noise from your mic")}</small></span>
          <input type="checkbox" role="switch" checked={call.prefs.noiseSuppression} onchange={(e) => call.setNoiseSuppression(e.currentTarget.checked)} />
        </label>
        <label>
          <span><strong>{tr("Push to talk")}</strong><small>{tr("Your mic is open only while you hold the talk button{value}", { value: isAndroid ? "" : ` or ${keyName(call.prefs.pttKey, call.prefs.pttKeyLabel)}` })}</small></span>
          <input type="checkbox" role="switch" checked={call.prefs.pushToTalk} onchange={(e) => call.setPushToTalk(e.currentTarget.checked)} />
        </label>
      </div>
    {/if}

    <div class="body" class:side-strip={sideStrip} bind:clientWidth={bodyW} bind:clientHeight={bodyH}>
    {#if featured}
      {@const f = featured.tile}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="featured" class:immersive class:shaped={!sideStrip && spotRatio} style:--spot-ratio={spotRatio} bind:this={spot} ondblclick={toggleFullscreen}>
        <!-- svelte-ignore a11y_media_has_caption -->
        {#if featured.what === "screen"}
          {#if f.screen}
            <video
              use:video={f.screen}
              autoplay
              playsinline
              muted
              onresize={(e) => noteRatio(`${f.id}:screen`, e.currentTarget.videoWidth, e.currentTarget.videoHeight)}
              onloadedmetadata={(e) => noteRatio(`${f.id}:screen`, e.currentTarget.videoWidth, e.currentTarget.videoHeight)}
            ></video>
          {:else if f.screenFrames}
            <NativeVideo frames={f.screenFrames} contain onsize={(w, h) => noteRatio(`${f.id}:screen`, w, h)} />
          {/if}
        {:else if featured.what === "camera"}
          {#if f.camera}
            <video class="backdrop" aria-hidden="true" use:video={f.camera} autoplay playsinline muted class:mirror={f.isMe}></video>
            <video class="contain" use:video={f.camera} autoplay playsinline muted class:mirror={f.isMe}></video>
          {:else if f.cameraFrames}
            <div class="backdrop" aria-hidden="true"><NativeVideo frames={f.cameraFrames} mirror={f.isMe} /></div>
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

    {#snippet picture(t: Tile, small: boolean)}
      {#if t.cameraFrames}
        <div class="backdrop" aria-hidden="true"><NativeVideo frames={t.cameraFrames} mirror={t.isMe} /></div>
        <NativeVideo frames={t.cameraFrames} mirror={t.isMe} contain onsize={(w, h) => noteRatio(t.id, w, h)} />
      {:else if t.camera}
        <!-- svelte-ignore a11y_media_has_caption -->
        <video class="backdrop" aria-hidden="true" use:video={t.camera} autoplay playsinline muted class:mirror={t.isMe}></video>
        <!-- svelte-ignore a11y_media_has_caption -->
        <video
          class="contain"
          use:video={t.camera}
          autoplay
          playsinline
          muted
          class:mirror={t.isMe}
          onresize={(e) => noteRatio(t.id, e.currentTarget.videoWidth, e.currentTarget.videoHeight)}
          onloadedmetadata={(e) => noteRatio(t.id, e.currentTarget.videoWidth, e.currentTarget.videoHeight)}
        ></video>
      {:else}
        <div class="face">
          <div class="ring"><Avatar name={t.name} mxc={t.avatar} size={small ? 44 : featured ? 56 : 88} /></div>
        </div>
      {/if}
    {/snippet}

    <div class="stage" bind:clientWidth={stageW} bind:clientHeight={stageH} class:strip-stage={featured}>
      <div class="grid" class:strip={featured} style:--cols={layout.cols}>
        {#each stripItems as item (item.key)}
          {@const t = item.tile}
          <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
          <div
            class="tile"
            class:speaking={t.speaking && item.what !== "screen"}
            class:focused={focus === item.key}
            style:width={!featured && layout.w ? `${layout.w}px` : null}
            style:height={!featured && layout.h ? `${layout.h}px` : null}
            onclick={() => toggleFocus(item.key)}
            onpointerdown={(e) => pressStart(e, t)}
            onpointerup={pressEnd}
            onpointercancel={pressEnd}
            onpointerleave={pressEnd}
            oncontextmenu={(e) => (e.preventDefault(), (e as unknown as PointerEvent).pointerType !== "touch" && call.openMenu(person(t), e))}
            transition:fade={{ duration: 150 }}
          >
            {#if item.what === "screen"}
              {#if t.screen}
                <!-- svelte-ignore a11y_media_has_caption -->
                <video class="contain" use:video={t.screen} autoplay playsinline muted></video>
              {:else if t.screenFrames}
                <NativeVideo frames={t.screenFrames} contain />
              {/if}
              <span class="name"><MonitorUp size={13} />{t.isMe ? tr("Your screen") : tr("{name}'s screen", { name: t.name })}</span>
            {:else}
            {@render picture(t, false)}
            <span class="name">
              {#if t.isMe && call.deafened}<HeadphoneOff size={13} />{:else if !t.micOn}<MicOff size={13} />{/if}{t.isMe ? tr("You") : t.name}
              {#if !t.isMe && call.isMutedForMe(t.userId)}<small><VolumeX size={13} /></small>{:else if !t.isMe && call.volumeOf(t.userId) !== 1}<small>{Math.round(call.volumeOf(t.userId) * 100)}%</small>{/if}
            </span>
            {/if}
          </div>
        {/each}
      </div>

      {#if floatMe && me}
        {#if selfHidden}
          <button class="self-pill {selfCorner}" onclick={() => (selfHidden = false)} title={tr("Show my video")}>
            {#if !me.micOn}<MicOff size={13} />{/if}{tr("You")}<Eye size={14} />
          </button>
        {:else}
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div
            class="self {selfCorner}"
            class:speaking={me.speaking}
            class:dragging={drag}
            class:has-video={!!(me.camera || me.cameraFrames)}
            style:--ratio={ratios[me.id] ?? 16 / 9}
            style:translate={drag ? `${drag.x}px ${drag.y}px` : null}
            onpointerdown={selfDown}
            onpointermove={selfMove}
            onpointerup={selfUp}
            onpointercancel={() => (drag = null)}
            transition:fade={{ duration: 150 }}
          >
            {@render picture(me, true)}
            <span class="name">{#if call.deafened}<HeadphoneOff size={12} />{:else if !me.micOn}<MicOff size={12} />{/if}{tr("You")}</span>
            <button class="self-hide" onclick={() => (selfHidden = true)} aria-label={tr("Hide my video")} title={tr("Hide my video")}><EyeOff size={14} /></button>
          </div>
        {/if}
      {/if}
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

    </div>
    <div class="controls">
      <button class="ctl" class:off={!call.micOn} onclick={() => call.toggleMic()} aria-label={call.micOn ? tr("Mute") : tr("Unmute")}>
        {#if call.micOn}<Mic size={24} />{:else}<MicOff size={24} />{/if}
      </button>
      <button class="ctl" class:off={call.deafened} onclick={() => call.toggleDeafen()} aria-label={call.deafened ? tr("Undeafen") : tr("Deafen")}>
        {#if call.deafened}<HeadphoneOff size={24} />{:else}<Headphones size={24} />{/if}
      </button>
      {#if isAndroid}
        <button class="ctl" class:on={call.speaker} onclick={() => call.toggleSpeaker()} aria-pressed={call.speaker} aria-label={call.speaker ? tr("Speaker on") : tr("Speaker off")} title={call.speaker ? tr("Speaker on") : tr("Speaker off")}>
          {#if call.speaker}<Volume2 size={24} />{:else}<Ear size={24} />{/if}
        </button>
      {/if}
      <button class="ctl" class:on={call.camOn} onclick={() => call.toggleCamera()} aria-label={call.camOn ? tr("Turn camera off") : tr("Turn camera on")}>
        {#if call.camOn}<Video size={24} />{:else}<VideoOff size={24} />{/if}
      </button>
      {#if call.canShareScreen}
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
  /* Android picture-in-picture: the main picture fills the window, our camera in a corner. */
  .pip-view {
    position: fixed;
    inset: 0;
    z-index: 60;
    overflow: hidden;
    background: radial-gradient(circle at 50% 30%, color-mix(in srgb, var(--accent-a) 30%, #000), #000 80%);
  }
  .pip-view > video,
  .pip-view > :global(canvas) {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .pip-view > video.contain {
    object-fit: contain;
  }
  .pip-view video.mirror {
    transform: scaleX(-1);
  }
  .pip-face {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
  }
  .pip-face :global(.avatar) {
    border-radius: 50%;
    box-shadow: 0 0 0 3px transparent;
    transition: box-shadow 0.15s;
  }
  .pip-face.speaking :global(.avatar) {
    box-shadow: 0 0 0 3px #3ba55d;
  }
  .pip-name {
    position: absolute;
    left: 6px;
    bottom: 6px;
    display: inline-flex;
    align-items: center;
    gap: 3px;
    max-width: calc(100% - 12px);
    padding: 2px 6px;
    border-radius: 6px;
    background: rgb(0 0 0 / 0.55);
    color: #fff;
    font-size: 0.7rem;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .pip-inset {
    position: absolute;
    right: 6px;
    top: 6px;
    /* Also fits the wide window of a screen share. */
    width: min(32%, 40vh);
    aspect-ratio: 3 / 4;
    border-radius: 8px;
    overflow: hidden;
    background: #000;
    box-shadow: 0 2px 8px rgb(0 0 0 / 0.5);
  }
  .pip-inset video,
  .pip-inset :global(canvas) {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  /* The call's chat docked on the right (ChatsScreen), same width. */
  .call.side {
    right: var(--side-chat);
  }
  header {
    display: flex;
    align-items: center;
    gap: 10px;
    padding-right: var(--window-controls);
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
  .lock {
    display: inline-flex;
    vertical-align: -1px;
    margin-right: 3px;
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
  /* The big spot and the people: a column, or a row with the strip on the right. */
  .body {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 12px;
  }
  .body.side-strip {
    flex-direction: row;
  }
  .featured {
    position: relative;
    flex: 1;
    min-height: 0;
    min-width: 0;
    border-radius: 18px;
    overflow: hidden;
    background: black;
  }
  /* Shaped like its picture (no black bands), smaller if the space is short. */
  .featured.shaped {
    flex: 0 1 auto;
    width: 100%;
    aspect-ratio: var(--spot-ratio);
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
  .featured > :global(canvas),
  .featured video.contain {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
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
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: contain;
  }
  .featured video.mirror:not(.backdrop) {
    transform: scaleX(-1);
  }
  .stage {
    position: relative;
    flex: 1;
    min-height: 0;
    display: flex;
  }
  .stage.strip-stage {
    flex: none;
  }
  .grid {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    align-content: center;
    gap: 10px;
    overflow: hidden;
  }
  .grid.strip {
    flex-wrap: nowrap;
    justify-content: flex-start;
    overflow-x: auto;
  }
  .grid.strip .tile {
    flex: none;
    width: 120px;
    height: 120px;
  }
  .side-strip .stage.strip-stage {
    width: 168px;
  }
  .side-strip .grid.strip {
    flex-direction: column;
    align-content: stretch;
    overflow: hidden auto;
  }
  .side-strip .grid.strip .tile {
    width: 100%;
    height: auto;
    aspect-ratio: 4 / 3;
  }
  .tile {
    position: relative;
    display: grid;
    flex: none;
    place-items: center;
    border-radius: 18px;
    overflow: hidden;
    background: var(--surface);
    transition:
      box-shadow 0.15s var(--ease),
      width 0.2s var(--ease),
      height 0.2s var(--ease);
  }
  /* The video whole (contain), over a blurred, dimmed copy filling the tile. */
  .tile video,
  .self video,
  .tile :global(canvas),
  .self :global(canvas) {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: contain;
  }
  .backdrop,
  .backdrop :global(canvas) {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
  }
  .backdrop,
  .backdrop :global(canvas) {
    object-fit: cover !important;
    filter: blur(22px) brightness(0.55);
    transform: scale(1.15);
  }
  video.backdrop.mirror,
  .backdrop :global(canvas.mirror) {
    transform: scale(1.15) scaleX(-1);
  }
  /* Our own camera: a small self-view in a corner (Teams / FaceTime). */
  .self {
    --ratio: 1.78;
    position: absolute;
    z-index: 3;
    width: clamp(96px, 24%, 260px);
    aspect-ratio: var(--ratio);
    max-height: 45%;
    border-radius: 14px;
    overflow: hidden;
    background: var(--surface-2);
    box-shadow: 0 8px 28px rgb(0 0 0 / 0.45);
    touch-action: none;
    cursor: grab;
    transition:
      top 0.2s var(--ease),
      left 0.2s var(--ease),
      right 0.2s var(--ease),
      bottom 0.2s var(--ease);
  }
  .self:not(.has-video) {
    width: 104px;
    aspect-ratio: 1;
    display: grid;
    place-items: center;
  }
  .self.dragging {
    cursor: grabbing;
    transition: none;
  }
  .self.speaking {
    box-shadow:
      inset 0 0 0 3px #31d158,
      0 8px 28px rgb(0 0 0 / 0.45);
  }
  .self .name {
    left: 6px;
    bottom: 6px;
    padding: 2px 7px;
    font-size: 0.72rem;
  }
  .self-hide {
    position: absolute;
    top: 6px;
    right: 6px;
    display: grid;
    width: 26px;
    height: 26px;
    place-items: center;
    border: none;
    border-radius: 50%;
    background: rgb(0 0 0 / 0.55);
    color: #fff;
    cursor: pointer;
  }
  .self-pill {
    position: absolute;
    z-index: 3;
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 6px 11px;
    border: none;
    border-radius: 999px;
    background: rgb(0 0 0 / 0.6);
    color: #fff;
    font: inherit;
    font-size: 0.8rem;
    cursor: pointer;
  }
  .tl {
    top: 10px;
    left: 10px;
  }
  .tr {
    top: 10px;
    right: 10px;
  }
  .bl {
    bottom: 10px;
    left: 10px;
  }
  .br {
    bottom: 10px;
    right: 10px;
  }
  .tile.speaking {
    box-shadow: inset 0 0 0 3px #31d158;
  }
  .tile video.mirror:not(.backdrop),
  .self video.mirror:not(.backdrop) {
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
  .chat ~ .options {
    margin-left: 0;
  }
  .chat.open,
  .sounds.open,
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
