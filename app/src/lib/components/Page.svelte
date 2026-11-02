<script lang="ts" module>
  import { getContext, setContext } from "svelte";

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
  import type { Snippet } from "svelte";
  import { fly } from "svelte/transition";
  import { sidePanels } from "$lib/stores/panel.svelte";

  /** A full-screen page (settings and the like) that slides in over the app. */
  let {
    title,
    onClose,
    children,
    layout = getContext<PageLayout | undefined>(LAYOUT) ?? "full",
  }: { title: string; onClose: () => void; children: Snippet; layout?: PageLayout } = $props();

  // A side panel makes the chats layout narrower rather than covering the chat.
  $effect(() => {
    if (layout !== "panel") return;
    sidePanels.open++;
    return () => void sidePanels.open--;
  });
</script>

<div class="page {layout}" role="dialog" aria-modal="true" aria-label={title} transition:fly={{ x: 48, duration: 220 }}>
  <header>
    <button class="back" onclick={onClose} aria-label={tr("Back")}><ArrowLeft size={22} /></button>
    <h2>{title}</h2>
  </header>
  <div class="body">
    <div class="inner">{@render children()}</div>
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
    padding: 10px 12px;
    border-bottom: 1px solid var(--border);
  }
  h2 {
    margin: 0;
    font-size: 1.25rem;
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
