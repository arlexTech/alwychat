<script lang="ts">
  import { onMount } from "svelte";
  import Search from "@lucide/svelte/icons/search";
  import type { KindFilter } from "$lib/backend";
  import { MessageSearch } from "$lib/stores/search.svelte";
  import Page from "./Page.svelte";
  import SearchResults from "./SearchResults.svelte";

  let { roomId, onPick, onClose }: { roomId: string; onPick: (eventId: string) => void; onClose: () => void } =
    $props();

  // svelte-ignore state_referenced_locally
  const search = new MessageSearch(roomId);
  let input = $state<HTMLInputElement>();

  const KINDS: { id: KindFilter | null; label: string }[] = [
    { id: null, label: "All" },
    { id: "media", label: "Photos & videos" },
    { id: "files", label: "Files" },
    { id: "links", label: "Links" },
  ];

  onMount(() => {
    search.start();
    input?.focus();
    return () => search.stop();
  });
</script>

<Page title="Search in chat" {onClose}>
  <label class="field">
    <Search size={18} />
    <input
      bind:this={input}
      type="search"
      placeholder="Words, names of files…"
      bind:value={search.query}
      oninput={() => search.changed()}
      enterkeyhint="search"
    />
  </label>
  <nav class="chips">
    {#each KINDS as k (k.label)}
      <button class:active={search.kind === k.id} onclick={() => ((search.kind = k.id), search.changed())}>{k.label}</button>
    {/each}
  </nav>

  {#if search.error}
    <p class="note error">{search.error}</p>
  {:else if !search.active}
    <p class="note">Type at least 2 letters. Accents and capitals don't matter, and the end of a word can be left out.</p>
  {:else if search.hits.length}
    <SearchResults hits={search.hits} showChat={false} onPick={(h) => onPick(h.event_id)} onMore={() => search.more()} />
  {:else if !search.loading}
    <p class="note">No messages found.</p>
  {/if}
  {#if search.status?.chat_done === false}
    <p class="note small">Still reading this chat's older messages: more results may appear.</p>
  {/if}
</Page>

<style>
  .field {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 14px;
    border-radius: 14px;
    background: var(--surface-2);
    color: var(--muted);
  }
  .field input {
    flex: 1;
    padding: 12px 0;
    border: none;
    outline: none;
    background: none;
    color: var(--text);
  }
  .chips {
    display: flex;
    gap: 8px;
    margin: 12px 0 8px;
    overflow-x: auto;
  }
  .chips button {
    flex: none;
    padding: 6px 14px;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: none;
    cursor: pointer;
    font-size: 14px;
  }
  .chips button.active {
    border-color: transparent;
    background: var(--accent);
    color: white;
  }
  .note {
    padding: 16px 8px;
    color: var(--muted);
    text-align: center;
  }
  .note.small {
    padding-top: 0;
    font-size: 13px;
  }
  .note.error {
    color: var(--danger);
  }
</style>
