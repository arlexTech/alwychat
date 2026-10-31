<script lang="ts">
  import Check from "@lucide/svelte/icons/check";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import type { Folder, RoomSummary } from "$lib/backend";
  import { untrack } from "svelte";
  import { chatList } from "$lib/stores/chatList.svelte";
  import { spaces } from "$lib/stores/spaces.svelte";
  import Avatar from "./Avatar.svelte";
  import Page from "./Page.svelte";

  /** Create a folder (`folder` null) or edit one. Saved on "Save" (synced to our devices). */
  let { folder = null, onClose }: { folder?: Folder | null; onClose: () => void } = $props();

  const blank = (): Folder => ({
    id: crypto.randomUUID(),
    name: "",
    people: false,
    groups: false,
    rooms: [],
    exclude: [],
    unread_only: false,
    hide_muted: false,
  });
  // An editable copy, taken once when the page opens.
  let draft = $state<Folder>(untrack(() => (folder ? structuredClone($state.snapshot(folder)) : blank())));
  let query = $state("");
  let error = $state<string | null>(null);
  let saving = $state(false);

  const byType = (r: RoomSummary) =>
    !r.is_archived && spaces.inChats(r) && ((draft.people && r.is_dm) || (draft.groups && !r.is_dm));
  const included = (r: RoomSummary) =>
    !draft.exclude.includes(r.id) && (draft.rooms.includes(r.id) || byType(r));

  function toggle(r: RoomSummary) {
    const on = included(r);
    draft.rooms = draft.rooms.filter((id) => id !== r.id);
    draft.exclude = draft.exclude.filter((id) => id !== r.id);
    if (on && byType(r)) draft.exclude.push(r.id);
    else if (!on && !byType(r)) draft.rooms.push(r.id);
  }

  // Chats already in the folder first, then the rest; both in the chat list's order.
  const rooms = $derived.by(() => {
    const q = query.trim().toLocaleLowerCase();
    const list = chatList.rooms.filter((r) => !q || r.name.toLocaleLowerCase().includes(q));
    return [...list.filter(included), ...list.filter((r) => !included(r))];
  });
  const count = $derived(chatList.rooms.filter(included).length);
  const valid = $derived(draft.name.trim() !== "" && count > 0);

  async function save() {
    if (!valid) return;
    saving = true;
    try {
      await spaces.saveFolder({ ...$state.snapshot(draft), name: draft.name.trim() });
      onClose();
    } catch (e) {
      error = String(e);
    } finally {
      saving = false;
    }
  }

  async function remove() {
    if (!folder) return;
    try {
      await spaces.deleteFolder(folder.id);
      onClose();
    } catch (e) {
      error = String(e);
    }
  }
</script>

<Page title={folder ? "Edit folder" : "New folder"} {onClose}>
  <input class="name" placeholder="Folder name" maxlength="40" bind:value={draft.name} />

  <h3 class="section">Include</h3>
  <label class="toggle">
    <span><strong>All 1:1 chats</strong></span>
    <input type="checkbox" role="switch" bind:checked={draft.people} />
  </label>
  <label class="toggle">
    <span><strong>All groups</strong><small>Chats inside spaces only if they show in Chats</small></span>
    <input type="checkbox" role="switch" bind:checked={draft.groups} />
  </label>
  <label class="toggle">
    <span><strong>Only unread</strong><small>Chats leave the folder once read</small></span>
    <input type="checkbox" role="switch" bind:checked={draft.unread_only} />
  </label>
  <label class="toggle">
    <span><strong>Hide muted chats</strong></span>
    <input type="checkbox" role="switch" bind:checked={draft.hide_muted} />
  </label>

  <h3 class="section">Chats <small>{count} in this folder</small></h3>
  <input class="filter" type="search" placeholder="Find a chat" bind:value={query} />
  <ul class="rooms">
    {#each rooms as r (r.id)}
      {@const on = included(r)}
      <li>
        <button class:on onclick={() => toggle(r)} aria-pressed={on}>
          <Avatar name={r.name} mxc={r.avatar} size={36} />
          <span class="room-name">{r.name}</span>
          <span class="mark">{#if on}<Check size={16} />{/if}</span>
        </button>
      </li>
    {/each}
  </ul>

  {#if error}<p class="error">{error}</p>{/if}
  <div class="buttons">
    {#if folder}
      <button class="delete" onclick={remove}><Trash2 size={18} />Delete folder</button>
    {/if}
    <button class="save" disabled={!valid || saving} onclick={save}>Save</button>
  </div>
</Page>

<style>
  .name,
  .filter {
    width: 100%;
    box-sizing: border-box;
    padding: 11px 14px;
    border: 1px solid var(--border);
    border-radius: 12px;
    background: var(--surface);
    color: inherit;
    font: inherit;
  }
  .name {
    font-size: 1.05rem;
  }
  h3.section {
    display: flex;
    align-items: baseline;
    gap: 8px;
    margin-top: 24px;
  }
  h3 small {
    font-weight: normal;
  }
  .toggle {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 12px 0;
    border-top: 1px solid var(--border);
    cursor: pointer;
  }
  .toggle > span {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  small {
    color: var(--muted);
    line-height: 1.35;
  }
  .rooms {
    margin: 8px 0 0;
    padding: 0;
    list-style: none;
  }
  .rooms button {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
    padding: 8px 4px;
    border: none;
    border-radius: 10px;
    background: none;
    color: inherit;
    text-align: left;
    cursor: pointer;
  }
  .rooms button:hover {
    background: var(--surface);
  }
  .room-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .mark {
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    border: 2px solid var(--border);
    border-radius: 50%;
    color: white;
  }
  .on .mark {
    border-color: transparent;
    background: var(--accent);
  }
  .buttons {
    position: sticky;
    bottom: 0;
    display: flex;
    gap: 10px;
    padding: 14px 0 6px;
    background: var(--bg);
  }
  .buttons button {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 11px 18px;
    border: none;
    border-radius: 12px;
    font: inherit;
    cursor: pointer;
  }
  .save {
    flex: 1;
    background: var(--accent);
    color: white;
    font-weight: 600;
  }
  .save:disabled {
    opacity: 0.45;
    cursor: default;
  }
  .delete {
    background: var(--surface);
    color: var(--danger);
  }
  .error {
    color: var(--danger);
  }
</style>
