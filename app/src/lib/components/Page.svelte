<script lang="ts">
  import type { Snippet } from "svelte";
  import { fly } from "svelte/transition";

  /** A full-screen page (settings and the like) that slides in over the app. */
  let { title, onClose, children }: { title: string; onClose: () => void; children: Snippet } = $props();
</script>

<div class="page" role="dialog" aria-modal="true" aria-label={title} transition:fly={{ x: 48, duration: 220 }}>
  <header>
    <button class="back" onclick={onClose} aria-label="Back">←</button>
    <h2>{title}</h2>
  </header>
  <div class="body">
    <div class="inner">{@render children()}</div>
  </div>
</div>

<style>
  .page {
    position: fixed;
    inset: 0;
    z-index: 50;
    display: flex;
    flex-direction: column;
    background: var(--bg);
  }
  header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 12px;
    border-bottom: 1px solid var(--border);
  }
  h2 {
    margin: 0;
    font-size: 1.25rem;
  }
  .back {
    width: 42px;
    height: 42px;
    border: none;
    border-radius: 50%;
    background: none;
    font-size: 1.4rem;
    cursor: pointer;
  }
  .back:hover {
    background: var(--surface-hover);
  }
  .body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    overscroll-behavior: contain;
  }
  .inner {
    max-width: 640px;
    margin-inline: auto;
    padding: 16px 20px 32px;
  }
</style>
