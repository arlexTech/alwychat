<script lang="ts">
  import { mediaUrl } from "$lib/media";
  import { initials } from "$lib/format";

  let { name, mxc = null, size = 48 }: { name: string; mxc?: string | null; size?: number } = $props();

  let failed = $state(false);
  const src = $derived(failed ? null : mediaUrl(mxc, size));

  // Stable per-name hue so fallbacks are colourful but consistent.
  const hue = $derived([...name].reduce((h, c) => (h * 31 + c.charCodeAt(0)) % 360, 7));

  $effect(() => {
    mxc; // retry when the avatar changes
    failed = false;
  });
</script>

<div
  class="avatar"
  style:width="{size}px"
  style:height="{size}px"
  style:font-size="{size * 0.4}px"
  style:--h={hue}
>
  {#if src}
    <img {src} alt="" loading="lazy" decoding="async" onerror={() => (failed = true)} />
  {:else}
    {initials(name)}
  {/if}
</div>

<style>
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
  img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
</style>
