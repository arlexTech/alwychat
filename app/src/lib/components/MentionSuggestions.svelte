<script lang="ts">
  import { fly } from "svelte/transition";
  import type { Suggestion } from "$lib/mentions.svelte";
  import Avatar from "./Avatar.svelte";

  let {
    suggestions,
    index,
    onPick,
  }: { suggestions: Suggestion[]; index: number; onPick: (s: Suggestion) => void } = $props();
</script>

<ul class="suggestions" role="listbox" transition:fly={{ y: 6, duration: 140 }}>
  {#each suggestions as s, i (s.user_id)}
    <li role="option" aria-selected={i === index}>
      <!-- mousedown, not click: keep the textarea focused -->
      <button type="button" class:active={i === index} onmousedown={(e) => (e.preventDefault(), onPick(s))}>
        {#if s.user_id === "@room"}
          <span class="room">📣</span>
          <span class="name">@room</span>
          <span class="id">Notify everyone in this chat</span>
        {:else}
          <Avatar name={s.name} mxc={s.avatar} size={28} />
          <span class="name">{s.name}</span>
          <span class="id">{s.user_id}</span>
        {/if}
      </button>
    </li>
  {/each}
</ul>

<style>
  .suggestions {
    list-style: none;
    margin: 0 0 6px;
    padding: 6px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--surface-2);
    box-shadow: 0 10px 28px rgb(0 0 0 / 0.35);
  }
  button {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 6px 8px;
    border: none;
    border-radius: 10px;
    background: none;
    text-align: left;
    cursor: pointer;
  }
  button.active,
  button:hover {
    background: var(--surface-hover);
  }
  .room {
    width: 28px;
    text-align: center;
  }
  .name {
    font-weight: 600;
  }
  .id {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--muted);
    font-size: 0.82rem;
  }
</style>
