<script lang="ts">
  // Chat settings → Dates & countdowns: the chat's shared dates (everyone in it sees them).
  import { tr } from "$lib/i18n/index.svelte";
  import Plus from "@lucide/svelte/icons/plus";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import Pencil from "@lucide/svelte/icons/pencil";
  import { setCountdowns, type Countdown } from "$lib/backend";
  import { chat } from "$lib/stores/chat.svelte";
  import { describe } from "$lib/countdown";
  import Page from "./Page.svelte";

  let { roomId, onClose }: { roomId: string; onClose: () => void } = $props();

  const items = $derived(chat.style?.countdowns ?? []);
  const canEdit = $derived(!!chat.style?.can_edit);
  let editing = $state<Countdown | null>(null);
  let error = $state<string | null>(null);
  let busy = $state(false);

  const SUGGESTED = ["💞", "🎂", "💍", "✈️", "🎄", "🏖️", "🎓", "🏠", "⏳"];

  function startNew(kind: "until" | "since") {
    editing = { id: crypto.randomUUID(), title: "", date: new Date().toISOString().slice(0, 10), kind, emoji: kind === "since" ? "💞" : "⏳", yearly: kind === "since" };
  }

  async function save(next: Countdown[]) {
    busy = true;
    error = null;
    const previous = chat.style;
    // Shown at once; the chat's style stream brings the server's copy back.
    if (previous) chat.style = { ...previous, countdowns: next };
    try {
      await setCountdowns(roomId, next);
      editing = null;
    } catch (e) {
      chat.style = previous;
      error = String(e);
    } finally {
      busy = false;
    }
  }

  function submit() {
    if (!editing || !editing.title.trim() || !editing.date) return;
    const item = { ...editing, title: editing.title.trim() };
    const exists = items.some((c) => c.id === item.id);
    void save(exists ? items.map((c) => (c.id === item.id ? item : c)) : [...items, item]);
  }
</script>

<Page title={tr("Dates & countdowns")} {onClose}>
  <p class="m-hint">{tr("Shown at the top of the chat for everyone in it: a countdown to a date, or the days since one (\"days together\").")}</p>

  {#if editing}
    <div class="form">
      <div class="kinds">
        <button class:on={editing.kind === "until"} onclick={() => (editing!.kind = "until")}>{tr("Countdown to a date")}</button>
        <button class:on={editing.kind === "since"} onclick={() => (editing!.kind = "since")}>{tr("Days since a date")}</button>
      </div>
      <label class="field">
        <span>{tr("Title")}</span>
        <input class="m-input" bind:value={editing.title} maxlength="80" placeholder={editing.kind === "since" ? tr("Together") : tr("Margot's birthday")} />
      </label>
      <label class="field">
        <span>{tr("Date")}</span>
        <input class="m-input" type="date" bind:value={editing.date} />
      </label>
      <div class="field">
        <span>{tr("Emoji")}</span>
        <div class="emojis">
          {#each SUGGESTED as e (e)}<button class:on={editing.emoji === e} onclick={() => (editing!.emoji = e)}>{e}</button>{/each}
          <input class="m-input own" bind:value={editing.emoji} maxlength="8" aria-label={tr("Other emoji")} />
        </div>
      </div>
      <label class="check"><input type="checkbox" bind:checked={editing.yearly} />{editing.kind === "until" ? tr("Every year (birthday, anniversary)") : tr("Celebrate it every year")}</label>
      {#if editing.title.trim() && editing.date}<p class="preview">{describe(editing).emoji} {describe(editing).text}</p>{/if}
      <div class="buttons">
        <button class="m-btn" onclick={() => (editing = null)}>{tr("Cancel")}</button>
        <button class="m-btn primary" disabled={busy || !editing.title.trim() || !editing.date} onclick={submit}>{busy ? tr("Saving…") : tr("Save")}</button>
      </div>
    </div>
  {:else if canEdit}
    <div class="buttons">
      <button class="m-btn" onclick={() => startNew("until")}><Plus size={16} />{tr("Countdown")}</button>
      <button class="m-btn" onclick={() => startNew("since")}><Plus size={16} />{tr("Days since")}</button>
    </div>
  {:else}
    <p class="m-hint">{tr("Only people who can change this chat's settings can add dates.")}</p>
  {/if}
  {#if error}<p class="m-error">{error}</p>{/if}

  <ul>
    {#each items as c (c.id)}
      {@const v = describe(c)}
      <li>
        <span class="emoji">{v.emoji}</span>
        <span class="text"><strong>{v.text}</strong><small>{new Date(c.date + "T12:00").toLocaleDateString()}{c.yearly ? ` · ${tr("every year")}` : ""}</small></span>
        {#if canEdit}
          <button class="icon" onclick={() => (editing = { ...c })} aria-label={tr("Edit")}><Pencil size={16} /></button>
          <button class="icon" onclick={() => save(items.filter((x) => x.id !== c.id))} aria-label={tr("Remove")}><Trash2 size={16} /></button>
        {/if}
      </li>
    {/each}
  </ul>
</Page>

<style>
  .form {
    display: grid;
    gap: 12px;
    padding: 12px;
    border-radius: 14px;
    background: var(--surface-2);
  }
  .kinds {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }
  .kinds button,
  .emojis button {
    padding: 6px 12px;
    border: 1px solid var(--border);
    border-radius: 16px;
    background: none;
    color: var(--text);
    cursor: pointer;
  }
  .kinds button.on,
  .emojis button.on {
    background: var(--accent);
    border-color: transparent;
    color: #fff;
  }
  .emojis {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    align-items: center;
  }
  .emojis button {
    padding: 4px 8px;
    font-size: 1.15rem;
  }
  .emojis .own {
    width: 70px;
  }
  .field {
    display: grid;
    gap: 4px;
  }
  .field > span {
    font-size: 0.85rem;
    font-weight: 600;
  }
  .check {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .preview {
    margin: 0;
    color: var(--muted);
  }
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
  ul {
    list-style: none;
    margin: 12px 0 0;
    padding: 0;
  }
  li {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 4px;
  }
  .emoji {
    font-size: 1.4rem;
  }
  .text {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .text small {
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
