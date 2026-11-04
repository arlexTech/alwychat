<script lang="ts">
  // A space category (a sub-space shown as a section): its name, and which of the space's
  // chats it holds. Moving a chat = listing it in the category instead of the space.
  import { tr } from "$lib/i18n/index.svelte";
  import { addToSpace, leaveRoom, removeFromSpace, setRoomName, type SpaceCategory, type SpaceChild } from "$lib/backend";
  import Page from "./Page.svelte";
  import { untrack } from "svelte";
  import Avatar from "./Avatar.svelte";

  let {
    spaceId,
    category,
    loose,
    onChanged,
    onClose,
  }: {
    spaceId: string;
    category: SpaceCategory;
    /** The space's own chats (in no category): can move in. */
    loose: SpaceChild[];
    onChanged: () => void;
    onClose: () => void;
  } = $props();

  let name = $state(untrack(() => category.name));
  let busy = $state<string | null>(null);
  let error = $state<string | null>(null);

  async function rename() {
    if (!name.trim() || name.trim() === category.name) return;
    busy = "name";
    error = null;
    try {
      await setRoomName(category.id, name);
      onChanged();
    } catch (e) {
      error = String(e);
    } finally {
      busy = null;
    }
  }

  let confirmDelete = $state(false);
  /** Its chats go back to the space first; then it's taken out of the space and left. */
  async function remove() {
    busy = "delete";
    error = null;
    try {
      for (const chat of category.children) {
        await addToSpace(spaceId, chat.id);
        await removeFromSpace(category.id, chat.id).catch(() => {});
      }
      await removeFromSpace(spaceId, category.id);
      await leaveRoom(category.id).catch(() => {});
      onChanged();
      onClose();
    } catch (e) {
      error = String(e);
    } finally {
      busy = null;
    }
  }

  /** Into the category (out of the space's own list), or back out. */
  async function move(chat: SpaceChild, into: boolean) {
    busy = chat.id;
    error = null;
    try {
      if (into) {
        await addToSpace(category.id, chat.id);
        await removeFromSpace(spaceId, chat.id);
      } else {
        await addToSpace(spaceId, chat.id);
        await removeFromSpace(category.id, chat.id);
      }
      onChanged();
    } catch (e) {
      error = String(e);
    } finally {
      busy = null;
    }
  }
</script>

<Page title={tr("Category")} {onClose}>
  <form class="rename" onsubmit={(e) => (e.preventDefault(), rename())}>
    <input class="m-input" bind:value={name} maxlength="60" placeholder={tr("Category name")} />
    <button class="m-btn" disabled={busy === "name" || !name.trim() || name.trim() === category.name}>{tr("Rename")}</button>
  </form>
  {#if error}<p class="m-error">{error}</p>{/if}

  <h3 class="m-section">{tr("In this category")}</h3>
  {#each category.children.filter((c) => !c.is_space) as chat (chat.id)}
    <div class="row">
      <Avatar name={chat.name} mxc={chat.avatar} size={36} />
      <span class="name">{chat.name}</span>
      <button class="m-btn" disabled={busy === chat.id} onclick={() => move(chat, false)}>{tr("Take out")}</button>
    </div>
  {:else}
    <p class="m-hint">{tr("Nothing yet: add chats from the space below.")}</p>
  {/each}

  <h3 class="m-section">{tr("Other chats of the space")}</h3>
  {#each loose as chat (chat.id)}
    <div class="row">
      <Avatar name={chat.name} mxc={chat.avatar} size={36} />
      <span class="name">{chat.name}</span>
      <button class="m-btn" disabled={busy === chat.id} onclick={() => move(chat, true)}>{tr("Move here")}</button>
    </div>
  {:else}
    <p class="m-hint">{tr("All of the space's chats are in categories.")}</p>
  {/each}

  <h3 class="m-section">{tr("Delete")}</h3>
  {#if confirmDelete}
    <p class="m-hint">{tr("Its chats go back to the space; nothing is deleted from them.")}</p>
    <div class="buttons">
      <button class="m-btn" onclick={() => (confirmDelete = false)}>{tr("Cancel")}</button>
      <button class="m-btn danger" disabled={busy === "delete"} onclick={remove}>{busy === "delete" ? tr("Deleting…") : tr("Delete category")}</button>
    </div>
  {:else}
    <button class="m-btn danger" onclick={() => (confirmDelete = true)}>{tr("Delete category")}</button>
  {/if}
</Page>

<style>
  .rename {
    display: flex;
    gap: 8px;
  }
  .rename input {
    flex: 1;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 6px 4px;
  }
  .buttons {
    display: flex;
    gap: 8px;
  }
  .danger {
    color: var(--danger);
  }
  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
