<script lang="ts">
  import Send from "@lucide/svelte/icons/send-horizontal";
  import type { RoomSummary } from "$lib/backend";
  import type { Snippet } from "svelte";
  import { chatList } from "$lib/stores/chatList.svelte";
  import Avatar from "./Avatar.svelte";
  import Page from "./Page.svelte";

  let {
    title = "Forward to…",
    onPick,
    onClose,
    children,
  }: {
    title?: string;
    onPick: (room: RoomSummary) => void;
    onClose: () => void;
    /** Shown above the search (what's being sent). */
    children?: Snippet;
  } = $props();

  let query = $state("");
  const rooms = $derived(
    chatList.rooms.filter((r) => !r.is_invite && !r.is_space).filter((r) => !query.trim() || r.name.toLowerCase().includes(query.trim().toLowerCase())),
  );
</script>

<Page {title} {onClose}>
  {@render children?.()}
  <input class="search" bind:value={query} placeholder="Search chats" />
  <ul>
    {#each rooms as r (r.id)}
      <li>
        <button onclick={() => onPick(r)}>
          <Avatar name={r.name} mxc={r.avatar} size={40} />
          <span>{r.name}</span>
          <Send size={18} />
        </button>
      </li>
    {/each}
  </ul>
</Page>

<style>
  .search {
    width: 100%;
    margin-bottom: 8px;
    padding: 11px 16px;
    border: none;
    border-radius: 999px;
    background: var(--surface-2);
    outline: none;
  }
  ul {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  button {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
    padding: 8px 4px;
    border: none;
    border-radius: 12px;
    background: none;
    color: inherit;
    text-align: left;
    cursor: pointer;
  }
  button:active,
  button:hover {
    background: var(--surface-hover);
  }
  span {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  button :global(svg) {
    color: var(--accent-a);
  }
</style>
