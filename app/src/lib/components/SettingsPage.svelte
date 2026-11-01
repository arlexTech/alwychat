<script lang="ts">
  import { onMount } from "svelte";
  import { profiles, ourZone } from "$lib/stores/profiles.svelte";
  import { isAndroid, setStatus, setTimeZone, setupPush, systemStatus, type AppTheme, type Settings, type SystemStatus } from "$lib/backend";
  import { prefs } from "$lib/stores/prefs.svelte";
  import { spaces } from "$lib/stores/spaces.svelte";
  import { presence } from "$lib/stores/presence.svelte";
  import Page from "./Page.svelte";
  import FolderPage from "./FolderPage.svelte";
  import PinPad from "./PinPad.svelte";
  import { appLock } from "$lib/stores/applock.svelte";
  import { setAppLock, unlockApp } from "$lib/backend";
  import ChevronUp from "@lucide/svelte/icons/chevron-up";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import Plus from "@lucide/svelte/icons/plus";
  import type { Folder } from "$lib/backend";
  import { onBack } from "$lib/back.svelte";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import ExternalLink from "@lucide/svelte/icons/external-link";
  import KeyRound from "@lucide/svelte/icons/key-round";
  import Laptop from "@lucide/svelte/icons/laptop";
  import LogOut from "@lucide/svelte/icons/log-out";
  import UserX from "@lucide/svelte/icons/user-x";
  import X from "@lucide/svelte/icons/x";
  import Bell from "@lucide/svelte/icons/bell";
  import Info from "@lucide/svelte/icons/info";
  import Lock from "@lucide/svelte/icons/lock";
  import MessagesSquare from "@lucide/svelte/icons/messages-square";
  import Palette from "@lucide/svelte/icons/palette";
  import Smile from "@lucide/svelte/icons/smile";
  import {
    accountUrl,
    appVersion,
    hasBackend,
    notificationRules,
    openExternal,
    setDisplayName,
    setKeyword,
    setNotificationRule,
    setOwnAvatar,
    type NotificationRules,
  } from "$lib/backend";
  import { me } from "$lib/stores/me.svelte";
  import AvatarEdit from "./AvatarEdit.svelte";
  import BlockedPage from "./BlockedPage.svelte";
  import DevicesPage from "./DevicesPage.svelte";
  import SecurityPage from "./SecurityPage.svelte";

  let { userId, onClose, onSignOut }: { userId: string; onClose: () => void; onSignOut: () => void } = $props();

  // ── Profile ──
  let nameDraft = $state<string | null>(null);
  let nameBusy = $state(false);
  async function saveName() {
    if (nameDraft === null) return;
    nameBusy = true;
    try {
      await setDisplayName(nameDraft);
      await me.refresh();
      nameDraft = null;
    } catch (e) {
      error = String(e);
    } finally {
      nameBusy = false;
    }
  }
  async function savePhoto(data: string) {
    await setOwnAvatar(data);
    await me.refresh();
  }

  // ── Account pages ──
  let sub = $state<"devices" | "security" | "blocked" | null>(null);
  /** The settings page open on top of the main list. */
  let section = $state<"status" | "notifications" | "privacy" | "appearance" | "chats" | "about" | null>(null);
  $effect(() => {
    if (section) return onBack(() => (section = null));
  });
  $effect(() => {
    if (sub) return onBack(() => (sub = null));
  });
  async function openAccount(action: string) {
    try {
      await openExternal(await accountUrl(action));
    } catch (e) {
      error = String(e);
    }
  }

  // ── Notification rules (push rules, shared with other apps) ──
  let rules = $state<NotificationRules | null>(null);
  let keyword = $state("");
  $effect(() => {
    if (hasBackend) notificationRules().then((r) => (rules = r)).catch(() => {});
  });
  async function changeRule(rule: "room_mentions" | "invites", on: boolean) {
    if (!rules) return;
    rules[rule] = on;
    await setNotificationRule(rule, on).catch((e) => ((error = String(e)), rules && (rules[rule] = !on)));
  }
  async function changeKeyword(word: string, add: boolean) {
    word = word.trim();
    if (!word || !rules) return;
    try {
      await setKeyword(word, add);
      rules.keywords = add ? [...rules.keywords.filter((k) => k !== word), word] : rules.keywords.filter((k) => k !== word);
      if (add) keyword = "";
    } catch (e) {
      error = String(e);
    }
  }

  let version = $state("");
  $effect(() => {
    appVersion().then((v) => (version = v)).catch(() => {});
  });
  const server = $derived(userId.split(":").slice(1).join(":"));
  const TEXT_SIZES = [
    { value: 90, label: "Small" },
    { value: 100, label: "Default" },
    { value: 115, label: "Large" },
    { value: 130, label: "Huge" },
  ];

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
  // ── App lock: set, change or turn off the PIN, step by step ──
  type PinFlow = {
    mode: "set" | "change" | "disable";
    step: "current" | "new" | "confirm";
    first?: string;
    /** Shown on the next step's keypad (each step gets a fresh one). */
    notice?: string;
  };
  let pinFlow = $state<PinFlow | null>(null);
  $effect(() => {
    if (pinFlow) return onBack(() => (pinFlow = null));
  });
  const LOCK_AFTER = [
    { s: 60, label: "After 1 minute away" },
    { s: 300, label: "After 5 minutes away" },
    { s: 900, label: "After 15 minutes away" },
    { s: 3600, label: "After 1 hour away" },
  ];
  $effect(() => {
    void appLock.refresh().catch(() => {});
  });
  const pinTitle = $derived(
    !pinFlow
      ? ""
      : pinFlow.step === "current"
        ? "Enter your current PIN"
        : pinFlow.step === "new"
          ? "Choose a PIN"
          : "Enter it again",
  );

  async function pinStep(pin: string): Promise<string | null> {
    const flow = pinFlow;
    if (!flow) return null;
    if (flow.step === "current") {
      const r = await unlockApp(pin);
      if (r.result === "wait") return `Too many tries. Wait ${r.seconds} s`;
      if (r.result === "wrong") return "Wrong PIN";
      if (flow.mode === "disable") {
        await setAppLock(null);
        await appLock.refresh();
        pinFlow = null;
      } else pinFlow = { ...flow, step: "new", notice: undefined };
      return null;
    }
    if (flow.step === "new") {
      pinFlow = { ...flow, step: "confirm", first: pin, notice: undefined };
      return null;
    }
    if (pin !== flow.first) {
      pinFlow = { ...flow, step: "new", first: undefined, notice: "The PINs didn't match. Try again" };
      return null;
    }
    await setAppLock(pin);
    await appLock.refresh();
    pinFlow = null;
    return null;
  }

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
    <div class="profile">
      <AvatarEdit name={me.profile?.name ?? userId} mxc={me.profile?.avatar ?? null} onPicked={savePhoto} />
      {#if nameDraft !== null}
        <form class="name-row" onsubmit={(e) => (e.preventDefault(), saveName())}>
          <!-- svelte-ignore a11y_autofocus -->
          <input class="m-input" bind:value={nameDraft} maxlength="60" placeholder="Your name" autofocus />
          <button class="m-btn" disabled={nameBusy}>Save</button>
        </form>
      {:else}
        <button class="name" onclick={() => (nameDraft = me.profile?.name ?? "")} title="Change your name">{me.profile?.name ?? userId}</button>
      {/if}
      <small>{userId}</small>
    </div>

    <button class="m-row" onclick={() => (section = "status")}>
      <span class="m-icon"><Smile size={20} /></span><span class="m-text"><strong>Status</strong><small>{profiles.statusLabel(userId) ?? "Set a status"}</small></span><ChevronRight size={18} />
    </button>
    <button class="m-row" onclick={() => (section = "notifications")}>
      <span class="m-icon"><Bell size={20} /></span><span class="m-text"><strong>Notifications</strong><small>{notificationsOn ? "On" : "Off"} · mentions, keywords</small></span><ChevronRight size={18} />
    </button>
    <button class="m-row" onclick={() => (section = "privacy")}>
      <span class="m-icon"><Lock size={20} /></span><span class="m-text"><strong>Privacy & security</strong><small>Read receipts, app lock, sessions, blocked people</small></span><ChevronRight size={18} />
    </button>
    <button class="m-row" onclick={() => (section = "appearance")}>
      <span class="m-icon"><Palette size={20} /></span><span class="m-text"><strong>Appearance</strong><small>Theme, text size, effects</small></span><ChevronRight size={18} />
    </button>
    <button class="m-row" onclick={() => (section = "chats")}>
      <span class="m-icon"><MessagesSquare size={20} /></span><span class="m-text"><strong>Chats & media</strong><small>Spaces in Chats, folders, photo quality</small></span><ChevronRight size={18} />
    </button>
    <button class="m-row" onclick={() => (section = "about")}>
      <span class="m-icon"><Info size={20} /></span><span class="m-text"><strong>About</strong><small>{version ? `AlwyChat ${version}` : "AlwyChat"}</small></span><ChevronRight size={18} />
    </button>
    <button class="m-row" onclick={() => openAccount("profile")}>
      <span class="m-icon"><ExternalLink size={20} /></span><span class="m-text"><strong>Manage account</strong><small>Email, password, deactivation (your server's account page)</small></span>
    </button>
    <button class="m-row danger" onclick={onSignOut}>
      <span class="m-icon"><LogOut size={20} /></span><span class="m-text"><strong>Sign out</strong></span>
    </button>
  {:else if !error}
    <p class="muted">Loading…</p>
  {/if}
  {#if error}<p class="error">{error}</p>{/if}
</Page>

{#if section === "status"}
  <Page title="Status" onClose={() => (section = null)}>
    {#if settings}
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
    {/if}
    {#if error}<p class="error">{error}</p>{/if}
  </Page>
{/if}
{#if section === "notifications"}
  <Page title="Notifications" onClose={() => (section = null)}>
    {#if settings}
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

    {#if rules}
      <label class="toggle">
        <span><strong>@room mentions</strong><small>Notify when someone alerts a whole group.</small></span>
        <input type="checkbox" role="switch" checked={rules.room_mentions} onchange={(e) => changeRule("room_mentions", e.currentTarget.checked)} />
      </label>
      <label class="toggle">
        <span><strong>Invites</strong><small>Notify when you're invited to a chat or space.</small></span>
        <input type="checkbox" role="switch" checked={rules.invites} onchange={(e) => changeRule("invites", e.currentTarget.checked)} />
      </label>
      <div class="keywords">
        <strong>Keywords</strong>
        <small>Messages containing these notify you, even in chats set to mentions only. Synced with your other apps.</small>
        <div class="m-chips">
          {#each rules.keywords as k (k)}
            <button class="m-chip kw" onclick={() => changeKeyword(k, false)} aria-label="Remove {k}">{k}<X size={14} /></button>
          {/each}
        </div>
        <form class="name-row" onsubmit={(e) => (e.preventDefault(), changeKeyword(keyword, true))}>
          <input class="m-input" bind:value={keyword} placeholder="Add a keyword" maxlength="40" />
          <button class="m-btn plain" disabled={!keyword.trim()}>Add</button>
        </form>
      </div>
    {/if}
    {/if}
    {#if error}<p class="error">{error}</p>{/if}
  </Page>
{/if}
{#if section === "privacy"}
  <Page title="Privacy &amp; security" onClose={() => (section = null)}>
    {#if settings}
    <h3 class="section">Privacy</h3>
    {#each privacy as t (t.key)}{@render row(t)}{/each}
    <h3 class="section">App lock</h3>
    <label class="toggle">
      <span>
        <strong>Lock with a PIN</strong>
        <small>Asked when AlwyChat opens and after you've been away. It hides the app; messages on this device aren't encrypted with it.</small>
      </span>
      <input
        type="checkbox"
        role="switch"
        checked={appLock.state.enabled}
        onclick={(e) => {
          e.preventDefault();
          pinFlow = appLock.state.enabled ? { mode: "disable", step: "current" } : { mode: "set", step: "new" };
        }}
      />
    </label>
    {#if appLock.state.enabled}
      <label class="toggle">
        <span><strong>Lock again</strong></span>
        <select
          value={settings.app_lock_after_s}
          onchange={(e) => save({ app_lock_after_s: Number(e.currentTarget.value) })}
        >
          {#each LOCK_AFTER as o (o.s)}<option value={o.s}>{o.label}</option>{/each}
        </select>
      </label>
      {#if appLock.state.biometry}
        <label class="toggle">
          <span><strong>Unlock with {appLock.state.biometry === "face" ? "face" : "fingerprint"}</strong></span>
          <input type="checkbox" role="switch" checked={settings.app_lock_biometric} onchange={() => toggle("app_lock_biometric")} />
        </label>
      {/if}
      <button class="link-row" onclick={() => (pinFlow = { mode: "change", step: "current" })}>Change PIN</button>
    {/if}
    {#if isAndroid}
      <label class="toggle">
        <span>
          <strong>Hide in app switcher</strong>
          <small>Recent apps shows a blank screen, and screenshots are blocked.</small>
        </span>
        <input type="checkbox" role="switch" checked={settings.hide_in_switcher} onchange={() => toggle("hide_in_switcher")} />
      </label>
    {/if}
    <h3 class="section">Account security</h3>
    <button class="m-row" onclick={() => (sub = "devices")}>
      <span class="m-icon"><Laptop size={20} /></span><span class="m-text"><strong>Sessions</strong><small>Where you're signed in</small></span><ChevronRight size={18} />
    </button>
    <button class="m-row" onclick={() => (sub = "security")}>
      <span class="m-icon"><KeyRound size={20} /></span><span class="m-text"><strong>Security & recovery</strong><small>Verification, key backup, recovery key</small></span><ChevronRight size={18} />
    </button>
    <button class="m-row" onclick={() => (sub = "blocked")}>
      <span class="m-icon"><UserX size={20} /></span><span class="m-text"><strong>Blocked people</strong></span><ChevronRight size={18} />
    </button>
    {/if}
    {#if error}<p class="error">{error}</p>{/if}
  </Page>
{/if}
{#if section === "appearance"}
  <Page title="Appearance" onClose={() => (section = null)}>
    {#if settings}
    <h3 class="section">Appearance</h3>
    <div class="themes" role="radiogroup" aria-label="Theme">
      {#each THEMES as t (t.id)}
        <button role="radio" aria-checked={settings.app_theme === t.id} class:on={settings.app_theme === t.id} onclick={() => save({ app_theme: t.id })}>
          <span class="swatch {t.id}"></span>{t.label}
        </button>
      {/each}
    </div>
    {#each appearance as t (t.key)}{@render row(t)}{/each}
    <h3 class="section">Text size</h3>
    <div class="m-seg">
      {#each TEXT_SIZES as t (t.value)}
        <button class:on={settings.text_scale === t.value} onclick={() => save({ text_scale: t.value })}>{t.label}</button>
      {/each}
    </div>
    {/if}
    {#if error}<p class="error">{error}</p>{/if}
  </Page>
{/if}
{#if section === "chats"}
  <Page title="Chats &amp; media" onClose={() => (section = null)}>
    {#if settings}
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
    <h3 class="section">Media</h3>
    {#each media as t (t.key)}{@render row(t)}{/each}
    {#if !isAndroid}
      <label class="toggle">
        <span><strong>Enter sends</strong><small>{settings.enter_sends ? "Shift+Enter adds a new line." : "Enter adds a new line, Ctrl+Enter sends."}</small></span>
        <input type="checkbox" role="switch" checked={settings.enter_sends} onchange={() => toggle("enter_sends")} />
      </label>
    {/if}
    {/if}
    {#if error}<p class="error">{error}</p>{/if}
  </Page>
{/if}
{#if section === "about"}
  <Page title="About" onClose={() => (section = null)}>
    {#if settings}
    <div class="about">
      <p><strong>AlwyChat</strong> {version}</p>
      <p>Server: {server}</p>
      <p>Free software (AGPL-3.0-or-later). <button class="link" onclick={() => openExternal("https://github.com/arlexTech/alwychat")}>Source code</button></p>
    </div>
    {/if}
    {#if error}<p class="error">{error}</p>{/if}
  </Page>
{/if}
{#if pinFlow}
  <Page title={pinFlow.mode === "disable" ? "Turn off app lock" : pinFlow.mode === "change" ? "Change PIN" : "App lock"} onClose={() => (pinFlow = null)}>
    <div class="pin-page">
      {#key pinFlow.step}
        <PinPad
          title={pinTitle}
          hint={pinFlow.notice ?? (pinFlow.step === "new" ? "4 to 12 digits" : null)}
          onSubmit={pinStep}
        />
      {/key}
    </div>
  </Page>
{/if}
{#if sub === "devices"}
  <DevicesPage onClose={() => (sub = null)} />
{:else if sub === "security"}
  <SecurityPage onClose={() => (sub = null)} />
{:else if sub === "blocked"}
  <BlockedPage onClose={() => (sub = null)} />
{/if}
{#if folderEdit}
  <FolderPage folder={folderEdit === "new" ? null : folderEdit} onClose={() => (folderEdit = null)} />
{/if}

<style>
  .profile {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    margin: 6px 0 4px;
  }
  .profile small {
    color: var(--muted);
  }
  .name {
    margin-top: 8px;
    border: none;
    background: none;
    color: var(--text);
    font: inherit;
    font-size: 1.3rem;
    font-weight: 700;
    cursor: pointer;
  }
  .name-row {
    display: flex;
    gap: 8px;
    width: 100%;
    margin-top: 8px;
  }
  .keywords {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 12px 0;
    border-top: 1px solid var(--border);
  }
  .keywords small {
    color: var(--muted);
  }
  .kw {
    padding: 5px 8px 5px 12px;
  }
  .about p {
    margin: 4px 4px;
    color: var(--muted);
  }
  .about strong {
    color: var(--text);
  }
  .link {
    padding: 0;
    border: none;
    background: none;
    color: var(--accent-a);
    font: inherit;
    text-decoration: underline;
    cursor: pointer;
  }
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
  .pin-page {
    display: grid;
    place-items: center;
    padding-top: 24px;
  }
  .toggle select {
    padding: 8px 10px;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--surface);
    color: inherit;
    font: inherit;
  }
  .link-row {
    display: block;
    width: 100%;
    padding: 12px 0;
    border: none;
    border-top: 1px solid var(--border);
    background: none;
    color: var(--accent-b, inherit);
    font: inherit;
    text-align: left;
    cursor: pointer;
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
