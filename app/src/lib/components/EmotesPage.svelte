<script lang="ts">
  // Settings → Chats & media → Emoji & stickers: our own pack (MSC2545, synced in account data,
  // usable in FluffyChat / Cinny / Nheko too), and the packs we can use but not change.
  import { tr, trn } from "$lib/i18n/index.svelte";
  import Plus from "@lucide/svelte/icons/plus";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import { addOwnEmote, removeOwnEmote, uploadEmote } from "$lib/backend";
  import { emotes } from "$lib/stores/emotes.svelte";
  import { mediaUrl } from "$lib/media";
  import { pickEmoteImage, type PickedEmote } from "$lib/pickImage";
  import Page from "./Page.svelte";

  let { onClose }: { onClose: () => void } = $props();

  let error = $state<string | null>(null);
  let busy = $state(false);
  /** The image being added, with its shortcode and kind. */
  let adding = $state<{ image: PickedEmote; shortcode: string; emoticon: boolean; sticker: boolean } | null>(null);

  void emotes.load(null, true);
  const own = $derived(emotes.packs.find((p) => p.editable));
  const others = $derived(emotes.packs.filter((p) => !p.editable));

  const valid = (code: string) => /^[\p{L}\p{N}_-]{1,64}$/u.test(code);

  async function add(kind: "emoticon" | "sticker") {
    error = null;
    try {
      const image = await pickEmoteImage(kind === "sticker" ? 512 : 128);
      if (!image) return;
      adding = { image, shortcode: "", emoticon: kind === "emoticon", sticker: kind === "sticker" };
    } catch (e) {
      error = String(e);
    }
  }

  async function save() {
    if (!adding) return;
    const code = adding.shortcode.trim().replace(/^:|:$/g, "");
    if (!valid(code)) return void (error = tr("Use letters, numbers, _ or - (no spaces)"));
    if (own?.emotes.some((e) => e.shortcode === code)) return void (error = tr(":{code}: is already used", { code }));
    busy = true;
    error = null;
    try {
      const { image } = adding;
      const url = await uploadEmote(image.data, image.mime);
      await addOwnEmote(code, url, { w: image.width, h: image.height, mimetype: image.mime, size: image.size }, adding.emoticon, adding.sticker);
      URL.revokeObjectURL(image.preview);
      adding = null;
      await emotes.load(null, true);
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  function cancel() {
    if (adding) URL.revokeObjectURL(adding.image.preview);
    adding = null;
  }

  async function remove(code: string) {
    error = null;
    try {
      await removeOwnEmote(code);
      await emotes.load(null, true);
    } catch (e) {
      error = String(e);
    }
  }
</script>

<Page title={tr("Emoji & stickers")} {onClose}>
  <p class="m-hint">{tr("Your own emoji and stickers, on all your devices. Type :name: in a message, or pick them from the emoji panel. FluffyChat and Cinny show them too.")}</p>

  {#if adding}
    <div class="adding">
      <img src={adding.image.preview} alt="" class:sticker={adding.sticker && !adding.emoticon} />
      <label class="field">
        <span>{tr("Name")}</span>
        <input class="m-input" bind:value={adding.shortcode} placeholder="party_parrot" autocapitalize="off" spellcheck="false" />
      </label>
      <label class="check"><input type="checkbox" bind:checked={adding.emoticon} />{tr("Emoji (in text)")}</label>
      <label class="check"><input type="checkbox" bind:checked={adding.sticker} />{tr("Sticker")}</label>
      <div class="buttons">
        <button class="m-btn" onclick={cancel}>{tr("Cancel")}</button>
        <button class="m-btn primary" disabled={busy || !adding.shortcode.trim() || (!adding.emoticon && !adding.sticker)} onclick={save}>
          {busy ? tr("Saving…") : tr("Add")}
        </button>
      </div>
    </div>
  {:else}
    <div class="buttons">
      <button class="m-btn" onclick={() => add("emoticon")}><Plus size={16} />{tr("Add an emoji")}</button>
      <button class="m-btn" onclick={() => add("sticker")}><Plus size={16} />{tr("Add a sticker")}</button>
    </div>
  {/if}
  {#if error}<p class="m-error">{error}</p>{/if}

  <h3 class="m-section">{tr("Mine")}</h3>
  {#if own?.emotes.length}
    <ul class="grid">
      {#each own.emotes as e (e.shortcode)}
        <li>
          <img src={mediaUrl(e.url, 64)} alt="" />
          <span>:{e.shortcode}:<small>{e.emoticon && e.sticker ? tr("Emoji & sticker") : e.sticker ? tr("Sticker") : tr("Emoji")}</small></span>
          <button class="icon" onclick={() => remove(e.shortcode)} aria-label={tr("Remove :{code}:", { code: e.shortcode })}><Trash2 size={16} /></button>
        </li>
      {/each}
    </ul>
  {:else}
    <p class="m-hint">{tr("None yet.")}</p>
  {/if}

  {#if others.length}
    <h3 class="m-section">{tr("From your chats")}</h3>
    {#each others as p (p.id)}
      <div class="pack">
        {#if p.avatar ?? p.emotes[0]}<img src={mediaUrl(p.avatar ?? p.emotes[0].url, 32)} alt="" />{/if}
        <span><strong>{p.name}</strong><small>{trn(p.emotes.length, "{n} image", "{n} images")}</small></span>
      </div>
    {/each}
  {/if}
</Page>

<style>
  .buttons {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
    margin: 8px 0;
  }
  .m-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .m-btn.primary {
    background: var(--accent);
    color: #fff;
  }
  .adding {
    display: grid;
    gap: 10px;
    padding: 12px;
    border-radius: 14px;
    background: var(--surface-2);
  }
  .adding img {
    width: 64px;
    height: 64px;
    object-fit: contain;
  }
  .adding img.sticker {
    width: 128px;
    height: 128px;
  }
  .field {
    display: grid;
    gap: 4px;
  }
  .field span {
    font-size: 0.85rem;
    font-weight: 600;
  }
  .check {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .grid {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 6px;
  }
  .grid li,
  .pack {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 6px 4px;
  }
  .grid img,
  .pack img {
    width: 36px;
    height: 36px;
    object-fit: contain;
    border-radius: 6px;
  }
  .grid span,
  .pack span {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .grid small,
  .pack small {
    color: var(--muted);
  }
  .icon {
    display: grid;
    place-items: center;
    width: 34px;
    height: 34px;
    border: 0;
    border-radius: 50%;
    background: none;
    color: var(--muted);
    cursor: pointer;
  }
</style>
