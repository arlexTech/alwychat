<script lang="ts">
  // "Mute for…": a chat or a whole space, for a while or until turned back on.
  import { tr, trc } from "$lib/i18n/index.svelte";
  import { fade, fly } from "svelte/transition";
  import BellOff from "@lucide/svelte/icons/bell-off";
  import Clock from "@lucide/svelte/icons/clock";
  import CalendarClock from "@lucide/svelte/icons/calendar-clock";
  import { onBack } from "$lib/back.svelte";

  let {
    title,
    onPick,
    onClose,
  }: {
    /** "Mute Arnaud", "Mute the space Friends"… */
    title: string;
    /** `until`: ms since 1970, or null = until turned back on. */
    onPick: (until: number | null) => void;
    onClose: () => void;
  } = $props();

  const MIN = 60_000;
  const CHOICES: { label: string; ms: number }[] = [
    { label: tr("For 5 minutes"), ms: 5 * MIN },
    { label: tr("For 1 hour"), ms: 60 * MIN },
    { label: tr("For 8 hours"), ms: 8 * 60 * MIN },
    { label: tr("For 1 day"), ms: 24 * 60 * MIN },
    { label: tr("For 1 week"), ms: 7 * 24 * 60 * MIN },
  ];

  let custom = $state(false);
  /** `<input type="datetime-local">` value, local time ("2026-11-05T18:30"). */
  let when = $state(localInput(Date.now() + 2 * 60 * MIN));
  const whenMs = $derived(when ? new Date(when).getTime() : NaN);

  function localInput(ms: number) {
    const d = new Date(ms);
    const pad = (n: number) => String(n).padStart(2, "0");
    return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`;
  }

  // Act before closing: the caller's props go away once it's closed.
  function pick(until: number | null) {
    onPick(until);
    onClose();
  }

  $effect(() => onBack(() => (custom ? (custom = false) : onClose())));
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="backdrop" transition:fade={{ duration: 140 }} onclick={onClose}></div>
<div class="sheet" role="dialog" aria-label={title} transition:fly={{ y: 40, duration: 200 }}>
  <div class="grip"></div>
  <strong class="name">{title}</strong>
  {#if custom}
    <label class="custom">
      <span>{tr("Muted until")}</span>
      <input type="datetime-local" bind:value={when} min={localInput(Date.now())} />
    </label>
    <div class="actions">
      <button class="text" onclick={() => (custom = false)}>{tr("Back")}</button>
      <button class="text accent" disabled={!(whenMs > Date.now())} onclick={() => pick(whenMs)}>{trc("chat", "Mute")}</button>
    </div>
  {:else}
    {#each CHOICES as c (c.ms)}
      <button onclick={() => pick(Date.now() + c.ms)}><Clock size={20} />{c.label}</button>
    {/each}
    <button onclick={() => (custom = true)}><CalendarClock size={20} />{tr("Until a date and time…")}</button>
    <button onclick={() => pick(null)}><BellOff size={20} />{tr("Until I turn it back on")}</button>
  {/if}
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
    width: min(520px, 100%);
    padding: 8px 12px calc(16px + env(safe-area-inset-bottom));
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
  .name {
    padding: 4px 10px 10px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  button {
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
  button:active,
  button:hover {
    background: var(--surface-hover);
  }
  .custom {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 6px 10px 12px;
    color: var(--muted);
  }
  .custom input {
    padding: 10px 12px;
    border: 1px solid var(--border);
    border-radius: 12px;
    background: var(--bg);
    color: var(--text);
    font: inherit;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
  .text {
    padding: 10px 16px;
  }
  .text.accent {
    color: var(--accent-a);
    font-weight: 600;
  }
  .text:disabled {
    opacity: 0.4;
  }
</style>
