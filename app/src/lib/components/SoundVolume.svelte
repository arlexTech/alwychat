<script lang="ts">
  // How loud the soundboard's or the reactions' sounds are for us: mute button (a second tap
  // brings the volume back) + slider, the same in both panels.
  import Volume2 from "@lucide/svelte/icons/volume-2";
  import VolumeX from "@lucide/svelte/icons/volume-x";
  import { call } from "$lib/stores/call.svelte";
  import { tr } from "$lib/i18n/index.svelte";

  let { kind }: { kind: "soundboard" | "reactions" } = $props();
  const value = $derived(kind === "soundboard" ? call.prefs.soundboardVolume : call.prefs.reactionVolume);
  const off = $derived(value < 0.01);
  const set = (v: number) => (kind === "soundboard" ? call.setSoundboardVolume(v) : call.setReactionVolume(v));
  const toggle = () => (kind === "soundboard" ? call.toggleSoundboardMute() : call.toggleReactionMute());
</script>

<div class="volume" title={kind === "soundboard" ? tr("How loud sounds are for you") : tr("How loud reaction sounds are for you")}>
  <button
    class="mute"
    onclick={toggle}
    aria-label={off ? tr("Turn sounds back on") : tr("Mute sounds")}
    title={off ? tr("Turn sounds back on") : tr("Mute sounds")}
  >
    {#if off}<VolumeX size={18} />{:else}<Volume2 size={18} />{/if}
  </button>
  <input
    type="range"
    min="0"
    max="1"
    step="0.05"
    {value}
    oninput={(e) => set(Number(e.currentTarget.value))}
    aria-label={kind === "soundboard" ? tr("Soundboard volume") : tr("Reaction sounds volume")}
  />
</div>

<style>
  .volume {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0 8px 0 auto;
    color: var(--muted);
  }
  .mute {
    width: 34px;
    height: 34px;
    flex: none;
    display: grid;
    place-items: center;
    padding: 0;
    border: none;
    border-radius: 50%;
    background: none;
    color: inherit;
    cursor: pointer;
  }
  .mute:hover,
  .mute:focus-visible {
    background: var(--surface-hover);
    color: var(--text);
  }
  .volume input {
    width: 110px;
    accent-color: var(--accent);
  }
</style>
