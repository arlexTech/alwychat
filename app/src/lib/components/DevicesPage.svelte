<script lang="ts">
  import ExternalLink from "@lucide/svelte/icons/external-link";
  import Laptop from "@lucide/svelte/icons/laptop";
  import Pencil from "@lucide/svelte/icons/pencil";
  import ShieldAlert from "@lucide/svelte/icons/shield-alert";
  import ShieldCheck from "@lucide/svelte/icons/shield-check";
  import { accountUrl, devices as loadDevices, openExternal, renameDevice, type DeviceInfo } from "$lib/backend";
  import Page from "./Page.svelte";

  /** Where we're signed in. Signing a session out happens on the account page (OAuth). */
  let { onClose }: { onClose: () => void } = $props();

  let list = $state<DeviceInfo[]>([]);
  let loaded = $state(false);
  let renaming = $state<{ id: string; name: string } | null>(null);
  let error = $state<string | null>(null);

  function load() {
    loadDevices()
      .then((d) => ((list = d), (loaded = true)))
      .catch((e) => ((error = String(e)), (loaded = true)));
  }
  $effect(load);
  // Back from the browser: the list may have changed.
  $effect(() => {
    const onVisible = () => document.visibilityState === "visible" && load();
    document.addEventListener("visibilitychange", onVisible);
    return () => document.removeEventListener("visibilitychange", onVisible);
  });

  const seen = (d: DeviceInfo) => {
    if (d.is_current) return "This device";
    if (!d.last_seen_ts) return "Never seen";
    const days = Math.floor((Date.now() - d.last_seen_ts) / 86_400_000);
    const when = days < 1 ? "today" : days === 1 ? "yesterday" : days < 30 ? `${days} days ago` : new Date(d.last_seen_ts).toLocaleDateString();
    return `Active ${when}${d.last_seen_ip ? ` · ${d.last_seen_ip}` : ""}`;
  };

  async function open(action: string) {
    error = null;
    try {
      await openExternal(await accountUrl(action));
    } catch (e) {
      error = String(e);
    }
  }

  async function saveName() {
    const r = renaming!;
    renaming = null;
    try {
      await renameDevice(r.id, r.name);
      load();
    } catch (e) {
      error = String(e);
    }
  }

  const unverified = $derived(list.filter((d) => d.verified === false && !d.is_current).length);
</script>

<Page title="Sessions" {onClose}>
  {#if unverified}
    <p class="m-hint warn"><ShieldAlert size={14} /> {unverified} {unverified === 1 ? "session isn't" : "sessions aren't"} verified. If you don't recognise one, sign it out.</p>
  {/if}
  {#each list as d (d.device_id)}
    <div class="device">
      <div class="m-row">
        <span class="m-icon"><Laptop size={20} /></span>
        <span class="m-text">
          <strong>{d.name ?? d.device_id}</strong>
          <small>{seen(d)} · {d.device_id}</small>
        </span>
        {#if d.verified === true}<span class="ok" title="Verified"><ShieldCheck size={18} /></span>
        {:else if d.verified === false}<span class="bad" title="Not verified"><ShieldAlert size={18} /></span>{/if}
      </div>
      {#if renaming?.id === d.device_id}
        <form class="rename" onsubmit={(e) => (e.preventDefault(), saveName())}>
          <input class="m-input" bind:value={renaming.name} maxlength="60" />
          <button class="m-btn">Save</button>
        </form>
      {:else}
        <div class="actions">
          <button onclick={() => (renaming = { id: d.device_id, name: d.name ?? "" })}><Pencil size={14} /> Rename</button>
          {#if !d.is_current}<button class="danger" onclick={() => open(`sign_out:${d.device_id}`)}>Sign out</button>{/if}
        </div>
      {/if}
    </div>
  {/each}
  {#if !loaded}<p class="m-hint">Loading…</p>{/if}
  <button class="m-row" onclick={() => open("devices")}>
    <span class="m-icon"><ExternalLink size={20} /></span>
    <span class="m-text"><strong>Manage on your account page</strong><small>Signing out other sessions opens your server's account page</small></span>
  </button>
  {#if error}<p class="m-error">{error}</p>{/if}
</Page>

<style>
  .device {
    padding: 4px 0 8px;
    border-bottom: 1px solid var(--border);
  }
  .ok {
    color: #31d158;
  }
  .bad {
    color: #ffb020;
  }
  .warn {
    display: flex;
    align-items: center;
    gap: 6px;
    color: #ffb020;
  }
  .actions {
    display: flex;
    gap: 8px;
    padding-left: 42px;
  }
  .actions button {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 5px 12px;
    border: none;
    border-radius: 999px;
    background: var(--surface);
    color: var(--text);
    font: inherit;
    font-size: 0.85rem;
    cursor: pointer;
  }
  .actions .danger {
    color: var(--danger);
  }
  .rename {
    display: flex;
    gap: 8px;
    padding-left: 42px;
  }
</style>
