<script lang="ts">
  import { nativeVideo } from "$lib/stores/nativeVideo.svelte";

  /** A camera or screen of a native call (Linux desktop), drawn from its JPEG frames. */
  let {
    frames,
    mirror = false,
    contain = false,
    onsize,
  }: { frames: string; mirror?: boolean; contain?: boolean; /** The picture's size changed. */ onsize?: (w: number, h: number) => void } = $props();

  let canvas = $state<HTMLCanvasElement>();
  $effect(() => {
    const el = canvas;
    if (!el) return;
    const ctx = el.getContext("2d");
    return nativeVideo.watch(frames, (bitmap) => {
      if (el.width !== bitmap.width || el.height !== bitmap.height) {
        el.width = bitmap.width;
        el.height = bitmap.height;
        onsize?.(bitmap.width, bitmap.height);
      }
      ctx?.drawImage(bitmap, 0, 0);
    });
  });
</script>

<canvas bind:this={canvas} class:mirror class:contain></canvas>

<style>
  canvas {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  canvas.contain {
    object-fit: contain;
  }
  .mirror {
    transform: scaleX(-1);
  }
</style>
