<script lang="ts" module>
  import { getContext, setContext, untrack } from "svelte";

  /**
   * How pages sit on a wide screen (900 px+; phones always get full-screen pages):
   * `full` covers the app; `list` / `section` / `detail` are app settings, Discord style (the
   * list stays on the left, the chosen section on the right, pages opened from a section over
   * it); `panel` is chat settings, Messenger style (a column on the right, the chat visible).
   */
  export type PageLayout = "full" | "list" | "section" | "detail" | "panel";
  const LAYOUT = Symbol("page layout");

  /** Every page opened inside the calling component (and below) uses `layout`. */
  export function setPageLayout(layout: PageLayout) {
    setContext(LAYOUT, layout);
  }
</script>

<script lang="ts">
  import { tr } from "$lib/i18n/index.svelte";
  import ArrowLeft from "@lucide/svelte/icons/arrow-left";
  import X from "@lucide/svelte/icons/x";
  import type { Snippet } from "svelte";
  import { fly } from "svelte/transition";
  import { sidePanels } from "$lib/stores/panel.svelte";
  import { onBack } from "$lib/back.svelte";

  /** A full-screen page (settings and the like) that slides in over the app. */
  let {
    title,
    onClose,
    children,
    layout = getContext<PageLayout | undefined>(LAYOUT) ?? "full",
    cancel = false,
    actions,
  }: {
    title: string;
    onClose: () => void;
    children: Snippet;
    layout?: PageLayout;
    /** Closing abandons something (a share from another app): ✕ "Cancel" instead of ←. */
    cancel?: boolean;
    /** Buttons on the right of the title (search…). */
    actions?: Snippet;
  } = $props();

  // Android's back gesture does what the ← (or Cancel) button does, for every page: pages that
  // forgot to register it let the gesture fall through to what was under them (a profile opened
  // from a chat closed the chat instead). Registered after its opener's layers, so it's on top.
  $effect(() => onBack(() => onClose()));

  // A side panel makes the chats layout narrower rather than covering the chat.
  $effect(() => {
    if (layout !== "panel") return;
    // untrack: `++` also reads it, and the effect would re-run itself without end.
    untrack(() => sidePanels.open++);
    return () => void sidePanels.open--;
  });
</script>

<div class="page {layout}" role="dialog" aria-modal="true" aria-label={title} transition:fly={{ x: 48, duration: 220 }}>
  <header data-tauri-drag-region="deep">
    {#if cancel}
      <h2>{title}</h2>
      <button class="cancel" onclick={onClose}><X size={18} />{tr("Cancel")}</button>
    {:else}
      <button class="back" onclick={onClose} aria-label={tr("Back")}><ArrowLeft size={22} /></button>
      <h2>{title}</h2>
      {#if actions}<div class="actions">{@render actions()}</div>{/if}
    {/if}
  </header>
  <div class="body">
    <!-- A failing page says so (it used to just not open: "nothing happens"). -->
    <div class="inner">
      <svelte:boundary onerror={(e) => console.error("page", e)}>
        {@render children()}
        {#snippet failed(error, reset)}
          <p class="m-error">{tr("Something went wrong on this page: {error}", { error: String((error as Error)?.message ?? error) })}</p>
          <button class="m-btn" onclick={reset}>{tr("Try again")}</button>
        {/snippet}
      </svelte:boundary>
    </div>
  </div>
</div>

<style>
  .page {
    position: fixed;
    inset: 0;
    z-index: 50;
    display: flex;
    flex-direction: column;
    background: var(--bg);
  }
  header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px calc(12px + var(--window-controls)) 10px 12px;
    border-bottom: 1px solid var(--border);
  }
  .cancel {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-left: auto;
    padding: 8px 14px;
    border: none;
    border-radius: 999px;
    background: var(--surface-2);
    color: var(--text);
    font: inherit;
    font-weight: 600;
    cursor: pointer;
  }
  h2:has(+ .cancel) {
    padding-left: 8px;
  }
  h2 {
    margin: 0;
    font-size: 1.25rem;
  }
  .actions {
    display: flex;
    align-items: center;
    gap: 4px;
    margin-left: auto;
  }
  .back {
    display: grid;
    place-items: center;
    padding: 0;
    color: inherit;
    width: 42px;
    height: 42px;
    border: none;
    border-radius: 50%;
    background: none;
    cursor: pointer;
  }
  .back:hover {
    background: var(--surface-hover);
  }
  .body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    overscroll-behavior: contain;
  }
  .inner {
    max-width: 640px;
    margin-inline: auto;
    padding: 16px 20px 32px;
  }

  @media (min-width: 900px) {
    .page.list {
      right: auto;
      width: var(--settings-list);
      border-right: 1px solid var(--border);
      background: var(--surface);
    }
    .page.section,
    .page.detail {
      left: var(--settings-list);
    }
    .page.section .inner,
    .page.detail .inner {
      max-width: 760px;
      margin-inline: 32px auto;
      padding-top: 24px;
    }
    /* The section is chosen in the list: its back arrow would only empty the right side. */
    .page.section .back {
      display: none;
    }
    .page.section header {
      padding-left: 32px;
    }
    .page.panel {
      left: auto;
      width: var(--side-panel);
      border-left: 1px solid var(--border);
      box-shadow: -16px 0 40px rgb(0 0 0 / 0.25);
    }
  }
</style>
