<script lang="ts">
  import { tr } from "$lib/i18n/index.svelte";
  import ImageIcon from "@lucide/svelte/icons/image";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import { setQuickReaction, setTheme, uploadWallpaper } from "$lib/backend";
  import { chat } from "$lib/stores/chat.svelte";
  import { pickImage } from "$lib/pickImage";
  import { DEFAULT_QUICK_REACTION, DEFAULT_THEME, QUICK_REACTIONS, THEMES, themeById, type Theme } from "$lib/themes";
  import ChatScene from "./ChatScene.svelte";
  import Page from "./Page.svelte";
  import { fade, fly } from "svelte/transition";
  import { untrack } from "svelte";
  import { onBack } from "$lib/back.svelte";

  /** Messenger's "Customize": the chat's theme (or a photo) and quick reaction, for everyone. */
  let {
    roomId,
    startTab = "themes",
    onClose,
  }: { roomId: string; startTab?: "themes" | "reaction"; onClose: () => void } = $props();

  let tab = $state(untrack(() => startTab));
  const style = $derived(chat.style);
  const canEdit = $derived(style?.can_edit ?? false);
  const current = $derived(themeById(style?.theme));
  const quick = $derived(style?.quick_reaction ?? DEFAULT_QUICK_REACTION);

  /** Being previewed: a preset, or a picked photo (base64 + object URL). */
  let preview = $state<{ theme: Theme; photo?: { data: string; url: string } } | null>(null);
  let alsoEmoji = $state(true);
  let busy = $state(false);
  let error = $state<string | null>(null);
  let customEmoji = $state("");
  $effect(() => {
    if (preview) return onBack(() => (preview = null));
  });

  async function pickPhoto() {
    error = null;
    try {
      const image = await pickImage(1600);
      if (image) preview = { theme: current, photo: { data: image.data, url: image.preview } };
    } catch (e) {
      error = String(e);
    }
  }

  async function select() {
    const p = preview;
    if (!p) return;
    busy = true;
    error = null;
    const previous = chat.style;
    try {
      const wallpaper = p.photo ? await uploadWallpaper(p.photo.data) : null;
      if (previous) chat.style = { ...previous, theme: p.theme.id, wallpaper };
      await setTheme(roomId, p.theme.id, wallpaper);
      if (!p.photo && alsoEmoji && p.theme.emoji !== quick) {
        if (chat.style) chat.style = { ...chat.style, quick_reaction: p.theme.emoji };
        await setQuickReaction(roomId, p.theme.emoji);
      }
      preview = null;
      onClose();
    } catch (e) {
      chat.style = previous;
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function removePhoto() {
    const previous = chat.style;
    if (previous) chat.style = { ...previous, wallpaper: null };
    await setTheme(roomId, style?.theme ?? DEFAULT_THEME.id, null).catch((e) => ((chat.style = previous), (error = String(e))));
  }

  async function chooseQuick(emoji: string) {
    const previous = chat.style;
    if (previous) chat.style = { ...previous, quick_reaction: emoji };
    await setQuickReaction(roomId, emoji).catch((e) => ((chat.style = previous), (error = String(e))));
  }
</script>

<Page title={tr("Customize")} {onClose}>
  <div class="m-seg tabs">
    <button class:on={tab === "themes"} onclick={() => (tab = "themes")}>{tr("Themes")}</button>
    <button class:on={tab === "reaction"} onclick={() => (tab = "reaction")}>{tr("Quick reaction")}</button>
  </div>
  {#if !canEdit && style}<p class="m-hint">{tr("Only the chat's admins can change these.")}</p>{/if}

  {#if tab === "themes"}
    <div class="grid">
      <button class="card upload" disabled={!canEdit} onclick={pickPhoto}>
        <ImageIcon size={26} />
        <span>{tr("Use a photo")}</span>
      </button>
      {#if style?.wallpaper}
        <button class="card upload" disabled={!canEdit} onclick={removePhoto}>
          <Trash2 size={24} />
          <span>{tr("Remove photo")}</span>
        </button>
      {/if}
      {#each THEMES as t (t.id)}
        <button class="card" class:on={t.id === current.id && !style?.wallpaper} disabled={!canEdit} onclick={() => (preview = { theme: t })} style:--a={t.a} style:--b={t.b}>
          <span class="thumb" class:plain={!t.scene}>
            {#if t.scene}<ChatScene scene={t.scene} mini />{/if}
            <span class="sample in" style:background={t.incoming ?? "var(--surface-2)"}></span>
            <span class="sample out"></span>
            <span class="sample out short"></span>
          </span>
          <span class="name">{t.name} {t.emoji}</span>
        </button>
      {/each}
    </div>
  {:else}
    <p class="m-hint">{tr("Sent when you tap the button next to the message field. A double tap on a message puts a ❤️.")}</p>
    <div class="emojis">
      {#each QUICK_REACTIONS as e (e)}
        <button class:on={e === quick} disabled={!canEdit} onclick={() => chooseQuick(e)}>{e}</button>
      {/each}
      {#if !QUICK_REACTIONS.includes(quick)}<button class="on" disabled>{quick}</button>{/if}
    </div>
    <form
      class="custom"
      onsubmit={(e) => {
        e.preventDefault();
        const emoji = [...new Intl.Segmenter().segment(customEmoji.trim())][0]?.segment;
        if (emoji) void chooseQuick(emoji);
        customEmoji = "";
      }}
    >
      <input class="m-input" bind:value={customEmoji} placeholder={tr("Any emoji")} maxlength="16" disabled={!canEdit} />
      <button class="m-btn plain" disabled={!canEdit || !customEmoji.trim()}>{tr("Use")}</button>
    </form>
  {/if}
  {#if error}<p class="m-error">{error}</p>{/if}
</Page>

{#if preview}
  {@const t = preview.theme}
  <div class="preview" class:plain={!preview.photo && !t.scene} style:--accent-a={t.a} style:--accent-b={t.b} transition:fly={{ y: 40, duration: 220 }}>
    {#if preview.photo}
      <div class="bg photo" style:background-image="url({preview.photo.url})"></div>
    {:else if t.scene}
      <div class="bg" in:fade><ChatScene scene={t.scene} /></div>
    {:else}
      <div class="bg plain"></div>
    {/if}
    <header>
      <button class="back" onclick={() => (preview = null)} aria-label={tr("Back")}>←</button>
      <h2>{tr("Previewing {value}", { value: preview.photo ? "your photo" : t.name })}</h2>
    </header>
    <div class="messages">
      <p class="msg out">{tr("There are many themes to choose from, and each one is a little different.")}</p>
      <p class="msg out">{tr("Your messages in this chat look like this, for everyone.")}</p>
      <p class="msg in" style:background={t.incoming ?? (preview.photo ? "rgb(20 22 30 / 0.82)" : "var(--surface-2)")}>{tr("And the others' messages look like this.")}</p>
      <p class="msg out">{tr("Tap Select to choose it, or Cancel to look at others. {emoji}", { emoji: t.emoji })}</p>
    </div>
    <div class="actions">
      {#if !preview.photo && t.emoji !== quick}
        <label class="emoji-opt"><input type="checkbox" bind:checked={alsoEmoji} /> {tr("Also use {emoji} as the quick reaction", { emoji: t.emoji })}</label>
      {/if}
      {#if error}<p class="m-error">{error}</p>{/if}
      <div class="m-buttons">
        <button class="m-btn plain" onclick={() => (preview = null)}>{tr("Cancel")}</button>
        <button class="m-btn select" disabled={busy} onclick={select}>{busy ? tr("Saving…") : tr("Select")}</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .tabs {
    margin-bottom: 14px;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(104px, 1fr));
    gap: 12px;
  }
  .card {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 0;
    border: none;
    background: none;
    color: var(--text);
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .card:disabled {
    cursor: default;
  }
  .thumb {
    position: relative;
    display: block;
    aspect-ratio: 9 / 16;
    overflow: hidden;
    border: 2px solid transparent;
    border-radius: 16px;
    background: var(--bg);
    box-shadow: inset 0 0 0 1px var(--border);
  }
  .card.on .thumb {
    border-color: var(--a);
    box-shadow: 0 0 0 2px var(--b);
  }
  .sample {
    position: absolute;
    height: 14px;
    border-radius: 7px;
  }
  .sample.in {
    top: 26%;
    left: 8%;
    width: 55%;
  }
  .sample.out {
    top: 44%;
    right: 8%;
    width: 62%;
    background: linear-gradient(to bottom, var(--a), var(--b));
  }
  .sample.out.short {
    top: 58%;
    width: 38%;
  }
  .name {
    overflow: hidden;
    font-size: 0.9rem;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .upload {
    aspect-ratio: 9 / 16;
    align-items: center;
    justify-content: center;
    border-radius: 16px;
    background: var(--surface);
    color: var(--text);
    text-align: center;
  }
  .emojis {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin: 10px 0;
  }
  .emojis button {
    width: 52px;
    height: 52px;
    border: 2px solid transparent;
    border-radius: 14px;
    background: var(--surface);
    font-size: 1.6rem;
    cursor: pointer;
  }
  .emojis button.on {
    border-color: var(--accent-a);
  }
  .custom {
    display: flex;
    gap: 8px;
  }
  .preview {
    position: fixed;
    inset: 0;
    z-index: 60;
    display: flex;
    flex-direction: column;
    color: #f2f4f8;
  }
  .bg {
    position: absolute;
    inset: 0;
  }
  /* A theme without scenery is previewed on the app's own background and text colours. */
  .preview.plain {
    color: var(--text);
  }
  .bg.plain {
    background: var(--bg);
  }
  .bg.photo {
    background-position: center;
    background-size: cover;
  }
  .bg.photo::after {
    content: "";
    position: absolute;
    inset: 0;
    background: linear-gradient(rgb(0 0 0 / 0.25), rgb(0 0 0 / 0.55));
  }
  header,
  .messages,
  .actions {
    position: relative;
  }
  header {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 12px;
  }
  header h2 {
    margin: 0;
    font-size: 1.25rem;
  }
  .back {
    width: 42px;
    height: 42px;
    border: none;
    border-radius: 50%;
    background: none;
    color: inherit;
    font-size: 1.4rem;
    cursor: pointer;
  }
  .messages {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 4px;
    padding: 16px 12px;
    overflow: hidden;
  }
  .msg {
    max-width: 75%;
    margin: 0;
    padding: 9px 14px;
    border-radius: 18px;
    line-height: 1.35;
  }
  .msg.out {
    align-self: flex-end;
    background: linear-gradient(to bottom, var(--accent-a), var(--accent-b));
    background-attachment: fixed;
    color: white;
  }
  .msg.in {
    align-self: flex-start;
    margin: 6px 0;
  }
  .actions {
    padding: 12px 14px calc(14px + env(safe-area-inset-bottom));
  }
  .emoji-opt {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 4px;
    font-size: 0.9rem;
  }
  .select {
    background: linear-gradient(135deg, var(--accent-a), var(--accent-b));
  }
</style>
