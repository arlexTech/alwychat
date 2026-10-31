<script lang="ts">
  import { fade, fly } from "svelte/transition";
  import Copy from "@lucide/svelte/icons/copy";
  import Download from "@lucide/svelte/icons/download";
  import Forward from "@lucide/svelte/icons/forward";
  import History from "@lucide/svelte/icons/history";
  import Pencil from "@lucide/svelte/icons/pencil";
  import Pin from "@lucide/svelte/icons/pin";
  import PinOff from "@lucide/svelte/icons/pin-off";
  import Reply from "@lucide/svelte/icons/reply";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import MessagesSquare from "@lucide/svelte/icons/messages-square";
  import type { MessageEntry } from "$lib/backend";

  export interface Actions {
    react: (key: string) => void;
    reply?: () => void;
    thread?: () => void;
    copy?: () => void;
    forward?: () => void;
    /** Present when we may pin in this chat. */
    pin?: { pinned: boolean; toggle: () => void };
    save?: { label: string; run: () => void };
    edit?: () => void;
    history?: () => void;
    delete?: () => void;
  }

  let { msg, quickReaction, actions, onClose }: {
    msg: MessageEntry;
    quickReaction: string;
    actions: Actions;
    onClose: () => void;
  } = $props();

  const reactions = $derived([...new Set(["❤️", "😂", "😮", "😢", "😡", "👍", quickReaction])]);
  const mine = $derived(new Set(msg.reactions.filter((r) => r.mine).map((r) => r.key)));

  /** Act, then close: `actions` is derived from the message, which is gone once closed. */
  function run(action: () => void) {
    action();
    onClose();
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="backdrop" transition:fade={{ duration: 140 }} onclick={onClose}></div>
<div class="sheet" role="dialog" aria-label="Message actions" transition:fly={{ y: 40, duration: 200 }}>
  <div class="grip"></div>
  <div class="reactions">
    {#each reactions as key (key)}
      <button class:mine={mine.has(key)} onclick={() => run(() => actions.react(key))}>{key}</button>
    {/each}
  </div>
  <div class="list">
    {#if actions.reply}<button onclick={() => run(actions.reply!)}><Reply size={20} />Reply</button>{/if}
    {#if actions.thread}<button onclick={() => run(actions.thread!)}><MessagesSquare size={20} />Reply in thread</button>{/if}
    {#if actions.copy}<button onclick={() => run(actions.copy!)}><Copy size={20} />Copy text</button>{/if}
    {#if actions.forward}<button onclick={() => run(actions.forward!)}><Forward size={20} />Forward</button>{/if}
    {#if actions.pin}
      {@const pin = actions.pin}
      <button onclick={() => run(pin.toggle)}>
        {#if pin.pinned}<PinOff size={20} />Unpin{:else}<Pin size={20} />Pin{/if}
      </button>
    {/if}
    {#if actions.save}
      {@const save = actions.save}
      <button onclick={() => run(save.run)}><Download size={20} />{save.label}</button>
    {/if}
    {#if actions.edit}<button onclick={() => run(actions.edit!)}><Pencil size={20} />Edit</button>{/if}
    {#if actions.history}<button onclick={() => run(actions.history!)}><History size={20} />Edit history</button>{/if}
    {#if actions.delete}<button class="danger" onclick={() => run(actions.delete!)}><Trash2 size={20} />Delete</button>{/if}
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 40;
    background: rgb(0 0 0 / 0.45);
  }
  .sheet {
    position: fixed;
    left: 50%;
    bottom: 0;
    z-index: 41;
    width: min(520px, 100%);
    padding: 8px 12px 16px;
    border-radius: 22px 22px 0 0;
    background: var(--surface);
    box-shadow: 0 -12px 40px rgb(0 0 0 / 0.4);
    translate: -50% 0;
  }
  .grip {
    width: 36px;
    height: 4px;
    margin: 0 auto 10px;
    border-radius: 2px;
    background: var(--border);
  }
  .reactions {
    display: flex;
    justify-content: space-between;
    gap: 4px;
    padding: 4px 4px 12px;
    border-bottom: 1px solid var(--border);
  }
  .reactions button {
    width: 44px;
    height: 44px;
    border: none;
    border-radius: 50%;
    background: var(--surface-2);
    font-size: 1.5rem;
    cursor: pointer;
    transition: transform 0.12s var(--ease);
  }
  .reactions button:active {
    transform: scale(0.85);
  }
  .reactions button.mine {
    box-shadow: inset 0 0 0 2px var(--accent-a);
  }
  .list {
    display: flex;
    flex-direction: column;
    padding-top: 6px;
  }
  .list button {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 13px 10px;
    border: none;
    border-radius: 12px;
    background: none;
    color: inherit;
    font-size: 1rem;
    text-align: left;
    cursor: pointer;
  }
  .list button:active,
  .list button:hover {
    background: var(--surface-hover);
  }
  .list :global(svg) {
    color: var(--muted);
  }
  .danger,
  .danger :global(svg) {
    color: var(--danger) !important;
  }
</style>
