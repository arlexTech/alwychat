<script lang="ts">
  // Windows: what to share (screens, then windows grouped by app). A window's share follows its
  // app to its other windows (a game and its launcher), so picking the app's window is enough.
  import { tr } from "$lib/i18n/index.svelte";
  import { call } from "$lib/stores/call.svelte";
  import { onBack } from "$lib/back.svelte";
  import { fade, fly } from "svelte/transition";
  import Monitor from "@lucide/svelte/icons/monitor";
  import AppWindow from "@lucide/svelte/icons/app-window";
  import X from "@lucide/svelte/icons/x";
  import type { WinSource } from "$lib/backend";

  const picker = $derived(call.screenPicker);
  const screens = $derived(picker?.sources.filter((s) => s.kind === "screen") ?? []);
  const windows = $derived(picker?.sources.filter((s) => s.kind === "window") ?? []);
  const pick = (s: WinSource | null) => picker?.pick(s);

  // Windows open and close meanwhile.
  $effect(() => {
    if (!picker) return;
    const timer = setInterval(() => void call.refreshScreenSources(), 2500);
    return () => clearInterval(timer);
  });
  $effect(() => onBack(() => pick(null)));
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && pick(null)} />

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="backdrop" transition:fade={{ duration: 120 }} onclick={() => pick(null)}>
  <div class="sheet" role="dialog" tabindex="-1" aria-label={tr("Share your screen")} transition:fly={{ y: 16, duration: 160 }} onclick={(e) => e.stopPropagation()}>
    <header>
      <strong>{tr("Share your screen")}</strong>
      <button class="close" onclick={() => pick(null)} aria-label={tr("Cancel")}><X size={20} /></button>
    </header>
    <div class="list">
      {#if screens.length}
        <h3>{tr("Screens")}</h3>
        <div class="grid">
          {#each screens as s (s.id)}
            <button class="source" onclick={() => pick(s)}>
              <span class="icon"><Monitor size={28} /></span>
              <strong>{s.title}</strong>
              <small>{s.width} × {s.height}</small>
            </button>
          {/each}
        </div>
      {/if}
      {#if windows.length}
        <h3>{tr("Windows")}</h3>
        <p class="hint">{tr("Sends only that app's sound, and follows it to its other windows (a game and its launcher).")}</p>
        <div class="grid">
          {#each windows as s (s.id)}
            <button class="source" onclick={() => pick(s)}>
              <span class="icon"><AppWindow size={28} /></span>
              <strong>{s.title}</strong>
              <small>{s.app}</small>
            </button>
          {/each}
        </div>
      {/if}
    </div>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 120;
    display: grid;
    place-items: center;
    padding: 16px;
    background: rgb(0 0 0 / 0.55);
  }
  .sheet {
    display: flex;
    flex-direction: column;
    width: min(760px, 100%);
    max-height: min(640px, 100%);
    border-radius: 20px;
    background: var(--surface);
    color: var(--text);
    box-shadow: 0 20px 60px rgb(0 0 0 / 0.4);
    overflow: hidden;
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 14px 14px 6px 20px;
  }
  header strong {
    font-size: 1.05rem;
  }
  .close {
    display: grid;
    width: 36px;
    height: 36px;
    place-items: center;
    border: none;
    border-radius: 50%;
    background: var(--surface-2);
    color: inherit;
    cursor: pointer;
  }
  .list {
    padding: 4px 20px 20px;
    overflow-y: auto;
  }
  h3 {
    margin: 14px 0 8px;
    color: var(--muted);
    font-size: 0.78rem;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }
  .hint {
    margin: -4px 0 10px;
    color: var(--muted);
    font-size: 0.8rem;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
    gap: 10px;
  }
  .source {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 4px;
    min-width: 0;
    padding: 14px;
    border: 2px solid transparent;
    border-radius: 14px;
    background: var(--surface-2);
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
    transition:
      border-color 0.15s var(--ease),
      transform 0.12s var(--ease);
  }
  .source:hover,
  .source:focus-visible {
    border-color: var(--accent);
    outline: none;
  }
  .source:active {
    transform: scale(0.98);
  }
  .icon {
    display: grid;
    width: 48px;
    height: 48px;
    margin-bottom: 4px;
    place-items: center;
    border-radius: 12px;
    background: color-mix(in srgb, var(--accent) 22%, transparent);
    color: var(--accent);
  }
  .source strong {
    display: -webkit-box;
    max-width: 100%;
    overflow: hidden;
    font-size: 0.88rem;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    word-break: break-word;
  }
  .source small {
    max-width: 100%;
    overflow: hidden;
    color: var(--muted);
    font-size: 0.78rem;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
