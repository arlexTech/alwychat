<script lang="ts">
  import X from "@lucide/svelte/icons/x";
  import { tick } from "svelte";
  import { fly } from "svelte/transition";
  import { messageActivity, messageAt, type Activity } from "$lib/backend";

  /**
   * The chat's history as a strip under its header: one bar per day / week / month (you and
   * the others, stacked). Swipe sideways; the period under the centre line is the one
   * shown, and "Go" moves the chat there. The strip stays open to keep exploring.
   */
  let { roomId, onJump, onClose }: { roomId: string; onJump: (eventId: string) => void; onClose: () => void } = $props();

  type Grain = "day" | "week" | "month";
  type Bucket = { start: number; mine: number; others: number };

  let activity = $state<Activity | null>(null);
  let error = $state<string | null>(null);
  let grain = $state<Grain>("week");
  let strip = $state<HTMLDivElement>();
  let centre = $state(0);
  let jumping = $state(false);
  let wentTo = $state<number | null>(null);

  const load = () => messageActivity(roomId).then((a) => (activity = a)).catch((e) => (error = String(e)));
  $effect(() => {
    void load();
    const timer = setInterval(() => activity && !activity.complete && load(), 5000);
    return () => clearInterval(timer);
  });

  function periodStart(t: number, g: Grain) {
    const d = new Date(t);
    if (g === "month") return new Date(d.getFullYear(), d.getMonth(), 1).getTime();
    const day = new Date(d.getFullYear(), d.getMonth(), d.getDate());
    if (g === "week") day.setDate(day.getDate() - ((day.getDay() + 6) % 7));
    return day.getTime();
  }
  function next(t: number, g: Grain) {
    const d = new Date(t);
    if (g === "month") return new Date(d.getFullYear(), d.getMonth() + 1, 1).getTime();
    return new Date(d.getFullYear(), d.getMonth(), d.getDate() + (g === "week" ? 7 : 1)).getTime();
  }

  const buckets = $derived.by((): Bucket[] => {
    const days = activity?.days ?? [];
    if (!days.length) return [];
    const out: Bucket[] = [];
    const index = new Map<number, Bucket>();
    for (let t = periodStart(days[0].day, grain), end = Date.now(); t <= end; t = next(t, grain)) {
      const b = { start: t, mine: 0, others: 0 };
      out.push(b);
      index.set(t, b);
    }
    for (const d of days) {
      const b = index.get(periodStart(d.day + 12 * 3600_000, grain));
      if (b) {
        b.mine += d.mine;
        b.others += d.others;
      }
    }
    return out;
  });
  const max = $derived(Math.max(1, ...buckets.map((b) => b.mine + b.others)));

  const SLOT = 16;
  const BAR_H = 56;
  const h = (n: number) => (n ? Math.max(3, (n / max) * BAR_H) : 0);

  // Open on the present; keep the same moment centred when the grain changes.
  let shownGrain: Grain | null = null;
  let keepAt: number | null = null;
  $effect(() => {
    const count = buckets.length;
    if (!count || !strip || shownGrain === grain) return;
    shownGrain = grain;
    const at = keepAt;
    tick().then(() => {
      if (!strip) return;
      let i = count - 1;
      if (at !== null) {
        const found = buckets.findIndex((b) => b.start > at);
        i = Math.max(0, (found === -1 ? count : found) - 1);
      }
      strip.scrollLeft = i * SLOT;
      onScroll();
    });
  });

  function onScroll() {
    if (!strip) return;
    centre = Math.max(0, Math.min(buckets.length - 1, Math.round(strip.scrollLeft / SLOT)));
  }
  function setGrain(g: Grain) {
    keepAt = buckets[centre]?.start ?? null;
    grain = g;
  }
  function scrollToBar(i: number) {
    strip?.scrollTo({ left: i * SLOT, behavior: "smooth" });
  }

  const current = $derived(buckets[centre] ?? null);
  function label(b: Bucket) {
    const d = new Date(b.start);
    if (grain === "month") return d.toLocaleDateString([], { month: "long", year: "numeric" });
    const day = d.toLocaleDateString([], { day: "numeric", month: "short", year: "numeric" });
    return grain === "week" ? `Week of ${day}` : d.toLocaleDateString([], { weekday: "short", day: "numeric", month: "short", year: "numeric" });
  }
  /** A small date under the first bar of each month / year. */
  function tickLabel(b: Bucket, i: number) {
    const d = new Date(b.start);
    const prev = i ? new Date(buckets[i - 1].start) : null;
    if (grain === "month") return !prev || d.getFullYear() !== prev.getFullYear() ? String(d.getFullYear()) : "";
    if (!prev || d.getMonth() !== prev.getMonth())
      return d.getMonth() === 0 || !prev ? `${d.toLocaleDateString([], { month: "short" })} ${String(d.getFullYear()).slice(2)}` : d.toLocaleDateString([], { month: "short" });
    return "";
  }

  async function go() {
    const b = current;
    if (!b || jumping) return;
    jumping = true;
    error = null;
    try {
      const eventId = await messageAt(roomId, b.start);
      if (eventId) {
        wentTo = b.start;
        onJump(eventId);
      } else error = "No messages after this date";
    } catch (e) {
      error = String(e);
    } finally {
      jumping = false;
    }
  }
</script>

<div class="timeline" transition:fly={{ y: -10, duration: 180 }}>
  <div class="top">
    <div class="what">
      {#if current}
        <strong>{label(current)}</strong>
        <small>{(current.mine + current.others).toLocaleString()} messages{current.mine + current.others ? ` · you ${current.mine}` : ""}{error ? ` · ${error}` : ""}</small>
      {:else if activity && !activity.days.length}
        <strong>No history yet</strong>
        <small>{activity.complete ? "Nothing to show" : "Messages are still being read…"}</small>
      {:else}
        <strong>Loading the history…</strong>
      {/if}
    </div>
    <button class="go" disabled={!current || jumping} onclick={go}>{jumping ? "…" : wentTo === current?.start ? "Again" : "Go"}</button>
    <button class="close" onclick={onClose} aria-label="Close the timeline"><X size={18} /></button>
  </div>

  <div class="viewport">
    <div class="marker" aria-hidden="true"></div>
    <div class="strip" bind:this={strip} onscroll={onScroll} role="listbox" aria-label="Messages over time" tabindex="0"
      onkeydown={(e) => {
        if (e.key === "ArrowLeft") scrollToBar(centre - 1);
        else if (e.key === "ArrowRight") scrollToBar(centre + 1);
        else if (e.key === "Enter") void go();
        else return;
        e.preventDefault();
      }}
    >
      {#each buckets as b, i (b.start)}
        <button
          class="slot"
          class:on={i === centre}
          role="option"
          aria-selected={i === centre}
          aria-label="{label(b)}: {b.mine + b.others} messages"
          onclick={() => (i === centre ? void go() : scrollToBar(i))}
        >
          <span class="bars">
            {#if b.others}<span class="others" style:height="{h(b.others)}px"></span>{/if}
            {#if b.mine}<span class="mine" style:height="{h(b.mine)}px"></span>{/if}
            {#if !b.mine && !b.others}<span class="empty"></span>{/if}
          </span>
          <span class="tick">{tickLabel(b, i)}</span>
        </button>
      {/each}
    </div>
  </div>
  <div class="legend">
    <span><i class="sw mine"></i>You</span><span><i class="sw others"></i>Others</span>
    {#if activity && !activity.complete}<span class="loading">still loading older ones</span>{/if}
    <span class="grains">
      {#each [["day", "D"], ["week", "W"], ["month", "M"]] as [g, short] (g)}
        <button class:on={grain === g} onclick={() => setGrain(g as Grain)} aria-label="By {g}" title="By {g}">{short}</button>
      {/each}
    </span>
  </div>
</div>

<style>
  .timeline {
    position: relative;
    z-index: 2;
    margin: 6px 10px 0;
    padding: 8px 0 6px;
    border: 1px solid var(--border);
    border-radius: 16px;
    background: color-mix(in srgb, var(--surface) 92%, transparent);
    backdrop-filter: blur(12px);
    box-shadow: 0 8px 24px rgb(0 0 0 / 0.25);
  }
  /* Validated pair (dataviz validator: dark on #0e1014, light on #fcfcfb). */
  .timeline {
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
  }
  .what small {
    overflow: hidden;
    color: var(--muted);
    font-size: 0.8rem;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .go {
    min-width: 58px;
    padding: 7px 16px;
    border: none;
    border-radius: 999px;
    background: var(--accent);
    color: white;
    font: inherit;
    font-weight: 600;
    cursor: pointer;
  }
  .go:disabled {
    opacity: 0.5;
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
    margin-top: 6px;
  }
  .marker {
    position: absolute;
    top: 0;
    bottom: 16px;
    left: 50%;
    z-index: 1;
    width: 2px;
    margin-left: -1px;
    border-radius: 1px;
    background: var(--text);
    opacity: 0.55;
    pointer-events: none;
  }
  .strip {
    display: flex;
    /* Half a viewport of padding: the first and last bars can reach the centre line. */
    padding: 0 calc(50% - 8px);
    overflow-x: auto;
    scroll-snap-type: x mandatory;
    scrollbar-width: none;
    outline: none;
  }
  .strip::-webkit-scrollbar {
    display: none;
  }
  .slot {
    display: flex;
    flex: none;
    flex-direction: column;
    align-items: center;
    width: 16px;
    padding: 0;
    border: none;
    background: none;
    color: inherit;
    scroll-snap-align: center;
    cursor: pointer;
  }
  .bars {
    display: flex;
    flex-direction: column;
    justify-content: flex-end;
    gap: 2px;
    width: 10px;
    height: 58px;
    opacity: 0.7;
    transition: opacity 0.12s;
  }
  .slot.on .bars {
    opacity: 1;
  }
  .bars span {
    border-radius: 3px;
  }
  .bars .mine,
  .sw.mine {
    background: var(--mine);
  }
  .bars .others,
  .sw.others {
    background: var(--others);
  }
  .bars .empty {
    height: 2px;
    background: var(--border);
  }
  .tick {
    height: 14px;
    margin-top: 2px;
    color: var(--muted);
    font-size: 10px;
    white-space: nowrap;
  }
  .legend {
    display: flex;
    gap: 12px;
    padding: 2px 14px 0;
    color: var(--muted);
    font-size: 0.75rem;
  }
  .legend span {
    display: flex;
    align-items: center;
    gap: 5px;
  }
  .sw {
    width: 8px;
    height: 8px;
    border-radius: 2px;
  }
  .grains {
    margin-left: auto;
    gap: 2px !important;
  }
  .grains button {
    width: 26px;
    height: 22px;
    border: none;
    border-radius: 6px;
    background: none;
    color: var(--muted);
    font: inherit;
    font-size: 0.75rem;
    font-weight: 600;
    cursor: pointer;
  }
  .grains button.on {
    background: var(--surface-2);
    color: var(--text);
  }
</style>
