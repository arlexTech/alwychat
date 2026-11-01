<script lang="ts">
  import Hash from "@lucide/svelte/icons/hash";
  import LayoutGrid from "@lucide/svelte/icons/layout-grid";
  import Users from "@lucide/svelte/icons/users";
  import { onBack } from "$lib/back.svelte";
  import { joinAddress, openDm, type UserResult } from "$lib/backend";
  import { chatList } from "$lib/stores/chatList.svelte";
  import CreateRoomPage from "./CreateRoomPage.svelte";
  import Page from "./Page.svelte";
  import UserPicker from "./UserPicker.svelte";

  /** "New chat": a 1:1 with someone, a new group or space, or joining by address. */
  let { onClose, onOpen }: { onClose: () => void; onOpen: (roomId: string, isSpace?: boolean) => void } = $props();

  let creating = $state<"group" | "space" | null>(null);
  let joining = $state(false);
  let address = $state("");
  let busy = $state(false);
  let error = $state<string | null>(null);

  $effect(() => {
    if (creating) return onBack(() => (creating = null));
  });

  async function waitFor(id: string) {
    const started = Date.now();
    while (!chatList.rooms.some((r) => r.id === id) && Date.now() - started < 10_000) {
      await new Promise((r) => setTimeout(r, 250));
    }
  }

  async function message(user: UserResult) {
    busy = true;
    error = null;
    try {
      const id = await openDm(user.user_id);
      await waitFor(id);
      onOpen(id);
    } catch (e) {
      error = `Couldn't start the chat: ${e}`;
    } finally {
      busy = false;
    }
  }

  async function join() {
    if (!address.trim()) return;
    busy = true;
    error = null;
    try {
      const id = await joinAddress(address);
      await waitFor(id);
      onOpen(id, !chatList.rooms.some((r) => r.id === id));
    } catch (e) {
      error = `Couldn't join: ${e}`;
    } finally {
      busy = false;
    }
  }
</script>

<Page title="New chat" {onClose}>
  <button class="m-row" onclick={() => (creating = "group")}>
    <span class="m-icon"><Users size={20} /></span><span class="m-text"><strong>New group</strong></span>
  </button>
  <button class="m-row" onclick={() => (creating = "space")}>
    <span class="m-icon"><LayoutGrid size={20} /></span
    ><span class="m-text"><strong>New space</strong><small>Group chats and voice channels together</small></span>
  </button>
  <button class="m-row" onclick={() => (joining = !joining)}>
    <span class="m-icon"><Hash size={20} /></span
    ><span class="m-text"><strong>Join with an address</strong><small>#name:server, or a matrix.to link</small></span>
  </button>
  {#if joining}
    <form class="join" onsubmit={(e) => (e.preventDefault(), join())}>
      <input class="m-input" bind:value={address} placeholder="#room:server or https://matrix.to/#/…" autocapitalize="off" spellcheck="false" />
      <button class="m-btn" disabled={!address.trim() || busy}>Join</button>
    </form>
  {/if}

  <h3 class="m-section">Message someone</h3>
  <UserPicker onPick={(u) => !busy && message(u)} />
  {#if error}<p class="m-error">{error}</p>{/if}
</Page>

{#if creating}
  <CreateRoomPage
    kind={creating}
    onClose={() => (creating = null)}
    onCreated={(id) => onOpen(id, creating === "space")}
  />
{/if}

<style>
  .join {
    display: flex;
    gap: 8px;
    margin: 4px 0 8px;
  }
</style>
