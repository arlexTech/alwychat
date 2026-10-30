<script lang="ts">
  import { onMount } from "svelte";
  import { fade, scale } from "svelte/transition";
  import { saveMedia } from "$lib/backend";
  import { eventMediaUrl } from "$lib/media";

  let {
    media,
    kind,
    caption,
    onClose,
  }: { media: string; kind: "image" | "video"; caption: string | null; onClose: () => void } = $props();

  let status = $state<string | null>(null);
  let saving = $state(false);

  async function save() {
    saving = true;
    try {
      const path = await saveMedia(media, true);
      status = `Saved to ${path.replace(/^\/home\/[^/]+/, "~")}`;
    } catch (e) {
      status = `Couldn't save: ${e}`;
    } finally {
      saving = false;
    }
  }

  onMount(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="viewer" transition:fade={{ duration: 160 }} onclick={onClose}>
  <div class="bar" onclick={(e) => e.stopPropagation()}>
    <button onclick={save} disabled={saving}>{saving ? "Saving…" : "⬇ Save"}</button>
    <button onclick={onClose} aria-label="Close">✕</button>
  </div>

  <div class="stage" transition:scale={{ duration: 200, start: 0.92 }}>
    {#if kind === "image"}
      <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
      <img src={eventMediaUrl(media)} alt={caption ?? ""} onclick={(e) => e.stopPropagation()} />
    {:else}
      <!-- svelte-ignore a11y_media_has_caption -->
      <video src={eventMediaUrl(media)} controls autoplay onclick={(e) => e.stopPropagation()}></video>
    {/if}
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
</style>
