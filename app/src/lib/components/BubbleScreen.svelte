<script lang="ts">
  // Android chat bubble: one chat, alone in a small floating window (BubbleActivity). The app
  // itself may be running in its own window meanwhile: nothing here takes over what only one
  // window can have (the chat list, encryption status, calls, push); the core's one open chat
  // goes to whichever window is on screen (chat.svelte.ts).
  import ChatView from "./ChatView.svelte";
  import { tr } from "$lib/i18n/index.svelte";
  import { bubbleRoom, leaveBubble, roomSummary, type Account, type RoomSummary } from "$lib/backend";
  import { drafts } from "$lib/stores/drafts.svelte";
  import { saved } from "$lib/stores/saved.svelte";
  import { me } from "$lib/stores/me.svelte";
  import { viewOnce } from "$lib/stores/viewOnce.svelte";
  import { chat } from "$lib/stores/chat.svelte";
  import { onMount } from "svelte";

  // Same props as ChatsScreen (the page loads either); the bubble needs neither.
  let _props: { account: Account; onSignOut: () => void } = $props();

  let room = $state<RoomSummary | null>(null);
  let error = $state<string | null>(null);

  async function load() {
    try {
      room = await roomSummary(bubbleRoom!);
      error = null;
    } catch (e) {
      error = String(e);
    }
  }

  onMount(() => {
    void load();
    void me.refresh();
    void drafts.start();
    saved.start();
    void viewOnce.load();
    // Name, photo, unread count may have changed while folded.
    const refresh = () => document.visibilityState === "visible" && void load();
    document.addEventListener("visibilitychange", refresh);
    return () => {
      document.removeEventListener("visibilitychange", refresh);
      chat.close();
    };
  });
</script>

{#if room}
  <div class="bubble-chat">
    <ChatView {room} onBack={() => void leaveBubble({})} />
  </div>
{:else if error}
  <div class="state">
    <p>{tr("This chat can't be opened here.")}</p>
    <button onclick={() => void leaveBubble({ roomId: bubbleRoom! })}>{tr("Open in AlwyChat")}</button>
  </div>
{/if}

<style>
  .bubble-chat {
    display: flex;
    flex-direction: column;
    height: 100dvh;
    overflow: hidden;
    background: var(--bg);
  }
  .state {
    display: grid;
    gap: 12px;
    place-content: center;
    height: 100dvh;
    padding: 24px;
    color: var(--muted);
    text-align: center;
  }
  .state button {
    padding: 10px 18px;
    border: none;
    border-radius: 999px;
    background: var(--accent);
    color: white;
    font: inherit;
    cursor: pointer;
  }
</style>
