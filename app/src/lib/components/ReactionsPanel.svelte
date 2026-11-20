<script lang="ts">
  // Reactions panel, from the call view's smiley: raise hand and Element Call's emoji reactions
  // (its events: everyone sees them on our picture, and each app plays the reaction's sound
  // itself — a small speaker marks the ones with a sound). "Custom": any emoji picked with +,
  // remembered on this device (long-press / right-click removes one). Same tiles as the
  // soundboard; its own volume (0 or the speaker icon = no reaction sounds).
  import { tr } from "$lib/i18n/index.svelte";
  import { onBack } from "$lib/back.svelte";
  import { fly } from "svelte/transition";
  import X from "@lucide/svelte/icons/x";
  import Plus from "@lucide/svelte/icons/plus";
  import SoundVolume from "./SoundVolume.svelte";
  import SoundTile from "./SoundTile.svelte";
  import EmojiPicker from "./EmojiPicker.svelte";
  import { call } from "$lib/stores/call.svelte";
  import { stored, asList } from "$lib/stored";
  import { hasReactionSound } from "$lib/soundboard/reactions";

  let { onClose }: { onClose: () => void } = $props();

  /** Element Call's reactions (its names, so its users get the matching sound). */
  const REACTIONS: [string, string, () => string][] = [
    ["👍", "thumbsup", () => tr("Like")],
    ["🎉", "party", () => tr("Party")],
    ["👏", "clapping", () => tr("Clap")],
    ["😄", "laugh", () => tr("Laugh")],
    ["🥰", "heart", () => tr("Love")],
    ["👌", "ok", () => tr("OK")],
    ["👎", "thumbsdown", () => tr("Dislike")],
    ["💡", "lightbulb", () => tr("Idea")],
    ["🤘", "rock", () => tr("Rock")],
    ["👋", "wave", () => tr("Wave")],
    ["🦗", "crickets", () => tr("Crickets")],
    ["😵‍💫", "dizzy", () => tr("Dizzy")],
    ["🐶", "dog", () => tr("Dog")],
    ["🐱", "cat", () => tr("Cat")],
    ["🦌", "deer", () => tr("Deer")],
    ["🥁", "drum", () => tr("Drum")],
    ["🛎️", "bell", () => tr("Bell")],
  ];

  /** Emoji picked with + (newest first). */
  const MINE_KEY = "alwychat.callReactions";
  let mine = $state(asList(stored(MINE_KEY, [])).slice(0, 12));
  let picking = $state(false);

  $effect(() => onBack(() => (picking ? (picking = false) : onClose())));

  function keep(list: string[]) {
    mine = list;
    try {
      localStorage.setItem(MINE_KEY, JSON.stringify(list));
    } catch {
      /* only a convenience */
    }
  }
  function react(emoji: string, name: string) {
    call.react(emoji, name);
    onClose();
  }
  function pickOwn(emoji: string) {
    picking = false;
    if (!REACTIONS.some(([e]) => e === emoji)) keep([emoji, ...mine.filter((e) => e !== emoji)].slice(0, 12));
    // Element Call shows any emoji; an unknown name gets its generic sound there.
    react(emoji, emoji);
  }
</script>

<div class="panel" transition:fly={{ y: -8, duration: 150 }}>
  <header>
    <strong>{tr("Reactions")}</strong>
    <SoundVolume kind="reactions" />
    <button class="icon" onclick={onClose} aria-label={tr("Close")}><X size={20} /></button>
  </header>

  {#if picking}
    <div class="picker"><EmojiPicker onPick={pickOwn} /></div>
  {:else}
    <button class="hand-toggle" class:up={call.handUp} onclick={() => (onClose(), void call.toggleHand())}>
      <span class="hand-icon">✋</span>{call.handUp ? tr("Lower hand") : tr("Raise hand")}
    </button>

    <div class="grid first">
      {#each REACTIONS as [emoji, name, label] (name)}
        <SoundTile {emoji} label={label()} sound={hasReactionSound(emoji)} onclick={() => react(emoji, name)} />
      {/each}
    </div>

    <h4>{tr("Custom")}</h4>
    <div class="grid">
      {#each mine as emoji (emoji)}
        <SoundTile {emoji} label={tr("Custom")} sound={hasReactionSound(emoji)} onclick={() => react(emoji, emoji)} onHold={() => keep(mine.filter((e) => e !== emoji))} />
      {/each}
      <SoundTile add label={tr("Emoji")} onclick={() => (picking = true)}>
        {#snippet icon()}<Plus size={22} />{/snippet}
      </SoundTile>
    </div>
    {#if mine.length}<p class="hint">{tr("Hold or right-click one to remove it.")}</p>{/if}
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
  .hand-toggle {
    display: flex;
    width: 100%;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 10px;
    border: none;
    border-radius: 12px;
    background: var(--surface-hover);
    color: inherit;
    font: inherit;
    font-weight: 600;
    cursor: pointer;
  }
  .hand-toggle.up {
    background: #f5b301;
    color: #1b1300;
  }
  .hand-icon {
    font-size: 1.15rem;
    line-height: 1;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(76px, 1fr));
    gap: 6px;
  }
  .grid.first {
    margin-top: 10px;
  }
  h4 {
    margin: 14px 2px 8px;
    font-size: 0.8rem;
    font-weight: 600;
    color: var(--muted);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .hint {
    margin: 10px 2px 0;
    font-size: 0.78rem;
    color: var(--muted);
  }
  .picker {
    display: flex;
    justify-content: center;
  }
</style>
