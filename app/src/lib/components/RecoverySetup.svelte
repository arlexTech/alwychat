<script lang="ts">
  import { tr } from "$lib/i18n/index.svelte";
  import { fade } from "svelte/transition";
  import { untrack } from "svelte";
  import { isWeb, newRecoveryKey } from "$lib/backend";
  import { encryption } from "$lib/stores/encryption.svelte";
  import Copy from "@lucide/svelte/icons/copy";
  import Check from "@lucide/svelte/icons/check";
  import Download from "@lucide/svelte/icons/download";
  import KeyRound from "@lucide/svelte/icons/key-round";
  import ShieldAlert from "@lucide/svelte/icons/shield-alert";
  import ShieldCheck from "@lucide/svelte/icons/shield-check";

  /** The recovery key, made and saved in three short steps, full screen. On a new account it
   * can't be skipped (`encryption.mustSaveRecovery`): people don't read warnings, so the key has
   * to be given back whole before the app opens. The clipboard is emptied first, so a paste has
   * to come from wherever it was saved. `replacing` = made from Settings on
   * an account that already had one (the old key stops working). */
  let { replacing: replacingProp = false }: { replacing?: boolean } = $props();
  // Decided when the screen opens (making the key changes the account's status under us).
  const replacing = untrack(() => replacingProp);

  type Step = "intro" | "key" | "check" | "done";
  let step = $state<Step>("intro");
  let key = $state<string | null>(null);
  let busy = $state(false);
  let error = $state<string | null>(null);
  let copied = $state(false);
  let saved = $state(false);
  let answer = $state("");
  let wrong = $state(false);

  const groups = $derived(key ? key.split(/\s+/) : []);
  /** Spaces and line breaks don't count; capitals do (base58: "a" and "A" are different). */
  const bare = (text: string) => text.replace(/\s+/g, "");
  const matches = $derived(!!key && bare(answer) === bare(key));

  async function make() {
    busy = true;
    error = null;
    try {
      // From here until the key is typed back, the screen comes back even after a restart.
      encryption.recoveryPending();
      key = await newRecoveryKey();
      step = "key";
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function copy() {
    try {
      await navigator.clipboard.writeText(key!);
      copied = saved = true;
    } catch {
      error = tr("Couldn't copy: select the key and copy it by hand.");
    }
  }

  function download() {
    const text = `${tr("AlwyChat recovery key")}\n\n${key}\n\n${tr("Keep this file private. It unlocks your encrypted messages.")}\n`;
    const a = document.createElement("a");
    a.href = URL.createObjectURL(new Blob([text], { type: "text/plain" }));
    a.download = "alwychat-recovery-key.txt";
    a.click();
    setTimeout(() => URL.revokeObjectURL(a.href), 10_000);
    saved = true;
  }

  /** To the check, emptying the clipboard on the way (from the click, which browsers require). */
  function toCheck() {
    if (copied) navigator.clipboard?.writeText("").catch(() => {});
    copied = false;
    answer = "";
    wrong = false;
    step = "check";
  }

  function check(event?: SubmitEvent) {
    event?.preventDefault();
    if (matches) {
      // Keep the screen up for the last step, then let the app open.
      encryption.recoveryOpen = true;
      encryption.recoverySaved();
      step = "done";
    } else {
      wrong = true;
    }
  }

  const close = () => (encryption.recoveryOpen = false);
</script>

<div class="screen" transition:fade={{ duration: 150 }} role="dialog" aria-modal="true" aria-labelledby="recovery-title">
  <div class="box">
    {#if step === "intro"}
      <span class="badge"><KeyRound size={30} /></span>
      <h2 id="recovery-title">{replacing ? tr("New recovery key") : tr("One last step: your recovery key")}</h2>
      <p class="lead">
        {tr("Your messages are encrypted. If you lose this device, this key is the only way to read them again.")}
      </p>
      <div class="warn">
        <ShieldAlert size={20} />
        <span>{tr("Nobody can give it back to you: not us, not the server.")}</span>
      </div>
      {#if replacing}<p class="small">{tr("Your current key will stop working.")}</p>{/if}
      <button class="m-btn wide" disabled={busy} onclick={make}>{busy ? tr("Working…") : tr("Create my recovery key")}</button>
      {#if replacing}<button class="m-btn plain wide" onclick={close}>{tr("Cancel")}</button>{/if}
    {:else if step === "key"}
      <h2 id="recovery-title">{tr("Save this key now")}</h2>
      <p class="lead">{tr("In a password manager, or as a file somewhere safe. You'll be asked for it next.")}</p>
      <div class="key">
        {#each groups as g, i (i)}<span>{g}</span>{/each}
      </div>
      <div class="m-buttons">
        <button class="m-btn plain" onclick={copy}>
          {#if copied}<Check size={16} /> {tr("Copied")}{:else}<Copy size={16} /> {tr("Copy")}{/if}
        </button>
        {#if isWeb}
          <button class="m-btn plain" onclick={download}><Download size={16} /> {tr("Save as file")}</button>
        {/if}
      </div>
      <p class="small">{tr("Don't keep it only in this browser or on this phone: if you lose it, you lose the key too.")}</p>
      <button class="m-btn wide" onclick={toCheck}>
        {saved ? tr("I've saved it") : tr("I've written it down")}
      </button>
    {:else if step === "check"}
      <h2 id="recovery-title">{tr("Check your key")}</h2>
      <p class="lead">{tr("Paste or type your recovery key from where you saved it.")}</p>
      <form onsubmit={check}>
        <!-- svelte-ignore a11y_autofocus -->
        <textarea
          class="answer"
          class:bad={wrong}
          class:good={matches}
          bind:value={answer}
          oninput={() => ((wrong = false), matches && check())}
          rows="3"
          autocomplete="off"
          autocapitalize="off"
          spellcheck="false"
          autofocus
          aria-label={tr("Recovery key")}
        ></textarea>
        {#if wrong}<p class="m-error">{tr("That's not it. Check it against the key you saved (capitals matter).")}</p>{/if}
        <button class="m-btn wide" disabled={bare(answer).length < 8}>{tr("Check")}</button>
      </form>
      <button class="link" onclick={() => (step = "key")}>{tr("Show the key again")}</button>
    {:else}
      <span class="badge ok"><ShieldCheck size={30} /></span>
      <h2 id="recovery-title">{tr("You're protected")}</h2>
      <p class="lead">{tr("If you ever lose your devices, sign in and enter this key to get your messages back.")}</p>
      <button class="m-btn wide" onclick={close}>{tr("Done")}</button>
    {/if}
    {#if error}<p class="m-error">{error}</p>{/if}
  </div>
</div>

<style>
  .screen {
    position: fixed;
    inset: 0;
    z-index: 60;
    display: grid;
    place-items: center;
    overflow-y: auto;
    padding: 24px 16px calc(24px + var(--safe-bottom));
    background:
      radial-gradient(50rem 24rem at 50% -6rem, rgb(123 92 255 / 0.2), transparent 70%),
      var(--bg);
  }
  .box {
    width: min(420px, 100%);
    display: flex;
    flex-direction: column;
    align-items: stretch;
    text-align: center;
  }
  .badge {
    align-self: center;
    display: grid;
    place-items: center;
    width: 64px;
    height: 64px;
    margin-bottom: 18px;
    border-radius: 50%;
    color: #fff;
    background: var(--accent);
    box-shadow: 0 10px 26px rgb(123 92 255 / 0.3);
  }
  .badge.ok {
    background: #2fb36b;
    box-shadow: none;
  }
  h2 {
    margin: 0 0 10px;
    font-size: 1.5rem;
    line-height: 1.2;
  }
  .lead {
    margin: 0 0 18px;
    color: var(--muted);
    line-height: 1.45;
  }
  .small {
    margin: 4px 0 16px;
    color: var(--muted);
    font-size: 0.88rem;
    line-height: 1.4;
  }
  .warn {
    display: flex;
    gap: 10px;
    align-items: center;
    margin-bottom: 20px;
    padding: 12px 14px;
    border-radius: 14px;
    text-align: left;
    font-weight: 600;
    color: var(--text);
    background: rgb(255 107 107 / 0.12);
  }
  .warn :global(svg) {
    flex: none;
    color: var(--danger);
  }
  .key {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 8px 6px;
    margin-bottom: 14px;
    padding: 16px 12px;
    border: 1px solid var(--border);
    border-radius: 16px;
    background: var(--surface);
    font: 600 1.1rem/1.4 ui-monospace, monospace;
    -webkit-user-select: all;
    user-select: all;
  }
  .m-buttons {
    margin-bottom: 4px;
  }
  .m-buttons :global(svg) {
    vertical-align: -3px;
  }
  .wide {
    width: 100%;
    margin-top: 8px;
  }
  .answer {
    display: block;
    width: 100%;
    box-sizing: border-box;
    margin: 0 0 10px;
    padding: 14px;
    border: 2px solid var(--border);
    border-radius: 14px;
    background: var(--surface);
    color: var(--text);
    font: 600 1.05rem/1.5 ui-monospace, monospace;
    text-align: center;
    resize: none;
  }
  .answer:focus {
    outline: none;
    border-color: var(--accent-a);
  }
  .answer.good {
    border-color: #2fb36b;
  }
  .answer.bad {
    border-color: var(--danger);
  }
  .link {
    margin-top: 14px;
    border: none;
    background: none;
    color: var(--muted);
    text-decoration: underline;
    cursor: pointer;
    font: inherit;
  }
  .m-error {
    margin: 8px 0 0;
  }
</style>
