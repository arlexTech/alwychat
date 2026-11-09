<script lang="ts">
  // Edit a photo before sending it (optional: the Edit button on a waiting photo): crop, rotate,
  // draw, text, and a blur brush (pixelates faces, plates, numbers). Every tool works on one
  // canvas at the photo's resolution (at most MAX_SIDE), with undo; Done gives a JPEG.
  import { tr } from "$lib/i18n/index.svelte";
  import { onBack } from "$lib/back.svelte";
  import { onMount } from "svelte";
  import { fade } from "svelte/transition";
  import X from "@lucide/svelte/icons/x";
  import Check from "@lucide/svelte/icons/check";
  import Undo2 from "@lucide/svelte/icons/undo-2";
  import Crop from "@lucide/svelte/icons/crop";
  import RotateCw from "@lucide/svelte/icons/rotate-cw";
  import Brush from "@lucide/svelte/icons/brush";
  import Type from "@lucide/svelte/icons/type";
  import ScanFace from "@lucide/svelte/icons/scan-face";

  let { src, onDone, onCancel }: { src: string; onDone: (jpeg: Blob) => void; onCancel: () => void } = $props();

  const MAX_SIDE = 4096;
  const HISTORY = 15;
  const COLORS = ["#ffffff", "#111111", "#ff3b30", "#ffcc00", "#34c759", "#0a84ff", "#af52de"];

  type Tool = "crop" | "draw" | "text" | "blur";
  let tool = $state<Tool | null>(null);
  let color = $state("#ff3b30");
  /** Brush / text size, in screen pixels (scaled to the photo). */
  let size = $state(8);
  let ready = $state(false);
  let error = $state<string | null>(null);
  let busy = $state(false);

  let canvas = $state<HTMLCanvasElement>();
  let stage = $state<HTMLDivElement>();
  /** Where the canvas is drawn inside the stage (CSS pixels), for overlays and pointers. */
  let view = $state({ left: 0, top: 0, width: 0, height: 0 });
  let history: HTMLCanvasElement[] = [];
  let canUndo = $state(false);

  $effect(() => onBack(onCancel));

  onMount(() => {
    const img = new Image();
    // media:// answers with CORS headers: the canvas stays readable (toBlob).
    img.crossOrigin = "anonymous";
    img.onload = () => {
      const k = Math.min(1, MAX_SIDE / Math.max(img.naturalWidth, img.naturalHeight));
      canvas!.width = Math.round(img.naturalWidth * k);
      canvas!.height = Math.round(img.naturalHeight * k);
      canvas!.getContext("2d")!.drawImage(img, 0, 0, canvas!.width, canvas!.height);
      ready = true;
      requestAnimationFrame(measure);
    };
    img.onerror = () => (error = tr("Couldn't open this photo"));
    img.src = src;
    const observer = new ResizeObserver(measure);
    observer.observe(stage!);
    return () => observer.disconnect();
  });

  /** Fit the photo in the stage (contain, centred) and remember where it is drawn. */
  function measure() {
    if (!canvas || !stage || !canvas.width) return;
    const pad = 8;
    const sw = stage.clientWidth - pad * 2;
    const sh = stage.clientHeight - pad * 2;
    const k = Math.min(sw / canvas.width, sh / canvas.height);
    const width = canvas.width * k;
    const height = canvas.height * k;
    view = { left: pad + (sw - width) / 2, top: pad + (sh - height) / 2, width, height };
    canvas.style.left = `${view.left}px`;
    canvas.style.top = `${view.top}px`;
    canvas.style.width = `${width}px`;
    canvas.style.height = `${height}px`;
  }

  const ctx = () => canvas!.getContext("2d")!;
  /** Photo pixels per screen pixel. */
  const scale = () => canvas!.width / Math.max(1, view.width);

  function copyOf(source: HTMLCanvasElement) {
    const c = document.createElement("canvas");
    c.width = source.width;
    c.height = source.height;
    c.getContext("2d")!.drawImage(source, 0, 0);
    return c;
  }

  function remember() {
    history.push(copyOf(canvas!));
    if (history.length > HISTORY) history.shift();
    canUndo = true;
  }

  function undo() {
    const last = history.pop();
    if (!last) return;
    canvas!.width = last.width;
    canvas!.height = last.height;
    ctx().drawImage(last, 0, 0);
    canUndo = history.length > 0;
    crop = null;
    requestAnimationFrame(measure);
  }

  /** Replace the photo with `next` (rotation, crop). */
  function replace(next: HTMLCanvasElement) {
    remember();
    canvas!.width = next.width;
    canvas!.height = next.height;
    ctx().drawImage(next, 0, 0);
    requestAnimationFrame(measure);
  }

  function rotate() {
    const src = canvas!;
    const next = document.createElement("canvas");
    next.width = src.height;
    next.height = src.width;
    const g = next.getContext("2d")!;
    g.translate(next.width, 0);
    g.rotate(Math.PI / 2);
    g.drawImage(src, 0, 0);
    crop = null;
    replace(next);
  }

  function choose(next: Tool) {
    if (tool === "crop" && next !== "crop") crop = null;
    tool = tool === next ? null : next;
    text = null;
    if (tool === "crop") startCrop();
  }

  /** Pointer position in photo pixels. */
  function at(e: PointerEvent) {
    const r = canvas!.getBoundingClientRect();
    return {
      x: ((e.clientX - r.left) * canvas!.width) / r.width,
      y: ((e.clientY - r.top) * canvas!.height) / r.height,
    };
  }

  // ── Draw and blur: strokes on the photo ──
  let stroking = false;
  let last = { x: 0, y: 0 };
  /** The photo pixelated (blur brush): painted through a round brush. */
  let mosaic: HTMLCanvasElement | null = null;

  function strokeStart(e: PointerEvent) {
    if (tool === "text") return placeText(e);
    if (tool !== "draw" && tool !== "blur") return;
    e.preventDefault();
    (e.currentTarget as Element).setPointerCapture(e.pointerId);
    remember();
    stroking = true;
    last = at(e);
    if (tool === "blur") {
      // Blocks of ~1/40 of the photo: faces and text become unreadable.
      const block = Math.max(8, Math.round(Math.max(canvas!.width, canvas!.height) / 40));
      const small = document.createElement("canvas");
      small.width = Math.max(1, Math.round(canvas!.width / block));
      small.height = Math.max(1, Math.round(canvas!.height / block));
      small.getContext("2d")!.drawImage(canvas!, 0, 0, small.width, small.height);
      mosaic = document.createElement("canvas");
      mosaic.width = canvas!.width;
      mosaic.height = canvas!.height;
      const m = mosaic.getContext("2d")!;
      m.imageSmoothingEnabled = false;
      m.drawImage(small, 0, 0, mosaic.width, mosaic.height);
    }
    strokeTo(e);
  }

  function strokeTo(e: PointerEvent) {
    if (!stroking) return;
    const p = at(e);
    const g = ctx();
    if (tool === "draw") {
      g.strokeStyle = color;
      g.lineWidth = size * scale();
      g.lineCap = "round";
      g.lineJoin = "round";
      g.beginPath();
      g.moveTo(last.x, last.y);
      g.lineTo(p.x + 0.01, p.y);
      g.stroke();
    } else if (tool === "blur" && mosaic) {
      // A brush ~4× the pen, so a face takes a stroke or two.
      const r = size * 4 * scale();
      const steps = Math.max(1, Math.ceil(Math.hypot(p.x - last.x, p.y - last.y) / (r / 2)));
      for (let i = 1; i <= steps; i++) {
        const x = last.x + ((p.x - last.x) * i) / steps;
        const y = last.y + ((p.y - last.y) * i) / steps;
        g.save();
        g.beginPath();
        g.arc(x, y, r, 0, Math.PI * 2);
        g.clip();
        g.drawImage(mosaic, 0, 0);
        g.restore();
      }
    }
    last = p;
  }

  function strokeEnd() {
    stroking = false;
    mosaic = null;
  }

  // ── Text: tap where it goes, type, done ──
  let text = $state<{ x: number; y: number; left: number; top: number; value: string } | null>(null);
  let textInput = $state<HTMLInputElement>();

  function placeText(e: PointerEvent) {
    if (text?.value.trim()) commitText();
    const p = at(e);
    const s = stage!.getBoundingClientRect();
    text = { x: p.x, y: p.y, left: e.clientX - s.left, top: e.clientY - s.top, value: "" };
    requestAnimationFrame(() => textInput?.focus());
  }

  function commitText() {
    if (!text) return;
    const value = text.value.trim();
    if (value) {
      remember();
      const g = ctx();
      const px = Math.max(12, size * 3.5 * scale());
      g.font = `700 ${px}px Inter, system-ui, sans-serif`;
      g.textAlign = "center";
      g.textBaseline = "middle";
      g.lineJoin = "round";
      // An outline in the opposite tone keeps it readable on any photo.
      g.strokeStyle = color === "#111111" ? "#ffffff" : "#000000";
      g.lineWidth = px / 7;
      g.strokeText(value, text.x, text.y);
      g.fillStyle = color;
      g.fillText(value, text.x, text.y);
    }
    text = null;
  }

  // ── Crop: a frame with corner handles, applied with "Crop" ──
  let crop = $state<{ x: number; y: number; w: number; h: number } | null>(null);
  /** Width / height, or null for free. */
  let aspect = $state<number | null>(null);
  const ASPECTS: [string, number | null][] = [
    ["Free", null],
    ["1:1", 1],
    ["4:3", 4 / 3],
    ["3:4", 3 / 4],
    ["16:9", 16 / 9],
  ];

  function startCrop() {
    const W = canvas!.width;
    const H = canvas!.height;
    crop = { x: W * 0.05, y: H * 0.05, w: W * 0.9, h: H * 0.9 };
    if (aspect) fitAspect();
  }

  function setAspect(a: number | null) {
    aspect = a;
    if (!crop) startCrop();
    else if (a) fitAspect();
  }

  /** Largest frame of the chosen shape inside the current one, centred. */
  function fitAspect() {
    if (!crop || !aspect) return;
    let w = crop.w;
    let h = w / aspect;
    if (h > crop.h) {
      h = crop.h;
      w = h * aspect;
    }
    crop = { x: crop.x + (crop.w - w) / 2, y: crop.y + (crop.h - h) / 2, w, h };
  }

  let drag: { kind: "move" | "nw" | "ne" | "sw" | "se"; start: { x: number; y: number }; from: { x: number; y: number; w: number; h: number } } | null = null;

  function cropStart(e: PointerEvent, kind: "move" | "nw" | "ne" | "sw" | "se") {
    if (!crop) return;
    e.preventDefault();
    e.stopPropagation();
    (e.currentTarget as Element).setPointerCapture(e.pointerId);
    drag = { kind, start: at(e), from: { ...crop } };
  }

  function cropMove(e: PointerEvent) {
    if (!drag || !crop) return;
    const p = at(e);
    const dx = p.x - drag.start.x;
    const dy = p.y - drag.start.y;
    const W = canvas!.width;
    const H = canvas!.height;
    const f = drag.from;
    const min = Math.max(W, H) * 0.05;
    if (drag.kind === "move") {
      crop = {
        ...f,
        x: Math.min(Math.max(0, f.x + dx), W - f.w),
        y: Math.min(Math.max(0, f.y + dy), H - f.h),
      };
      return;
    }
    // The corner opposite the dragged one stays put.
    const west = drag.kind === "nw" || drag.kind === "sw";
    const north = drag.kind === "nw" || drag.kind === "ne";
    const ax = west ? f.x + f.w : f.x;
    const ay = north ? f.y + f.h : f.y;
    let w = Math.max(min, west ? f.w - dx : f.w + dx);
    let h = Math.max(min, north ? f.h - dy : f.h + dy);
    w = Math.min(w, west ? ax : W - ax);
    h = Math.min(h, north ? ay : H - ay);
    if (aspect) {
      if (w / h > aspect) w = h * aspect;
      else h = w / aspect;
    }
    crop = { x: west ? ax - w : ax, y: north ? ay - h : ay, w, h };
  }

  function applyCrop() {
    if (!crop) return;
    const next = document.createElement("canvas");
    next.width = Math.max(1, Math.round(crop.w));
    next.height = Math.max(1, Math.round(crop.h));
    next.getContext("2d")!.drawImage(canvas!, crop.x, crop.y, crop.w, crop.h, 0, 0, next.width, next.height);
    crop = null;
    tool = null;
    replace(next);
  }

  /** The crop frame on screen (CSS pixels within the stage). */
  const frame = $derived.by(() => {
    if (!crop || !canvas || !view.width) return null;
    const k = view.width / canvas.width;
    return { left: view.left + crop.x * k, top: view.top + crop.y * k, width: crop.w * k, height: crop.h * k };
  });

  function done() {
    if (text) commitText();
    if (crop) applyCrop();
    busy = true;
    canvas!.toBlob(
      (blob) => {
        busy = false;
        if (blob) onDone(blob);
        else error = tr("Couldn't save the edited photo");
      },
      "image/jpeg",
      0.92,
    );
  }
</script>

<div class="editor" transition:fade={{ duration: 150 }}>
  <header>
    <button class="icon" onclick={onCancel} aria-label={tr("Cancel")} title={tr("Cancel")}><X size={24} /></button>
    <strong>{tr("Edit photo")}</strong>
    <button class="icon" onclick={undo} disabled={!canUndo} aria-label={tr("Undo")} title={tr("Undo")}><Undo2 size={22} /></button>
    <button class="done" onclick={done} disabled={!ready || busy}><Check size={18} />{tr("Done")}</button>
  </header>

  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="stage" bind:this={stage} onpointermove={cropMove} onpointerup={() => (drag = null)}>
    <canvas
      bind:this={canvas}
      class:hidden={!ready}
      class:paint={tool === "draw" || tool === "blur" || tool === "text"}
      onpointerdown={strokeStart}
      onpointermove={strokeTo}
      onpointerup={strokeEnd}
      onpointercancel={strokeEnd}
    ></canvas>
    {#if !ready && !error}<span class="loading">{tr("Loading…")}</span>{/if}
    {#if error}<p class="error">{error}</p>{/if}

    {#if frame}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="frame" style:left="{frame.left}px" style:top="{frame.top}px" style:width="{frame.width}px" style:height="{frame.height}px" onpointerdown={(e) => cropStart(e, "move")}>
        {#each ["nw", "ne", "sw", "se"] as const as corner (corner)}
          <span class="handle {corner}" onpointerdown={(e) => cropStart(e, corner)}></span>
        {/each}
      </div>
    {/if}

    {#if text}
      <input
        class="text-entry"
        bind:this={textInput}
        bind:value={text.value}
        style:left="{text.left}px"
        style:top="{text.top}px"
        style:color={text ? color : undefined}
        placeholder={tr("Type something")}
        onkeydown={(e) => e.key === "Enter" && commitText()}
        onblur={commitText}
      />
    {/if}
  </div>

  <footer>
    {#if tool === "crop"}
      <div class="options">
        {#each ASPECTS as [label, a] (label)}
          <button class="chip" class:on={aspect === a} onclick={() => setAspect(a)}>{label === "Free" ? tr("Free") : label}</button>
        {/each}
        <button class="chip apply" onclick={applyCrop}><Crop size={16} />{tr("Crop")}</button>
      </div>
    {:else if tool === "draw" || tool === "text" || tool === "blur"}
      <div class="options">
        {#if tool !== "blur"}
          {#each COLORS as c (c)}
            <button class="swatch" class:on={color === c} style:background={c} onclick={() => (color = c)} aria-label={c}></button>
          {/each}
        {/if}
        <input class="size" type="range" min="3" max="24" bind:value={size} aria-label={tr("Size")} />
      </div>
      <p class="hint">
        {tool === "draw" ? tr("Draw with your finger or the mouse") : tool === "text" ? tr("Tap where the text goes") : tr("Paint over what to hide")}
      </p>
    {/if}
    <div class="tools">
      <button class:on={tool === "crop"} onclick={() => choose("crop")}><Crop size={22} /><small>{tr("Crop")}</small></button>
      <button onclick={rotate} disabled={!ready}><RotateCw size={22} /><small>{tr("Rotate")}</small></button>
      <button class:on={tool === "draw"} onclick={() => choose("draw")}><Brush size={22} /><small>{tr("Draw")}</small></button>
      <button class:on={tool === "text"} onclick={() => choose("text")}><Type size={22} /><small>{tr("Text")}</small></button>
      <button class:on={tool === "blur"} onclick={() => choose("blur")}><ScanFace size={22} /><small>{tr("Blur")}</small></button>
    </div>
  </footer>
</div>

<style>
  .editor {
    position: fixed;
    inset: 0;
    z-index: 80;
    display: flex;
    flex-direction: column;
    background: #000;
    color: #fff;
  }
  header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px calc(12px + var(--window-controls)) 10px 12px;
  }
  header strong {
    flex: 1;
  }
  .icon {
    display: grid;
    place-items: center;
    width: 40px;
    height: 40px;
    border: 0;
    border-radius: 50%;
    background: rgb(255 255 255 / 0.1);
    color: #fff;
    cursor: pointer;
  }
  .icon:disabled {
    opacity: 0.35;
  }
  .done {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 9px 16px;
    border: 0;
    border-radius: 20px;
    background: var(--accent);
    color: #fff;
    font-weight: 600;
    cursor: pointer;
  }
  .done:disabled {
    opacity: 0.5;
  }
  .stage {
    position: relative;
    flex: 1;
    min-height: 0;
    padding: 8px;
    overflow: hidden;
    touch-action: none;
  }
  /* Sized by the script to fit the stage (CSS max sizes don't shrink a canvas reliably). */
  canvas {
    position: absolute;
    touch-action: none;
  }
  canvas.hidden {
    visibility: hidden;
  }
  canvas.paint {
    cursor: crosshair;
  }
  .loading,
  .error {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    margin: 0;
    color: rgb(255 255 255 / 0.7);
  }
  .frame {
    position: absolute;
    box-shadow: 0 0 0 9999px rgb(0 0 0 / 0.55);
    border: 2px solid #fff;
    cursor: move;
    touch-action: none;
  }
  .handle {
    position: absolute;
    width: 26px;
    height: 26px;
    border: 4px solid #fff;
    touch-action: none;
  }
  .handle.nw {
    left: -6px;
    top: -6px;
    border-right: 0;
    border-bottom: 0;
    cursor: nwse-resize;
  }
  .handle.ne {
    right: -6px;
    top: -6px;
    border-left: 0;
    border-bottom: 0;
    cursor: nesw-resize;
  }
  .handle.sw {
    left: -6px;
    bottom: -6px;
    border-right: 0;
    border-top: 0;
    cursor: nesw-resize;
  }
  .handle.se {
    right: -6px;
    bottom: -6px;
    border-left: 0;
    border-top: 0;
    cursor: nwse-resize;
  }
  .text-entry {
    position: absolute;
    transform: translate(-50%, -50%);
    min-width: 160px;
    padding: 6px 10px;
    border: 1px dashed rgb(255 255 255 / 0.7);
    border-radius: 8px;
    background: rgb(0 0 0 / 0.45);
    font: 700 1.2rem Inter, system-ui, sans-serif;
    text-align: center;
    outline: none;
  }
  footer {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 8px 12px 14px;
  }
  .options {
    display: flex;
    align-items: center;
    justify-content: center;
    flex-wrap: wrap;
    gap: 8px;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 6px 12px;
    border: 1px solid rgb(255 255 255 / 0.25);
    border-radius: 16px;
    background: transparent;
    color: #fff;
    cursor: pointer;
  }
  .chip.on {
    background: #fff;
    color: #000;
  }
  .chip.apply {
    background: var(--accent);
    border-color: transparent;
    font-weight: 600;
  }
  .swatch {
    width: 26px;
    height: 26px;
    border-radius: 50%;
    border: 2px solid rgb(255 255 255 / 0.4);
    cursor: pointer;
  }
  .swatch.on {
    border-color: #fff;
    box-shadow: 0 0 0 2px var(--accent-a);
  }
  .size {
    width: 120px;
  }
  .hint {
    margin: 0;
    text-align: center;
    font-size: 0.8rem;
    color: rgb(255 255 255 / 0.6);
  }
  .tools {
    display: flex;
    justify-content: space-around;
  }
  .tools button {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    min-width: 56px;
    padding: 6px;
    border: 0;
    border-radius: 12px;
    background: transparent;
    color: rgb(255 255 255 / 0.85);
    cursor: pointer;
  }
  .tools button.on {
    background: rgb(255 255 255 / 0.12);
    color: #fff;
  }
  .tools button:disabled {
    opacity: 0.4;
  }
  .tools small {
    font-size: 0.72rem;
  }
</style>
