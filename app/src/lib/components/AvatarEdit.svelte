<script lang="ts">
  import { tr } from "$lib/i18n/index.svelte";
  import Camera from "@lucide/svelte/icons/camera";
  import { pickImage } from "$lib/pickImage";
  import Avatar from "./Avatar.svelte";

  /**
   * A big avatar with a camera badge: tap to pick a new photo. `onPicked` gets the base64 JPEG
   * (and should save it); the preview shows until `mxc` changes.
   */
  let {
    name,
    mxc,
    square = false,
    disabled = false,
    onPicked,
  }: {
    name: string;
    mxc: string | null;
    square?: boolean;
    disabled?: boolean;
    onPicked: (data: string) => Promise<void> | void;
  } = $props();

  let preview = $state<string | null>(null);
  let busy = $state(false);
  let error = $state<string | null>(null);

  async function pick() {
    error = null;
    try {
      const image = await pickImage();
      if (!image) return;
      preview = image.preview;
      busy = true;
      await onPicked(image.data);
    } catch (e) {
      error = String(e);
      preview = null;
    } finally {
      busy = false;
    }
  }
</script>

<div class="wrap">
  <button class="avatar" class:square class:busy onclick={pick} {disabled} aria-label={tr("Change photo")}>
    {#if preview}
      <img src={preview} alt="" />
    {:else}
      <Avatar {name} {mxc} size={96} {square} />
    {/if}
    {#if !disabled}<span class="badge"><Camera size={16} /></span>{/if}
  </button>
  {#if error}<p class="m-error">{error}</p>{/if}
</div>

<style>
  .wrap {
    display: flex;
    flex-direction: column;
    align-items: center;
  }
  .avatar {
    position: relative;
    width: 96px;
    height: 96px;
    padding: 0;
    border: none;
    border-radius: 50%;
    background: none;
    cursor: pointer;
  }
  .avatar.square,
  .avatar.square img {
    border-radius: 24px;
  }
  .avatar:disabled {
    cursor: default;
  }
  .busy {
    opacity: 0.6;
  }
  img {
    width: 96px;
    height: 96px;
    border-radius: 50%;
    object-fit: cover;
  }
  .badge {
    position: absolute;
    right: -2px;
    bottom: -2px;
    display: grid;
    width: 32px;
    height: 32px;
    place-items: center;
    border: 3px solid var(--bg);
    border-radius: 50%;
    background: var(--surface-2);
    color: var(--text);
  }
</style>
