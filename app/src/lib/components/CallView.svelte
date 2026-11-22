<script lang="ts">
  import { tr, trn } from "$lib/i18n/index.svelte";
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
  import { onDestroy, untrack } from "svelte";
  import type { Track } from "livekit-client";
  import { fullScreen, isAndroid, isTauri, pipShape, type AudioRoute, type NativeScreenSound, type SoundChoice } from "$lib/backend";
  import Gauge from "@lucide/svelte/icons/gauge";
  import AppWindow from "@lucide/svelte/icons/app-window";
  import Volume2 from "@lucide/svelte/icons/volume-2";
  import SwitchCamera from "@lucide/svelte/icons/switch-camera";
  import { call, SCREEN_CHOICES } from "$lib/stores/call.svelte";
  import Maximize2 from "@lucide/svelte/icons/maximize-2";
  import Minimize2 from "@lucide/svelte/icons/minimize-2";
  import X from "@lucide/svelte/icons/x";
  import Lock from "@lucide/svelte/icons/lock";
  import Eye from "@lucide/svelte/icons/eye";
  import EyeOff from "@lucide/svelte/icons/eye-off";
  import ReactionsPanel from "./ReactionsPanel.svelte";
  import SoundboardPanel from "./SoundboardPanel.svelte";
  import Drum from "@lucide/svelte/icons/drum";
  import Bluetooth from "@lucide/svelte/icons/bluetooth";
  import Check from "@lucide/svelte/icons/check";
  import Smartphone from "@lucide/svelte/icons/smartphone";
  import RingPanel from "./RingPanel.svelte";
  import PhoneCall from "@lucide/svelte/icons/phone-call";
  import LoaderCircle from "@lucide/svelte/icons/loader-circle";
  import SmilePlus from "@lucide/svelte/icons/smile-plus";
  import { onBack } from "$lib/back.svelte";
  import Avatar from "./Avatar.svelte";
  import { keyName } from "$lib/keys";
  import NativeVideo from "./NativeVideo.svelte";
  import ScreenPicker from "./ScreenPicker.svelte";

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
          ? // Their AlwyChat devices say when they ring; other apps don't, so it stays "Calling…".
            call.ringingDevices.length > 1
            ? trn(call.ringingDevices.length, "Ringing on {n} device", "Ringing on {n} devices")
            : call.ringingDevices.length
              ? tr("Ringing…")
              : tr("Calling…")
          : null,
  );
  /** Ringing people in: group calls and voice channels (a 1:1 call already rings). */
  // Voice channels and group calls (a voice channel isn't always in the chat list).
  const canRing = $derived(!!call.roomId && !call.isDm);
  type Tile = (typeof call.tiles)[number];
  type Spot = "screen" | "camera" | "person";
  const hasScreen = (t: Tile) => !!(t.screen || t.screenFrames || t.sharing);
  /** A share we see the picture of (not ours hidden, not one we stopped watching). */
  const watched = (t: Tile) => (t.isMe ? call.showMyScreen && !call.screenPending && !call.screenStarting : !call.unwatched.has(t.id));
  const hasCamera = (t: Tile) => !!(t.camera || t.cameraFrames);
  /** What can be shown: each person (camera, else avatar) and, apart, each shared screen, so
   * someone sharing can have either their screen or their face in the big spot. */
  type Item = { key: string; tile: Tile; what: Spot };
  const personItem = (t: Tile): Item => ({ key: t.id, tile: t, what: hasCamera(t) ? "camera" : "person" });
  const items = $derived(
    // Our own screen is a card with the share's controls (its picture only when asked for).
    call.tiles.flatMap((t): Item[] =>
      hasScreen(t) || (t.isMe && call.screenOn) ? [personItem(t), { key: `${t.id}:screen`, tile: t, what: "screen" }] : [personItem(t)],
    ),
  );
  /** Clicked item key (Discord: click something to focus it); null = automatic. */
  let focus = $state<string | null>(null);
  /** The big spot: the focused item, or the first screen share. */
  const featured = $derived.by((): Item | null => {
    // Automatically: someone else's screen we watch (our own card stays in the grid).
    return (focus ? items.find((i) => i.key === focus) : undefined) ?? items.find((i) => i.what === "screen" && !i.tile.isMe && watched(i.tile)) ?? null;
  });

  // ── Layout (Discord / Teams): others share the stage in a grid sized for the window and
  // their videos' shapes (shown whole, on a blurred copy instead of black bars); our own
  // camera is a small self-view in a corner (drag it to another, or hide it) once anyone else
  // is there. ──
  const me = $derived(call.tiles.find((t) => t.isMe) ?? null);
  const others = $derived(call.tiles.filter((t) => !t.isMe));
  // (With someone in the big spot, everyone, us too, is in the strip below it.)
  /** People being rung who haven't joined: tiles of their own ("Calling…" / "Ringing…"). */
  const waiting = $derived(call.waiting);
  const floatMe = $derived(!!me && (others.length > 0 || waiting.length > 0) && !featured);
  const gridTiles = $derived(floatMe ? others : call.tiles);
  /** The grid without a big spot: people, then the screen cards (ours, ones not watched). */
  const gridItems = $derived([...gridTiles.map(personItem), ...items.filter((i) => i.what === "screen")]);
  let stageW = $state(0);
  let stageH = $state(0);
  /** Width / height of each tile's video, once known. */
  let ratios = $state<Record<string, number>>({});

  // ── Someone's camera is on but no picture came after a few seconds: usually their media
  // can't be decrypted (their app never sent us its key). Say so, instead of a grey tile. ──
  let cameraSince = $state<Record<string, number>>({});
  let clockNow = $state(Date.now());
  $effect(() => {
    const before = untrack(() => cameraSince);
    const next: Record<string, number> = {};
    for (const t of call.tiles) if (!t.isMe && (t.camera || t.cameraFrames) && !ratios[t.id]) next[t.id] = before[t.id] ?? Date.now();
    const same = Object.keys(next).length === Object.keys(before).length && Object.keys(next).every((k) => before[k] === next[k]);
    if (!same) cameraSince = next;
  });
  $effect(() => {
    if (!Object.keys(cameraSince).length) return;
    const timer = setInterval(() => (clockNow = Date.now()), 1000);
    return () => clearInterval(timer);
  });
  const noPicture = (t: Tile) => cameraSince[t.id] !== undefined && clockNow - cameraSince[t.id] > 3000;
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
    const n = gridItems.length + (featured ? 0 : waiting.length);
    const fallback = { cols: Math.ceil(Math.sqrt(n || 1)), w: 0, h: 0 };
    if (!n || !stageW || !stageH) return fallback;
    let best = { ...fallback, score: -1 };
    for (let cols = 1; cols <= n; cols++) {
      const rows = Math.ceil(n / cols);
      const w = (stageW - GAP * (cols - 1)) / cols;
      const h = (stageH - GAP * (rows - 1)) / rows;
      if (w < 90 || h < 90) continue;
      let score = 0;
      for (const { tile: t, what } of gridItems) {
        const r = what === "screen" ? (ratios[`${t.id}:screen`] ?? 16 / 9) : ratios[t.id];
        if (r && (what === "screen" || t.camera || t.cameraFrames)) {
          const vw = Math.min(w, h * r);
          score += vw * (vw / r);
        } else {
          const side = Math.min(w, h);
          score += side * side * 0.5;
        }
      }
      if (!featured) score += waiting.length * Math.min(w, h) ** 2 * 0.5;
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
    // A tap (no drag): our picture goes to the big spot, like a tap on anyone's tile.
    if (!moved) return void (me && toggleFocus(me.id));
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
  // Android: a screen or a landscape camera gets a wide picture-in-picture window (a portrait
  // window cropped the sides off them).
  $effect(() => {
    const main = pipMain;
    const wide = main?.what === "screen" || (main?.what === "camera" && (ratios[main.tile.id] ?? 0) > 1.1);
    if (isAndroid && call.active) void pipShape(wide).catch(() => {});
  });
  /** Our front camera shows as in a mirror; the back one (and everyone else's) as filmed. */
  const mirrored = (t: { isMe: boolean }) => t.isMe && call.facing === "user";
  function toggleFocus(key: string) {
    if (longPressed) return void (longPressed = false);
    focus = focus === key ? null : key;
  }
  /** Small tiles: with a big spot, everything else (screens too); otherwise the grid's people. */
  const stripItems = $derived(featured ? items.filter((i) => i.key !== featured.key) : gridItems);

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
    // The apps fill the window and make it full screen themselves (Android's webview has no
    // element full screen; WebKitGTK's left no way out of it).
    if (isTauri) return void (immersive = true);
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
  // The big picture went away (share stopped, person left) or the call shrank to its pill:
  // leave full screen with it.
  $effect(() => {
    if ((!featured || call.minimized) && immersive) immersive = false;
  });
  // Apps: the window goes full screen (Android: system bars hidden, the picture turns with the
  // phone). Esc leaves it on a keyboard.
  let systemFull = false;
  // The window's own buttons (TitleBar) step aside: they'd cover the call's top controls.
  $effect(() => {
    document.documentElement.toggleAttribute("data-call-fullscreen", immersive);
    return () => document.documentElement.removeAttribute("data-call-fullscreen");
  });
  $effect(() => {
    if (!isTauri || immersive === systemFull) return;
    systemFull = immersive;
    void fullScreen(immersive).catch(() => {});
  });
  $effect(() => {
    if (!immersive) return;
    const key = (e: KeyboardEvent) => e.key === "Escape" && (immersive = false);
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  });
  // Full screen: the call's controls float over the picture while wanted: on a computer when
  // the mouse moves (gone after 3 s still, the pointer with them), on a phone a tap shows or
  // hides them (gone after 3 s too). Kept while the pointer rests on them or a menu is open.
  let chrome = $state(true);
  let chromeTimer: ReturnType<typeof setTimeout> | undefined;
  let overControls = false;
  function showChrome() {
    chrome = true;
    clearTimeout(chromeTimer);
    chromeTimer = setTimeout(() => {
      if (!overControls && !audioMenu) chrome = false;
      else showChrome();
    }, 3000);
  }
  $effect(() => {
    if (immersive) untrack(showChrome);
    else {
      clearTimeout(chromeTimer);
      chrome = true;
    }
  });
  function immersiveMove(e: PointerEvent) {
    if (immersive && e.pointerType === "mouse") showChrome();
  }
  function immersiveTap(e: MouseEvent) {
    if (!immersive || (e.target as Element).closest("button, [role=menu]")) return;
    if (chrome && lastPointer !== "mouse") {
      clearTimeout(chromeTimer);
      chrome = false;
    } else showChrome();
  }
  let lastPointer = "mouse";
  onDestroy(() => clearTimeout(chromeTimer));
  onDestroy(() => {
    if (systemFull) void fullScreen(false).catch(() => {});
  });

  const screenChoice = $derived(SCREEN_CHOICES.find((c) => c.id === call.prefs.screenQuality) ?? SCREEN_CHOICES[2]);

  /** The screen sound menu: "all", "none", "app:<id>" (an app picked earlier that stopped
   * playing stays listed). */
  const soundValue = (c: SoundChoice) => (c.kind === "app" ? `app:${c.app}` : c.kind);
  const soundChoice = (v: string): SoundChoice => (v.startsWith("app:") ? { kind: "app", app: v.slice(4) } : { kind: v === "none" ? "none" : "all" });
  const soundApps = (s: NativeScreenSound) =>
    s.choice.kind === "app" && !s.apps.some((a) => a.id === (s.choice as { app: string }).app) ? [...s.apps, { id: s.choice.app, name: s.choice.app }] : s.apps;
  // The apps playing change while sharing (a game starts its sound…).
  $effect(() => {
    if (!call.screenOn || call.screenPending || call.screenStarting || !call.native) return;
    void call.refreshScreenSound();
    const timer = setInterval(() => void call.refreshScreenSound(), 2000);
    return () => clearInterval(timer);
  });

  const person = (t: (typeof call.tiles)[number]) => ({
    userId: t.userId,
    name: t.name,
    avatar: t.avatar,
    isMe: t.isMe,
    live: true,
  });
  /** A tap outside the soundboard, ring or options panel closes it (its own header button
   * toggles it, sheets and dialogs it opens count as inside). */
  function closePanels(e: PointerEvent) {
    const target = e.target as Element | null;
    if (audioMenu && !target?.closest(".audio-out")) audioMenu = false;
    if (cardMenu && !target?.closest(".pick-wrap")) cardMenu = null;
    if (screenVolFor && !target?.closest(".screen-vol")) screenVolFor = null;
    if (!soundsOpen && !optionsOpen && !reactOpen && !call.ringPanel) return;
    if (target?.closest(".panel, .options-panel, .icon.sounds, .icon.ring, .icon.options, .icon.react, [role=dialog]")) return;
    soundsOpen = optionsOpen = reactOpen = call.ringPanel = false;
  }
  let optionsOpen = $state(false);
  /** The share card's quality / sound menu. */
  let cardMenu = $state<"quality" | "sound" | null>(null);
  /** Whose screen's volume slider is open. */
  let screenVolFor = $state<string | null>(null);
  $effect(() => {
    if (!call.screenOn) cardMenu = null;
  });
  let soundsOpen = $state(false);
  /** The audio output menu (phone with a headset connected). */
  let audioMenu = $state(false);
  function routeName(r: AudioRoute) {
    if (r.kind === "earpiece") return tr("Phone");
    if (r.kind === "speaker") return tr("Speaker");
    if (r.kind === "bluetooth") return (call.prefs.btNames && r.name) || tr("Bluetooth");
    return r.name || tr("Headphones");
  }
  /** The reactions panel (raise hand, emoji reactions, sounds). */
  let reactOpen = $state(false);
  /** Raised hands in order (first raised first), for the "✋ 2" on tiles. */
  const handOrder = $derived(Object.entries(call.hands).sort((a, b) => a[1] - b[1]).map(([user]) => user));
  // This view outlives calls: the next one starts with panels closed and nothing focused.
  $effect(() => {
    if (call.active) return;
    soundsOpen = optionsOpen = reactOpen = call.ringPanel = immersive = audioMenu = false;
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

<svelte:window onkeydown={(e) => onKey(e, true)} onkeyup={(e) => onKey(e, false)} onblur={() => call.talk(false)} onpointerdown={closePanels} />

{#if call.screenPicker}
  <ScreenPicker />
{/if}
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
        <video use:video={main.tile.camera} autoplay playsinline muted class:mirror={mirrored(main.tile)}></video>
      {:else if main.what === "camera" && main.tile.cameraFrames}
        <NativeVideo frames={main.tile.cameraFrames} mirror={mirrored(main.tile)} />
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
          <video use:video={pipInset.camera} autoplay playsinline muted class:mirror={mirrored(pipInset)}></video>
        {:else if pipInset.cameraFrames}
          <NativeVideo frames={pipInset.cameraFrames} mirror={mirrored(pipInset)} />
        {/if}
      </div>
    {/if}
  </div>
{:else if call.active}
  <div
    class="call"
    class:side={call.sideChat}
    class:full={immersive}
    class:bare={immersive && !chrome}
    transition:fade={{ duration: 160 }}
    onpointermove={immersiveMove}
    onpointerdown={(e) => (lastPointer = e.pointerType)}
    onclick={immersiveTap}
    role="presentation"
  >
    <div class="head">
    <header data-tauri-drag-region="deep">
      <button class="icon" onclick={() => (call.minimized = true)} aria-label={tr("Back to chats")}><ChevronDown size={24} /></button>
      <div class="title">
        <strong>{call.roomName}</strong>
        <small>
          {#if call.encrypted}<span class="lock" title={tr("End-to-end encrypted")}><Lock size={12} /></span>{/if}
          {statusText ?? tr("{length} in call", { length: call.tiles.length })}{#if !statusText && call.duration}<span class="duration">{" · "}{call.duration}</span>{/if}
        </small>
      </div>
      <button class="icon chat" class:open={call.sideChat} onclick={() => call.toggleChat()} aria-label={tr("Chat")} title={tr("Chat")} aria-pressed={call.sideChat}>
        <MessageSquare size={20} />
      </button>
      {#if canRing}
        <button class="icon ring" class:open={call.ringPanel} onclick={() => ((call.ringPanel = !call.ringPanel), (soundsOpen = optionsOpen = reactOpen = false))} aria-label={tr("Ring someone")} title={tr("Ring someone")} aria-expanded={call.ringPanel}>
          <PhoneCall size={20} />
        </button>
      {/if}
      <button class="icon react" class:open={reactOpen} class:raised={call.handUp} onclick={() => ((reactOpen = !reactOpen), (soundsOpen = optionsOpen = call.ringPanel = false))} aria-label={tr("Reactions")} title={tr("Reactions")} aria-expanded={reactOpen}>
        {#if call.handUp}<span class="hand-icon">✋</span>{:else}<SmilePlus size={20} />{/if}
      </button>
      <button class="icon sounds" class:open={soundsOpen} onclick={() => ((soundsOpen = !soundsOpen), (optionsOpen = reactOpen = call.ringPanel = false))} aria-label={tr("Soundboard")} title={tr("Soundboard")} aria-expanded={soundsOpen}>
        <Drum size={20} />
      </button>
      <button class="icon options" class:open={optionsOpen} onclick={() => ((optionsOpen = !optionsOpen), (soundsOpen = reactOpen = call.ringPanel = false))} aria-label={tr("Call options")} aria-expanded={optionsOpen}>
        <SlidersHorizontal size={20} />
      </button>
    </header>
    {#if call.ringPanel}
      <RingPanel onClose={() => (call.ringPanel = false)} />
    {/if}
    {#if reactOpen}
      <ReactionsPanel onClose={() => (reactOpen = false)} />
    {/if}
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
    </div>

    <div class="body" class:side-strip={sideStrip} bind:clientWidth={bodyW} bind:clientHeight={bodyH}>
    {#if featured}
      {@const f = featured.tile}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="featured" class:immersive class:shaped={!sideStrip && spotRatio} style:--spot-ratio={spotRatio} bind:this={spot} ondblclick={toggleFullscreen}>
        <!-- svelte-ignore a11y_media_has_caption -->
        {#if featured.what === "screen"}
          {@render screenCard(f, true)}
        {:else if featured.what === "camera"}
          {#if f.camera}
            <video class="backdrop" aria-hidden="true" use:video={f.camera} autoplay playsinline muted class:mirror={mirrored(f)}></video>
            <video class="contain" use:video={f.camera} autoplay playsinline muted class:mirror={mirrored(f)}></video>
          {:else if f.cameraFrames}
            <div class="backdrop" aria-hidden="true"><NativeVideo frames={f.cameraFrames} mirror={mirrored(f)} /></div>
            <NativeVideo frames={f.cameraFrames} mirror={mirrored(f)} contain />
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
          {#if featured.what === "screen" && !f.isMe && watched(f)}
            {@render remoteScreenButtons(f)}
          {/if}
          {#if f.isMe && featured.what === "camera" && call.canFlip}
            <button onclick={() => call.flipCamera()} aria-label={tr("Switch camera")} title={tr("Switch camera")}><SwitchCamera size={18} /></button>
          {/if}
          <button onclick={toggleFullscreen} aria-label={tr("Full screen")} title={tr("Full screen")}>
            {#if immersive}<Minimize2 size={18} />{:else}<Maximize2 size={18} />{/if}
          </button>
          {#if focus}
            <button onclick={() => (focus = null)} aria-label={tr("Stop focusing")} title={tr("Stop focusing")}><X size={18} /></button>
          {/if}
        </div>
      </div>
    {/if}

    {#snippet screenCard(t: Tile, big: boolean)}
      {#if watched(t) && t.screen}
        <!-- svelte-ignore a11y_media_has_caption -->
        <video
          class="contain"
          use:video={t.screen}
          autoplay
          playsinline
          muted
          onresize={(e) => noteRatio(`${t.id}:screen`, e.currentTarget.videoWidth, e.currentTarget.videoHeight)}
          onloadedmetadata={(e) => noteRatio(`${t.id}:screen`, e.currentTarget.videoWidth, e.currentTarget.videoHeight)}
        ></video>
      {:else if watched(t) && t.screenFrames}
        <NativeVideo frames={t.screenFrames} contain onsize={(w, h) => noteRatio(`${t.id}:screen`, w, h)} />
      {:else if t.isMe}
        <div class="screen-card">
          <MonitorUp size={big ? 36 : 26} />
          {#if big || !featured}<strong>{call.screenPending ? tr("Choose what to share…") : call.screenStarting ? tr("Starting your share…") : tr("You're sharing your screen")}</strong>{/if}
          {#if call.screenPending}<small>{tr("Nothing is sent until you pick a screen or a window.")}</small>{:else if call.screenNote}<small>{call.screenNote}</small>{/if}
          {#if call.screenStats && (big || !featured)}
            {@const st = call.screenStats}
            <small class="send-stats" class:held={st.limit === "cpu" || st.limit === "bandwidth"}>
              {[st.encoder, `${Math.round(st.fps)} fps`, `${st.width}×${st.height}`, `${st.mbps.toFixed(1)} Mbit/s`].filter(Boolean).join(" · ")}
              {#if st.limit === "cpu"}<br />{tr("Held back by the processor: try another codec or a lighter quality")}{:else if st.limit === "bandwidth"}<br />{tr("Held back by the connection: try less data use")}{/if}
            </small>
          {/if}
        </div>
      {:else}
        <div class="screen-card">
          <Avatar name={t.name} mxc={t.avatar} size={big ? 56 : 36} />
          <strong>{tr("{name} is sharing their screen", { name: t.name })}</strong>
          <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
          <div class="card-controls" onclick={(e) => e.stopPropagation()} onpointerdown={(e) => e.stopPropagation()}>
            <button class="primary" onclick={() => call.watchScreen(t.id, true)}><Eye size={16} />{tr("Watch")}</button>
          </div>
        </div>
      {/if}
      {#if t.isMe && !call.screenPending && !call.screenStarting && (big || !featured)}
        <!-- The share's controls stay on the card, its picture shown or not (not on a small
             strip tile: a tap brings it to the big spot, with them). -->
        <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
        <div class="card-controls own" class:over={watched(t)} class:open={cardMenu} onclick={(e) => e.stopPropagation()} onpointerdown={(e) => e.stopPropagation()}>
          <button onclick={() => call.setShowMyScreen(!call.showMyScreen)} title={call.showMyScreen ? tr("Hide it") : tr("Show it")}>
            {#if call.showMyScreen}<EyeOff size={16} />{:else}<Eye size={16} />{/if}<span>{call.showMyScreen ? tr("Hide it") : tr("Show it")}</span>
          </button>
          <div class="pick-wrap">
            <button class="pick" class:open={cardMenu === "quality"} aria-haspopup="menu" aria-expanded={cardMenu === "quality"} title={tr("Quality")} onclick={() => (cardMenu = cardMenu === "quality" ? null : "quality")}>
              <Gauge size={16} /><span>{screenChoice.label()}</span><ChevronDown size={14} class="chev" />
            </button>
            {#if cardMenu === "quality"}
              <div class="card-menu" role="menu" transition:fly={{ y: 6, duration: 120 }}>
                {#each SCREEN_CHOICES as c (c.id)}
                  <button role="menuitemradio" aria-checked={c.id === screenChoice.id} class:current={c.id === screenChoice.id} onclick={() => ((cardMenu = null), call.setScreenQuality(c.id))}>
                    <span class="item"><strong>{c.label()}</strong><small>{c.hint()}</small></span>
                    {#if c.id === screenChoice.id}<Check size={16} />{/if}
                  </button>
                {/each}
              </div>
            {/if}
          </div>
          {#if call.screenSound}
            {@const sound = call.screenSound}
            {@const current = soundValue(sound.choice)}
            {@const options = [
              { value: "all", label: tr("All apps' sound") },
              { value: "none", label: tr("No sound") },
              ...soundApps(sound).map((a) => ({ value: `app:${a.id}`, label: a.name })),
            ]}
            <div class="pick-wrap">
              <button
                class="pick"
                class:open={cardMenu === "sound"}
                class:hint={sound.choice.kind === "none" && sound.apps.length > 0}
                aria-haspopup="menu"
                aria-expanded={cardMenu === "sound"}
                title={tr("Which sound goes with your screen (never the call's voices)")}
                onclick={() => (cardMenu === "sound" ? (cardMenu = null) : ((cardMenu = "sound"), call.refreshScreenSound()))}
              >
                {#if sound.choice.kind === "none"}<VolumeX size={16} />{:else}<Volume2 size={16} />{/if}
                <span>{options.find((o) => o.value === current)?.label ?? tr("Sound")}</span><ChevronDown size={14} class="chev" />
              </button>
              {#if cardMenu === "sound"}
                <div class="card-menu" role="menu" transition:fly={{ y: 6, duration: 120 }}>
                  <p class="menu-title">{tr("Sound sent with your screen")}</p>
                  {#each options as o (o.value)}
                    <button role="menuitemradio" aria-checked={o.value === current} class:current={o.value === current} onclick={() => ((cardMenu = null), call.setScreenSound(soundChoice(o.value)))}>
                      {#if o.value === "none"}<VolumeX size={16} />{:else if o.value === "all"}<Volume2 size={16} />{:else}<AppWindow size={16} />{/if}
                      <span class="item"><strong>{o.label}</strong></span>
                      {#if o.value === current}<Check size={16} />{/if}
                    </button>
                  {/each}
                  {#if !sound.apps.length}<p class="menu-note">{tr("No other app is playing sound right now.")}</p>{/if}
                </div>
              {/if}
            </div>
          {/if}
          <button class="stop" onclick={() => call.toggleScreen()} title={tr("Stop sharing")}><X size={16} /><span>{tr("Stop")}</span></button>
        </div>
      {/if}
    {/snippet}

    {#snippet remoteScreenButtons(t: Tile)}
      {@const off = call.screenSoundOff.has(t.id)}
      <div class="screen-vol" role="presentation" onclick={(e) => e.stopPropagation()} onpointerdown={(e) => e.stopPropagation()}>
        <button class:open={screenVolFor === t.id} aria-haspopup="dialog" aria-expanded={screenVolFor === t.id} onclick={() => (screenVolFor = screenVolFor === t.id ? null : t.id)} aria-label={tr("Screen volume")} title={tr("Screen volume")}>
          {#if off}<VolumeX size={18} />{:else}<Volume2 size={18} />{/if}
        </button>
        {#if screenVolFor === t.id}
          <div class="vol-pop" transition:fly={{ y: -6, duration: 120 }}>
            <button class="mute" onclick={() => call.setScreenSoundOff(t.id, !off)} aria-label={off ? tr("Hear their screen") : tr("Mute their screen")} title={off ? tr("Hear their screen") : tr("Mute their screen")}>
              {#if off}<VolumeX size={18} />{:else}<Volume2 size={18} />{/if}
            </button>
            <input type="range" min="0" max="100" step="1" aria-label={tr("Screen volume")} value={off ? 0 : Math.round(call.screenVolumeOf(t.id) * 100)} oninput={(e) => call.setScreenVolume(t.id, Number(e.currentTarget.value) / 100)} />
            <span>{off ? 0 : Math.round(call.screenVolumeOf(t.id) * 100)}%</span>
          </div>
        {/if}
      </div>
      <button onclick={(e) => (e.stopPropagation(), focus === `${t.id}:screen` && (focus = null), call.watchScreen(t.id, false))} aria-label={tr("Stop watching")} title={tr("Stop watching")}><EyeOff size={18} /></button>
    {/snippet}

    {#snippet picture(t: Tile, small: boolean)}
      {#if t.cameraFrames}
        <div class="backdrop" aria-hidden="true"><NativeVideo frames={t.cameraFrames} mirror={mirrored(t)} /></div>
        <NativeVideo frames={t.cameraFrames} mirror={mirrored(t)} contain onsize={(w, h) => noteRatio(t.id, w, h)} />
      {:else if t.camera}
        <!-- svelte-ignore a11y_media_has_caption -->
        <video class="backdrop" aria-hidden="true" use:video={t.camera} autoplay playsinline muted class:mirror={mirrored(t)}></video>
        <!-- svelte-ignore a11y_media_has_caption -->
        <video
          class="contain"
          use:video={t.camera}
          autoplay
          playsinline
          muted
          class:mirror={mirrored(t)}
          onresize={(e) => noteRatio(t.id, e.currentTarget.videoWidth, e.currentTarget.videoHeight)}
          onloadedmetadata={(e) => noteRatio(t.id, e.currentTarget.videoWidth, e.currentTarget.videoHeight)}
        ></video>
      {/if}
      {#if (t.camera || t.cameraFrames) && noPicture(t)}
        <div class="no-picture">
          <Avatar name={t.name} mxc={t.avatar} size={small ? 32 : 48} />
          {#if !small}
            <strong>{tr("Can't show {name}'s video", { name: t.name })}</strong>
            <small>{tr("Their app hasn't sent the key to decrypt it.")}</small>
          {/if}
        </div>
      {:else if !t.camera && !t.cameraFrames}
        <div class="face">
          <div class="ring"><Avatar name={t.name} mxc={t.avatar} size={small ? 44 : featured ? 56 : 88} /></div>
        </div>
      {/if}
      {#if call.hands[t.userId]}
        {@const place = handOrder.indexOf(t.userId) + 1}
        <span class="hand-badge" class:small title={tr("Hand raised")}>✋{#if handOrder.length > 1}<b>{place}</b>{/if}</span>
      {/if}
      {#if call.reactions[t.userId]}
        {@const r = call.reactions[t.userId]}
        {#key r.at}<span class="reaction-pop" class:small aria-hidden="true">{r.emoji}</span>{/key}
      {/if}
    {/snippet}

    <div class="stage" bind:clientWidth={stageW} bind:clientHeight={stageH} class:strip-stage={featured}>
      <div class="grid" class:strip={featured} style:--cols={layout.cols}>
        {#each stripItems as item (item.key)}
          {@const t = item.tile}
          <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
          <div
            class="tile"
            class:lost={t.lost}
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
              {@render screenCard(t, false)}
              {#if !t.isMe && watched(t)}
                <div class="tile-actions">{@render remoteScreenButtons(t)}</div>
              {/if}
              {#if watched(t)}
                <span class="name"><MonitorUp size={13} />{t.isMe ? tr("Your screen") : tr("{name}'s screen", { name: t.name })}</span>
              {/if}
            {:else}
            {@render picture(t, false)}
            {#if t.lost}
              <span class="state lost-state"><LoaderCircle size={14} />{tr("Reconnecting…")}</span>
            {/if}
            <span class="name">
              {#if t.isMe && call.deafened}<HeadphoneOff size={13} />{:else if !t.micOn}<MicOff size={13} />{/if}{t.isMe ? tr("You") : t.name}
              {#if !t.isMe && call.isMutedForMe(t.userId)}<small><VolumeX size={13} /></small>{/if}
            </span>
            {/if}
          </div>
        {/each}
        {#each waiting as w (w.userId)}
          <div
            class="tile waiting"
            style:width={!featured && layout.w ? `${layout.w}px` : null}
            style:height={!featured && layout.h ? `${layout.h}px` : null}
            transition:fade={{ duration: 150 }}
          >
            <div class="face">
              <div class="ring pulse"><Avatar name={w.name} mxc={w.avatar} size={featured ? 56 : 88} /></div>
            </div>
            <span class="state">{w.status === "ringing" ? tr("Ringing…") : tr("Calling…")}</span>
            <span class="name">{w.name}</span>
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
            {#if call.canFlip}
              <button class="self-flip" onclick={() => call.flipCamera()} aria-label={tr("Switch camera")} title={tr("Switch camera")}><SwitchCamera size={14} /></button>
            {/if}
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
    <div class="controls" role="toolbar" tabindex="-1" onpointerenter={() => (overControls = true)} onpointerleave={() => (overControls = false)}>
      <button class="ctl" class:off={!call.micOn} onclick={() => call.toggleMic()} aria-label={call.micOn ? tr("Mute") : tr("Unmute")}>
        {#if call.micOn}<Mic size={24} />{:else}<MicOff size={24} />{/if}
      </button>
      <button class="ctl" class:off={call.deafened} onclick={() => call.toggleDeafen()} aria-label={call.deafened ? tr("Undeafen") : tr("Deafen")}>
        {#if call.deafened}<HeadphoneOff size={24} />{:else}<Headphones size={24} />{/if}
      </button>
      {#if isAndroid}
        {@const now = call.audioNow}
        <div class="audio-out">
          <button
            class="ctl"
            class:on={call.speaker || (!!now && now.kind !== "earpiece")}
            onclick={() => (call.hasHeadset ? (audioMenu = !audioMenu) : call.toggleSpeaker())}
            aria-label={call.hasHeadset ? tr("Audio output") : call.speaker ? tr("Speaker on") : tr("Speaker off")}
            title={now ? routeName(now) : call.speaker ? tr("Speaker on") : tr("Speaker off")}
            aria-expanded={call.hasHeadset ? audioMenu : undefined}
          >
            {#if now?.kind === "bluetooth"}<Bluetooth size={24} />{:else if now && now.kind !== "speaker" && now.kind !== "earpiece"}<Headphones size={24} />{:else if call.speaker}<Volume2 size={24} />{:else}<Smartphone size={24} />{/if}
          </button>
          {#if audioMenu && call.hasHeadset}
            <div class="audio-menu" role="menu" transition:fly={{ y: 8, duration: 140 }}>
              {#each call.audioRoutes as r (r.id)}
                <button role="menuitemradio" aria-checked={r.id === call.audioCurrent} class:current={r.id === call.audioCurrent} onclick={() => ((audioMenu = false), call.chooseAudio(r))}>
                  {#if r.kind === "bluetooth"}<Bluetooth size={20} />{:else if r.kind === "speaker"}<Volume2 size={20} />{:else if r.kind === "earpiece"}<Smartphone size={20} />{:else}<Headphones size={20} />{/if}
                  <span>{routeName(r)}</span>
                  {#if r.id === call.audioCurrent}<Check size={18} />{/if}
                </button>
              {/each}
            </div>
          {/if}
        </div>
      {/if}
      <button class="ctl" class:on={call.camOn} onclick={() => call.toggleCamera()} aria-label={call.camOn ? tr("Turn camera off") : tr("Turn camera on")}>
        {#if call.camOn}<Video size={24} />{:else}<VideoOff size={24} />{/if}
      </button>
      {#if call.canShareScreen}
        <button class="ctl" class:on={call.screenOn} onclick={() => call.toggleScreen()} aria-label={call.screenOn ? tr("Stop sharing") : tr("Share screen")} title={call.screenOn ? tr("Stop sharing") : tr("Share screen")}><MonitorUp size={24} /></button>
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
  .send-stats {
    font-variant-numeric: tabular-nums;
    opacity: 0.75;
    text-align: center;
  }
  .send-stats.held {
    color: var(--warning, #f5a524);
    opacity: 1;
  }
  .call.side {
    right: var(--side-chat);
  }
  header {
    display: flex;
    align-items: center;
    gap: 10px;
    padding-right: var(--window-controls);
  }
  /* The header's panels (reactions, soundboard, ring, options) all drop down from it the
   * same way everywhere: under its buttons on the right, as wide as a phone screen at most. */
  .head {
    position: relative;
    z-index: 6;
  }
  .options-panel {
    position: absolute;
    top: calc(100% + 8px);
    right: 0;
    width: min(420px, 100%);
    box-sizing: border-box;
    max-height: min(70vh, 560px);
    overflow-y: auto;
    box-shadow: 0 10px 40px rgb(0 0 0 / 0.45);
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
  /* Full screen: the controls float over the picture, and leave with the pointer when idle. */
  .call.full .controls {
    position: fixed;
    left: 50%;
    bottom: max(24px, env(safe-area-inset-bottom));
    z-index: 81;
    padding: 10px 14px;
    border-radius: 999px;
    background: rgb(10 12 16 / 0.6);
    backdrop-filter: blur(12px);
    transform: translateX(-50%);
    transition: opacity 0.2s var(--ease), transform 0.2s var(--ease);
  }
  .call.full .featured :is(.spot-actions, .label) {
    transition: opacity 0.2s var(--ease);
  }
  .call.bare .controls {
    opacity: 0;
    transform: translate(-50%, 16px);
    pointer-events: none;
  }
  .call.bare .featured :is(.spot-actions, .label) {
    opacity: 0;
    pointer-events: none;
  }
  .call.bare {
    cursor: none;
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
  .self-hide,
  .self-flip {
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
  .self-flip {
    right: auto;
    left: 6px;
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
  /* Being rung: a slow pulse around their photo. */
  .ring.pulse {
    animation: ring-pulse 1.6s ease-out infinite;
  }
  @keyframes ring-pulse {
    from {
      box-shadow: 0 0 0 0 color-mix(in srgb, var(--accent-a) 55%, transparent);
    }
    to {
      box-shadow: 0 0 0 26px transparent;
    }
  }
  .no-picture {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 12px;
    background: var(--surface);
    text-align: center;
  }
  .no-picture strong {
    font-size: 0.9rem;
  }
  .no-picture small {
    max-width: 260px;
    color: var(--muted);
    font-size: 0.78rem;
  }
  .tile.waiting {
    background: color-mix(in srgb, var(--surface) 70%, transparent);
    cursor: default;
  }
  /* Connection lost: dimmed until they're back (or leave after LiveKit's timeout). */
  .tile.lost > :not(.state):not(.name) {
    opacity: 0.4;
    filter: grayscale(0.6);
  }
  .state {
    position: absolute;
    top: 10px;
    left: 50%;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 4px 11px;
    transform: translateX(-50%);
    border-radius: 999px;
    background: rgb(0 0 0 / 0.55);
    color: #fff;
    font-size: 0.8rem;
    white-space: nowrap;
  }
  .lost-state :global(svg) {
    animation: spin 1s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .ring.pulse,
    .lost-state :global(svg) {
      animation: none;
    }
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
  .audio-out {
    position: relative;
  }
  /* Our share's card (and someone's we don't watch): what it is, and its controls. */
  .screen-card {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 12px 12px 56px;
    background: linear-gradient(160deg, color-mix(in srgb, var(--accent) 26%, var(--surface)), var(--surface));
    text-align: center;
  }
  .screen-card strong {
    font-size: 0.92rem;
  }
  .screen-card small {
    max-width: 280px;
    color: var(--muted);
    font-size: 0.78rem;
  }
  .screen-card .card-controls {
    position: static;
    margin-top: 4px;
  }
  .card-controls {
    position: absolute;
    left: 50%;
    bottom: 10px;
    z-index: 3;
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: 6px;
    width: max-content;
    max-width: calc(100% - 16px);
    transform: translateX(-50%);
    cursor: default;
  }
  /* Over the picture: on a dark pill, shown when the pointer is on the card (always on touch). */
  .card-controls.over {
    padding: 4px;
    border-radius: 999px;
    background: rgb(0 0 0 / 0.55);
    backdrop-filter: blur(8px);
    opacity: 0;
    transition: opacity 0.15s;
  }
  .tile:hover .card-controls.over,
  .featured:hover .card-controls.over,
  .card-controls.over:focus-within {
    opacity: 1;
  }
  @media (hover: none) {
    .card-controls.over {
      opacity: 1;
    }
  }
  .card-controls button,
  .card-controls .pick {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 32px;
    padding: 0 12px;
    border: none;
    border-radius: 999px;
    background: rgb(255 255 255 / 0.14);
    color: #fff;
    font: inherit;
    font-size: 0.82rem;
    cursor: pointer;
  }
  .card-controls button:hover,
  .card-controls .pick:hover {
    background: rgb(255 255 255 / 0.24);
  }
  .card-controls button.primary {
    background: var(--accent);
  }
  .card-controls button.stop {
    background: #e5484d;
  }
  .card-controls .pick.hint {
    box-shadow: inset 0 0 0 2px var(--accent);
  }
  .card-controls .pick.open {
    background: rgb(255 255 255 / 0.28);
  }
  .card-controls .pick span {
    max-width: 140px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .card-controls :global(.chev) {
    margin-left: -2px;
    opacity: 0.7;
  }
  .card-controls.open {
    opacity: 1;
  }
  .pick-wrap {
    position: relative;
  }
  /* Opens upwards from its pill, like the call's other menus. */
  .card-menu {
    position: absolute;
    bottom: calc(100% + 8px);
    left: 50%;
    z-index: 9;
    display: grid;
    gap: 2px;
    width: max-content;
    min-width: 200px;
    max-width: min(300px, calc(100vw - 32px));
    padding: 6px;
    border-radius: 14px;
    background: var(--surface);
    color: var(--text);
    box-shadow: 0 10px 40px rgb(0 0 0 / 0.45);
    transform: translateX(-50%);
  }
  .card-controls .card-menu button {
    display: flex;
    align-items: center;
    gap: 10px;
    height: auto;
    padding: 8px 10px;
    border-radius: 10px;
    background: none;
    color: inherit;
    text-align: left;
  }
  .card-controls .card-menu button:hover {
    background: var(--surface-hover);
  }
  .card-controls .card-menu button.current {
    color: var(--accent);
  }
  .card-menu .item {
    display: grid;
    flex: 1;
    gap: 1px;
    min-width: 0;
  }
  .card-menu .item strong {
    overflow: hidden;
    font-size: 0.86rem;
    font-weight: 600;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .card-menu .item small {
    color: var(--muted);
    font-size: 0.75rem;
    line-height: 1.3;
    white-space: normal;
  }
  .menu-title,
  .menu-note {
    margin: 4px 10px 6px;
    color: var(--muted);
    font-size: 0.75rem;
  }
  .menu-note {
    margin: 6px 10px 4px;
  }
  /* Small strip tiles: icons only. */
  .grid.strip .card-controls button span {
    display: none;
  }
  .grid.strip .card-controls button,
  .grid.strip .card-controls .pick {
    padding: 0 9px;
  }
  .grid.strip .screen-card small {
    display: none;
  }
  .screen-vol {
    position: relative;
  }
  .tile-actions:has(.vol-pop),
  .spot-actions:has(.vol-pop) {
    opacity: 1;
  }
  .vol-pop {
    position: absolute;
    top: calc(100% + 8px);
    right: 0;
    z-index: 9;
    display: flex;
    align-items: center;
    gap: 10px;
    width: 200px;
    padding: 6px 10px 6px 6px;
    border-radius: 14px;
    background: var(--surface);
    color: var(--text);
    box-shadow: 0 10px 40px rgb(0 0 0 / 0.45);
  }
  .vol-pop .mute {
    flex: none;
    background: var(--surface-2) !important;
    color: inherit !important;
  }
  .vol-pop input {
    flex: 1;
    min-width: 0;
    accent-color: var(--accent);
  }
  .vol-pop span {
    min-width: 3ch;
    font-size: 0.8rem;
    font-variant-numeric: tabular-nums;
    text-align: right;
  }
  .tile-actions {
    position: absolute;
    top: 8px;
    right: 8px;
    z-index: 3;
    display: flex;
    gap: 6px;
    opacity: 0;
    transition: opacity 0.15s;
  }
  .tile:hover .tile-actions {
    opacity: 1;
  }
  @media (hover: none) {
    .tile-actions {
      opacity: 1;
    }
  }
  .tile-actions button {
    display: grid;
    width: 32px;
    height: 32px;
    place-items: center;
    border: none;
    border-radius: 50%;
    background: rgb(0 0 0 / 0.55);
    color: #fff;
    cursor: pointer;
  }
  .audio-menu {
    position: absolute;
    bottom: calc(100% + 10px);
    left: 50%;
    transform: translateX(-50%);
    z-index: 8;
    min-width: 220px;
    max-width: calc(100vw - 32px);
    padding: 6px;
    border-radius: 16px;
    background: var(--surface);
    box-shadow: 0 10px 40px rgb(0 0 0 / 0.45);
    display: grid;
    gap: 2px;
  }
  .audio-menu button {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 11px 12px;
    border: none;
    border-radius: 11px;
    background: none;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .audio-menu button span {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .audio-menu button.current {
    background: var(--surface-hover);
    color: var(--accent);
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
  .ring.open,
  .options.open {
    background: var(--accent);
    color: #fff;
  }
  .duration {
    font-variant-numeric: tabular-nums;
  }
  .icon.react.raised {
    background: #f5b301;
  }
  /* A raised hand on someone's picture (with its place when several are up). */
  .hand-badge {
    position: absolute;
    top: 8px;
    left: 8px;
    z-index: 2;
    display: inline-flex;
    align-items: center;
    gap: 2px;
    padding: 3px 8px;
    border-radius: 999px;
    background: #f5b301;
    color: #1b1300;
    font-size: 1rem;
    box-shadow: 0 2px 8px rgb(0 0 0 / 0.3);
    animation: hand-in 0.3s var(--ease);
  }
  .hand-badge.small {
    top: 4px;
    left: 4px;
    padding: 1px 5px;
    font-size: 0.8rem;
  }
  .hand-badge b {
    font-size: 0.8rem;
  }
  @keyframes hand-in {
    from {
      transform: scale(0.4) rotate(-20deg);
      opacity: 0;
    }
  }
  /* A reaction rises from the bottom of the picture and fades (3 s, like Element Call). */
  .reaction-pop {
    position: absolute;
    bottom: 18%;
    left: 50%;
    z-index: 2;
    font-size: 3rem;
    pointer-events: none;
    animation: rise 3s ease-out forwards;
  }
  .reaction-pop.small {
    font-size: 1.8rem;
  }
  @keyframes rise {
    0% {
      transform: translate(-50%, 20px) scale(0.5);
      opacity: 0;
    }
    12% {
      transform: translate(-50%, 0) scale(1.15);
      opacity: 1;
    }
    75% {
      opacity: 1;
    }
    100% {
      transform: translate(-50%, -70px) scale(1);
      opacity: 0;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .reaction-pop,
    .hand-badge {
      animation: none;
    }
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
