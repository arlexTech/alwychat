<script lang="ts">
  // Who reacted with what: a tab per emoji (and "All"). Long-press a reaction on a phone,
  // right-click it on a computer.
  import { untrack } from "svelte";
  import { fade, fly } from "svelte/transition";
  import type { Reaction } from "$lib/backend";
  import { onBack } from "$lib/back.svelte";
  import { tr } from "$lib/i18n/index.svelte";
  import { members } from "$lib/stores/members.svelte";
  import Avatar from "./Avatar.svelte";

  let {
    roomId,
    reactions,
    initial = null,
    onClose,
  }: { roomId: string; reactions: Reaction[]; initial?: string | null; onClose: () => void } = $props();

  let tab = $state<string | null>(untrack(() => initial));
  $effect(() => onBack(onClose));
  $effect(() => void members.load(roomId, true));

  const total = $derived(reactions.reduce((n, r) => n + r.count, 0));
  const rows = $derived(
    reactions
      .filter((r) => tab === null || r.key === tab)
      .flatMap((r) => r.senders.map((userId) => ({ key: r.key, userId, ...members.get(roomId, userId) }))),
  );
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="backdrop" transition:fade={{ duration: 140 }} onclick={onClose}></div>
<div class="sheet" role="dialog" aria-label={tr("Reactions")} transition:fly={{ y: 40, duration: 200 }}>
  <div class="grip"></div>
  <div class="tabs" role="tablist">
    <button role="tab" aria-selected={tab === null} class:on={tab === null} onclick={() => (tab = null)}>{tr("All")} {total}</button>
    {#each reactions as r (r.key)}
      <button role="tab" aria-selected={tab === r.key} class:on={tab === r.key} onclick={() => (tab = r.key)}>{r.key} {r.count}</button>
    {/each}
  </div>
  <ul>
    {#each rows as row (`${row.key}|${row.userId}`)}
      <li>
        <Avatar name={row.name} mxc={row.avatar} size={36} />
        <span class="who"><strong>{row.isMe ? tr("You") : row.name}</strong><small>{row.userId}</small></span>
        <span class="emoji">{row.key}</span>
      </li>
    {/each}
  </ul>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 70;
    background: rgb(0 0 0 / 0.45);
  }
  .sheet {
    position: fixed;
    left: 50%;
    bottom: 0;
    z-index: 71;
    display: flex;
    flex-direction: column;
    width: min(480px, 100%);
    max-height: 70vh;
    padding: 8px 12px calc(14px + env(safe-area-inset-bottom));
    border-radius: 22px 22px 0 0;
    background: var(--surface);
    box-shadow: 0 -12px 40px rgb(0 0 0 / 0.4);
    translate: -50% 0;
  }
  /* A computer: a card in the middle rather than a sheet from the bottom. */
  @media (pointer: fine) and (min-width: 700px) {
    .sheet {
      top: 50%;
      bottom: auto;
      border-radius: 18px;
      translate: -50% -50%;
    }
    .grip {
      display: none;
    }
  }
  .grip {
    width: 36px;
    height: 4px;
    margin: 0 auto 10px;
    border-radius: 2px;
    background: var(--border);
  }
  .tabs {
    display: flex;
    gap: 6px;
    padding-bottom: 10px;
    overflow-x: auto;
    border-bottom: 1px solid var(--border);
  }
  .tabs button {
    flex: none;
    padding: 6px 12px;
    border: none;
    border-radius: 999px;
    background: var(--surface-2);
    color: inherit;
    font: inherit;
    cursor: pointer;
  }
  .tabs button.on {
    background: var(--accent);
    color: #fff;
  }
  ul {
    margin: 0;
    padding: 6px 0 0;
    overflow-y: auto;
    list-style: none;
  }
  li {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 4px;
  }
  .who {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-width: 0;
  }
  .who strong,
  .who small {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .who small {
    color: var(--muted);
    font-size: 0.78rem;
  }
  .emoji {
    font-size: 1.4rem;
  }
</style>
