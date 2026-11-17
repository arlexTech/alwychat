<script lang="ts">
  import { tr } from "$lib/i18n/index.svelte";
  import ExternalLink from "@lucide/svelte/icons/external-link";
  import KeyRound from "@lucide/svelte/icons/key-round";
  import ShieldCheck from "@lucide/svelte/icons/shield-check";
  import {
    accountUrl,
    openExternal,
    securityStatus,
    type SecurityStatus,
  } from "$lib/backend";
  import { encryption } from "$lib/stores/encryption.svelte";
  import Page from "./Page.svelte";

  /** Encryption: this device's verification, key backup, the recovery key. */
  let { onClose }: { onClose: () => void } = $props();

  let status = $state<SecurityStatus | null>(null);
  let error = $state<string | null>(null);

  const load = () => securityStatus().then((s) => (status = s)).catch((e) => (error = String(e)));
  // Back from the recovery key screen: show the new state.
  $effect(() => void (!encryption.recoveryOpen && load()));

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
  <p class="m-hint">
    {recoveryOn ? tr("Recovery is set up. If you lost your recovery key, make a new one (this device must be verified).") : tr("Recovery isn't set up: without it, losing your devices means losing your encrypted history.")}
  </p>
  <button class="m-btn wide" disabled={!status?.cross_signed} onclick={() => (encryption.recoveryOpen = true)}>
    {recoveryOn ? tr("Make a new recovery key") : tr("Set up recovery")}
  </button>

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
</style>
