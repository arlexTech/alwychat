<script lang="ts" generics="T extends { id: string; name: string; avatar: string | null; icon?: Component<{ size?: number }> }">
  import GripVertical from "@lucide/svelte/icons/grip-vertical";
  import ChevronUp from "@lucide/svelte/icons/chevron-up";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import { tr } from "$lib/i18n/index.svelte";
  import Avatar from "./Avatar.svelte";
  import type { Component } from "svelte";

  /** Rows dragged by their grip (finger or mouse); arrows for keyboards. `onMove` gets the new order. */
  let {
    items,
    square = false,
    disabled = false,
    onMove,
  }: { items: T[]; square?: boolean; disabled?: boolean; onMove: (ids: string[]) => void } = $props();

  /** The row being dragged: where it started, how far it moved, where it would land. */
  let drag = $state<{ from: number; to: number; dy: number; height: number; startY: number; pointer: number } | null>(null);

  function down(e: PointerEvent, index: number) {
    if (disabled || e.button > 0) return;
    const row = (e.currentTarget as HTMLElement).closest(".row") as HTMLElement;
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    drag = { from: index, to: index, dy: 0, height: row.offsetHeight, startY: e.clientY, pointer: e.pointerId };
    navigator.vibrate?.(10);
  }
  function move(e: PointerEvent) {
    if (!drag || e.pointerId !== drag.pointer) return;
    const dy = e.clientY - drag.startY;
    const to = Math.max(0, Math.min(items.length - 1, drag.from + Math.round(dy / drag.height)));
    if (to !== drag.to) navigator.vibrate?.(5);
    drag = { ...drag, dy, to };
  }
  function up(e: PointerEvent) {
    if (!drag || e.pointerId !== drag.pointer) return;
    const { from, to } = drag;
    drag = null;
    if (from !== to) commit(from, to);
  }
  function commit(from: number, to: number) {
    const ids = items.map((i) => i.id);
    const [moved] = ids.splice(from, 1);
    ids.splice(to, 0, moved);
    onMove(ids);
  }

  /** How far a row slides out of the dragged row's way. */
  function shift(index: number): number {
    if (!drag || index === drag.from) return 0;
    if (drag.from < index && index <= drag.to) return -drag.height;
    if (drag.to <= index && index < drag.from) return drag.height;
    return 0;
  }
</script>

<div class="list">
  {#each items as item, i (item.id)}
    <div
      class="row"
      class:dragging={drag?.from === i}
      class:settling={!!drag && drag.from !== i}
      style:translate={drag?.from === i ? `0 ${drag.dy}px` : `0 ${shift(i)}px`}
    >
      {#if item.icon}<span class="glyph"><item.icon size={20} /></span>{:else}<Avatar name={item.name} mxc={item.avatar} size={36} {square} />{/if}
      <span class="name">{item.name}</span>
      <button class="arrow" disabled={disabled || i === 0} onclick={() => commit(i, i - 1)} aria-label={tr("Move up")}><ChevronUp size={18} /></button>
      <button class="arrow" disabled={disabled || i === items.length - 1} onclick={() => commit(i, i + 1)} aria-label={tr("Move down")}><ChevronDown size={18} /></button>
      <span
        class="grip"
        role="button"
        tabindex="-1"
        aria-label={tr("Drag to reorder")}
        onpointerdown={(e) => down(e, i)}
        onpointermove={move}
        onpointerup={up}
        onpointercancel={up}><GripVertical size={20} /></span
      >
    </div>
  {/each}
</div>

<style>
  .list {
    display: flex;
    flex-direction: column;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 12px;
    background: var(--bg);
    border-radius: 12px;
    position: relative;
  }
  .row.settling {
    transition: translate 0.18s ease;
  }
  .row.dragging {
    z-index: 2;
    background: var(--surface);
    box-shadow: 0 6px 24px rgb(0 0 0 / 0.25);
  }
  .glyph {
    width: 36px;
    height: 36px;
    display: grid;
    place-items: center;
    border-radius: 10px;
    background: var(--surface);
    color: var(--muted);
  }
  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .arrow {
    width: 32px;
    height: 32px;
    display: grid;
    place-items: center;
    border: none;
    border-radius: 50%;
    background: none;
    color: var(--muted);
    cursor: pointer;
  }
  .arrow:disabled {
    opacity: 0.3;
    cursor: default;
  }
  /* Phones drag; the arrows are for mice and keyboards. */
  @media (hover: none) {
    .arrow {
      display: none;
    }
  }
  .grip {
    width: 40px;
    height: 40px;
    display: grid;
    place-items: center;
    color: var(--muted);
    cursor: grab;
    touch-action: none;
  }
  .dragging .grip {
    cursor: grabbing;
  }
</style>
