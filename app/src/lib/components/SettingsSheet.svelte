<script lang="ts">
  import { onMount } from "svelte";
  import { fade, fly } from "svelte/transition";
  import { getSettings, setSettings, type Settings } from "$lib/backend";
  import { presence } from "$lib/stores/presence.svelte";

  let { onClose }: { onClose: () => void } = $props();

  let settings = $state<Settings | null>(null);
  let error = $state<string | null>(null);

  const media: { key: keyof Settings; label: string; hint: string }[] = [
    {
      key: "hd_media",
      label: "Send photos in HD",
      hint: "Original quality by default. Off: photos are compressed (faster, smaller). Each chat can override this.",
    },
  ];

  const toggles: { key: keyof Settings; label: string; hint: string }[] = [
    {
      key: "send_read_receipts",
      label: "Send read receipts",
      hint: "Others see when you've read their messages. Off: your unread counts still sync between your devices.",
    },
    { key: "send_typing", label: "Show when I'm typing", hint: "Others see the typing dots while you write." },
    {
      key: "share_presence",
      label: "Show when I'm active",
      hint: "Others see when you're online or when you were last active.",
    },
    {
      key: "notifications",
      label: "Desktop notifications",
      hint: "New messages while AlwyChat isn't in front. Follows your mute and mention settings.",
    },
  ];

  onMount(async () => {
    try {
      settings = await getSettings();
    } catch (e) {
      error = String(e);
    }
  });

  async function toggle(key: keyof Settings) {
    if (!settings) return;
    const next = { ...settings, [key]: !settings[key] };
    const previous = settings;
    settings = next;
    try {
      await setSettings(next);
      if (key === "share_presence") presence.reportNow();
    } catch (e) {
      settings = previous;
      error = String(e);
    }
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="backdrop" transition:fade={{ duration: 150 }} onclick={onClose}></div>
<div class="sheet" role="dialog" aria-modal="true" aria-labelledby="settings-title" transition:fly={{ y: 24, duration: 220 }}>
  <button class="close" onclick={onClose} aria-label="Close">×</button>
  <h2 id="settings-title">Settings</h2>
  <h3>Privacy</h3>
  {#if settings}
    {#each toggles as t (t.key)}
      <label class="toggle">
        <span>
          <strong>{t.label}</strong>
          <small>{t.hint}</small>
        </span>
        <input type="checkbox" role="switch" checked={settings[t.key]} onchange={() => toggle(t.key)} />
      </label>
    {/each}
    <h3 class="section">Media</h3>
    {#each media as t (t.key)}
      <label class="toggle">
        <span>
          <strong>{t.label}</strong>
          <small>{t.hint}</small>
        </span>
        <input type="checkbox" role="switch" checked={settings[t.key]} onchange={() => toggle(t.key)} />
      </label>
    {/each}
  {:else if !error}
    <p class="muted">Loading…</p>
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
    width: min(460px, calc(100vw - 32px));
    padding: 24px;
    border: 1px solid var(--border);
    border-radius: 22px;
    background: var(--surface);
    box-shadow: 0 24px 64px rgb(0 0 0 / 0.5);
    translate: -50% -50%;
  }
  h2 {
    margin: 0 0 12px;
  }
  h3 {
    margin: 0 0 8px;
    font-size: 0.8rem;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--muted);
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
    line-height: 1.35;
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
  .muted {
    color: var(--muted);
  }
  .error {
    color: var(--danger);
  }
  h3.section {
    margin-top: 18px;
  }
</style>
