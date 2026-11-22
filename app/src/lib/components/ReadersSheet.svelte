<script lang="ts">
  // Who read a message: everyone whose read receipt is on it or on a later message, with when
  // they last read. Opened from the small read avatars under a message.
  import { fade, fly } from "svelte/transition";
  import type { Reader } from "$lib/backend";
  import { onBack } from "$lib/back.svelte";
  import { tr } from "$lib/i18n/index.svelte";
  import { messageStamp } from "$lib/format";
  import Avatar from "./Avatar.svelte";
  import { portal } from "$lib/portal";

  let {
    readers,
    onClose,
    title = tr("Seen by"),
    sections = null,
  }: {
    readers: Reader[];
    onClose: () => void;
    title?: string;
    /** Grouped instead (poll votes: one group per answer). */
    sections?: { label: string; readers: Reader[] }[] | null;
  } = $props();
  $effect(() => onBack(onClose));

  // ── Pull down to close: on a phone, a swipe down while the list is at its top drags the
  // sheet (let go past 90 px = closed); with a mouse, scrolling up past the top does it. ──
  let list = $state<HTMLUListElement>();
  let sheet = $state<HTMLDivElement>();
  let drag = $state(0);
  $effect(() => {
    const el = sheet;
    if (!el) return;
    let startY: number | null = null;
    const atTop = () => (list?.scrollTop ?? 0) <= 0;
    const start = (e: TouchEvent) => (startY = atTop() ? e.touches[0].clientY : null);
    const move = (e: TouchEvent) => {
      if (startY === null) return;
      const dy = e.touches[0].clientY - startY;
      if (dy <= 0 || !atTop()) return void (drag = 0);
      e.preventDefault();
      drag = dy;
    };
    const end = () => {
      if (drag > 90) onClose();
      drag = 0;
      startY = null;
    };
    let pulled = 0;
    let idle: ReturnType<typeof setTimeout> | undefined;
    const wheel = (e: WheelEvent) => {
      if (e.deltaY >= 0 || !atTop()) return void (pulled = 0);
      pulled -= e.deltaY;
      clearTimeout(idle);
      idle = setTimeout(() => (pulled = 0), 400);
      if (pulled > 300) onClose();
    };
    el.addEventListener("touchstart", start, { passive: true });
    el.addEventListener("touchmove", move, { passive: false });
    el.addEventListener("touchend", end);
    el.addEventListener("touchcancel", end);
    el.addEventListener("wheel", wheel, { passive: true });
    return () => {
      clearTimeout(idle);
      el.removeEventListener("touchstart", start);
      el.removeEventListener("touchmove", move);
      el.removeEventListener("touchend", end);
      el.removeEventListener("touchcancel", end);
      el.removeEventListener("wheel", wheel);
    };
  });
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="backdrop" use:portal transition:fade={{ duration: 140 }} onclick={onClose}></div>
<div
  class="sheet"
  class:dragging={drag > 0}
  bind:this={sheet}
  style:--drag="{drag}px"
  use:portal
  role="dialog"
  aria-label={title}
  transition:fly={{ y: 40, duration: 200 }}
>
  <div class="grip"></div>
  <h2>{title} <span>{readers.length}</span></h2>
  <ul bind:this={list}>
    {#each sections ?? [{ label: "", readers }] as group, g (g)}
      {#if group.label}<li class="section"><strong>{group.label}</strong><span>{group.readers.length}</span></li>{/if}
      {#each group.readers as r, i (r.user_id ?? `${r.name}|${i}`)}
        <li>
          <Avatar name={r.name} mxc={r.avatar} size={36} />
          <span class="who"><strong>{r.name}</strong><small>{r.user_id}</small></span>
          {#if r.ts}<span class="when">{messageStamp(r.ts)}</span>{/if}
        </li>
      {/each}
    {/each}
  </ul>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 70;
    background: rgb(0 0 0 / 0.45);
  }
  .sheet {
    position: fixed;
    left: 50%;
    bottom: 0;
    z-index: 71;
    display: flex;
    flex-direction: column;
    width: min(480px, 100%);
    max-height: 70vh;
    padding: 8px 12px calc(14px + var(--safe-bottom));
    border-radius: 22px 22px 0 0;
    background: var(--surface);
    box-shadow: 0 -12px 40px rgb(0 0 0 / 0.4);
    translate: -50% var(--drag, 0px);
    transition: translate 0.18s var(--ease);
  }
  .sheet.dragging {
    transition: none;
  }
  /* A computer: a card in the middle rather than a sheet from the bottom. */
  @media (pointer: fine) and (min-width: 700px) {
    .sheet {
      top: 50%;
      bottom: auto;
      border-radius: 18px;
      translate: -50% -50%;
    }
    .grip {
      display: none;
    }
  }
  .grip {
    width: 36px;
    height: 4px;
    margin: 0 auto 10px;
    border-radius: 2px;
    background: var(--border);
  }
  h2 {
    margin: 0 4px 6px;
    font-size: 1rem;
  }
  h2 span {
    color: var(--muted);
    font-weight: normal;
  }
  ul {
    margin: 0;
    padding: 0;
    overflow-y: auto;
    list-style: none;
  }
  li {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 4px;
  }
  li.section {
    justify-content: space-between;
    padding: 12px 4px 2px;
    font-size: 0.85rem;
  }
  li.section span {
    color: var(--muted);
  }
  .who {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-width: 0;
  }
  .who strong,
  .who small {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .who small,
  .when {
    color: var(--muted);
    font-size: 0.78rem;
  }
  .when {
    flex: none;
  }
</style>
