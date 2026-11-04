<script lang="ts">
  // The Calls tab (Messenger / WhatsApp style): this device's calls, newest first, by day.
  // Tap a call to open its chat, the phone/camera button to call back; right-click or
  // long-press removes one.
  import { tr } from "$lib/i18n/index.svelte";
  import { onMount } from "svelte";
  import PhoneIncoming from "@lucide/svelte/icons/phone-incoming";
  import PhoneOutgoing from "@lucide/svelte/icons/phone-outgoing";
  import PhoneMissed from "@lucide/svelte/icons/phone-missed";
  import PhoneOff from "@lucide/svelte/icons/phone-off";
  import Phone from "@lucide/svelte/icons/phone";
  import Video from "@lucide/svelte/icons/video";
  import type { CallRecord } from "$lib/backend";
  import { calls } from "$lib/stores/calls.svelte";
  import { call } from "$lib/stores/call.svelte";
  import { chatList } from "$lib/stores/chatList.svelte";
  import { clockTime, dayLabel } from "$lib/format";
  import Avatar from "./Avatar.svelte";

  let { onOpen }: { onOpen: (roomId: string) => void } = $props();

  let menu = $state<CallRecord | null>(null);

  onMount(() => {
    void calls.refresh().then(() => calls.seen());
  });
  // New missed calls while the tab is open are seen at once.
  $effect(() => {
    if (calls.unseenMissed) calls.seen();
  });

  const room = (r: CallRecord) => chatList.rooms.find((x) => x.id === r.room_id);

  /** Calls grouped under day headers. */
  const days = $derived.by(() => {
    const groups: { label: string; items: CallRecord[] }[] = [];
    for (const r of calls.records) {
      const label = dayLabel(r.at);
      if (groups.at(-1)?.label !== label) groups.push({ label, items: [] });
      groups.at(-1)!.items.push(r);
    }
    return groups;
  });

  function describe(r: CallRecord) {
    const length = r.duration_s
      ? r.duration_s < 60
        ? tr("{n} s", { n: r.duration_s })
        : tr("{n} min", { n: Math.round(r.duration_s / 60) })
      : null;
    switch (r.kind) {
      case "missed":
        return r.video ? tr("Missed video call") : tr("Missed call");
      case "declined":
        return tr("Declined");
      case "incoming":
        return [r.video ? tr("Incoming video call") : tr("Incoming call"), length].filter(Boolean).join(" · ");
      case "outgoing":
        return r.connected || r.duration_s === null
          ? [r.video ? tr("Outgoing video call") : tr("Outgoing call"), length].filter(Boolean).join(" · ")
          : tr("No answer");
      case "joined":
        return [tr("Joined the call"), length].filter(Boolean).join(" · ");
    }
  }

  function callBack(r: CallRecord) {
    const summary = room(r);
    if (summary) void call.join(summary, r.video);
  }

  let pressTimer: ReturnType<typeof setTimeout> | undefined;
  let longPressed = false;
  function down(r: CallRecord) {
    longPressed = false;
    pressTimer = setTimeout(() => ((longPressed = true), (menu = r)), 550);
  }
  const up = () => clearTimeout(pressTimer);
</script>

{#if calls.records.length === 0}
  <p class="empty">{tr("Your calls will show here")}</p>
{:else}
  {#each days as day (day.label)}
    <h4>{day.label}</h4>
    {#each day.items as r (r.id)}
      {@const summary = room(r)}
      {@const bad = r.kind === "missed" || r.kind === "declined" || (r.kind === "outgoing" && !r.connected && r.duration_s !== null)}
      <div class="row" class:menu={menu?.id === r.id}>
        <button
          class="main"
          onclick={() => !longPressed && onOpen(r.room_id)}
          oncontextmenu={(e) => (e.preventDefault(), (menu = r))}
          onpointerdown={() => down(r)}
          onpointerup={up}
          onpointerleave={up}
        >
          <Avatar name={summary?.name ?? "?"} mxc={summary?.avatar ?? null} size={44} />
          <span class="text">
            <strong class:bad={r.kind === "missed"}>{summary?.name ?? tr("Unknown chat")}</strong>
            <small class:bad>
              {#if r.kind === "missed"}<PhoneMissed size={14} />
              {:else if r.kind === "declined"}<PhoneOff size={14} />
              {:else if r.kind === "incoming"}<PhoneIncoming size={14} />
              {:else}<PhoneOutgoing size={14} />{/if}
              {describe(r)}
            </small>
          </span>
          <span class="time">{clockTime(r.at)}</span>
        </button>
        {#if summary}
          <button class="back" onclick={() => callBack(r)} aria-label={r.video ? tr("Video call") : tr("Call")} title={r.video ? tr("Video call") : tr("Call")}>
            {#if r.video}<Video size={20} />{:else}<Phone size={20} />{/if}
          </button>
        {/if}
      </div>
      {#if menu?.id === r.id}
        <div class="actions">
          <button onclick={() => (menu = null)}>{tr("Cancel")}</button>
          <button class="danger" onclick={() => (void calls.remove(r.id), (menu = null))}>{tr("Remove from history")}</button>
        </div>
      {/if}
    {/each}
  {/each}
  <button class="clear" onclick={() => void calls.remove(null)}>{tr("Clear call history")}</button>
{/if}

<style>
  .empty {
    margin: 48px 24px;
    text-align: center;
    color: var(--muted);
  }
  h4 {
    margin: 14px 16px 4px;
    font-size: 0.78rem;
    font-weight: 600;
    color: var(--muted);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 0 8px 0 0;
    border-radius: 14px;
  }
  .row.menu,
  .row:hover {
    background: var(--surface-hover);
  }
  .main {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 12px;
    border: none;
    background: none;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
    -webkit-touch-callout: none;
  }
  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .text strong,
  .text small {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .text small {
    display: flex;
    align-items: center;
    gap: 5px;
    color: var(--muted);
  }
  .bad {
    color: var(--danger) !important;
  }
  .time {
    font-size: 0.8rem;
    color: var(--muted);
  }
  .back {
    width: 40px;
    height: 40px;
    flex: none;
    border: none;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: none;
    color: var(--accent);
    cursor: pointer;
  }
  .back:hover {
    background: var(--surface);
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding: 4px 12px 10px;
  }
  .actions button,
  .clear {
    border: none;
    border-radius: 999px;
    padding: 8px 14px;
    background: var(--surface-hover);
    color: inherit;
    font: inherit;
    cursor: pointer;
  }
  .actions .danger {
    background: var(--danger);
    color: #fff;
  }
  .clear {
    display: block;
    margin: 18px auto 24px;
    font-size: 0.85rem;
    color: var(--muted);
  }
</style>
