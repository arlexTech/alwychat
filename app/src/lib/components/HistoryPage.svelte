<script lang="ts">
  import { messageActivity, messageAt, type Activity } from "$lib/backend";
  import Page from "./Page.svelte";

  /**
   * A chat's history over time: messages per day / week / month as stacked bars (you and the
   * others), from the local index. Hover or drag to read a period; tap (or let go) to jump
   * to its first message.
   */
  let { roomId, onJump, onClose }: { roomId: string; onJump: (eventId: string) => void; onClose: () => void } = $props();

  type Grain = "day" | "week" | "month";
  type Bucket = { start: number; end: number; mine: number; others: number };

  let activity = $state<Activity | null>(null);
  let error = $state<string | null>(null);
  let grain = $state<Grain | null>(null);
  let width = $state(320);
  let active = $state<number | null>(null);
  let jumping = $state(false);

  const load = () => messageActivity(roomId).then((a) => (activity = a)).catch((e) => (error = String(e)));
  $effect(() => {
    void load();
    // Older messages are still being indexed: refresh while the page is open.
    const timer = setInterval(() => activity && !activity.complete && load(), 5000);
    return () => clearInterval(timer);
  });

  const span = $derived(activity?.days.length ? activity.days.at(-1)!.day - activity.days[0].day : 0);
  const autoGrain = $derived<Grain>(span <= 75 * 86_400_000 ? "day" : span <= 2 * 365 * 86_400_000 ? "week" : "month");
  const shownGrain = $derived(grain ?? autoGrain);

  /** Local start of the period containing `t`. */
  function periodStart(t: number, g: Grain) {
    const d = new Date(t);
    if (g === "month") return new Date(d.getFullYear(), d.getMonth(), 1).getTime();
    const day = new Date(d.getFullYear(), d.getMonth(), d.getDate());
    if (g === "week") day.setDate(day.getDate() - ((day.getDay() + 6) % 7)); // Monday
    return day.getTime();
  }
  function next(t: number, g: Grain) {
    const d = new Date(t);
    if (g === "month") return new Date(d.getFullYear(), d.getMonth() + 1, 1).getTime();
    return new Date(d.getFullYear(), d.getMonth(), d.getDate() + (g === "week" ? 7 : 1)).getTime();
  }

  // Every period from the first message to now, empty ones included (gaps are information).
  const buckets = $derived.by((): Bucket[] => {
    const days = activity?.days ?? [];
    if (!days.length) return [];
    const g = shownGrain;
    const out: Bucket[] = [];
    const index = new Map<number, Bucket>();
    for (let t = periodStart(days[0].day, g), end = Date.now(); t <= end; t = next(t, g)) {
      const b = { start: t, end: next(t, g), mine: 0, others: 0 };
      out.push(b);
      index.set(t, b);
    }
    for (const d of days) {
      const b = index.get(periodStart(d.day + 12 * 3600_000, g));
      if (b) {
        b.mine += d.mine;
        b.others += d.others;
      }
    }
    return out;
  });

  const total = $derived(buckets.reduce((n, b) => n + b.mine + b.others, 0));
  const mineTotal = $derived(buckets.reduce((n, b) => n + b.mine, 0));
  const max = $derived(Math.max(1, ...buckets.map((b) => b.mine + b.others)));
  const busiest = $derived(buckets.reduce<Bucket | null>((best, b) => (!best || b.mine + b.others > best.mine + best.others ? b : best), null));

  const HEIGHT = 180;
  const AXIS = 22;
  const slot = $derived(buckets.length ? width / buckets.length : 0);
  const gap = $derived(slot > 6 ? 2 : slot > 3 ? 1 : 0);
  const barW = $derived(Math.max(1, slot - gap));
  const y = (n: number) => (n / max) * (HEIGHT - 8);

  const fmt = (t: number, g: Grain, long = false) =>
    new Date(t).toLocaleDateString([], g === "month"
      ? { month: long ? "long" : "short", year: "numeric" }
      : { day: "numeric", month: "short", year: long || g === "day" ? "numeric" : "2-digit" });
  const periodLabel = (b: Bucket) =>
    shownGrain === "week" ? `Week of ${fmt(b.start, "day", true)}` : fmt(b.start, shownGrain, true);

  // A few x labels: years, or months for shorter histories.
  const ticks = $derived.by(() => {
    if (!buckets.length) return [];
    const first = buckets[0].start;
    const last = buckets.at(-1)!.end;
    const years = (last - first) / (365 * 86_400_000);
    const out: { x: number; label: string }[] = [];
    const d = new Date(first);
    let t = years > 1.5 ? new Date(d.getFullYear() + 1, 0, 1).getTime() : new Date(d.getFullYear(), d.getMonth() + 1, 1).getTime();
    while (t < last) {
      const x = ((t - first) / (last - first)) * width;
      const date = new Date(t);
      out.push({ x, label: years > 1.5 ? String(date.getFullYear()) : date.toLocaleDateString([], { month: "short" }) });
      t = years > 1.5 ? new Date(date.getFullYear() + 1, 0, 1).getTime() : new Date(date.getFullYear(), date.getMonth() + (years > 0.6 ? 2 : 1), 1).getTime();
    }
    // Keep labels apart.
    return out.filter((tick, i) => i === 0 || tick.x - out[i - 1].x > 36);
  });

  let dragging = false;
  function pick(e: PointerEvent) {
    const rect = (e.currentTarget as SVGElement).getBoundingClientRect();
    const i = Math.floor(((e.clientX - rect.left) / rect.width) * buckets.length);
    active = Math.max(0, Math.min(buckets.length - 1, i));
  }
  async function go(b: Bucket) {
    if (jumping) return;
    jumping = true;
    error = null;
    try {
      // An empty period goes to the next message after it.
      const eventId = await messageAt(roomId, b.start);
      if (eventId) onJump(eventId);
      else error = "No messages after this date.";
    } catch (e) {
      error = String(e);
    } finally {
      jumping = false;
    }
  }
  const current = $derived(active !== null ? buckets[active] : null);
</script>

<Page title="History" {onClose}>
  {#if !activity && !error}
    <p class="m-hint">Counting messages…</p>
  {:else if activity && !activity.days.length}
    <p class="m-hint">{activity.complete ? "No messages yet." : "The history is still being read; come back in a moment."}</p>
  {:else if activity}
    <div class="stats">
      <div><strong>{total.toLocaleString()}</strong><small>messages since {fmt(buckets[0].start, "month", true)}</small></div>
      {#if busiest}<div><strong>{(busiest.mine + busiest.others).toLocaleString()}</strong><small>busiest: {periodLabel(busiest)}</small></div>{/if}
    </div>

    <div class="m-seg grains">
      {#each [["day", "Days"], ["week", "Weeks"], ["month", "Months"]] as [g, label] (g)}
        <button class:on={shownGrain === g} onclick={() => ((grain = g as Grain), (active = null))}>{label}</button>
      {/each}
    </div>

    <div class="legend">
      <span><i class="sw mine"></i>You · {mineTotal.toLocaleString()}</span>
      <span><i class="sw others"></i>Others · {(total - mineTotal).toLocaleString()}</span>
    </div>

    <div class="readout" aria-live="polite">
      {#if current}
        <strong>{periodLabel(current)}</strong>
        <span>{(current.mine + current.others).toLocaleString()} messages · you {current.mine}</span>
        <button class="go" disabled={jumping} onclick={() => go(current)}>Go there</button>
      {:else}
        <span class="muted">Tap a bar to go to that moment in the chat.</span>
      {/if}
    </div>

    <div class="chart" bind:clientWidth={width}>
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <svg
        width={width}
        height={HEIGHT + AXIS}
        role="img"
        aria-label="Messages per {shownGrain}, {total} in total"
        onpointerdown={(e) => ((dragging = true), (e.currentTarget as SVGElement).setPointerCapture(e.pointerId), pick(e))}
        onpointermove={(e) => (dragging || e.pointerType === "mouse") && pick(e)}
        onpointerup={() => {
          if (dragging && current) void go(current);
          dragging = false;
        }}
        onpointerleave={() => !dragging && (active = null)}
      >
        <line class="grid" x1="0" x2={width} y1={HEIGHT - y(max)} y2={HEIGHT - y(max)} />
        <text class="ymax" x="2" y={HEIGHT - y(max) - 4}>{max}</text>
        {#each buckets as b, i (b.start)}
          {@const x = i * slot + gap / 2}
          {@const hm = y(b.mine)}
          {@const ho = y(b.others)}
          <g class:dim={active !== null && active !== i}>
            {#if hm > 0}<rect class="mine" {x} y={HEIGHT - hm} width={barW} height={hm} rx={Math.min(2, barW / 2)} />{/if}
            {#if ho > 0}<rect class="others" {x} y={HEIGHT - hm - ho - (hm > 0 && barW > 3 ? 2 : 0)} width={barW} height={ho} rx={Math.min(2, barW / 2)} />{/if}
          </g>
        {/each}
        <line class="base" x1="0" x2={width} y1={HEIGHT + 0.5} y2={HEIGHT + 0.5} />
        {#each ticks as t (t.x)}
          <text class="tick" x={t.x} y={HEIGHT + 16}>{t.label}</text>
        {/each}
      </svg>
    </div>
    {#if !activity.complete}
      <p class="m-hint">Older messages are still being read from the server; the graph fills in as they arrive.</p>
    {/if}
  {/if}
  {#if error}<p class="m-error">{error}</p>{/if}
</Page>

<style>
  .stats {
    display: flex;
    gap: 24px;
    margin: 6px 4px 14px;
  }
  .stats div {
    display: flex;
    flex-direction: column;
  }
  .stats strong {
    font-size: 1.6rem;
    font-variant-numeric: tabular-nums;
  }
  .stats small,
  .muted {
    color: var(--muted);
  }
  .grains {
    margin-bottom: 12px;
  }
  .legend {
    display: flex;
    gap: 16px;
    margin: 0 4px 8px;
    color: var(--muted);
    font-size: 0.85rem;
  }
  .legend span {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .sw {
    width: 10px;
    height: 10px;
    border-radius: 3px;
  }
  /* Validated pair (dataviz validator, dark #0e1014 and light #fcfcfb surfaces). */
  .chart,
  .legend {
    --mine: #8a6cf0;
    --others: #2ea3ae;
  }
  :global([data-theme="light"]) .chart,
  :global([data-theme="light"]) .legend {
    --mine: #7a5ce6;
    --others: #1e9aa8;
  }
  .sw.mine,
  rect.mine {
    fill: var(--mine);
    background: var(--mine);
  }
  .sw.others,
  rect.others {
    fill: var(--others);
    background: var(--others);
  }
  .readout {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px 10px;
    min-height: 44px;
    margin: 0 4px 6px;
  }
  .readout span {
    color: var(--muted);
    font-size: 0.9rem;
  }
  .go {
    margin-left: auto;
    padding: 6px 14px;
    border: none;
    border-radius: 999px;
    background: var(--accent);
    color: white;
    font: inherit;
    font-size: 0.9rem;
    cursor: pointer;
  }
  .chart {
    width: 100%;
    touch-action: none;
    user-select: none;
  }
  svg {
    display: block;
    cursor: pointer;
  }
  g {
    transition: opacity 0.12s;
  }
  g.dim {
    opacity: 0.45;
  }
  .grid {
    stroke: var(--border);
    stroke-dasharray: 3 4;
  }
  .base {
    stroke: var(--border);
  }
  .tick,
  .ymax {
    fill: var(--muted);
    font-size: 11px;
  }
  .tick {
    text-anchor: middle;
  }
</style>
