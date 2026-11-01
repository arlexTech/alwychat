<script lang="ts">
  import Copy from "@lucide/svelte/icons/copy";
  import ExternalLink from "@lucide/svelte/icons/external-link";
  import KeyRound from "@lucide/svelte/icons/key-round";
  import ShieldCheck from "@lucide/svelte/icons/shield-check";
  import {
    accountUrl,
    newRecoveryKey,
    openExternal,
    securityStatus,
    type SecurityStatus,
  } from "$lib/backend";
  import { encryption } from "$lib/stores/encryption.svelte";
  import Page from "./Page.svelte";

  /** Encryption: this device's verification, key backup, the recovery key. */
  let { onClose }: { onClose: () => void } = $props();

  let status = $state<SecurityStatus | null>(null);
  let key = $state<string | null>(null);
  let confirm = $state(false);
  let busy = $state(false);
  let error = $state<string | null>(null);
  let copied = $state(false);

  const load = () => securityStatus().then((s) => (status = s)).catch((e) => (error = String(e)));
  $effect(() => void load());

  async function makeKey() {
    confirm = false;
    busy = true;
    error = null;
    try {
      key = await newRecoveryKey();
      void load();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function resetIdentity() {
    try {
      await openExternal(await accountUrl("reset_identity"));
    } catch (e) {
      error = String(e);
    }
  }

  const recoveryOn = $derived(status?.recovery === "enabled");
</script>

<Page title="Security & recovery" {onClose}>
  <h3 class="m-section">This device</h3>
  <div class="m-row">
    <span class="m-icon" class:ok={status?.cross_signed}><ShieldCheck size={20} /></span>
    <span class="m-text">
      <strong>{status?.cross_signed ? "Verified" : "Not verified"}</strong>
      <small>{status?.cross_signed ? "Your other sessions trust this one: it can read your encrypted history." : "Verify it with another session or your recovery key to read encrypted history."}</small>
    </span>
  </div>
  {#if status && !status.cross_signed}
    <button class="m-btn wide" onclick={() => ((encryption.sheetOpen = true), onClose())}>Verify this device</button>
  {/if}

  <h3 class="m-section">Key backup</h3>
  <div class="m-row">
    <span class="m-icon"><KeyRound size={20} /></span>
    <span class="m-text">
      <strong>{status?.backup === "enabled" ? "On" : status?.backup ?? "…"}</strong>
      <small>Your message keys are stored on the server, encrypted with your recovery key, so a new device can read old messages.</small>
    </span>
  </div>

  <h3 class="m-section">Recovery key</h3>
  {#if key}
    <p class="m-hint">Your new recovery key. Save it somewhere safe (a password manager): it's the only way back into your encrypted history if you lose all your devices. The old key no longer works.</p>
    <div class="key">{key}</div>
    <button class="m-btn plain wide" onclick={() => navigator.clipboard?.writeText(key!).then(() => (copied = true))}><Copy size={16} /> {copied ? "Copied" : "Copy"}</button>
  {:else}
    <p class="m-hint">
      {recoveryOn ? "Recovery is set up. If you lost your recovery key, make a new one (this device must be verified)." : "Recovery isn't set up: without it, losing your devices means losing your encrypted history."}
    </p>
    {#if confirm}
      <p class="m-hint">{recoveryOn ? "The old key will stop working." : ""} Continue?</p>
      <div class="m-buttons">
        <button class="m-btn plain" onclick={() => (confirm = false)}>Cancel</button>
        <button class="m-btn" disabled={busy} onclick={makeKey}>{busy ? "Working…" : "Continue"}</button>
      </div>
    {:else}
      <button class="m-btn wide" disabled={busy || !status?.cross_signed} onclick={() => (confirm = true)}>
        {recoveryOn ? "Make a new recovery key" : "Set up recovery"}
      </button>
    {/if}
  {/if}

  <h3 class="m-section">Lost everything?</h3>
  <button class="m-row danger" onclick={resetIdentity}>
    <span class="m-icon"><ExternalLink size={20} /></span>
    <span class="m-text"><strong>Reset your identity</strong><small>Last resort, on your account page: you lose your encrypted history and must verify again.</small></span>
  </button>
  {#if error}<p class="m-error">{error}</p>{/if}
</Page>

<style>
  .ok {
    color: #31d158 !important;
  }
  .wide {
    width: 100%;
    margin-top: 6px;
  }
  .key {
    margin: 10px 0;
    padding: 14px;
    border-radius: 12px;
    background: var(--surface);
    font-family: ui-monospace, monospace;
    font-size: 1.05rem;
    line-height: 1.6;
    text-align: center;
    word-spacing: 4px;
    user-select: all;
  }
</style>
