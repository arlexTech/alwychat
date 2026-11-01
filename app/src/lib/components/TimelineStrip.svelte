<script lang="ts">
  import X from "@lucide/svelte/icons/x";
  import { tick } from "svelte";
  import { fly } from "svelte/transition";
  import { messageActivity, messageAt, type Activity } from "$lib/backend";

  /**
   * The chat's history as a waveform under its header: one rounded bar per day, centred on
   * the middle line, newest on the right. Scroll freely sideways; the day under the centre is
   * shown top left, and the chat follows it (loading that day's messages) as soon as the
   * scrolling pauses. Stays open until closed.
   */
  let { roomId, onJump, onClose }: { roomId: string; onJump: (eventId: string) => void; onClose: () => void } = $props();

  const DAY = 86_400_000;
  const SLOT = 7;
  const BAR = 4;
  const HALF = 26;

  let activity = $state<Activity | null>(null);
  let error = $state<string | null>(null);
  let strip = $state<HTMLDivElement>();
  let centre = $state(0);
  let following = $state(false);

  const load = () => messageActivity(roomId).then((a) => (activity = a)).catch((e) => (error = String(e)));
  $effect(() => {
    void load();
    // Older messages are fetched in the background (this chat first while the strip is
    // open): the waveform grows to the right as they arrive.
    const timer = setInterval(() => activity && !activity.complete && load(), 3000);
    return () => clearInterval(timer);
  });

  const localDay = (t: number) => {
    const d = new Date(t);
    return new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
  };
  const nextDay = (t: number) => {
    const d = new Date(t);
    return new Date(d.getFullYear(), d.getMonth(), d.getDate() + 1).getTime();
  };

  type Day = { start: number; mine: number; others: number };
  const days = $derived.by((): Day[] => {
    const counts = activity?.days ?? [];
    if (!counts.length) return [];
    const out: Day[] = [];
    const index = new Map<number, Day>();
    for (let t = localDay(counts[0].day), end = Date.now(); t <= end; t = nextDay(t)) {
      const d = { start: t, mine: 0, others: 0 };
      out.push(d);
      index.set(t, d);
    }
    for (const c of counts) {
      const d = index.get(localDay(c.day + 12 * 3600_000));
      if (d) {
        d.mine += c.mine;
        d.others += c.others;
      }
    }
    return out;
  });
  // Scale on a high percentile, so one exceptional day doesn't flatten the rest.
  const scale = $derived.by(() => {
    const totals = days.map((d) => d.mine + d.others).filter(Boolean).sort((a, b) => a - b);
    return Math.max(4, totals[Math.floor(totals.length * 0.95)] ?? 1);
  });
  const height = (n: number) => (n ? Math.max(BAR, Math.min(2 * HALF, (n / scale) * 2 * HALF)) : 0);

  // Older days get added on the left as they're read: keep the view on the same day.
  let shown = 0;
  let firstDay: number | null = null;
  $effect(() => {
    const count = days.length;
    if (!count || !strip) return;
    const first = days[0].start;
    if (!shown) {
      shown = count;
      firstDay = first;
      tick().then(() => strip && ((strip.scrollLeft = strip.scrollWidth), onScroll()));
      return;
    }
    if (firstDay !== null && first < firstDay) {
      const added = Math.round((firstDay - first) / DAY);
      firstDay = first;
      shown = count;
      tick().then(() => strip && (strip.scrollLeft += added * SLOT));
    }
  });

  let settle: ReturnType<typeof setTimeout> | undefined;
  /** Only the user's own scrolling moves the chat (not opening, nor older days arriving). */
  let touched = false;
  let lastJump: string | null = null;
  function onScroll() {
    if (!strip || !days.length) return;
    centre = Math.max(0, Math.min(days.length - 1, Math.round(strip.scrollLeft / SLOT)));
    // Near the oldest loaded day while more is coming: ask again sooner.
    if (centre < 20 && activity && !activity.complete) void load();
    if (!touched) return;
    clearTimeout(settle);
    settle = setTimeout(follow, 280);
  }

  /** Move the chat to the centred day (or the next one with messages). */
  async function follow() {
    const d = days[centre];
    if (!d) return;
    following = true;
    try {
      const eventId = await messageAt(roomId, d.start);
      if (eventId && eventId !== lastJump && days[centre] === d) {
        lastJump = eventId;
        onJump(eventId);
      }
    } catch (e) {
      error = String(e);
    } finally {
      following = false;
    }
  }

  const current = $derived(days[centre] ?? null);
  const dateLabel = (t: number) =>
    new Date(t).toLocaleDateString([], { weekday: "short", day: "numeric", month: "short", year: "numeric" });
  /** Month names under the first day of each month (years in January). */
  const monthMark = (d: Day, i: number) => {
    const date = new Date(d.start);
    if (date.getDate() !== 1 && i !== 0) return "";
    return date.getMonth() === 0 || i === 0
      ? `${date.toLocaleDateString([], { month: "short" })} ${date.getFullYear()}`
      : date.toLocaleDateString([], { month: "short" });
  };
</script>

<div class="timeline" transition:fly={{ y: -10, duration: 180 }}>
  <div class="top">
    <div class="what">
      {#if current}
        <strong>{dateLabel(current.start)}</strong>
        <small>
          {current.mine + current.others
            ? `${current.mine + current.others} messages · you ${current.mine}`
            : "No messages this day"}{following ? " · …" : ""}
        </small>
      {:else if activity && !activity.days.length}
        <strong>No history yet</strong>
        <small>{activity.complete ? "Nothing to show" : "Messages are still being read…"}</small>
      {:else}
        <strong>Loading the history…</strong>
      {/if}
    </div>
    {#if activity && !activity.complete}<span class="reading" title="Older messages are still being read">reading older…</span>{/if}
    <button class="close" onclick={onClose} aria-label="Close the timeline"><X size={18} /></button>
  </div>

  <div class="viewport">
    <div class="marker" aria-hidden="true"></div>
    <div
      class="strip"
      bind:this={strip}
      onscroll={onScroll}
      onpointerdown={() => (touched = true)}
      onwheel={() => (touched = true)}
      role="slider"
      tabindex="0"
      aria-label="Chat history by day"
      aria-valuemin={0}
      aria-valuemax={Math.max(0, days.length - 1)}
      aria-valuenow={centre}
      aria-valuetext={current ? dateLabel(current.start) : ""}
      onkeydown={(e) => {
        if (!strip) return;
        touched = true;
        const step = e.shiftKey ? 7 : 1;
        if (e.key === "ArrowLeft") strip.scrollLeft -= step * SLOT;
        else if (e.key === "ArrowRight") strip.scrollLeft += step * SLOT;
        else return;
        e.preventDefault();
      }}
    >
      {#each days as d, i (d.start)}
        {@const hm = height(d.mine)}
        {@const ho = height(d.others)}
        {@const mark = monthMark(d, i)}
        <span class="day" class:on={i === centre}>
          {#if hm || ho}
            <span class="bar">
              {#if ho}<span class="others" style:height="{ho}px"></span>{/if}
              {#if hm}<span class="mine" style:height="{hm}px"></span>{/if}
            </span>
          {:else}
            <span class="dot"></span>
          {/if}
          {#if mark}<span class="month">{mark}</span>{/if}
        </span>
      {/each}
    </div>
  </div>
</div>

<style>
  .timeline {
    position: relative;
    z-index: 2;
    margin: 6px 10px 0;
    padding: 8px 0 4px;
    border: 1px solid var(--border);
    border-radius: 16px;
    background: color-mix(in srgb, var(--surface) 92%, transparent);
    backdrop-filter: blur(12px);
    box-shadow: 0 8px 24px rgb(0 0 0 / 0.25);
    /* Validated pair (dataviz validator: dark on #0e1014, light on #fcfcfb). */
    --mine: #8a6cf0;
    --others: #2ea3ae;
  }
  :global([data-theme="light"]) .timeline {
    --mine: #7a5ce6;
    --others: #1e9aa8;
  }
  .top {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 8px 0 14px;
  }
  .what {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-width: 0;
  }
  .what strong {
    font-size: 0.95rem;
    font-variant-numeric: tabular-nums;
  }
  .what small {
    color: var(--muted);
    font-size: 0.8rem;
  }
  .reading {
    color: var(--muted);
    font-size: 0.75rem;
  }
  .close {
    display: grid;
    width: 34px;
    height: 34px;
    place-items: center;
    border: none;
    border-radius: 50%;
    background: none;
    color: var(--muted);
    cursor: pointer;
  }
  .viewport {
    position: relative;
    margin-top: 4px;
  }
  .marker {
    position: absolute;
    top: 0;
    left: 50%;
    z-index: 1;
    width: 2px;
    height: 60px;
    margin-left: -1px;
    border-radius: 1px;
    background: var(--text);
    opacity: 0.5;
    pointer-events: none;
  }
  .strip {
    display: flex;
    padding: 0 calc(50% - 3.5px);
    overflow-x: auto;
    overscroll-behavior-x: contain;
    scrollbar-width: none;
    outline: none;
    touch-action: pan-x;
  }
  .strip::-webkit-scrollbar {
    display: none;
  }
  .day {
    position: relative;
    display: flex;
    flex: none;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    width: 7px;
    height: 60px;
  }
  /* Centred on the middle line, rounded at both ends. */
  .bar {
    display: flex;
    flex-direction: column;
    gap: 1px;
    width: 4px;
    overflow: hidden;
    border-radius: 2px;
    opacity: 0.75;
  }
  .day.on .bar {
    opacity: 1;
  }
  .bar .mine {
    background: var(--mine);
  }
  .bar .others {
    background: var(--others);
  }
  .dot {
    width: 3px;
    height: 3px;
    border-radius: 50%;
    background: var(--border);
  }
  .month {
    position: absolute;
    top: 62px;
    left: 0;
    color: var(--muted);
    font-size: 10px;
    white-space: nowrap;
  }
  .strip {
    padding-bottom: 14px;
  }
</style>
