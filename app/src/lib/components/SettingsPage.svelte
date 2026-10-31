<script lang="ts">
  import { onMount } from "svelte";
  import { profiles, ourZone } from "$lib/stores/profiles.svelte";
  import { isAndroid, setStatus, setTimeZone, setupPush, systemStatus, type AppTheme, type Settings, type SystemStatus } from "$lib/backend";
  import { prefs } from "$lib/stores/prefs.svelte";
  import { spaces } from "$lib/stores/spaces.svelte";
  import { presence } from "$lib/stores/presence.svelte";
  import Page from "./Page.svelte";
  import FolderPage from "./FolderPage.svelte";
  import ChevronUp from "@lucide/svelte/icons/chevron-up";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import Plus from "@lucide/svelte/icons/plus";
  import type { Folder } from "$lib/backend";
  import { onBack } from "$lib/back.svelte";

  let { userId, onClose }: { userId: string; onClose: () => void } = $props();

  // ── Status and time zone (extended profile, visible to everyone) ──
  const STATUS_EMOJI = ["💬", "📚", "💼", "🎧", "🎮", "🏃", "✈️", "🤒", "😴", "🌙"];
  let statusEmoji = $state("");
  let statusText = $state("");
  let statusSaved = $state<string | null>(null);
  let statusBusy = $state(false);
  let shareZone = $state(false);
  $effect(() => {
    void profiles.refresh([userId], true).then(() => {
      const mine = profiles.get(userId);
      statusEmoji = mine?.status?.emoji ?? "";
      statusText = mine?.status?.text ?? "";
      statusSaved = profiles.statusLabel(userId);
      shareZone = !!mine?.time_zone;
    });
  });
  const statusChanged = $derived(([statusEmoji, statusText.trim()].filter(Boolean).join(" ") || null) !== statusSaved);

  async function saveStatus(clear = false) {
    statusBusy = true;
    try {
      if (clear) {
        statusEmoji = "";
        statusText = "";
      }
      const empty = !statusEmoji && !statusText.trim();
      await setStatus(empty ? null : { emoji: statusEmoji, text: statusText.trim() });
      await profiles.refresh([userId], true);
      statusSaved = profiles.statusLabel(userId);
    } catch (e) {
      error = String(e);
    } finally {
      statusBusy = false;
    }
  }

  async function toggleZone(on: boolean) {
    try {
      await setTimeZone(on ? ourZone() : null);
      shareZone = on;
    } catch (e) {
      shareZone = !on;
      error = String(e);
    }
  }

  const settings = $derived(prefs.settings);
  let system = $state<SystemStatus | null>(null);
  let error = $state<string | null>(null);
  let folderEdit = $state<Folder | "new" | null>(null);
  $effect(() => {
    if (folderEdit) return onBack(() => (folderEdit = null));
  });
  const move = (id: string, by: -1 | 1) => spaces.moveFolder(id, by).catch((e) => (error = String(e)));

  type BoolKey = { [K in keyof Settings]: Settings[K] extends boolean ? K : never }[keyof Settings];
  type Toggle = { key: BoolKey; label: string; hint: string };

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

  const THEMES: { id: AppTheme; label: string }[] = [
    { id: "system", label: "System" },
    { id: "light", label: "Light" },
    { id: "dark", label: "Dark" },
    { id: "amoled", label: "Black" },
  ];

  const appearance: Toggle[] = [
    {
      key: "chat_effects",
      label: "Message effects",
      hint: "Confetti, fireworks, hearts, snow… when a message with 🎉 🎆 💝 ❄️ arrives (works with Element).",
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

  const oemName = (oem: string) =>
    ({ oneplus: "OnePlus", oppo: "OPPO", realme: "realme", xiaomi: "Xiaomi", redmi: "Xiaomi", poco: "Xiaomi", huawei: "Huawei", honor: "Honor", vivo: "vivo", samsung: "Samsung" })[oem] ?? oem;

  /** Where the maker hides its own background setting, from the app's system page. */
  function oemHint(oem: string) {
    if (["oneplus", "oppo", "realme"].includes(oem))
      return "Your phone has its own limit on top of Android's: open Battery usage and choose “Allow background activity”.";
    if (["xiaomi", "redmi", "poco"].includes(oem))
      return "Your phone has its own limit: turn on Autostart, and set Battery saver to “No restrictions”.";
    if (oem === "samsung") return "Your phone has its own limit: under Battery, choose “Unrestricted”.";
    return "Your phone has its own limit on top of Android's: in Battery, allow AlwyChat to run in the background.";
  }

  async function refreshSystem() {
    try {
      system = await systemStatus();
    } catch (e) {
      error = String(e);
    }
  }

  onMount(() => {
    if (!prefs.settings) prefs.load().catch((e) => (error = String(e)));
    void refreshSystem();
    // Coming back from Android's settings pages: show what the user changed there.
    const onVisible = () => document.visibilityState === "visible" && refreshSystem();
    document.addEventListener("visibilitychange", onVisible);
    return () => document.removeEventListener("visibilitychange", onVisible);
  });

  async function save(change: Partial<Settings>) {
    try {
      await prefs.save(change);
    } catch (e) {
      error = String(e);
    }
  }

  async function toggle(key: BoolKey) {
    if (!settings) return;
    await save({ [key]: !settings[key] });
    if (key === "share_presence") presence.reportNow();
  }

  async function toggleNotifications() {
    const on = !notificationsOn;
    await save({ notifications: on });
    if (on && !system?.notifications) await fix("notifications");
    if (isAndroid) setupPush(on).catch((e) => (error = `Push: ${e}`));
  }

  async function fix(what: "notifications" | "battery" | "backgroundData" | "appSettings") {
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
    <h3>Status</h3>
    <div class="status-emoji" role="radiogroup" aria-label="Status emoji">
      {#each STATUS_EMOJI as e (e)}
        <button
          role="radio"
          aria-checked={statusEmoji === e}
          class:on={statusEmoji === e}
          onclick={() => (statusEmoji = statusEmoji === e ? "" : e)}>{e}</button
        >
      {/each}
    </div>
    <div class="status-row">
      <input
        class="status-text"
        placeholder="What's up?"
        maxlength="80"
        bind:value={statusText}
        onkeydown={(e) => e.key === "Enter" && statusChanged && saveStatus()}
      />
      {#if statusChanged}
        <button class="status-save" disabled={statusBusy} onclick={() => saveStatus()}>Save</button>
      {:else if statusSaved}
        <button class="status-clear" disabled={statusBusy} onclick={() => saveStatus(true)}>Clear</button>
      {/if}
    </div>
    <small class="status-hint">Anyone who can see your profile sees it, in AlwyChat and other apps that support statuses.</small>
    <label class="toggle">
      <span>
        <strong>Share my local time</strong>
        <small>People see your time zone ({ourZone()}) and the time it is for you.</small>
      </span>
      <input type="checkbox" role="switch" checked={shareZone} onchange={(e) => toggleZone(e.currentTarget.checked)} />
    </label>

    <h3 class="section">Privacy</h3>
    {#each privacy as t (t.key)}{@render row(t)}{/each}

    <h3 class="section">Appearance</h3>
    <div class="themes" role="radiogroup" aria-label="Theme">
      {#each THEMES as t (t.id)}
        <button role="radio" aria-checked={settings.app_theme === t.id} class:on={settings.app_theme === t.id} onclick={() => save({ app_theme: t.id })}>
          <span class="swatch {t.id}"></span>{t.label}
        </button>
      {/each}
    </div>
    {#each appearance as t (t.key)}{@render row(t)}{/each}

    <h3 class="section">Chats</h3>
    <label class="toggle">
      <span>
        <strong>Show chats from spaces in Chats</strong>
        <small>Off: they're only under Spaces. Each space and chat can choose for itself in its settings. Synced to your other devices.</small>
      </span>
      <input
        type="checkbox"
        role="switch"
        checked={spaces.prefs.show_space_chats}
        onchange={(e) => spaces.setGlobal(e.currentTarget.checked).catch((err) => (error = String(err)))}
      />
    </label>

    <h3 class="section">Folders</h3>
    <ul class="folders">
      {#each spaces.prefs.folders as f, i (f.id)}
        <li>
          <button class="folder-name" onclick={() => (folderEdit = f)}>{f.name}</button>
          <button class="icon" disabled={i === 0} onclick={() => move(f.id, -1)} aria-label="Move up"><ChevronUp size={18} /></button>
          <button class="icon" disabled={i === spaces.prefs.folders.length - 1} onclick={() => move(f.id, 1)} aria-label="Move down"><ChevronDown size={18} /></button>
        </li>
      {/each}
      <li><button class="folder-name new" onclick={() => (folderEdit = "new")}><Plus size={18} />New folder</button></li>
    </ul>

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
      {#if system.oemPowerManager}
        <div class="check">
          <span class="mark">?</span>
          <span>
            <strong>{oemName(system.oemPowerManager)} battery manager</strong>
            <small>{oemHint(system.oemPowerManager)} AlwyChat can't check this one itself.</small>
          </span>
          <button onclick={() => fix("appSettings")}>Open</button>
        </div>
      {/if}
    {/if}

    <h3 class="section">Media</h3>
    {#each media as t (t.key)}{@render row(t)}{/each}
  {:else if !error}
    <p class="muted">Loading…</p>
  {/if}
  {#if error}<p class="error">{error}</p>{/if}
</Page>
{#if folderEdit}
  <FolderPage folder={folderEdit === "new" ? null : folderEdit} onClose={() => (folderEdit = null)} />
{/if}

<style>
  .themes {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 8px;
    margin: 4px 0 8px;
  }
  .themes button {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    padding: 12px 4px 10px;
    border: 2px solid transparent;
    border-radius: 14px;
    background: var(--surface-2);
    cursor: pointer;
    font-size: 13px;
  }
  .themes button.on {
    border-color: var(--accent-a);
  }
  .swatch {
    width: 36px;
    height: 36px;
    border-radius: 50%;
    border: 1px solid var(--border);
  }
  .swatch.system {
    background: linear-gradient(135deg, #ffffff 50%, #0e1014 50%);
  }
  .swatch.light {
    background: #ffffff;
  }
  .swatch.dark {
    background: #0e1014;
  }
  .swatch.amoled {
    background: #000000;
  }
  h3 {
    margin: 8px 0 8px;
    font-size: 0.8rem;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--muted);
  }
  .status-emoji {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-bottom: 10px;
  }
  .status-emoji button {
    width: 40px;
    height: 40px;
    border: 2px solid transparent;
    border-radius: 12px;
    background: var(--surface);
    font-size: 1.25rem;
    cursor: pointer;
  }
  .status-emoji button.on {
    border-color: var(--accent-b, #7b5cff);
  }
  .status-row {
    display: flex;
    gap: 8px;
  }
  .status-text {
    flex: 1;
    min-width: 0;
    padding: 10px 14px;
    border: 1px solid var(--border);
    border-radius: 12px;
    background: var(--surface);
    color: inherit;
    font: inherit;
  }
  .status-row button {
    padding: 0 16px;
    border: none;
    border-radius: 12px;
    font: inherit;
    cursor: pointer;
  }
  .status-save {
    background: var(--accent);
    color: white;
  }
  .status-clear {
    background: var(--surface);
    color: var(--danger);
  }
  .status-hint {
    display: block;
    margin: 6px 0 4px;
  }
  .folders {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .folders li {
    display: flex;
    align-items: center;
    gap: 4px;
    border-top: 1px solid var(--border);
  }
  .folder-name {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 12px 0;
    border: none;
    background: none;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .folder-name.new {
    color: var(--muted);
  }
  .folders .icon {
    display: grid;
    place-items: center;
    width: 34px;
    height: 34px;
    border: none;
    border-radius: 8px;
    background: none;
    color: inherit;
    cursor: pointer;
  }
  .folders .icon:disabled {
    opacity: 0.3;
    cursor: default;
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
