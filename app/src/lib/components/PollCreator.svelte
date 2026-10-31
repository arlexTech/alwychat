<script lang="ts">
  import Plus from "@lucide/svelte/icons/plus";
  import X from "@lucide/svelte/icons/x";
  import { createPoll } from "$lib/backend";
  import Page from "./Page.svelte";

  let { roomId, onClose }: { roomId: string; onClose: () => void } = $props();

  let question = $state("");
  let answers = $state(["", ""]);
  let disclosed = $state(true);
  let multiple = $state(false);
  let sending = $state(false);
  let error = $state<string | null>(null);

  const filled = $derived(answers.filter((a) => a.trim()).length);
  const ready = $derived(question.trim() && filled >= 2 && !sending);

  async function send(event: SubmitEvent) {
    event.preventDefault();
    if (!ready) return;
    sending = true;
    error = null;
    try {
      await createPoll(roomId, question, answers, disclosed, multiple);
      onClose();
    } catch (e) {
      error = String(e);
      sending = false;
    }
  }
</script>

<Page title="New poll" {onClose}>
  <form onsubmit={send}>
    <label class="field">
      <span>Question</span>
      <!-- svelte-ignore a11y_autofocus -->
      <input bind:value={question} placeholder="Ask something" maxlength="300" autofocus />
    </label>

    <span class="caption">Answers</span>
    {#each answers as _, i (i)}
      <div class="answer">
        <input bind:value={answers[i]} placeholder="Answer {i + 1}" maxlength="200" />
        {#if answers.length > 2}
          <button type="button" class="icon" onclick={() => answers.splice(i, 1)} aria-label="Remove answer {i + 1}">
            <X size={18} />
          </button>
        {/if}
      </div>
    {/each}
    {#if answers.length < 20}
      <button type="button" class="add" onclick={() => answers.push("")}><Plus size={18} /> Add an answer</button>
    {/if}

    <label class="toggle">
      <span>
        <strong>Show results while voting</strong>
        <small>Off: results appear only when you end the poll.</small>
      </span>
      <input type="checkbox" role="switch" bind:checked={disclosed} />
    </label>
    <label class="toggle">
      <span>
        <strong>Allow several answers</strong>
      </span>
      <input type="checkbox" role="switch" bind:checked={multiple} />
    </label>

    {#if error}<p class="error">{error}</p>{/if}
    <button class="send" disabled={!ready}>{sending ? "Sending…" : "Send poll"}</button>
  </form>
</Page>

<style>
  form {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-bottom: 8px;
  }
  .field span,
  .caption {
    font-size: 0.8rem;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--muted);
  }
  input:not([type="checkbox"]) {
    flex: 1;
    min-width: 0;
    padding: 11px 14px;
    border: none;
    border-radius: 12px;
    background: var(--surface-2);
    outline: none;
  }
  .answer {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .icon {
    display: grid;
    place-items: center;
    width: 40px;
    height: 40px;
    padding: 0;
    border: none;
    border-radius: 50%;
    background: none;
    color: var(--muted);
    cursor: pointer;
  }
  .add {
    display: flex;
    align-items: center;
    gap: 8px;
    align-self: flex-start;
    padding: 8px 12px;
    border: none;
    border-radius: 999px;
    background: none;
    color: var(--accent-a);
    cursor: pointer;
  }
  .toggle {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 12px 0;
    border-top: 1px solid var(--border);
    cursor: pointer;
  }
  .toggle span {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .toggle small {
    color: var(--muted);
  }
  input[type="checkbox"] {
    appearance: none;
    flex: none;
    position: relative;
    width: 44px;
    height: 26px;
    margin: 0;
    border-radius: 999px;
    background: var(--surface-hover);
    cursor: pointer;
    transition: background 0.2s var(--ease);
  }
  input[type="checkbox"]::after {
    content: "";
    position: absolute;
    top: 3px;
    left: 3px;
    width: 20px;
    height: 20px;
    border-radius: 50%;
    background: white;
    transition: transform 0.2s var(--ease);
  }
  input[type="checkbox"]:checked {
    background: var(--accent);
  }
  input[type="checkbox"]:checked::after {
    transform: translateX(18px);
  }
  .send {
    margin-top: 10px;
    padding: 12px;
    border: none;
    border-radius: 999px;
    background: var(--accent);
    color: white;
    font-weight: 600;
    cursor: pointer;
  }
  .send:disabled {
    opacity: 0.4;
    cursor: default;
  }
  .error {
    color: var(--danger);
  }
</style>
