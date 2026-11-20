<script lang="ts">
  // One tile of the call's reactions or soundboard panels (both look the same): emoji over a
  // name. `onHold` = long-press on touch, right-click with a mouse (remove / hide).
  import type { Snippet } from "svelte";
  import Volume2 from "@lucide/svelte/icons/volume-2";
  import { tr } from "$lib/i18n/index.svelte";

  let {
    emoji,
    label,
    onclick,
    onHold,
    playing = false,
    disabled = false,
    add = false,
    sound = false,
    icon,
  }: {
    emoji?: string;
    label: string;
    onclick: () => void;
    onHold?: () => void;
    playing?: boolean;
    disabled?: boolean;
    /** The dashed "+" tile. */
    add?: boolean;
    /** A reaction that plays a sound: small speaker in the corner. */
    sound?: boolean;
    icon?: Snippet;
  } = $props();

  let timer: ReturnType<typeof setTimeout> | undefined;
  let held = false;
  function down() {
    held = false;
    if (!onHold) return;
    timer = setTimeout(() => {
      held = true;
      onHold?.();
    }, 550);
  }
  const up = () => clearTimeout(timer);
</script>

<button
  class="tile"
  class:add
  class:playing
  {disabled}
  onclick={() => !held && onclick()}
  oncontextmenu={(e) => {
    if (!onHold) return;
    e.preventDefault();
    onHold();
  }}
  onpointerdown={down}
  onpointerup={up}
  onpointerleave={up}
  onpointercancel={up}
>
  <span class="emoji">{#if icon}{@render icon()}{:else}{emoji}{/if}</span>
  <span class="name">{label}</span>
  {#if sound}<span class="badge" title={tr("Plays a sound")} aria-label={tr("Plays a sound")}><Volume2 size={11} /></span>{/if}
</button>

<style>
  .tile {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    min-width: 0;
    padding: 9px 4px 7px;
    border: 1px solid transparent;
    border-radius: 14px;
    background: var(--surface-hover);
    color: inherit;
    font: inherit;
    cursor: pointer;
    -webkit-touch-callout: none;
    transition:
      transform 0.12s ease,
      border-color 0.2s ease;
  }
  .tile:active {
    transform: scale(0.94);
  }
  .tile.playing {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 30%, transparent);
  }
  .tile:disabled {
    opacity: 0.4;
  }
  .tile.add {
    border: 1px dashed var(--border);
    background: none;
  }
  .emoji {
    font-size: 1.6rem;
    line-height: 1;
    display: grid;
    place-items: center;
    min-height: 1.6rem;
  }
  .name {
    font-size: 0.72rem;
    color: var(--muted);
    text-align: center;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 100%;
  }
  .badge {
    position: absolute;
    top: 5px;
    right: 6px;
    display: grid;
    place-items: center;
    color: var(--muted);
    opacity: 0.85;
  }
</style>
