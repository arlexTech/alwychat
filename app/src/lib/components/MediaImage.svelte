<script lang="ts">
  // A message's photo (or a video's thumbnail): its blurry preview (blurhash) right away, a
  // loading ring if it takes a while, quiet retries when it fails (network hiccups), then a
  // "Tap to retry". Fills its parent, which sets the size.
  import { tr } from "$lib/i18n/index.svelte";
  import { blurhashUrl } from "$lib/blurhash";
  import RotateCw from "@lucide/svelte/icons/rotate-cw";
  import { onDestroy } from "svelte";

  let {
    src,
    blurhash = null,
    alt = "",
    lazy = false,
    onload,
  }: {
    src: string;
    blurhash?: string | null;
    alt?: string;
    lazy?: boolean;
    /** Loaded, with the photo's own size. */
    onload?: (width: number, height: number) => void;
  } = $props();

  /** Pauses before the quiet retries (then the user decides). */
  const RETRIES = [1500, 4000, 10000];
  let attempt = $state(0);
  let loaded = $state(false);
  let failed = $state(false);
  /** The ring only shows if loading takes a moment (no flash for cached photos). */
  let slow = $state(false);
  let slowTimer: ReturnType<typeof setTimeout> | undefined;
  let retryTimer: ReturnType<typeof setTimeout> | undefined;

  const placeholder = $derived(blurhashUrl(blurhash));
  // A new address per attempt, so the WebView asks again instead of reusing the failure.
  const url = $derived(attempt ? `${src}${src.includes("?") ? "&" : "?"}retry=${attempt}` : src);

  $effect(() => {
    void url;
    clearTimeout(slowTimer);
    if (!loaded) slowTimer = setTimeout(() => (slow = true), 350);
  });

  function done(e: Event) {
    loaded = true;
    slow = false;
    clearTimeout(slowTimer);
    const img = e.currentTarget as HTMLImageElement;
    onload?.(img.naturalWidth, img.naturalHeight);
  }

  function error() {
    if (attempt < RETRIES.length) {
      retryTimer = setTimeout(() => attempt++, RETRIES[attempt]);
    } else {
      failed = true;
      slow = false;
    }
  }

  function retry(e: MouseEvent) {
    // The photo's own tap opens the viewer: this one only retries.
    e.stopPropagation();
    failed = false;
    attempt++;
  }

  onDestroy(() => {
    clearTimeout(slowTimer);
    clearTimeout(retryTimer);
  });
</script>

<span class="mi" class:shimmer={!placeholder && !loaded && !failed} style:background-image={placeholder && !loaded ? `url(${placeholder})` : undefined}>
  {#if !failed}
    {#key url}
      <img src={url} {alt} decoding="async" loading={lazy ? "lazy" : undefined} class:shown={loaded} onload={done} onerror={error} />
    {/key}
  {/if}
  {#if slow && !loaded && !failed}
    <!-- Something always moves while a photo loads: a light sweeping over its blurry preview,
         and a ring. -->
    <span class="sweep"></span>
    <span class="ring" aria-label={tr("Loading…")}></span>
  {/if}
  {#if failed}
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <span class="failed" role="button" tabindex="0" onclick={retry}>
      <RotateCw size={18} />
      <small>{tr("Couldn't load · Tap to retry")}</small>
    </span>
  {/if}
</span>

<style>
  .mi {
    position: absolute;
    inset: 0;
    display: block;
    background-size: cover;
    background-position: center;
  }
  .shimmer {
    background: linear-gradient(90deg, var(--surface-2), var(--surface-hover), var(--surface-2));
    background-size: 200% 100%;
    animation: shimmer 1.2s infinite linear;
  }
  @keyframes shimmer {
    to {
      background-position: -200% 0;
    }
  }
  img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    opacity: 0;
    transition: opacity 0.25s var(--ease, ease);
  }
  img.shown {
    opacity: 1;
  }
  .sweep {
    position: absolute;
    inset: 0;
    background: linear-gradient(100deg, transparent 30%, rgb(255 255 255 / 0.22) 50%, transparent 70%);
    background-size: 250% 100%;
    animation: sweep 1.3s infinite linear;
    pointer-events: none;
  }
  @keyframes sweep {
    from {
      background-position: 120% 0;
    }
    to {
      background-position: -130% 0;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .sweep {
      animation: none;
      opacity: 0;
    }
  }
  .ring {
    position: absolute;
    left: 50%;
    top: 50%;
    width: 28px;
    height: 28px;
    margin: -14px 0 0 -14px;
    border-radius: 50%;
    border: 3px solid rgb(255 255 255 / 0.3);
    border-top-color: #fff;
    animation: spin 0.8s linear infinite;
    filter: drop-shadow(0 1px 3px rgb(0 0 0 / 0.4));
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .failed {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 8px;
    text-align: center;
    color: #fff;
    background: rgb(0 0 0 / 0.45);
    cursor: pointer;
  }
  .failed small {
    font-size: 0.75rem;
    line-height: 1.2;
  }
</style>
