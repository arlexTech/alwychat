<script lang="ts">
  import { tr } from "$lib/i18n/index.svelte";
  import { fade, fly, scale } from "svelte/transition";
  import {
    acceptVerification,
    cancelVerification,
    confirmEmojis,
    recover,
    verifyThisDevice,
  } from "$lib/backend";
  import { encryption } from "$lib/stores/encryption.svelte";

  type Mode = "choose" | "key";

  let mode = $state<Mode>("choose");
  let key = $state("");
  let busy = $state(false);
  let error = $state<string | null>(null);

  const v = $derived(encryption.verification);
  const status = $derived(encryption.status);
  /** A verification is on screen (anything but a finished/idle one). */
  const verifying = $derived(!!v && v.state !== "cancelled" && v.state !== "done");

  async function run(action: () => Promise<void>) {
    busy = true;
    error = null;
    try {
      await action();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function submitKey(event: SubmitEvent) {
    event.preventDefault();
    await run(async () => {
      await recover(key);
      key = "";
      mode = "choose";
    });
  }

  function close() {
    if (verifying) cancelVerification().catch(() => {});
    encryption.sheetOpen = false;
    encryption.verification = null;
    mode = "choose";
    error = null;
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="backdrop" transition:fade={{ duration: 150 }} onclick={close}></div>
<div class="sheet" role="dialog" aria-modal="true" aria-labelledby="unlock-title" transition:fly={{ y: 24, duration: 220 }}>
  <button class="close" onclick={close} aria-label={tr("Close")}>×</button>

  {#if v?.state === "incoming"}
    <h2 id="unlock-title">{tr("Verification request")}</h2>
    <p><strong>{v.from}</strong> {tr("wants to verify this device.")}</p>
    <div class="buttons">
      <button class="secondary" onclick={close}>{tr("Decline")}</button>
      <button class="primary" disabled={busy} onclick={() => run(acceptVerification)}>{tr("Accept")}</button>
    </div>
  {:else if v?.state === "requested"}
    <h2 id="unlock-title">{tr("Check your other device")}</h2>
    <p>{tr("Open Element on a device where you're already signed in and accept the request.")}</p>
    <div class="spinner"></div>
  {:else if v?.state === "ready"}
    <h2 id="unlock-title">{tr("Getting the emoji ready…")}</h2>
    <div class="spinner"></div>
  {:else if v?.state === "emojis"}
    <h2 id="unlock-title">{tr("Do these emoji match?")}</h2>
    <p>{tr("Compare them with the ones shown on your other device, in the same order.")}</p>
    <div class="emojis">
      {#each v.emojis as e, i (i)}
        <div class="emoji" in:scale={{ delay: i * 50, duration: 200, start: 0.5 }}>
          <span>{e.symbol}</span>
          <small>{e.description}</small>
        </div>
      {/each}
    </div>
    <div class="buttons">
      <button class="secondary" disabled={busy} onclick={() => run(() => confirmEmojis(false))}>{tr("They don't match")}</button>
      <button class="primary" disabled={busy} onclick={() => run(() => confirmEmojis(true))}>{tr("They match")}</button>
    </div>
  {:else if v?.state === "confirmed"}
    <h2 id="unlock-title">{tr("Waiting for your other device…")}</h2>
    <div class="spinner"></div>
  {:else if v?.state === "done" || (status && !encryption.locked && !encryption.backupMissing)}
    <h2 id="unlock-title">{tr("You're all set ✨")}</h2>
    <p>{tr("This device is verified. Older messages will unlock as their keys arrive.")}</p>
    <div class="buttons">
      <button class="primary" onclick={close}>{tr("Done")}</button>
    </div>
  {:else if mode === "key"}
    <h2 id="unlock-title">{tr("Enter your recovery key")}</h2>
    <p>{tr("The key (or passphrase) you saved when you set up recovery in Element.")}</p>
    <form onsubmit={submitKey}>
      <!-- svelte-ignore a11y_autofocus -->
      <textarea
        bind:value={key}
        rows="2"
        autocomplete="off"
        autocapitalize="off"
        spellcheck="false"
        placeholder="EsT1 aBcD …"
        autofocus
      ></textarea>
      <div class="buttons">
        <button type="button" class="secondary" onclick={() => (mode = "choose")}>{tr("Back")}</button>
        <button class="primary" disabled={busy || !key.trim()}>{busy ? tr("Unlocking…") : tr("Unlock")}</button>
      </div>
    </form>
  {:else}
    <h2 id="unlock-title">{tr("Unlock your messages")}</h2>
    {#if v?.state === "cancelled"}
      <p class="error">{tr("Verification stopped: {reason}", { reason: v.reason })}</p>
    {/if}
    <p>{tr("This device can't read older encrypted messages yet. Choose how to unlock it:")}</p>
    <div class="options">
      <button
        class="option"
        disabled={busy || !status?.can_verify_with_device}
        onclick={() => run(verifyThisDevice)}
      >
        <span class="icon">📱</span>
        <span><strong>{tr("Use another device")}</strong><small>{tr("Compare emoji with Element")}</small></span>
      </button>
      {#if status?.recovery !== "disabled"}
        <button class="option" disabled={busy} onclick={() => (mode = "key")}>
          <span class="icon">🔑</span>
          <span><strong>{tr("Enter recovery key")}</strong><small>{tr("The key you saved when setting up recovery")}</small></span>
        </button>
      {/if}
    </div>
  {/if}

  {#if error}<p class="error">{error}</p>{/if}
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 50;
    background: rgb(0 0 0 / 0.55);
  }
  .sheet {
    position: fixed;
    left: 50%;
    top: 50%;
    z-index: 51;
    width: min(440px, calc(100vw - 32px));
    padding: 24px;
    border: 1px solid var(--border);
    border-radius: 22px;
    background: var(--surface);
    box-shadow: 0 24px 64px rgb(0 0 0 / 0.5);
    translate: -50% -50%;
  }
  h2 {
    margin: 0 0 8px;
    font-size: 1.3rem;
  }
  p {
    margin: 0 0 16px;
    color: var(--muted);
    line-height: 1.4;
  }
  p strong {
    color: var(--text);
  }
  .close {
    position: absolute;
    top: 12px;
    right: 14px;
    border: none;
    background: none;
    font-size: 1.5rem;
    color: var(--muted);
    cursor: pointer;
  }
  .options {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .option {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 14px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--surface-2);
    text-align: left;
    cursor: pointer;
    transition: background 0.15s var(--ease);
  }
  .option:hover:not(:disabled) {
    background: var(--surface-hover);
  }
  .option:disabled {
    opacity: 0.45;
    cursor: default;
  }
  .option span:last-child {
    display: flex;
    flex-direction: column;
  }
  .option small {
    color: var(--muted);
  }
  .icon {
    font-size: 1.6rem;
  }
  textarea {
    width: 100%;
    padding: 12px 14px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--surface-2);
    color: inherit;
    font: 0.95rem ui-monospace, monospace;
    resize: none;
    outline: none;
  }
  textarea:focus {
    border-color: var(--accent-a);
  }
  .emojis {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 12px 6px;
    margin-bottom: 20px;
  }
  .emoji {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
  }
  .emoji span {
    font-size: 2.2rem;
  }
  .emoji small {
    font-size: 0.75rem;
    color: var(--muted);
    text-align: center;
  }
  .buttons {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 16px;
  }
  .buttons button {
    padding: 10px 18px;
    border: none;
    border-radius: 999px;
    cursor: pointer;
    transition: transform 0.12s var(--ease), opacity 0.12s var(--ease);
  }
  .buttons button:active {
    transform: scale(0.96);
  }
  .buttons button:disabled {
    opacity: 0.5;
  }
  .primary {
    background: var(--accent);
    color: white;
  }
  .secondary {
    background: var(--surface-2);
  }
  .spinner {
    width: 32px;
    height: 32px;
    margin: 8px auto 4px;
    border-radius: 50%;
    border: 3px solid var(--border);
    border-top-color: var(--accent-b);
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .error {
    margin: 12px 0 0;
    color: var(--danger);
  }
</style>
