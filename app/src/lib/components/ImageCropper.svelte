<script lang="ts" module>
  /** The part of the source image kept (source pixels). */
  export interface Crop {
    sx: number;
    sy: number;
    sw: number;
    sh: number;
  }
</script>

<script lang="ts">
  import { tr } from "$lib/i18n/index.svelte";
  import { onMount, untrack } from "svelte";
  import { fade } from "svelte/transition";
  import { onBack } from "$lib/back.svelte";
  import ZoomIn from "@lucide/svelte/icons/zoom-in";
  import ZoomOut from "@lucide/svelte/icons/zoom-out";

  /** Choose the part of a photo to keep (avatars, banners): drag to move, pinch / wheel / slider
   * to zoom; the photo always fills the frame. Mounted by `cropImage` (pickImage.ts). */
  let {
    bitmap,
    url,
    aspect,
    round,
    onDone,
  }: {
    bitmap: ImageBitmap;
    /** Object URL of the same photo (drawn as an <img>, cheaper than a canvas while dragging). */
    url: string;
    /** Width / height of the frame. */
    aspect: number;
    round: boolean;
    onDone: (crop: Crop | null) => void;
  } = $props();

  // The photo doesn't change while the cropper is open.
  const W = untrack(() => bitmap.width);
  const H = untrack(() => bitmap.height);
  const MAX_ZOOM = 5;

  let stage = $state<HTMLDivElement>();
  /** The frame on screen, px. */
  let frameW = $state(300);
  const frameH = $derived(frameW / aspect);
  /** Smallest scale (screen px per source px) where the photo still covers the frame. */
  const minScale = $derived(Math.max(frameW / W, frameH / H));
  /** 1 = just covers the frame. */
  let zoom = $state(1);
  const scale = $derived(minScale * zoom);
  /** Centre of the frame, in source pixels. */
  let cx = $state(W / 2);
  let cy = $state(H / 2);

  /** Keep the photo covering the frame. */
  function clamp() {
    const halfW = frameW / scale / 2;
    const halfH = frameH / scale / 2;
    cx = Math.min(W - halfW, Math.max(halfW, cx));
    cy = Math.min(H - halfH, Math.max(halfH, cy));
  }

  function setZoom(z: number) {
    zoom = Math.min(MAX_ZOOM, Math.max(1, z));
    clamp();
  }

  onMount(() => {
    const fit = () => {
      if (!stage) return;
      const r = stage.getBoundingClientRect();
      // Leave a margin around the frame; a wide frame (banner) is limited by the width.
      frameW = Math.min(r.width - 32, (r.height - 32) * aspect, 560);
      clamp();
    };
    fit();
    const observer = new ResizeObserver(fit);
    observer.observe(stage!);
    return () => observer.disconnect();
  });
  $effect(() => onBack(() => onDone(null)));

  // Pointers: one = move, two = pinch.
  const pointers = new Map<number, { x: number; y: number }>();
  let pinch: { dist: number; zoom: number } | null = null;
  const distance = () => {
    const [a, b] = [...pointers.values()];
    return Math.hypot(a.x - b.x, a.y - b.y);
  };

  function down(e: PointerEvent) {
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    pointers.set(e.pointerId, { x: e.clientX, y: e.clientY });
    if (pointers.size === 2) pinch = { dist: distance(), zoom };
  }
  function move(e: PointerEvent) {
    const last = pointers.get(e.pointerId);
    if (!last) return;
    if (pointers.size === 1) {
      cx -= (e.clientX - last.x) / scale;
      cy -= (e.clientY - last.y) / scale;
      clamp();
    }
    pointers.set(e.pointerId, { x: e.clientX, y: e.clientY });
    if (pointers.size === 2 && pinch) setZoom((pinch.zoom * distance()) / pinch.dist);
  }
  function up(e: PointerEvent) {
    pointers.delete(e.pointerId);
    if (pointers.size < 2) pinch = null;
  }
  function wheel(e: WheelEvent) {
    e.preventDefault();
    setZoom(zoom * Math.exp(-e.deltaY / 400));
  }

  /** Previews at the sizes people will see it (avatars: profile, chat list, messages). */
  const previews = $derived(aspect === 1 ? [72, 44, 28] : [240]);

  function done() {
    const sw = frameW / scale;
    const sh = frameH / scale;
    onDone({ sx: cx - sw / 2, sy: cy - sh / 2, sw, sh });
  }
</script>

<div class="cropper" transition:fade={{ duration: 120 }} role="dialog" aria-modal="true" aria-label={tr("Move and zoom")}>
  <header>
    <button class="text" onclick={() => onDone(null)}>{tr("Cancel")}</button>
    <h2>{tr("Move and zoom")}</h2>
    <button class="text strong" onclick={done}>{tr("Use")}</button>
  </header>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="stage"
    bind:this={stage}
    onpointerdown={down}
    onpointermove={move}
    onpointerup={up}
    onpointercancel={up}
    onwheel={wheel}
  >
    <img
      src={url}
      alt=""
      draggable="false"
      style:width="{W * scale}px"
      style:height="{H * scale}px"
      style:transform="translate({-cx * scale}px, {-cy * scale}px)"
    />
    <div class="frame" class:round style:width="{frameW}px" style:height="{frameH}px"></div>
  </div>
  <div class="preview">
    <span class="label">{tr("What people will see")}</span>
    <div class="shots">
      {#each previews as size (size)}
        {@const k = size / frameW}
        <div class="shot" class:round style:width="{size}px" style:height="{size / aspect}px">
          <img
            src={url}
            alt=""
            draggable="false"
            style:width="{W * scale * k}px"
            style:height="{H * scale * k}px"
            style:transform="translate({-cx * scale * k}px, {-cy * scale * k}px)"
          />
        </div>
      {/each}
    </div>
  </div>
  <div class="zoom">
    <button class="icon" onclick={() => setZoom(zoom / 1.25)} aria-label={tr("Zoom out")}><ZoomOut size={20} /></button>
    <input type="range" min="1" max={MAX_ZOOM} step="0.01" value={zoom} oninput={(e) => setZoom(Number(e.currentTarget.value))} aria-label={tr("Zoom")} />
    <button class="icon" onclick={() => setZoom(zoom * 1.25)} aria-label={tr("Zoom in")}><ZoomIn size={20} /></button>
  </div>
</div>

<style>
  .cropper {
    position: fixed;
    inset: 0;
    z-index: 90;
    display: flex;
    flex-direction: column;
    background: #000;
    color: #fff;
    padding-bottom: env(safe-area-inset-bottom);
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 8px;
  }
  h2 {
    margin: 0;
    font-size: 1.05rem;
  }
  .text {
    padding: 10px 14px;
    border: none;
    background: none;
    color: #fff;
    font: inherit;
    cursor: pointer;
  }
  .text.strong {
    font-weight: 700;
    color: #b9a8ff;
  }
  .stage {
    position: relative;
    flex: 1;
    overflow: hidden;
    touch-action: none;
    cursor: grab;
  }
  .stage:active {
    cursor: grabbing;
  }
  img {
    position: absolute;
    left: 50%;
    top: 50%;
    max-width: none;
    user-select: none;
    -webkit-user-select: none;
    pointer-events: none;
  }
  /* The frame; everything outside it darkened by a huge shadow. */
  .frame {
    position: absolute;
    left: 50%;
    top: 50%;
    translate: -50% -50%;
    border: 2px solid rgb(255 255 255 / 0.9);
    border-radius: 12px;
    box-shadow: 0 0 0 100vmax rgb(0 0 0 / 0.6);
    pointer-events: none;
  }
  .frame.round {
    border-radius: 50%;
  }
  .preview {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    padding: 12px 16px 0;
  }
  .label {
    color: rgb(255 255 255 / 0.6);
    font-size: 0.8rem;
  }
  .shots {
    display: flex;
    align-items: center;
    gap: 14px;
  }
  .shot {
    position: relative;
    overflow: hidden;
    border-radius: 10px;
    background: #222;
  }
  .shot.round {
    border-radius: 50%;
  }
  .zoom {
    display: flex;
    align-items: center;
    gap: 12px;
    width: min(420px, 100%);
    margin: 0 auto;
    padding: 14px 16px 18px;
    box-sizing: border-box;
  }
  .zoom input {
    flex: 1;
    accent-color: #9b85ff;
  }
  .icon {
    display: grid;
    place-items: center;
    width: 40px;
    height: 40px;
    border: none;
    border-radius: 50%;
    background: rgb(255 255 255 / 0.12);
    color: #fff;
    cursor: pointer;
  }
</style>
