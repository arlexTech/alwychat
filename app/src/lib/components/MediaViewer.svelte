<script lang="ts">
  import X from "@lucide/svelte/icons/x";
  import ChevronLeft from "@lucide/svelte/icons/chevron-left";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import { onMount, untrack } from "svelte";
  import { fade, scale } from "svelte/transition";
  import { eventMediaUrl, playableUrl } from "$lib/media";
  import { saveToGallery } from "$lib/save";

  type Item = { media: string; kind: "image" | "video"; caption: string | null };
  let { items, index: start = 0, onClose }: { items: Item[]; index?: number; onClose: () => void } = $props();

  // The album position starts where the user clicked, then the viewer owns it.
  let index = $state(untrack(() => start));
  const item = $derived(items[index]);

  function step(delta: number) {
    const next = Math.max(0, Math.min(items.length - 1, index + delta));
    if (next === index) return;
    index = next;
    status = null;
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
    // Leave the video's own controls (seek bar) alone.
    if (e.target instanceof HTMLVideoElement && e.clientY > e.target.getBoundingClientRect().bottom - 64) return;
    drag = { id: e.pointerId, x: e.clientX, y: e.clientY, t: e.timeStamp, axis: null };
    swiped = false;
  }

  function move(e: PointerEvent) {
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
                onclick={(e) => e.stopPropagation()}
              />
            {:else if i === index}
              <!-- svelte-ignore a11y_media_has_caption -->
              {#await playableUrl(eventMediaUrl(it.media))}
                <div class="video-placeholder">…</div>
              {:then url}
                <video src={url} controls autoplay onclick={(e) => e.stopPropagation()}></video>
              {:catch}
                <div class="video-placeholder">Couldn't load the video</div>
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
    <button onclick={() => save(false)} disabled={saving}>{saving ? "Saving…" : "Save"}</button>
    {#if items.length > 1}
      <button onclick={() => save(true)} disabled={saving}>Save all ({items.length})</button>
    {/if}
    <button onclick={onClose} aria-label="Close"><X size={18} /></button>
  </div>

  {#if items.length > 1}
    {#if index > 0}
      <button class="nav prev" onclick={(e) => (e.stopPropagation(), step(-1))} aria-label="Previous"><ChevronLeft size={28} /></button>
    {/if}
    {#if index < items.length - 1}
      <button class="nav next" onclick={(e) => (e.stopPropagation(), step(1))} aria-label="Next"><ChevronRight size={28} /></button>
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
    place-items: center;
    min-width: 0;
    padding: 64px 4vw 72px;
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
    right: 12px;
    z-index: 1;
    display: flex;
    gap: 8px;
  }
  .bar button {
    padding: 8px 14px;
    border: none;
    border-radius: 999px;
    background: rgb(255 255 255 / 0.12);
    color: white;
    cursor: pointer;
  }
  .bar button:hover {
    background: rgb(255 255 255 / 0.2);
  }
  .footer {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 16px;
    pointer-events: none;
  }
  .caption,
  .status {
    margin: 6px 24px 0;
    color: rgb(255 255 255 / 0.85);
    text-align: center;
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
    background: rgb(255 255 255 / 0.12);
    color: white;
    display: grid;
    place-items: center;
    cursor: pointer;
    translate: 0 -50%;
  }
  .nav:hover {
    background: rgb(255 255 255 / 0.22);
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
    top: 20px;
    left: 20px;
    color: rgb(255 255 255 / 0.8);
    font-size: 0.9rem;
  }
</style>
