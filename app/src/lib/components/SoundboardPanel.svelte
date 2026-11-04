<script lang="ts">
  // Soundboard (Discord style), from the call view: tap a sound, everyone in the call hears it.
  // Built-in sounds, then the chat's own; people allowed to change the chat can add a short
  // audio file (trimmed to 6 s, sent as WAV) or remove one (right-click / long-press).
  import { tr } from "$lib/i18n/index.svelte";
  import { onBack } from "$lib/back.svelte";
  import { onMount } from "svelte";
  import { fly } from "svelte/transition";
  import X from "@lucide/svelte/icons/x";
  import Plus from "@lucide/svelte/icons/plus";
  import Play from "@lucide/svelte/icons/play";
  import Volume2 from "@lucide/svelte/icons/volume-2";
  import VolumeX from "@lucide/svelte/icons/volume-x";
  import { isAndroid, type Sound } from "$lib/backend";
  import { call } from "$lib/stores/call.svelte";
  import { builtinSounds, MAX_SECONDS, soundboard, type AnySound } from "$lib/stores/soundboard.svelte";

  let { roomId, onClose }: { roomId: string; onClose: () => void } = $props();

  let adding = $state(false);
  let name = $state("");
  let emoji = $state("");
  let picked = $state<AudioBuffer | null>(null);
  let trimmed = $state(false);
  let busy = $state(false);
  let error = $state<string | null>(null);
  let removing = $state<Sound | null>(null);
  let fileInput = $state<HTMLInputElement>();

  $effect(() => onBack(() => (removing ? (removing = null) : adding ? (adding = false) : onClose())));
  onMount(() => void soundboard.load(roomId));

  function play(s: AnySound) {
    error = null;
    soundboard.play(s).catch((e) => (error = String(e)));
  }

  async function usePicked(file: Blob | null, fileName = "") {
    if (!file) return;
    error = null;
    try {
      const buffer = await soundboard.decodeFile(file);
      picked = buffer;
      trimmed = buffer.duration >= MAX_SECONDS - 0.01;
      if (!name) name = fileName.replace(/\.[^.]+$/, "").slice(0, 32);
    } catch {
      error = tr("That file isn't a sound this app can read");
    }
  }

  async function choose() {
    if (isAndroid) {
      try {
        await usePicked(await soundboard.pickAndroid());
      } catch (e) {
        error = String(e);
      }
    } else fileInput?.click();
  }

  /** Hear it ourselves before adding it. */
  function preview() {
    if (!picked) return;
    const ctx = new AudioContext();
    const source = ctx.createBufferSource();
    source.buffer = picked;
    source.connect(ctx.destination);
    source.onended = () => void ctx.close();
    source.start();
  }

  async function save() {
    if (!picked || !name.trim()) return;
    busy = true;
    error = null;
    try {
      await soundboard.add(roomId, picked, name, emoji || null);
      adding = false;
      picked = null;
      name = "";
      emoji = "";
    } catch (e) {
      error = tr("Couldn't add the sound: {e}", { e: String(e) });
    } finally {
      busy = false;
    }
  }

  async function remove(sound: Sound) {
    removing = null;
    try {
      await soundboard.remove(roomId, sound.id);
    } catch (e) {
      error = String(e);
    }
  }

  // Long-press (touch) to remove a chat sound.
  let pressTimer: ReturnType<typeof setTimeout> | undefined;
  let longPressed = false;
  function pressStart(sound: Sound) {
    longPressed = false;
    if (!soundboard.canEdit) return;
    pressTimer = setTimeout(() => {
      longPressed = true;
      removing = sound;
    }, 550);
  }
  const pressEnd = () => clearTimeout(pressTimer);
</script>

<div class="panel" transition:fly={{ y: 24, duration: 160 }}>
  <header>
    <strong>{tr("Soundboard")}</strong>
    <label class="volume" title={tr("How loud sounds are for you")}>
      {#if call.prefs.soundboardVolume < 0.01}<VolumeX size={18} />{:else}<Volume2 size={18} />{/if}
      <input
        type="range"
        min="0"
        max="1"
        step="0.05"
        value={call.prefs.soundboardVolume}
        oninput={(e) => call.setSoundboardVolume(Number(e.currentTarget.value))}
        aria-label={tr("Soundboard volume")}
      />
    </label>
    <button class="icon" onclick={onClose} aria-label={tr("Close")}><X size={20} /></button>
  </header>
  {#if call.deafened}<p class="note">{tr("Undeafen to play sounds")}</p>{/if}
  {#if error}<p class="error">{error}</p>{/if}

  {#if adding}
    <div class="add">
      <button class="pick" onclick={choose}>{picked ? tr("Choose another file") : tr("Choose an audio file")}</button>
      <input bind:this={fileInput} type="file" accept="audio/*" hidden onchange={(e) => usePicked(e.currentTarget.files?.[0] ?? null, e.currentTarget.files?.[0]?.name)} />
      {#if picked}
        <p class="meta">
          <button class="icon small" onclick={preview} aria-label={tr("Listen")}><Play size={16} /></button>
          {tr("{s} s", { s: picked.duration.toFixed(1) })}
          {#if trimmed}· {tr("trimmed to the first {n} seconds", { n: MAX_SECONDS })}{/if}
        </p>
      {/if}
      <div class="fields">
        <input class="emoji" bind:value={emoji} maxlength="8" placeholder="🎺" aria-label={tr("Emoji")} />
        <input bind:value={name} maxlength="32" placeholder={tr("Name")} aria-label={tr("Name")} />
      </div>
      <div class="actions">
        <button class="text" onclick={() => ((adding = false), (picked = null))}>{tr("Cancel")}</button>
        <button class="text accent" disabled={!picked || !name.trim() || busy} onclick={save}>{busy ? tr("Adding…") : tr("Add")}</button>
      </div>
    </div>
  {:else}
    <div class="grid">
      {#each builtinSounds as sound (sound.id)}
        {@const s = { kind: "builtin", sound } as const}
        <button class="sound" class:playing={soundboard.playing === soundboard.key(s)} disabled={call.deafened} onclick={() => play(s)}>
          <span class="emoji">{sound.emoji}</span><span class="name">{sound.name()}</span>
        </button>
      {/each}
    </div>
    {#if soundboard.sounds.length || soundboard.canEdit}
      <h4>{tr("This chat's sounds")}</h4>
      <div class="grid">
        {#each soundboard.sounds as sound (sound.id)}
          {@const s = { kind: "room", sound } as const}
          <button
            class="sound"
            class:playing={soundboard.playing === soundboard.key(s)}
            disabled={call.deafened}
            onclick={() => !longPressed && play(s)}
            oncontextmenu={(e) => {
              if (!soundboard.canEdit) return;
              e.preventDefault();
              removing = sound;
            }}
            onpointerdown={() => pressStart(sound)}
            onpointerup={pressEnd}
            onpointerleave={pressEnd}
          >
            <span class="emoji">{sound.emoji ?? "🔊"}</span><span class="name">{sound.name}</span>
          </button>
        {/each}
        {#if soundboard.canEdit}
          <button class="sound add-tile" onclick={() => ((adding = true), (error = null))}>
            <span class="emoji"><Plus size={22} /></span><span class="name">{tr("Add a sound")}</span>
          </button>
        {/if}
      </div>
    {/if}
  {/if}

  {#if removing}
    <div class="confirm">
      <p>{tr("Remove “{name}” for everyone in this chat?", { name: removing.name })}</p>
      <div class="actions">
        <button class="text" onclick={() => (removing = null)}>{tr("Cancel")}</button>
        <button class="text danger" onclick={() => removing && remove(removing)}>{tr("Remove")}</button>
      </div>
    </div>
  {/if}
</div>

<style>
  .panel {
    position: absolute;
    left: 12px;
    right: 12px;
    bottom: calc(110px + env(safe-area-inset-bottom));
    max-width: 520px;
    margin: 0 auto;
    max-height: min(60vh, 520px);
    overflow-y: auto;
    z-index: 5;
    padding: 12px;
    border-radius: 18px;
    background: var(--surface);
    color: var(--text);
    box-shadow: 0 10px 40px rgb(0 0 0 / 0.45);
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 8px;
  }
  .volume {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0 8px 0 auto;
    color: var(--muted);
  }
  .volume input {
    width: 110px;
    accent-color: var(--accent);
  }
  h4 {
    margin: 14px 2px 8px;
    font-size: 0.8rem;
    font-weight: 600;
    color: var(--muted);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(92px, 1fr));
    gap: 8px;
  }
  .sound {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    padding: 10px 6px;
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
  .sound:active {
    transform: scale(0.94);
  }
  .sound.playing {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 30%, transparent);
  }
  .sound:disabled {
    opacity: 0.4;
  }
  .sound .emoji {
    font-size: 1.6rem;
    line-height: 1;
    display: grid;
    place-items: center;
    min-height: 1.6rem;
  }
  .sound .name {
    font-size: 0.78rem;
    text-align: center;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 100%;
  }
  .add-tile {
    border: 1px dashed var(--border);
    background: none;
  }
  .icon {
    width: 36px;
    height: 36px;
    border: none;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: none;
    color: inherit;
    cursor: pointer;
  }
  .icon.small {
    width: 28px;
    height: 28px;
    display: inline-grid;
    vertical-align: middle;
  }
  .note,
  .error,
  .meta {
    margin: 4px 2px 8px;
    font-size: 0.85rem;
    color: var(--muted);
  }
  .error {
    color: var(--danger);
  }
  .add {
    display: grid;
    gap: 10px;
  }
  .pick,
  .text {
    border: none;
    border-radius: 999px;
    padding: 9px 16px;
    background: var(--surface-hover);
    color: inherit;
    font: inherit;
    cursor: pointer;
  }
  .text.accent {
    background: var(--accent);
    color: #fff;
  }
  .text.danger {
    background: var(--danger);
    color: #fff;
  }
  .text:disabled {
    opacity: 0.5;
  }
  .fields {
    display: flex;
    gap: 8px;
  }
  .fields input {
    flex: 1;
    min-width: 0;
    padding: 9px 12px;
    border-radius: 10px;
    border: 1px solid var(--border);
    background: var(--bg);
    color: inherit;
    font: inherit;
  }
  .fields input.emoji {
    flex: 0 0 56px;
    text-align: center;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
  .confirm {
    margin-top: 12px;
    padding: 12px;
    border-radius: 12px;
    background: var(--surface-hover);
  }
  .confirm p {
    margin: 0 0 10px;
  }
</style>
