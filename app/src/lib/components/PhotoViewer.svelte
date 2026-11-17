<script lang="ts">
  import { tr } from "$lib/i18n/index.svelte";
  import X from "@lucide/svelte/icons/x";
  import { fade, scale } from "svelte/transition";
  import { mediaUrl, plainFileUrl } from "$lib/media";
  import { onBack } from "$lib/back.svelte";

  /** One profile photo full screen (someone's avatar, a chat's photo): the small version at once,
   * the original over it once loaded. Tap anywhere or back to close. */
  let { mxc, name, onClose }: { mxc: string; name: string; onClose: () => void } = $props();

  let full = $state(false);
  $effect(() => onBack(onClose));
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="viewer" transition:fade={{ duration: 150 }} onclick={onClose} role="dialog" aria-modal="true" aria-label={name} tabindex="-1">
  <button class="close" onclick={onClose} aria-label={tr("Close")}><X size={22} /></button>
  <div class="photo" transition:scale={{ start: 0.85, duration: 180 }}>
    <img class="small" src={mediaUrl(mxc, 320)} alt="" class:hidden={full} />
    <img src={plainFileUrl(mxc)} alt={name} onload={() => (full = true)} class:hidden={!full} />
  </div>
  <p>{name}</p>
</div>

<style>
  .viewer {
    position: fixed;
    inset: 0;
    z-index: 80;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 16px;
    padding: 16px;
    background: rgb(0 0 0 / 0.92);
    color: #fff;
  }
  .close {
    position: absolute;
    top: calc(12px + env(safe-area-inset-top));
    right: 12px;
    display: grid;
    place-items: center;
    width: 42px;
    height: 42px;
    border: none;
    border-radius: 50%;
    background: rgb(255 255 255 / 0.14);
    color: #fff;
    cursor: pointer;
  }
  .photo {
    display: grid;
    width: min(88vw, 70vh, 640px);
    aspect-ratio: 1;
  }
  img {
    grid-area: 1 / 1;
    width: 100%;
    height: 100%;
    object-fit: cover;
    border-radius: 18px;
  }
  .small {
    filter: blur(2px);
  }
  .hidden {
    visibility: hidden;
  }
  p {
    margin: 0;
    font-weight: 600;
    font-size: 1.1rem;
  }
</style>
