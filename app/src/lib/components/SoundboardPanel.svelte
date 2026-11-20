<script lang="ts">
  // Soundboard (Discord style), from the call view: tap a sound, everyone in the call hears it.
  // Built-in sounds (minus those a reaction already plays, and those this chat took off), then
  // the chat's own. People allowed to change the chat add a short audio file (trimmed to 6 s,
  // sent as WAV), remove one of its sounds or take a built-in one off (long-press /
  // right-click), and bring taken-off ones back.
  import { tr, trn } from "$lib/i18n/index.svelte";
  import { onBack } from "$lib/back.svelte";
  import { onMount } from "svelte";
  import { fly } from "svelte/transition";
  import X from "@lucide/svelte/icons/x";
  import Plus from "@lucide/svelte/icons/plus";
  import Play from "@lucide/svelte/icons/play";
  import SoundVolume from "./SoundVolume.svelte";
  import SoundTile from "./SoundTile.svelte";
  import { isAndroid, type Sound } from "$lib/backend";
  import { call } from "$lib/stores/call.svelte";
  import { builtinSounds, MAX_SECONDS, soundboard, type AnySound } from "$lib/stores/soundboard.svelte";
  import { REACTION_SOUND_IDS } from "$lib/soundboard/reactions";
  import type { BuiltinSound } from "$lib/soundboard/builtin";

  let { roomId, onClose }: { roomId: string; onClose: () => void } = $props();

  let adding = $state(false);
  let name = $state("");
  let emoji = $state("");
  let picked = $state<AudioBuffer | null>(null);
  let trimmed = $state(false);
  let busy = $state(false);
  let error = $state<string | null>(null);
  /** Asking before removing a chat sound or taking a built-in one off. */
  let removing = $state<{ kind: "room"; sound: Sound } | { kind: "builtin"; sound: BuiltinSound } | null>(null);
  let fileInput = $state<HTMLInputElement>();

  // Built-ins a reaction plays (🎉 ta-da, 👏 applause…) are in the reactions panel only.
  const offered = builtinSounds.filter((s) => !REACTION_SOUND_IDS.has(s.id));
  const shown = $derived(offered.filter((s) => !soundboard.hidden.includes(s.id)));
  const takenOff = $derived(offered.filter((s) => soundboard.hidden.includes(s.id)).length);

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

  async function confirmRemove() {
    const what = removing;
    removing = null;
    if (!what) return;
    try {
      if (what.kind === "room") await soundboard.remove(roomId, what.sound.id);
      else await soundboard.setHidden(roomId, [...soundboard.hidden, what.sound.id]);
    } catch (e) {
      error = String(e);
    }
  }

  async function bringBack() {
    try {
      await soundboard.setHidden(roomId, []);
    } catch (e) {
      error = String(e);
    }
  }
</script>

<div class="panel" transition:fly={{ y: -8, duration: 150 }}>
  <header>
    <strong>{tr("Soundboard")}</strong>
    <SoundVolume kind="soundboard" />
    <button class="icon" onclick={onClose} aria-label={tr("Close")}><X size={20} /></button>
  </header>
  <p class="note">{tr("Everyone in the call hears them")}</p>
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
      {#each shown as sound (sound.id)}
        {@const s = { kind: "builtin", sound } as const}
        <SoundTile
          emoji={sound.emoji}
          label={sound.name()}
          playing={soundboard.playing === soundboard.key(s)}
          disabled={call.deafened}
          onclick={() => play(s)}
          onHold={soundboard.canEdit ? () => (removing = { kind: "builtin", sound }) : undefined}
        />
      {/each}
      {#each soundboard.sounds as sound (sound.id)}
        {@const s = { kind: "room", sound } as const}
        <SoundTile
          emoji={sound.emoji ?? "🔊"}
          label={sound.name}
          playing={soundboard.playing === soundboard.key(s)}
          disabled={call.deafened}
          onclick={() => play(s)}
          onHold={soundboard.canEdit ? () => (removing = { kind: "room", sound }) : undefined}
        />
      {/each}
      {#if soundboard.canEdit}
        <SoundTile add label={tr("Add a sound")} onclick={() => ((adding = true), (error = null))}>
          {#snippet icon()}<Plus size={22} />{/snippet}
        </SoundTile>
      {/if}
    </div>
    {#if soundboard.canEdit}
      <p class="hint">
        {tr("Hold or right-click a sound to remove it.")}
        {#if takenOff}<button class="link" onclick={bringBack}>{trn(takenOff, "Bring back {n} removed sound", "Bring back {n} removed sounds")}</button>{/if}
      </p>
    {/if}
  {/if}

  {#if removing}
    <div class="confirm">
      <p>
        {removing.kind === "room"
          ? tr("Remove “{name}” for everyone in this chat?", { name: removing.sound.name })
          : tr("Take “{name}” off this chat's soundboard? You can bring it back later.", { name: removing.sound.name() })}
      </p>
      <div class="actions">
        <button class="text" onclick={() => (removing = null)}>{tr("Cancel")}</button>
        <button class="text danger" onclick={confirmRemove}>{tr("Remove")}</button>
      </div>
    </div>
  {/if}
</div>

<style>
  .panel {
    /* Drops down from the call header (CallView's .head), like the other panels. */
    position: absolute;
    top: calc(100% + 8px);
    right: 0;
    width: min(420px, 100%);
    box-sizing: border-box;
    max-height: min(70vh, 560px);
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
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(76px, 1fr));
    gap: 6px;
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
  .hint {
    margin: 10px 2px 0;
    font-size: 0.78rem;
    color: var(--muted);
  }
  .link {
    margin-left: 4px;
    padding: 0;
    border: none;
    background: none;
    color: var(--accent);
    font: inherit;
    cursor: pointer;
  }
</style>
