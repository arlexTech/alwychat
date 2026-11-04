<script lang="ts">
  // The chat's shared dates under its header: one at a time (today's first, then the nearest
  // coming up); a tap shows the next one. Messenger-ish pill, celebratory on the day itself.
  import type { Countdown } from "$lib/backend";
  import { ordered, today } from "$lib/countdown";
  import { fade } from "svelte/transition";

  let { items, onManage }: { items: Countdown[]; onManage: () => void } = $props();

  let day = $state(today());
  // The day changes at midnight: re-read every minute.
  $effect(() => {
    const t = setInterval(() => (day = today()), 60_000);
    return () => clearInterval(t);
  });
  const list = $derived(ordered(items, day));
  let index = $state(0);
  const shown = $derived(list.length ? list[index % list.length] : null);
</script>

{#if shown}
  <div class="strip countdown-strip" class:party={shown.view.isToday} transition:fade={{ duration: 150 }}>
    <button class="main" onclick={() => (list.length > 1 ? index++ : onManage())}>
      <span class="emoji">{shown.view.emoji}</span>
      <span class="text">{shown.view.text}</span>
      {#if list.length > 1}<span class="count">{(index % list.length) + 1}/{list.length}</span>{/if}
    </button>
  </div>
{/if}

<style>
  .strip {
    display: flex;
    justify-content: center;
    padding: 6px 12px;
    border-bottom: 1px solid var(--border);
  }
  .main {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    max-width: 100%;
    padding: 5px 14px;
    border: 0;
    border-radius: 999px;
    background: var(--surface-2);
    color: var(--text);
    font-size: 0.88rem;
    cursor: pointer;
  }
  .party .main {
    background: var(--accent);
    color: #fff;
    font-weight: 600;
  }
  .emoji {
    font-size: 1.05rem;
  }
  .text {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .count {
    font-size: 0.72rem;
    opacity: 0.7;
  }
</style>
