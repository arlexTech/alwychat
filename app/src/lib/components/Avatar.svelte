<script lang="ts">
  import Bookmark from "@lucide/svelte/icons/bookmark";
  import { mediaUrl } from "$lib/media";
  import { initials } from "$lib/format";

  let {
    name,
    mxc = null,
    size = 48,
    square = false,
    saved = false,
  }: {
    name: string;
    mxc?: string | null;
    size?: number;
    /** Spaces: rounded square. */
    square?: boolean;
    /** Our "Saved messages" chat: a bookmark instead of initials. */
    saved?: boolean;
  } = $props();

  let failed = $state(false);
  /** Web: the kept chat list is drawn before the engine has loaded, so its first photo
   * requests can fail; try again a few times instead of keeping the initials for good. */
  let attempt = $state(0);
  let retryTimer: ReturnType<typeof setTimeout> | undefined;
  /** The photo is on screen: no coloured backdrop behind it any more (it showed as a ring
   * around round photos with transparent corners). */
  let loaded = $state(false);
  const src = $derived.by(() => {
    if (failed) return null;
    const url = mediaUrl(mxc, size);
    return url && attempt ? `${url}&try=${attempt}` : url;
  });

  function onError() {
    failed = true;
    if (attempt >= 4) return;
    retryTimer = setTimeout(() => {
      attempt++;
      failed = false;
    }, [1500, 4000, 10000, 30000][attempt]);
  }

  // Stable per-name hue so fallbacks are colourful but consistent.
  const hue = $derived([...name].reduce((h, c) => (h * 31 + c.charCodeAt(0)) % 360, 7));

  $effect(() => {
    mxc; // retry when the avatar changes
    failed = false;
    loaded = false;
    attempt = 0;
    return () => clearTimeout(retryTimer);
  });
</script>

<div
  class="avatar"
  class:square
  class:saved
  class:loaded={loaded && !!src}
  style:width="{size}px"
  style:height="{size}px"
  style:font-size="{size * 0.4}px"
  style:--h={hue}
>
  {#if saved && !src}
    <Bookmark size={size * 0.46} fill="currentColor" />
  {:else if src}
    <img {src} alt="" loading="lazy" decoding="async" onload={() => (loaded = true)} onerror={onError} />
  {:else}
    {initials(name)}
  {/if}
</div>

<style>
  .avatar.saved {
    background: var(--accent);
    color: #fff;
  }
  .avatar.square,
  .avatar.square img {
    border-radius: 28%;
  }
  .avatar {
    flex: none;
    border-radius: 50%;
    overflow: hidden;
    display: grid;
    place-items: center;
    font-weight: 600;
    color: white;
    background: linear-gradient(135deg, hsl(var(--h) 70% 58%), hsl(calc(var(--h) + 40) 75% 52%));
    user-select: none;
  }
  .avatar.loaded {
    background: none;
  }
  img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
</style>
