<script lang="ts">
  import { onMount } from "svelte";
  import { roomMembers, setNickname, setQuickReaction, setTheme, type Member } from "$lib/backend";
  import { chat } from "$lib/stores/chat.svelte";
  import { chatList } from "$lib/stores/chatList.svelte";
  import { spaces } from "$lib/stores/spaces.svelte";
  import { DEFAULT_QUICK_REACTION, DEFAULT_THEME, QUICK_REACTIONS, THEMES } from "$lib/themes";
  import Avatar from "./Avatar.svelte";
  import Page from "./Page.svelte";
  import Images from "@lucide/svelte/icons/images";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";

  let {
    roomId,
    hd,
    onChooseHd,
    onSharedMedia,
    onClose,
  }: {
    roomId: string;
    /** This chat's photo quality: null = app default. */
    hd: boolean | null;
    onChooseHd: (hd: boolean | null) => void;
    onSharedMedia: () => void;
    onClose: () => void;
  } = $props();

  const summary = $derived(chatList.rooms.find((r) => r.id === roomId) ?? null);

  const style = $derived(chat.style);
  const canEdit = $derived(style?.can_edit ?? false);
  const theme = $derived(style?.theme ?? DEFAULT_THEME.id);
  const quick = $derived(style?.quick_reaction ?? DEFAULT_QUICK_REACTION);

  let members = $state<Member[]>([]);
  let customEmoji = $state("");
  /** User ID whose nickname is being edited, and the draft. */
  let editing = $state<{ userId: string; value: string } | null>(null);
  let error = $state<string | null>(null);

  onMount(() => {
    roomMembers(roomId)
      .then((list) => (members = list.sort((a, b) => Number(b.is_me) - Number(a.is_me))))
      .catch((e) => (error = String(e)));
  });

  /** Show the change at once; the server's copy comes back through the chat's style stream. */
  async function apply(change: Partial<NonNullable<typeof chat.style>>, save: () => Promise<void>) {
    const previous = chat.style;
    if (previous) chat.style = { ...previous, ...change };
    error = null;
    try {
      await save();
    } catch (e) {
      chat.style = previous;
      error = String(e);
    }
  }

  const chooseTheme = (id: string) => apply({ theme: id }, () => setTheme(roomId, id));
  const chooseQuick = (emoji: string) => apply({ quick_reaction: emoji }, () => setQuickReaction(roomId, emoji));

  function submitCustomEmoji(event: SubmitEvent) {
    event.preventDefault();
    const emoji = [...new Intl.Segmenter().segment(customEmoji.trim())][0]?.segment;
    if (emoji) chooseQuick(emoji);
    customEmoji = "";
  }

  function saveNickname(member: Member) {
    if (!editing) return;
    const value = editing.value.trim() || null;
    editing = null;
    if (member.is_me) {
      // Our per-room display name: shows in the timeline once the member event syncs back.
      error = null;
      setNickname(roomId, member.user_id, value)
        .then(() => (member.name = value ?? member.name))
        .catch((e) => (error = String(e)));
      return;
    }
    const nicknames = { ...(chat.style?.nicknames ?? {}) };
    if (value) nicknames[member.user_id] = value;
    else delete nicknames[member.user_id];
    apply({ nicknames }, () => setNickname(roomId, member.user_id, value));
  }

  function shownName(member: Member) {
    return member.is_me ? member.name : (style?.nicknames[member.user_id] ?? member.name);
  }
</script>

<Page title="Chat settings" {onClose}>
  {#if style && !canEdit}
    <p class="muted hint">Only the chat's admins can change its theme, quick reaction and nicknames.</p>
  {/if}

  <button class="link-row" onclick={onSharedMedia}>
    <Images size={20} />
    <span><strong>Shared media</strong><small>Photos, videos, files and links</small></span>
    <ChevronRight size={18} />
  </button>

  {#if summary && !summary.is_dm && spaces.spacesOf(roomId).length}
    <label class="switch-row">
      <span>
        <strong>Show in Chats</strong>
        <small>This chat is in a space. On: it's also listed in the Chats tab.</small>
      </span>
      <input
        type="checkbox"
        role="switch"
        checked={spaces.inChats(summary)}
        onchange={(e) => spaces.setRoom(roomId, e.currentTarget.checked).catch(() => {})}
      />
    </label>
  {/if}

  <h3 class="section">Theme</h3>
  <div class="themes">
    {#each THEMES as t (t.id)}
      <button
        class="swatch"
        class:active={t.id === theme}
        style:--a={t.a}
        style:--b={t.b}
        disabled={!canEdit}
        onclick={() => chooseTheme(t.id)}
        aria-label={t.name}
        title={t.name}
      ></button>
    {/each}
  </div>

  <h3 class="section">Quick reaction</h3>
  <div class="emojis">
    {#each QUICK_REACTIONS as e (e)}
      <button class="emoji" class:active={e === quick} disabled={!canEdit} onclick={() => chooseQuick(e)}>{e}</button>
    {/each}
    {#if !QUICK_REACTIONS.includes(quick)}
      <button class="emoji active" disabled>{quick}</button>
    {/if}
  </div>
  <form class="custom-emoji" onsubmit={submitCustomEmoji}>
    <input bind:value={customEmoji} placeholder="Or type any emoji" disabled={!canEdit} maxlength="16" />
    <button disabled={!canEdit || !customEmoji.trim()}>Use</button>
  </form>

  <h3 class="section">Nicknames</h3>
  <ul class="members">
    {#each members as m (m.user_id)}
      <li>
        <Avatar name={m.name} mxc={m.avatar} size={36} />
        {#if editing?.userId === m.user_id}
          <!-- svelte-ignore a11y_autofocus -->
          <input
            class="nick-input"
            bind:value={editing.value}
            placeholder={m.name}
            autofocus
            onkeydown={(e) => {
              if (e.key === "Enter") saveNickname(m);
              if (e.key === "Escape") editing = null;
            }}
            onblur={() => saveNickname(m)}
          />
        {:else}
          <button
            class="member"
            disabled={!canEdit && !m.is_me}
            onclick={() => (editing = { userId: m.user_id, value: m.is_me ? "" : (style?.nicknames[m.user_id] ?? "") })}
          >
            <strong>{shownName(m)}{m.is_me ? " (you)" : ""}</strong>
            <small>{style?.nicknames[m.user_id] || m.is_me ? m.user_id : "Set a nickname"}</small>
          </button>
        {/if}
      </li>
    {/each}
  </ul>

  <h3 class="section">Photos sent in this chat</h3>
  <div class="choices">
    <button class:active={hd === null} onclick={() => onChooseHd(null)}>App default</button>
    <button class:active={hd === true} onclick={() => onChooseHd(true)}>Always HD</button>
    <button class:active={hd === false} onclick={() => onChooseHd(false)}>Compressed</button>
  </div>

  {#if error}<p class="error">{error}</p>{/if}
</Page>

<style>
  .switch-row {
    display: flex;
    align-items: center;
    gap: 16px;
    margin-top: 12px;
    padding: 12px 14px;
    border-radius: 14px;
    background: var(--surface-2);
  }
  .switch-row span {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 2px;
  }
  .switch-row small {
    color: var(--muted);
  }
  .link-row {
    display: flex;
    align-items: center;
    gap: 14px;
    width: 100%;
    padding: 12px 14px;
    border: none;
    border-radius: 14px;
    background: var(--surface-2);
    color: var(--text);
    text-align: left;
    cursor: pointer;
  }
  .link-row span {
    display: flex;
    flex: 1;
    flex-direction: column;
  }
  .link-row small {
    color: var(--muted);
  }
  h3 {
    margin: 8px 0 10px;
    font-size: 0.8rem;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--muted);
  }
  h3.section {
    margin-top: 22px;
  }
  .hint {
    margin: -4px 0 14px;
    font-size: 0.85rem;
  }
  button:disabled {
    cursor: default;
  }
  .themes {
    display: grid;
    grid-template-columns: repeat(5, 1fr);
    gap: 12px;
  }
  .swatch {
    aspect-ratio: 1;
    border: none;
    border-radius: 50%;
    background: linear-gradient(135deg, var(--a), var(--b));
    cursor: pointer;
    transition: transform 0.15s var(--ease), box-shadow 0.15s var(--ease);
  }
  .swatch:not(:disabled):active {
    transform: scale(0.92);
  }
  .swatch.active {
    box-shadow: 0 0 0 3px var(--bg), 0 0 0 5px var(--text);
  }
  .swatch:disabled:not(.active) {
    opacity: 0.45;
  }
  .emojis {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .emoji {
    width: 44px;
    height: 44px;
    border: none;
    border-radius: 12px;
    background: var(--surface-2);
    font-size: 1.5rem;
    cursor: pointer;
    transition: transform 0.15s var(--ease);
  }
  .emoji:not(:disabled):active {
    transform: scale(0.88);
  }
  .emoji.active {
    background: color-mix(in srgb, var(--accent-a) 35%, var(--surface-2));
    box-shadow: inset 0 0 0 2px var(--accent-a);
  }
  .custom-emoji {
    display: flex;
    gap: 8px;
    margin-top: 10px;
  }
  .custom-emoji input,
  .nick-input {
    flex: 1;
    min-width: 0;
    padding: 9px 14px;
    border: none;
    border-radius: 999px;
    background: var(--surface-2);
    outline: none;
  }
  .custom-emoji button {
    padding: 8px 16px;
    border: none;
    border-radius: 999px;
    background: var(--accent);
    color: white;
    cursor: pointer;
  }
  .custom-emoji button:disabled {
    opacity: 0.4;
  }
  .members {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .members li {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 6px 0;
  }
  .member {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    padding: 6px 10px;
    border: none;
    border-radius: 10px;
    background: none;
    text-align: left;
    cursor: pointer;
  }
  .member:not(:disabled):hover {
    background: var(--surface-hover);
  }
  .member strong,
  .member small {
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .member small {
    color: var(--muted);
  }
  .choices {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }
  .choices button {
    padding: 7px 14px;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: transparent;
    cursor: pointer;
  }
  .choices button.active {
    border-color: transparent;
    background: var(--accent);
    color: white;
  }
  .muted {
    color: var(--muted);
  }
  .error {
    margin: 14px 0 0;
    color: var(--danger);
  }
</style>
