<script lang="ts">
  // Draws the app's right-click menu (stores/menu.svelte.ts), kept inside the window.
  import { tr } from "$lib/i18n/index.svelte";
  import { contextMenu } from "$lib/stores/menu.svelte";
  import { forwardMessages, type RoomSummary } from "$lib/backend";
  import { onBack } from "$lib/back.svelte";
  import { fade } from "svelte/transition";
  import ForwardPage from "./ForwardPage.svelte";

  const menu = $derived(contextMenu.open);
  let box = $state<HTMLElement>();
  const left = $derived(menu ? Math.max(8, Math.min(menu.x, innerWidth - (box?.offsetWidth ?? 220) - 8)) : 0);
  const top = $derived(menu ? Math.max(8, Math.min(menu.y, innerHeight - (box?.offsetHeight ?? 240) - 8)) : 0);

  $effect(() => {
    if (contextMenu.open) return onBack(() => contextMenu.close());
  });
  $effect(() => {
    if (contextMenu.forwarding) return onBack(() => (contextMenu.forwarding = null));
  });

  async function forwardTo(to: RoomSummary) {
    const f = contextMenu.forwarding;
    contextMenu.forwarding = null;
    if (!f) return;
    try {
      const sent = await forwardMessages(f.roomId, f.eventIds, to.id);
      contextMenu.flash(sent ? tr("Forwarded to {name}", { name: to.name }) : tr("Couldn't forward that message"));
    } catch (e) {
      contextMenu.flash(tr("Couldn't forward: {e}", { e: e }));
    }
  }
</script>

<svelte:window onkeydown={(e) => menu && e.key === "Escape" && contextMenu.close()} onresize={() => contextMenu.close()} />

{#if menu}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="catcher" onclick={() => contextMenu.close()} oncontextmenu={(e) => (e.preventDefault(), contextMenu.close())}></div>
  <div class="menu" role="menu" bind:this={box} style:left="{left}px" style:top="{top}px" transition:fade={{ duration: 100 }}>
    {#if menu.title}<div class="title">{menu.title}</div>{/if}
    {#each menu.items as item, i (i)}
      {#if item === "-"}
        <hr />
      {:else}
        <button role="menuitem" class:danger={item.danger} onclick={() => contextMenu.run(item)}>
          {#if item.icon}<item.icon size={16} />{/if}<span>{item.label}</span>
        </button>
      {/if}
    {/each}
  </div>
{/if}

{#if contextMenu.forwarding}
  <ForwardPage onPick={forwardTo} onClose={() => (contextMenu.forwarding = null)} />
{/if}

{#if contextMenu.toast}
  <div class="toast" role="status" transition:fade={{ duration: 150 }}>{contextMenu.toast}</div>
{/if}

<style>
  .catcher {
    position: fixed;
    inset: 0;
    z-index: 190;
  }
  .menu {
    position: fixed;
    z-index: 191;
    display: flex;
    flex-direction: column;
    min-width: 200px;
    max-width: min(320px, calc(100vw - 16px));
    padding: 6px;
    border: 1px solid var(--border);
    border-radius: 12px;
    background: var(--surface-2);
    color: var(--text);
    box-shadow: 0 12px 32px rgb(0 0 0 / 0.45);
  }
  .title {
    overflow: hidden;
    padding: 6px 10px 4px;
    color: var(--muted);
    font-size: 0.75rem;
    font-weight: 600;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  button {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 9px 10px;
    border: none;
    border-radius: 8px;
    background: none;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  button span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  button:hover {
    background: var(--surface-hover);
  }
  button.danger {
    color: #e5484d;
  }
  hr {
    width: 100%;
    margin: 4px 0;
    border: none;
    border-top: 1px solid var(--border);
  }
  .toast {
    position: fixed;
    bottom: calc(24px + env(safe-area-inset-bottom));
    left: 50%;
    z-index: 192;
    max-width: calc(100vw - 32px);
    padding: 10px 16px;
    border-radius: 999px;
    background: var(--surface-2);
    color: var(--text);
    box-shadow: 0 8px 24px rgb(0 0 0 / 0.35);
    font-size: 0.88rem;
    transform: translateX(-50%);
  }
</style>
