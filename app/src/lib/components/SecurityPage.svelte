<script lang="ts">
  import { tr } from "$lib/i18n/index.svelte";
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

<Page title={tr("Security & recovery")} {onClose}>
  <h3 class="m-section">{tr("This device")}</h3>
  <div class="m-row">
    <span class="m-icon" class:ok={status?.cross_signed}><ShieldCheck size={20} /></span>
    <span class="m-text">
      <strong>{status?.cross_signed ? tr("Verified") : tr("Not verified")}</strong>
      <small>{status?.cross_signed ? tr("Your other sessions trust this one: it can read your encrypted history.") : tr("Verify it with another session or your recovery key to read encrypted history.")}</small>
    </span>
  </div>
  {#if status && !status.cross_signed}
    <button class="m-btn wide" onclick={() => ((encryption.sheetOpen = true), onClose())}>{tr("Verify this device")}</button>
  {/if}

  <h3 class="m-section">{tr("Key backup")}</h3>
  <div class="m-row">
    <span class="m-icon"><KeyRound size={20} /></span>
    <span class="m-text">
      <strong>{status?.backup === "enabled" ? tr("On") : status?.backup ?? "…"}</strong>
      <small>{tr("Your message keys are stored on the server, encrypted with your recovery key, so a new device can read old messages.")}</small>
    </span>
  </div>

  {#if encryption.backupMissing}
    <p class="m-hint">{tr("This device doesn't have the key to your backup yet, so some older messages may stay locked. Enter your recovery key to get it.")}</p>
    <button class="m-btn wide" onclick={() => ((encryption.sheetOpen = true), onClose())}>{tr("Enter recovery key")}</button>
  {/if}

  <h3 class="m-section">{tr("Recovery key")}</h3>
  {#if key}
    <p class="m-hint">{tr("Your new recovery key. Save it somewhere safe (a password manager): it's the only way back into your encrypted history if you lose all your devices. The old key no longer works.")}</p>
    <div class="key">{key}</div>
    <button class="m-btn plain wide" onclick={() => navigator.clipboard?.writeText(key!).then(() => (copied = true))}><Copy size={16} /> {copied ? tr("Copied") : tr("Copy")}</button>
  {:else}
    <p class="m-hint">
      {recoveryOn ? tr("Recovery is set up. If you lost your recovery key, make a new one (this device must be verified).") : tr("Recovery isn't set up: without it, losing your devices means losing your encrypted history.")}
    </p>
    {#if confirm}
      <p class="m-hint">{tr("{value} Continue?", { value: recoveryOn ? "The old key will stop working." : "" })}</p>
      <div class="m-buttons">
        <button class="m-btn plain" onclick={() => (confirm = false)}>{tr("Cancel")}</button>
        <button class="m-btn" disabled={busy} onclick={makeKey}>{busy ? tr("Working…") : tr("Continue")}</button>
      </div>
    {:else}
      <button class="m-btn wide" disabled={busy || !status?.cross_signed} onclick={() => (confirm = true)}>
        {recoveryOn ? tr("Make a new recovery key") : tr("Set up recovery")}
      </button>
    {/if}
  {/if}

  <h3 class="m-section">{tr("Lost everything?")}</h3>
  <button class="m-row danger" onclick={resetIdentity}>
    <span class="m-icon"><ExternalLink size={20} /></span>
    <span class="m-text"><strong>{tr("Reset your identity")}</strong><small>{tr("Last resort, on your account page: you lose your encrypted history and must verify again.")}</small></span>
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
