<script lang="ts">
  import { untrack } from "svelte";
  import { createRoom, setRoomAvatar, type UserResult } from "$lib/backend";
  import { chatList } from "$lib/stores/chatList.svelte";
  import AvatarEdit from "./AvatarEdit.svelte";
  import Page from "./Page.svelte";
  import UserPicker from "./UserPicker.svelte";

  /** New group chat or space; `parent`: created inside that space. */
  let {
    kind,
    parent = null,
    parentName = "",
    onClose,
    onCreated,
  }: {
    kind: "group" | "space";
    parent?: string | null;
    parentName?: string;
    onClose: () => void;
    onCreated: (roomId: string) => void;
  } = $props();

  let name = $state("");
  let topic = $state("");
  let access = $state<"private" | "space" | "public">(untrack(() => (parent ? "space" : "private")));
  let encrypted = $state(true);
  let photo = $state<string | null>(null);
  let invite = $state<UserResult[]>([]);
  let busy = $state(false);
  let error = $state<string | null>(null);

  const what = $derived(kind === "space" ? "space" : "group");

  async function create() {
    if (!name.trim() || busy) return;
    busy = true;
    error = null;
    try {
      const id = await createRoom({
        kind,
        name,
        topic,
        access,
        encrypted: kind === "group" && encrypted && access !== "public",
        parent,
        invite: invite.map((u) => u.user_id),
      });
      // The photo needs the room to exist (and us to be in it).
      if (photo) await setRoomAvatar(id, photo).catch((e) => console.warn("room photo", e));
      // Spaces don't show in the chat list; chats do once the sync brings them.
      if (kind === "group") {
        const started = Date.now();
        while (!chatList.rooms.some((r) => r.id === id) && Date.now() - started < 10_000) {
          await new Promise((r) => setTimeout(r, 250));
        }
      }
      onCreated(id);
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }
</script>

<Page title={kind === "space" ? "New space" : parent ? `New chat in ${parentName}` : "New group"} {onClose}>
  <AvatarEdit name={name || "?"} mxc={null} square={kind === "space"} onPicked={(data) => void (photo = data)} />
  <h3 class="m-section">Name</h3>
  <input class="m-input" bind:value={name} maxlength="80" placeholder={kind === "space" ? "My space" : "Group name"} />
  <h3 class="m-section">Topic <small>(optional)</small></h3>
  <textarea class="m-input" bind:value={topic} maxlength="500" placeholder="What is it about?"></textarea>

  <h3 class="m-section">Who can join</h3>
  <div class="m-seg">
    <button class:on={access === "private"} onclick={() => (access = "private")}>Invited only</button>
    {#if parent}<button class:on={access === "space"} onclick={() => (access = "space")}>Space members</button>{/if}
    <button class:on={access === "public"} onclick={() => (access = "public")}>Anyone with the link</button>
  </div>
  <p class="m-hint">
    {#if access === "private"}Only people you invite can join.
    {:else if access === "space"}Everyone in {parentName} can find and join it.
    {:else}Anyone who has the link (or finds the address) can join.{/if}
  </p>

  {#if kind === "group"}
    <label class="m-row">
      <span class="m-text"
        ><strong>End-to-end encrypted</strong><small
          >{access === "public"
            ? "Not for chats anyone can join: new members couldn't read the history."
            : "Only members can read messages. It can't be turned off later."}</small
        ></span
      >
      <input type="checkbox" role="switch" bind:checked={encrypted} disabled={access === "public"} />
    </label>
  {:else}
    <p class="m-hint">A space groups chats together, like a Discord server. You can add chats and voice channels once it's created.</p>
  {/if}

  <h3 class="m-section">Invite people <small>(optional)</small></h3>
  <UserPicker multiple bind:picked={invite} />

  {#if error}<p class="m-error">{error}</p>{/if}
  <div class="m-buttons sticky">
    <button class="m-btn" disabled={!name.trim() || busy} onclick={create}>{busy ? "Creating…" : `Create ${what}`}</button>
  </div>
</Page>

<style>
  small {
    font-weight: 400;
    text-transform: none;
    letter-spacing: 0;
  }
  .sticky {
    position: sticky;
    bottom: 0;
    padding: 12px 0 6px;
    background: var(--bg);
  }
</style>
