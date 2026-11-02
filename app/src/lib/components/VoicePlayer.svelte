<script module lang="ts">
  /** Only one voice message plays at a time. */
  let playing: HTMLAudioElement | null = null;
</script>

<script lang="ts">
  import { tr } from "$lib/i18n/index.svelte";
  import { eventMediaUrl, playableUrl } from "$lib/media";

  let {
    media,
    durationMs,
    waveform,
    own,
  }: { media: string; durationMs: number | null; waveform: number[]; own: boolean } = $props();

  const BARS = 28;
  const SPEEDS = [1, 1.5, 2];

  let audio: HTMLAudioElement;
  let paused = $state(true);
  let current = $state(0);
  let duration = $state(0);
  let speed = $state(1);
  let failed = $state(false);
  let src = $state<string | undefined>();
  $effect(() => {
    const url = eventMediaUrl(media);
    playableUrl(url).then((u) => (src = u), () => (failed = true));
  });

  // The duration sent with the message first: recorded files can carry a wrong one (WebKitGTK).
  const total = $derived(durationMs ? durationMs / 1000 : duration);
  const progress = $derived(total ? Math.min(1, current / total) : 0);

  /** The sender's waveform squeezed to BARS bars (0–1); a gentle placeholder if there's none. */
  const bars = $derived.by(() => {
    if (!waveform.length) return Array.from({ length: BARS }, (_, i) => 0.25 + 0.2 * Math.abs(Math.sin(i * 0.7)));
    const out = Array.from({ length: BARS }, (_, i) => {
      const from = Math.floor((i * waveform.length) / BARS);
      const to = Math.max(from + 1, Math.floor(((i + 1) * waveform.length) / BARS));
      const slice = waveform.slice(from, to);
      return slice.reduce((a, b) => a + b, 0) / slice.length;
    });
    const max = Math.max(...out, 1);
    return out.map((v) => Math.max(0.08, v / max));
  });

  function toggle(event: MouseEvent) {
    event.stopPropagation();
    if (audio.paused) {
      if (playing && playing !== audio) playing.pause();
      playing = audio;
      audio.playbackRate = speed;
      audio.play().catch(() => (failed = true));
    } else {
      audio.pause();
    }
  }

  function cycleSpeed(event: MouseEvent) {
    event.stopPropagation();
    speed = SPEEDS[(SPEEDS.indexOf(speed) + 1) % SPEEDS.length];
    audio.playbackRate = speed;
  }

  /** Tap or drag on the waveform to jump. */
  function seek(event: PointerEvent) {
    event.stopPropagation();
    const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
    const ratio = Math.min(1, Math.max(0, (event.clientX - rect.left) / rect.width));
    if (total && Number.isFinite(total)) {
      audio.currentTime = ratio * total;
      current = audio.currentTime;
    }
  }

  const clock = (s: number) => {
    const sec = Math.max(0, Math.round(s));
    return `${Math.floor(sec / 60)}:${String(sec % 60).padStart(2, "0")}`;
  };
</script>

<div class="voice" class:own>
  <button class="play" onclick={toggle} aria-label={paused ? tr("Play voice message") : tr("Pause")}>
    {#if failed}⚠{:else if paused}▶{:else}❚❚{/if}
  </button>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="wave" onpointerdown={seek} style:--progress={progress}>
    {#each bars as b, i (i)}
      <span class:done={i / BARS < progress} style:height="{Math.round(b * 100)}%"></span>
    {/each}
  </div>
  <span class="time">{clock(paused && current === 0 ? total : current)}</span>
  <button class="speed" onclick={cycleSpeed} aria-label={tr("Playback speed")}>{speed}×</button>
  <audio
    bind:this={audio}
    {src}
    preload="metadata"
    onplay={() => (paused = false)}
    onpause={() => (paused = true)}
    onended={() => ((paused = true), (current = 0))}
    ontimeupdate={() => (current = audio.currentTime)}
    onloadedmetadata={() => Number.isFinite(audio.duration) && (duration = audio.duration)}
    onerror={() => (failed = true)}
  ></audio>
</div>

<style>
  .voice {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 240px;
    max-width: 100%;
    padding: 2px 0;
  }
  .play {
    flex: none;
    width: 36px;
    height: 36px;
    border: none;
    border-radius: 50%;
    background: var(--accent);
    color: white;
    font-size: 0.85rem;
    cursor: pointer;
    transition: transform 0.15s var(--ease);
  }
  .own .play {
    background: rgb(255 255 255 / 0.95);
    color: var(--accent-a);
  }
  .play:active {
    transform: scale(0.9);
  }
  .wave {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 2px;
    height: 30px;
    overflow: hidden;
    cursor: pointer;
    touch-action: none;
  }
  .wave span {
    flex: 1 1 0;
    min-width: 1px;
    max-width: 4px;
    border-radius: 2px;
    background: color-mix(in srgb, currentColor 35%, transparent);
    transition: background 0.1s linear;
  }
  .wave span.done {
    background: currentColor;
  }
  .time {
    flex: none;
    min-width: 2.2em;
    text-align: right;
    font-size: 0.8rem;
    font-variant-numeric: tabular-nums;
    opacity: 0.85;
  }
  .speed {
    flex: none;
    padding: 2px 7px;
    border: none;
    border-radius: 999px;
    background: color-mix(in srgb, currentColor 15%, transparent);
    color: inherit;
    font-size: 0.75rem;
    font-weight: 600;
    cursor: pointer;
  }
</style>
