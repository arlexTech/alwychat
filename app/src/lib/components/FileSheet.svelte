<script lang="ts">
  import { fade, fly } from "svelte/transition";
  import Download from "@lucide/svelte/icons/download";
  import ExternalLink from "@lucide/svelte/icons/external-link";
  import FolderDown from "@lucide/svelte/icons/folder-down";
  import { isAndroid } from "$lib/backend";

  type Action = "open" | "save_as" | "downloads";
  let { filename, onPick, onClose }: { filename: string; onPick: (action: Action) => void; onClose: () => void } =
    $props();

  function pick(action: Action) {
    onClose();
    onPick(action);
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="backdrop" transition:fade={{ duration: 140 }} onclick={onClose}></div>
<div class="sheet" role="dialog" aria-label={filename} transition:fly={{ y: 40, duration: 200 }}>
  <div class="grip"></div>
  <strong class="name">{filename}</strong>
  <button onclick={() => pick("open")}><ExternalLink size={20} />Open</button>
  {#if isAndroid}
    <button onclick={() => pick("save_as")}><FolderDown size={20} />Save to…</button>
  {/if}
  <button onclick={() => pick("downloads")}><Download size={20} />Save to Downloads</button>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 40;
    background: rgb(0 0 0 / 0.45);
  }
  .sheet {
    position: fixed;
    left: 50%;
    bottom: 0;
    z-index: 41;
    display: flex;
    flex-direction: column;
    width: min(520px, 100%);
    padding: 8px 12px 16px;
    border-radius: 22px 22px 0 0;
    background: var(--surface);
    box-shadow: 0 -12px 40px rgb(0 0 0 / 0.4);
    translate: -50% 0;
  }
  .grip {
    width: 36px;
    height: 4px;
    margin: 0 auto 10px;
    border-radius: 2px;
    background: var(--border);
  }
  .name {
    padding: 4px 10px 10px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  button {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 13px 10px;
    border: none;
    border-radius: 12px;
    background: none;
    color: inherit;
    font-size: 1rem;
    text-align: left;
    cursor: pointer;
  }
  button:active,
  button:hover {
    background: var(--surface-hover);
  }
  button :global(svg) {
    color: var(--muted);
  }
</style>
