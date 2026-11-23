<script lang="ts">
  // Chat settings (Android 11+): open this chat as a bubble / pick it to bubble — or, inside a
  // bubble, open it in the app.
  import { tr } from "$lib/i18n/index.svelte";
  import { bubbleStatus, isBubble, leaveBubble, setChatBubble, type BubbleStatus, type RoomSummary } from "$lib/backend";
  import { canBubble, floatChat } from "$lib/bubble";
  import MessageCircleMore from "@lucide/svelte/icons/message-circle-more";
  import Maximize2 from "@lucide/svelte/icons/maximize-2";

  let { room }: { room: RoomSummary } = $props();

  let status = $state<BubbleStatus | null>(null);
  let error = $state<string | null>(null);
  const load = () => void bubbleStatus().then((s) => (status = s)).catch(() => {});
  $effect(() => {
    if (!canBubble) return;
    load();
    const again = () => document.visibilityState === "visible" && load();
    document.addEventListener("visibilitychange", again);
    return () => document.removeEventListener("visibilitychange", again);
  });

  async function float() {
    error = null;
    try {
      await floatChat(room);
      load();
    } catch (e) {
      error = String(e);
    }
  }
</script>

{#if isBubble}
  <button class="m-row" onclick={() => void leaveBubble({ roomId: room.id })}>
    <span class="m-icon"><Maximize2 size={20} /></span>
    <span class="m-text"><strong>{tr("Open in AlwyChat")}</strong></span>
  </button>
{:else if canBubble && status?.supported && status.on && status.system !== "none"}
  <button class="m-row" onclick={float}>
    <span class="m-icon"><MessageCircleMore size={20} /></span>
    <span class="m-text"><strong>{tr("Open as a bubble")}</strong><small>{tr("Floats over your other apps")}</small></span>
  </button>
  <label class="m-row">
    <span class="m-icon"></span>
    <span class="m-text">
      <strong>{tr("Bubble this chat")}</strong>
      <small>
        {#if status.all}{tr("All chats can bubble (Settings → Notifications).")}
        {:else}{tr("Its new messages can float as a bubble.")}{/if}
      </small>
    </span>
    <input
      type="checkbox"
      role="switch"
      disabled={status.all}
      checked={status.all || status.rooms.includes(room.id)}
      onchange={(e) => void setChatBubble(room.id, e.currentTarget.checked).then(load, (err) => (error = String(err)))}
    />
  </label>
{/if}
{#if error}<p class="m-error">{error}</p>{/if}
