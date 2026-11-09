<script lang="ts">
  import { tr } from "$lib/i18n/index.svelte";
  import Minus from "@lucide/svelte/icons/minus";
  import Square from "@lucide/svelte/icons/square";
  import Copy from "@lucide/svelte/icons/copy";
  import X from "@lucide/svelte/icons/x";
  import { windowClose, windowIsMaximized, windowMinimize, windowToggleMaximize } from "$lib/backend";

  /** Computer apps have no title bar: the panes reach the top of the window, their headers
   * move it (`data-tauri-drag-region`), and these buttons float in the corner. Headers there
   * keep clear of them with `--window-controls`. Closing goes through the usual close (hides
   * to the tray where there is one). */
  let maximized = $state(false);
  const refresh = () => windowIsMaximized().then((m) => (maximized = m), () => {});
  $effect(() => {
    void refresh();
    document.documentElement.style.setProperty("--window-controls", "112px");
  });
</script>

<svelte:window onresize={refresh} />

<!-- The very top edge moves the window on every screen, even those without a header. -->
<div class="edge" data-tauri-drag-region></div>
<div class="buttons">
  <button onclick={() => windowMinimize()} aria-label={tr("Minimize")} title={tr("Minimize")}><Minus size={15} strokeWidth={1.5} /></button>
  <button onclick={() => windowToggleMaximize()} aria-label={maximized ? tr("Restore") : tr("Maximize")} title={maximized ? tr("Restore") : tr("Maximize")}>
    {#if maximized}<Copy size={12} strokeWidth={1.5} />{:else}<Square size={12} strokeWidth={1.5} />{/if}
  </button>
  <button class="close" onclick={() => windowClose()} aria-label={tr("Close")} title={tr("Close")}><X size={16} strokeWidth={1.5} /></button>
</div>

<style>
  .edge {
    position: fixed;
    top: 0;
    right: 0;
    left: 0;
    z-index: 2999;
    height: 6px;
  }
  /* Over any header, photo or chat theme: its own backing keeps it readable (no blur: that
     slows drawing over chat themes). */
  .buttons {
    position: fixed;
    top: 12px;
    right: 12px;
    z-index: 3000;
    display: flex;
    gap: 1px;
    padding: 2px;
    border-radius: 10px;
    background: color-mix(in srgb, var(--surface) 88%, transparent);
  }
  button {
    display: grid;
    width: 30px;
    height: 26px;
    place-items: center;
    border: none;
    border-radius: 8px;
    background: none;
    color: var(--muted);
    transition: background 0.12s var(--ease), color 0.12s var(--ease);
  }
  button:hover {
    background: var(--surface-hover);
    color: var(--text);
  }
  .close:hover {
    background: #e5484d;
    color: #fff;
  }
</style>
