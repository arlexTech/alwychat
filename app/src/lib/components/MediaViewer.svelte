<script lang="ts">
  import { tr } from "$lib/i18n/index.svelte";
  import X from "@lucide/svelte/icons/x";
  import ChevronLeft from "@lucide/svelte/icons/chevron-left";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import { onMount, untrack } from "svelte";
  import { fade, scale } from "svelte/transition";
  import { eventMediaUrl, playableUrl } from "$lib/media";
  import { saveToGallery } from "$lib/save";

  type Item = { media: string; kind: "image" | "video"; caption: string | null };
  let {
    items,
    index: start = 0,
    onClose,
    onShow,
  }: {
    items: Item[];
    index?: number;
    onClose: () => void;
    /** "Show in chat" for the photo on screen (shared media: jump to its message). */
    onShow?: (index: number) => void;
  } = $props();

  // The album position starts where the user clicked, then the viewer owns it.
  let index = $state(untrack(() => start));
  const item = $derived(items[index]);

  function step(delta: number) {
    const next = Math.max(0, Math.min(items.length - 1, index + delta));
    if (next === index) return;
    index = next;
    status = null;
    resetZoom();
  }

  // Zoom (photos and videos): pinch, wheel, double tap / double click; drag to move around when zoomed.
  // Coordinates are relative to the screen's centre, where the photo is centred.
  let zoom = $state(1);
  let panX = $state(0);
  let panY = $state(0);
  let height = $state(1);
  let zooming = $state(false);
  const MAX_ZOOM = 5;
  const pointers = new Map<number, { x: number; y: number }>();
  let pinch: { dist: number; zoom: number; midX: number; midY: number; panX: number; panY: number } | null = null;
  let pan: { id: number; x: number; y: number; panX: number; panY: number } | null = null;

  function resetZoom() {
    zoom = 1;
    panX = panY = 0;
    pinch = pan = null;
  }
  function clampPan() {
    const maxX = ((zoom - 1) * width) / 2;
    const maxY = ((zoom - 1) * height) / 2;
    panX = Math.max(-maxX, Math.min(maxX, panX));
    panY = Math.max(-maxY, Math.min(maxY, panY));
  }
  /** Zoom to `next` keeping the screen point (x, y) where it is. */
  function zoomAt(next: number, x: number, y: number) {
    next = Math.max(1, Math.min(MAX_ZOOM, next));
    const mx = x - width / 2;
    const my = y - height / 2;
    const k = next / zoom;
    panX = mx - (mx - panX) * k;
    panY = my - (my - panY) * k;
    zoom = next;
    if (zoom === 1) panX = panY = 0;
    clampPan();
  }
  function wheel(e: WheelEvent) {
    if (!item) return;
    e.preventDefault();
    zoomAt(zoom * Math.exp(-e.deltaY * 0.0015), e.clientX, e.clientY);
  }
  // Svelte's wheel listeners are passive (can't stop the page scrolling): add our own.
  let viewer = $state<HTMLElement>();
  $effect(() => {
    const el = viewer;
    if (!el) return;
    el.addEventListener("wheel", wheel, { passive: false });
    return () => el.removeEventListener("wheel", wheel);
  });
  function toggleZoom(e: MouseEvent) {
    e.stopPropagation();
    if (zoom > 1) resetZoom();
    else zoomAt(2.5, e.clientX, e.clientY);
  }

  let status = $state<string | null>(null);
  let saving = $state(false);

  async function save(all: boolean) {
    saving = true;
    status = await saveToGallery(all ? items.map((i) => i.media) : [item.media]);
    saving = false;
  }

  // Swipe: sideways to the previous/next photo, up or down to close.
  let width = $state(1);
  let dx = $state(0);
  let dy = $state(0);
  let dragging = $state(false);
  let drag: { id: number; x: number; y: number; t: number; axis: "x" | "y" | null } | null = null;
  /** A swipe ends with a click on the backdrop: don't let it close the viewer. */
  let swiped = false;

  function down(e: PointerEvent) {
    if (e.pointerType === "mouse" && e.button !== 0) return;
    pointers.set(e.pointerId, { x: e.clientX, y: e.clientY });
    if (item && pointers.size === 2) {
      // Two fingers: pinch, never a swipe.
      const [a, b] = [...pointers.values()];
      pinch = { dist: Math.hypot(a.x - b.x, a.y - b.y) || 1, zoom, midX: (a.x + b.x) / 2, midY: (a.y + b.y) / 2, panX, panY };
      drag = null;
      dx = dy = 0;
      dragging = false;
      zooming = swiped = true;
      return;
    }
    if (zoom > 1) {
      // Zoomed: one finger (or the mouse) moves the photo around.
      pan = { id: e.pointerId, x: e.clientX, y: e.clientY, panX, panY };
      zooming = true;
      swiped = false;
      return;
    }
    // Leave the video's own controls (seek bar) alone.
    if (e.target instanceof HTMLVideoElement && e.clientY > e.target.getBoundingClientRect().bottom - 64) return;
    drag = { id: e.pointerId, x: e.clientX, y: e.clientY, t: e.timeStamp, axis: null };
    swiped = false;
  }

  function move(e: PointerEvent) {
    if (pointers.has(e.pointerId)) pointers.set(e.pointerId, { x: e.clientX, y: e.clientY });
    if (pinch && pointers.size >= 2) {
      const [a, b] = [...pointers.values()];
      const next = Math.max(1, Math.min(MAX_ZOOM, pinch.zoom * (Math.hypot(a.x - b.x, a.y - b.y) / pinch.dist)));
      const mx = (a.x + b.x) / 2 - width / 2;
      const my = (a.y + b.y) / 2 - height / 2;
      const m0x = pinch.midX - width / 2;
      const m0y = pinch.midY - height / 2;
      const k = next / pinch.zoom;
      zoom = next;
      panX = mx - (m0x - pinch.panX) * k;
      panY = my - (m0y - pinch.panY) * k;
      clampPan();
      return;
    }
    if (pan && e.pointerId === pan.id) {
      if (Math.hypot(e.clientX - pan.x, e.clientY - pan.y) > 6) swiped = true;
      panX = pan.panX + e.clientX - pan.x;
      panY = pan.panY + e.clientY - pan.y;
      clampPan();
      return;
    }
    if (!drag || e.pointerId !== drag.id) return;
    const mx = e.clientX - drag.x;
    const my = e.clientY - drag.y;
    if (!drag.axis) {
      if (Math.hypot(mx, my) < 10) return;
      drag.axis = Math.abs(mx) > Math.abs(my) ? "x" : "y";
      dragging = swiped = true;
      (e.currentTarget as Element).setPointerCapture(e.pointerId);
    }
    if (drag.axis === "x") {
      const pastEnd = (mx > 0 && index === 0) || (mx < 0 && index === items.length - 1);
      dx = pastEnd ? mx / 3 : mx;
    } else {
      dy = my;
    }
  }

  function up(e: PointerEvent) {
    pointers.delete(e.pointerId);
    if (pinch || pan) {
      if (pointers.size < 2) pinch = null;
      if (pan?.id === e.pointerId) pan = null;
      if (!pinch && !pan) {
        zooming = false;
        if (zoom < 1.05) resetZoom();
      }
      return;
    }
    if (!drag || e.pointerId !== drag.id) return;
    const { axis, t } = drag;
    drag = null;
    dragging = false;
    const flick = e.timeStamp - t < 250;
    if (axis === "x") {
      if ((dx < -width * 0.2 || (flick && dx < -30)) && index < items.length - 1) step(1);
      else if ((dx > width * 0.2 || (flick && dx > 30)) && index > 0) step(-1);
    } else if (axis === "y" && (Math.abs(dy) > 120 || (flick && Math.abs(dy) > 40))) {
      return onClose();
    }
    dx = 0;
    dy = 0;
  }

  function backdropClick() {
    if (swiped) swiped = false;
    else onClose();
  }

  onMount(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
      else if (e.key === "ArrowRight") step(1);
      else if (e.key === "ArrowLeft") step(-1);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div
  class="viewer"
  transition:fade={{ duration: 160 }}
  style:background-color="rgb(0 0 0 / {0.92 * Math.max(0.2, 1 - Math.abs(dy) / 400)})"
  bind:clientWidth={width}
  bind:clientHeight={height}
  bind:this={viewer}
  onclick={backdropClick}
  onpointerdown={down}
  onpointermove={move}
  onpointerup={up}
  onpointercancel={up}
>
  <div class="stage" transition:scale={{ duration: 200, start: 0.92 }}>
    <div
      class="track"
      class:dragging
      style:transform="translate3d(calc({-index * 100}% + {dx}px), {dy}px, 0)"
    >
      {#each items as it, i (i)}
        <div class="slide">
          {#if Math.abs(i - index) <= 1}
            {#if it.kind === "image"}
              <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
              <img
                src={eventMediaUrl(it.media)}
                alt={it.caption ?? ""}
                draggable="false"
                class:zooming
                style:transform={i === index && zoom > 1 ? `translate(${panX}px, ${panY}px) scale(${zoom})` : undefined}
                onclick={(e) => e.stopPropagation()}
                ondblclick={toggleZoom}
              />
            {:else if i === index}
              <!-- svelte-ignore a11y_media_has_caption -->
              {#await playableUrl(eventMediaUrl(it.media))}
                <div class="video-placeholder">…</div>
              {:then url}
                <video
                  src={url}
                  controls
                  autoplay
                  class:zooming
                  style:transform={zoom > 1 ? `translate(${panX}px, ${panY}px) scale(${zoom})` : undefined}
                  onclick={(e) => e.stopPropagation()}
                  ondblclick={toggleZoom}
                ></video>
              {:catch}
                <div class="video-placeholder">{tr("Couldn't load the video")}</div>
              {/await}
            {:else}
              <div class="video-placeholder">▶</div>
            {/if}
          {/if}
        </div>
      {/each}
    </div>
  </div>

  <div class="bar" onclick={(e) => e.stopPropagation()}>
    {#if onShow}<button onclick={() => onShow(index)}>{tr("Show in chat")}</button>{/if}
    <button onclick={() => save(false)} disabled={saving}>{saving ? tr("Saving…") : tr("Save")}</button>
    {#if items.length > 1}
      <button onclick={() => save(true)} disabled={saving}>{tr("Save all ({length})", { length: items.length })}</button>
    {/if}
    <button onclick={onClose} aria-label={tr("Close")}><X size={18} /></button>
  </div>

  {#if items.length > 1}
    {#if index > 0}
      <button class="nav prev" onclick={(e) => (e.stopPropagation(), step(-1))} aria-label={tr("Previous")}><ChevronLeft size={28} /></button>
    {/if}
    {#if index < items.length - 1}
      <button class="nav next" onclick={(e) => (e.stopPropagation(), step(1))} aria-label={tr("Next")}><ChevronRight size={28} /></button>
    {/if}
    <span class="counter">{index + 1} / {items.length}</span>
  {/if}

  {#if item.caption || status}
    <div class="footer">
      {#if item.caption}<p class="caption">{item.caption}</p>{/if}
      {#if status}<p class="status" transition:fade>{status}</p>{/if}
    </div>
  {/if}
</div>

<style>
  .viewer {
    position: fixed;
    inset: 0;
    z-index: 60;
    overflow: hidden;
    touch-action: none;
    user-select: none;
  }
  .stage {
    position: absolute;
    inset: 0;
  }
  .track {
    display: flex;
    height: 100%;
    transition: transform 0.28s var(--ease, ease-out);
    will-change: transform;
  }
  .track.dragging {
    transition: none;
  }
  .slide {
    flex: 0 0 100%;
    display: grid;
    /* A definite cell, so the image's max-height has something to measure: without it, a tall
       screenshot kept its full height and ran off the screen. */
    grid-template: minmax(0, 1fr) / minmax(0, 1fr);
    place-items: center;
    min-width: 0;
    height: 100%;
    box-sizing: border-box;
    padding: 64px 4vw 72px;
  }
  img,
  video {
    transition: transform 0.2s var(--ease, ease-out);
    will-change: transform;
  }
  img.zooming,
  video.zooming {
    transition: none;
  }
  img,
  video {
    max-width: 100%;
    max-height: 100%;
    border-radius: 8px;
    object-fit: contain;
    -webkit-user-drag: none;
  }
  .video-placeholder {
    display: grid;
    place-items: center;
    width: min(60vw, 320px);
    aspect-ratio: 16 / 9;
    border-radius: 8px;
    background: rgb(255 255 255 / 0.08);
    color: white;
    font-size: 2.5rem;
  }
  .bar {
    position: absolute;
    top: 12px;
    right: calc(12px + var(--window-controls));
    z-index: 1;
    display: flex;
    gap: 8px;
  }
  /* Dark frosted pills: readable over a bright or zoomed-in photo too. */
  .bar button,
  .nav,
  .counter {
    background: rgb(0 0 0 / 0.55);
    backdrop-filter: blur(12px);
    -webkit-backdrop-filter: blur(12px);
    box-shadow: 0 0 0 1px rgb(255 255 255 / 0.1);
  }
  .bar button {
    padding: 8px 14px;
    border: none;
    border-radius: 999px;
    color: white;
    cursor: pointer;
  }
  .bar button:hover,
  .nav:hover {
    background: rgb(0 0 0 / 0.75);
  }
  .footer {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 16px;
    z-index: 1;
    pointer-events: none;
  }
  .caption,
  .status {
    margin: 6px 24px 0;
    color: rgb(255 255 255 / 0.9);
    text-align: center;
    text-shadow: 0 1px 3px rgb(0 0 0 / 0.9);
  }
  .status {
    font-size: 0.85rem;
  }
  .nav {
    position: absolute;
    top: 50%;
    z-index: 1;
    width: 48px;
    height: 48px;
    border: none;
    border-radius: 50%;
    color: white;
    display: grid;
    place-items: center;
    cursor: pointer;
    translate: 0 -50%;
  }
  /* Phones swipe instead. */
  @media (hover: none) {
    .nav {
      display: none;
    }
  }
  .prev {
    left: 16px;
  }
  .next {
    right: 16px;
  }
  .counter {
    position: absolute;
    top: 12px;
    left: 12px;
    z-index: 1;
    padding: 8px 14px;
    border-radius: 999px;
    color: white;
    font-size: 0.9rem;
    pointer-events: none;
  }
</style>
