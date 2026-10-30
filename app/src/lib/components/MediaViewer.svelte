<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { fade, scale } from "svelte/transition";
  import { eventMediaUrl } from "$lib/media";
  import { saveToGallery } from "$lib/save";

  type Item = { media: string; kind: "image" | "video"; caption: string | null };
  let { items, index: start = 0, onClose }: { items: Item[]; index?: number; onClose: () => void } = $props();

  // The album position starts where the user clicked, then the viewer owns it.
  let index = $state(untrack(() => start));
  const item = $derived(items[index]);
  const media = $derived(item.media);
  const kind = $derived(item.kind);
  const caption = $derived(item.caption);

  function step(delta: number) {
    index = (index + delta + items.length) % items.length;
    status = null;
  }

  let status = $state<string | null>(null);
  let saving = $state(false);

  async function save(all: boolean) {
    saving = true;
    status = await saveToGallery(all ? items.map((i) => i.media) : [media]);
    saving = false;
  }

  onMount(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
      else if (e.key === "ArrowRight" && items.length > 1) step(1);
      else if (e.key === "ArrowLeft" && items.length > 1) step(-1);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="viewer" transition:fade={{ duration: 160 }} onclick={onClose}>
  <div class="bar" onclick={(e) => e.stopPropagation()}>
    <button onclick={() => save(false)} disabled={saving}>{saving ? "Saving…" : "⬇ Save"}</button>
    {#if items.length > 1}
      <button onclick={() => save(true)} disabled={saving}>⬇ Save all ({items.length})</button>
    {/if}
    <button onclick={onClose} aria-label="Close">✕</button>
  </div>

  {#if items.length > 1}
    <button class="nav prev" onclick={(e) => (e.stopPropagation(), step(-1))} aria-label="Previous">‹</button>
    <button class="nav next" onclick={(e) => (e.stopPropagation(), step(1))} aria-label="Next">›</button>
    <span class="counter">{index + 1} / {items.length}</span>
  {/if}

  <div class="stage" transition:scale={{ duration: 200, start: 0.92 }}>
    {#key media}
    {#if kind === "image"}
      <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
      <img src={eventMediaUrl(media)} alt={caption ?? ""} onclick={(e) => e.stopPropagation()} />
    {:else}
      <!-- svelte-ignore a11y_media_has_caption -->
      <video src={eventMediaUrl(media)} controls autoplay onclick={(e) => e.stopPropagation()}></video>
    {/if}
    {/key}
  </div>

  {#if caption}<p class="caption">{caption}</p>{/if}
  {#if status}<p class="status" transition:fade>{status}</p>{/if}
</div>

<style>
  .viewer {
    position: fixed;
    inset: 0;
    z-index: 60;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    background: rgb(0 0 0 / 0.9);
  }
  .bar {
    position: absolute;
    top: 12px;
    right: 12px;
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
  .stage {
    display: grid;
    place-items: center;
    max-width: 92vw;
    max-height: 82vh;
  }
  img,
  video {
    max-width: 92vw;
    max-height: 82vh;
    border-radius: 8px;
    object-fit: contain;
  }
  .caption,
  .status {
    margin: 12px 24px 0;
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
    font-size: 2rem;
    line-height: 1;
    cursor: pointer;
    translate: 0 -50%;
  }
  .nav:hover {
    background: rgb(255 255 255 / 0.22);
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
