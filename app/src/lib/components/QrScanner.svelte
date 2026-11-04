<script lang="ts">
  // Scans the other device's verification QR code: camera frames, made small and grey, go to
  // the core (which reads the code; Matrix codes are binary, which browser scanners mangle)
  // a few times a second until one is read.
  import { tr } from "$lib/i18n/index.svelte";
  import { onBack } from "$lib/back.svelte";
  import { onMount } from "svelte";
  import { fade } from "svelte/transition";
  import X from "@lucide/svelte/icons/x";
  import { scanVerificationFrame } from "$lib/backend";

  let { onDone, onClose }: { onDone: () => void; onClose: () => void } = $props();

  /** Frames are read at most this wide. */
  const MAX_WIDTH = 720;
  const EVERY_MS = 250;

  let video = $state<HTMLVideoElement>();
  let error = $state<string | null>(null);
  let hint = $state<string | null>(null);

  $effect(() => onBack(onClose));

  onMount(() => {
    let stream: MediaStream | null = null;
    let stopped = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const canvas = document.createElement("canvas");
    const ctx = canvas.getContext("2d", { willReadFrequently: true })!;

    async function tick() {
      if (stopped) return;
      const v = video;
      if (v?.videoWidth) {
        const scale = Math.min(1, MAX_WIDTH / v.videoWidth);
        const w = Math.round(v.videoWidth * scale);
        const h = Math.round(v.videoHeight * scale);
        canvas.width = w;
        canvas.height = h;
        ctx.drawImage(v, 0, 0, w, h);
        const rgba = ctx.getImageData(0, 0, w, h).data;
        const luma = new Uint8Array(w * h);
        for (let i = 0, j = 0; j < luma.length; i += 4, j++) {
          luma[j] = (rgba[i] * 77 + rgba[i + 1] * 150 + rgba[i + 2] * 29) >> 8;
        }
        let binary = "";
        for (let k = 0; k < luma.length; k += 0x8000) binary += String.fromCharCode(...luma.subarray(k, k + 0x8000));
        try {
          if (await scanVerificationFrame(w, h, btoa(binary))) {
            stopped = true;
            onDone();
            return;
          }
          hint = null;
        } catch (e) {
          // A code was read but isn't the right one: say so and keep looking.
          hint = String(e).replace(/^Error: /, "");
        }
      }
      timer = setTimeout(tick, EVERY_MS);
    }

    navigator.mediaDevices
      .getUserMedia({ video: { facingMode: "environment", width: { ideal: 1280 }, height: { ideal: 720 } }, audio: false })
      .then((s) => {
        if (stopped) return s.getTracks().forEach((t) => t.stop());
        stream = s;
        if (video) video.srcObject = s;
        void tick();
      })
      .catch((e) => {
        error = (e as DOMException)?.name === "NotAllowedError" ? tr("The camera isn't allowed for AlwyChat") : tr("The camera couldn't start");
      });

    return () => {
      stopped = true;
      clearTimeout(timer);
      stream?.getTracks().forEach((t) => t.stop());
    };
  });
</script>

<div class="scanner" transition:fade={{ duration: 120 }}>
  <!-- svelte-ignore a11y_media_has_caption -->
  <video bind:this={video} autoplay playsinline muted></video>
  <div class="frame" aria-hidden="true"></div>
  <header>
    <button class="round" onclick={onClose} aria-label={tr("Close")}><X size={24} /></button>
  </header>
  <p class="label">
    {#if error}{error}{:else if hint}{hint}{:else}{tr("Point the camera at the QR code on your other device")}{/if}
  </p>
</div>

<style>
  .scanner {
    position: fixed;
    inset: 0;
    z-index: 90;
    background: #000;
    color: #fff;
  }
  video {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .frame {
    position: absolute;
    left: 50%;
    top: 45%;
    width: min(70vw, 70vh, 320px);
    aspect-ratio: 1;
    translate: -50% -50%;
    border-radius: 24px;
    box-shadow: 0 0 0 100vmax rgb(0 0 0 / 0.45);
    outline: 3px solid rgb(255 255 255 / 0.9);
  }
  header {
    position: absolute;
    top: 0;
    left: 0;
    padding: max(12px, env(safe-area-inset-top)) 12px 12px;
  }
  .round {
    width: 48px;
    height: 48px;
    border: none;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: rgb(0 0 0 / 0.45);
    color: #fff;
    cursor: pointer;
  }
  .label {
    position: absolute;
    left: 24px;
    right: 24px;
    bottom: max(48px, calc(env(safe-area-inset-bottom) + 32px));
    margin: 0;
    text-align: center;
    text-shadow: 0 1px 3px #000;
  }
</style>
