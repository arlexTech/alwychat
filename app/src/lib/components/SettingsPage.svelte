<script lang="ts">
  import { onMount } from "svelte";
  import { getSettings, isAndroid, setSettings, systemStatus, type Settings, type SystemStatus } from "$lib/backend";
  import { presence } from "$lib/stores/presence.svelte";
  import Page from "./Page.svelte";

  let { onClose }: { onClose: () => void } = $props();

  let settings = $state<Settings | null>(null);
  let system = $state<SystemStatus | null>(null);
  let error = $state<string | null>(null);

  type Toggle = { key: keyof Settings; label: string; hint: string };

  const privacy: Toggle[] = [
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
  ];

  const media: Toggle[] = [
    {
      key: "hd_media",
      label: "Send photos in HD",
      hint: "Original quality by default. Off: photos are compressed (faster, smaller). Each chat can override this.",
    },
  ];

  /** On only when both AlwyChat and the system allow it, so the switch matches the phone's settings. */
  const notificationsOn = $derived(!!settings?.notifications && !!system?.notifications);

  async function refreshSystem() {
    try {
      system = await systemStatus();
    } catch (e) {
      error = String(e);
    }
  }

  onMount(() => {
    getSettings().then((s) => (settings = s), (e) => (error = String(e)));
    void refreshSystem();
    // Coming back from Android's settings pages: show what the user changed there.
    const onVisible = () => document.visibilityState === "visible" && refreshSystem();
    document.addEventListener("visibilitychange", onVisible);
    return () => document.removeEventListener("visibilitychange", onVisible);
  });

  async function save(change: Partial<Settings>) {
    if (!settings) return;
    const previous = settings;
    settings = { ...settings, ...change };
    try {
      await setSettings(settings);
    } catch (e) {
      settings = previous;
      error = String(e);
    }
  }

  async function toggle(key: keyof Settings) {
    if (!settings) return;
    await save({ [key]: !settings[key] });
    if (key === "share_presence") presence.reportNow();
  }

  async function toggleNotifications() {
    if (notificationsOn) return save({ notifications: false });
    await save({ notifications: true });
    if (!system?.notifications) await fix("notifications");
  }

  async function fix(what: "notifications" | "battery" | "backgroundData") {
    error = null;
    try {
      system = await systemStatus(what);
    } catch (e) {
      error = String(e);
    }
  }
</script>

{#snippet row(t: Toggle)}
  <label class="toggle">
    <span>
      <strong>{t.label}</strong>
      <small>{t.hint}</small>
    </span>
    <input type="checkbox" role="switch" checked={settings?.[t.key]} onchange={() => toggle(t.key)} />
  </label>
{/snippet}

<Page title="Settings" {onClose}>
  {#if settings}
    <h3>Privacy</h3>
    {#each privacy as t (t.key)}{@render row(t)}{/each}

    <h3 class="section">Notifications</h3>
    <label class="toggle">
      <span>
        <strong>New messages</strong>
        <small>
          {isAndroid ? "While AlwyChat isn't on screen." : "While AlwyChat isn't in front."}
          Follows your mute and mention settings.
          {#if settings.notifications && system && !system.notifications}
            <em>Turned off in {isAndroid ? "Android" : "system"} settings.</em>
          {/if}
        </small>
      </span>
      <input type="checkbox" role="switch" checked={notificationsOn} onchange={toggleNotifications} />
    </label>
    {#if isAndroid && notificationsOn && system}
      <div class="check">
        <span class="mark" class:ok={system.batteryUnrestricted}>{system.batteryUnrestricted ? "✓" : "!"}</span>
        <span>
          <strong>Battery</strong>
          <small>
            {system.batteryUnrestricted
              ? "AlwyChat can keep running in the background."
              : "Battery optimisation pauses AlwyChat in the background: notifications come late or not at all."}
          </small>
        </span>
        {#if !system.batteryUnrestricted}<button onclick={() => fix("battery")}>Allow</button>{/if}
      </div>
      <div class="check">
        <span class="mark" class:ok={system.backgroundData}>{system.backgroundData ? "✓" : "!"}</span>
        <span>
          <strong>Background data</strong>
          <small>
            {system.backgroundData
              ? "AlwyChat can receive messages in the background."
              : "Data saver blocks AlwyChat in the background, so new messages can't arrive."}
          </small>
        </span>
        {#if !system.backgroundData}<button onclick={() => fix("backgroundData")}>Allow</button>{/if}
      </div>
    {/if}

    <h3 class="section">Media</h3>
    {#each media as t (t.key)}{@render row(t)}{/each}
  {:else if !error}
    <p class="muted">Loading…</p>
  {/if}
  {#if error}<p class="error">{error}</p>{/if}
</Page>

<style>
  h3 {
    margin: 8px 0 8px;
    font-size: 0.8rem;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--muted);
  }
  h3.section {
    margin-top: 26px;
  }
  .toggle,
  .check {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 12px 0;
    border-top: 1px solid var(--border);
  }
  .toggle {
    cursor: pointer;
  }
  .toggle > span,
  .check > span:not(.mark) {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  small {
    color: var(--muted);
    line-height: 1.35;
  }
  em {
    color: var(--danger);
    font-style: normal;
  }
  .mark {
    flex: none;
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    border-radius: 50%;
    background: #e5a00d;
    color: black;
    font-weight: 700;
  }
  .mark.ok {
    background: #31d158;
  }
  .check button {
    flex: none;
    padding: 8px 16px;
    border: none;
    border-radius: 999px;
    background: var(--accent);
    color: white;
    cursor: pointer;
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
</style>
