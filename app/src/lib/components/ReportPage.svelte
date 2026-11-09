<script lang="ts">
  // "Report a problem": what happened + the app's recent technical log, sent to a chat the
  // user picks (usually the person who helps them with AlwyChat). Nothing goes anywhere else.
  import { tr } from "$lib/i18n/index.svelte";
  import Bug from "@lucide/svelte/icons/bug";
  import { appVersion, isAndroid, isWeb, sendReport, type RoomSummary } from "$lib/backend";
  import Page from "./Page.svelte";
  import ForwardPage from "./ForwardPage.svelte";

  let {
    crashed = false,
    onClose,
  }: {
    /** Opened after a crash: the crash is attached with the log. */
    crashed?: boolean;
    onClose: () => void;
  } = $props();

  let text = $state("");
  let includeLog = $state(true);
  let picking = $state(false);
  let sending = $state(false);
  let sent = $state<string | null>(null);
  let error = $state<string | null>(null);

  function system() {
    if (isAndroid) return "Android";
    if (isWeb) return `Web (${navigator.userAgent})`;
    if (/Windows/.test(navigator.userAgent)) return "Windows";
    if (/Mac/.test(navigator.userAgent)) return "macOS";
    return "Linux";
  }

  async function send(room: RoomSummary) {
    picking = false;
    sending = true;
    error = null;
    try {
      const version = await appVersion().catch(() => "?");
      const what = text.trim() || (crashed ? tr("AlwyChat closed unexpectedly.") : "");
      const message = `🐞 ${tr("Problem report")}\n${what}\n\nAlwyChat ${version} · ${system()}`;
      await sendReport(room.id, message, includeLog);
      sent = room.name;
    } catch (e) {
      error = String(e);
    } finally {
      sending = false;
    }
  }
</script>

<Page title={tr("Report a problem")} {onClose}>
  {#if sent}
    <div class="done">
      <Bug size={36} />
      <p>{tr("Report sent to {name}. Thank you!", { name: sent })}</p>
      <button class="m-btn" onclick={onClose}>{tr("Done")}</button>
    </div>
  {:else}
    {#if crashed}<p class="m-hint">{tr("AlwyChat closed unexpectedly last time. Sending a report helps fix it.")}</p>{/if}
    <textarea class="m-input" rows="6" bind:value={text} placeholder={tr("What happened? What were you doing just before?")}></textarea>
    <label class="m-row">
      <span class="m-text">
        <strong>{tr("Include the technical log")}</strong>
        <small>{tr("What the app did recently: errors, chat and device IDs, no message text. Sent as a file.")}</small>
      </span>
      <input type="checkbox" role="switch" bind:checked={includeLog} />
    </label>
    <button class="m-btn" disabled={sending || (!text.trim() && !crashed && !includeLog)} onclick={() => (picking = true)}>
      {sending ? tr("Sending…") : tr("Choose who to send it to")}
    </button>
    <p class="m-hint">{tr("The report is sent as a message in the chat you pick, for example to the person who helps you with AlwyChat.")}</p>
    {#if error}<p class="m-error">{error}</p>{/if}
  {/if}
</Page>

{#if picking}
  <ForwardPage title={tr("Send the report to…")} onPick={send} onClose={() => (picking = false)} />
{/if}

<style>
  textarea {
    width: 100%;
    margin-bottom: 12px;
    resize: vertical;
  }
  .m-btn {
    width: 100%;
    margin-top: 12px;
  }
  .done {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
    padding: 32px 0;
    color: var(--muted);
    text-align: center;
  }
  .done p {
    color: var(--text);
  }
</style>
