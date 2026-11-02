<script lang="ts">
  import { LANGUAGES, chooseLanguage, languageChoice, languageName, locale, tr } from "$lib/i18n/index.svelte";
  import { onMount } from "svelte";
  import { MediaQuery } from "svelte/reactivity";
  import { profiles, ourZone } from "$lib/stores/profiles.svelte";
  import { isAndroid, setStatus, setTimeZone, setupPush, systemStatus, type AppTheme, type Settings, type SystemStatus, isTauri, type MessageLayout } from "$lib/backend";
  import { prefs } from "$lib/stores/prefs.svelte";
  import { spaces } from "$lib/stores/spaces.svelte";
  import { presence } from "$lib/stores/presence.svelte";
  import Page, { setPageLayout } from "./Page.svelte";
  import VoiceSettings from "./VoiceSettings.svelte";
  import { updates } from "$lib/stores/updates.svelte";
  import { clock } from "$lib/stores/clock.svelte";
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
  import Mic from "@lucide/svelte/icons/mic";
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
    testPush,
    type PushTestResult,
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
  let section = $state<"status" | "notifications" | "voice" | "privacy" | "appearance" | "chats" | "about" | null>(null);
  // Wide screens show the list and a section side by side (Discord): one is always open.
  const wide = new MediaQuery("min-width: 900px");
  $effect(() => {
    if (wide.current && section === null) section = "status";
  });
  // Pages opened from a section (sessions, security…) cover only the section's side.
  setPageLayout("detail");
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

  // ── Push test (Android) ──
  let pushTest = $state<PushTestResult | "running" | { error: string } | null>(null);
  async function runPushTest() {
    pushTest = "running";
    try {
      pushTest = await testPush();
    } catch (e) {
      pushTest = { error: String(e) };
    }
  }
  type Step = { label: string; ok: boolean | null; hint?: string };
  function pushSteps(t: PushTestResult | { error: string }): Step[] {
    if (!("permission" in t)) return [{ label: tr("The test couldn't run"), ok: false, hint: t.error }];
    const steps: Step[] = [
      { label: tr("Allowed to show notifications"), ok: t.permission, hint: t.permission ? undefined : tr("Turn on “New messages” above, or allow AlwyChat in Android's settings.") },
      { label: tr("Push available in this app"), ok: t.token, hint: t.token ? undefined : tr("This build has no Firebase setup: notifications only arrive while AlwyChat is open.") },
    ];
    if (!t.token) return steps;
    steps.push({ label: tr("Your server knows this phone"), ok: !!t.registered, hint: t.registered ? undefined : "The registration didn't stick; try again in a moment." });
    steps.push({
      label: tr("Push gateway → Google → this phone"),
      ok: !!t.gateway_ok && !t.rejected,
      hint: t.error ?? (t.rejected ? tr("Google doesn't know this phone's token: restart AlwyChat and try again.") : tr("A notification should appear within a few seconds.")),
    });
    return steps;
  }

  let version = $state("");
  $effect(() => {
    appVersion().then((v) => (version = v)).catch(() => {});
  });
  const server = $derived(userId.split(":").slice(1).join(":"));
  const TEXT_SIZES = [
    { value: 90, label: tr("Small") },
    { value: 100, label: "Default" },
    { value: 115, label: tr("Large") },
    { value: 130, label: tr("Huge") },
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
    { s: 60, label: tr("After 1 minute away") },
    { s: 300, label: tr("After 5 minutes away") },
    { s: 900, label: tr("After 15 minutes away") },
    { s: 3600, label: tr("After 1 hour away") },
  ];
  $effect(() => {
    void appLock.refresh().catch(() => {});
  });
  const pinTitle = $derived(
    !pinFlow
      ? ""
      : pinFlow.step === "current"
        ? tr("Enter your current PIN")
        : pinFlow.step === "new"
          ? tr("Choose a PIN")
          : tr("Enter it again"),
  );

  async function pinStep(pin: string): Promise<string | null> {
    const flow = pinFlow;
    if (!flow) return null;
    if (flow.step === "current") {
      const r = await unlockApp(pin);
      if (r.result === "wait") return tr("Too many tries. Wait {seconds} s", { seconds: r.seconds });
      if (r.result === "wrong") return tr("Wrong PIN");
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
      pinFlow = { ...flow, step: "new", first: undefined, notice: tr("The PINs didn't match. Try again") };
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
      label: tr("Send read receipts"),
      hint: tr("Others see when you've read their messages. Off: your unread counts still sync between your devices."),
    },
    { key: "send_typing", label: tr("Show when I'm typing"), hint: tr("Others see the typing dots while you write.") },
    {
      key: "share_presence",
      label: tr("Show when I'm active"),
      hint: tr("Others see when you're online or when you were last active."),
    },
  ];

  const THEMES: { id: AppTheme; label: string }[] = [
    { id: "system", label: tr("System") },
    { id: "light", label: tr("Light") },
    { id: "dark", label: tr("Dark") },
    { id: "amoled", label: tr("Black") },
  ];

  const appearance: Toggle[] = [
    {
      key: "chat_effects",
      label: tr("Message effects"),
      hint: tr("Confetti, fireworks, hearts, snow… when a message with 🎉 🎆 💝 ❄️ arrives (works with Element)."),
    },
  ];

  const media: Toggle[] = [
    {
      key: "hd_media",
      label: tr("Send photos in HD"),
      hint: tr("Original quality by default. Off: photos are compressed (faster, smaller). Each chat can override this."),
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

<Page title={tr("Settings")} {onClose} layout="list">
  {#if settings}
    <div class="profile">
      <AvatarEdit name={me.profile?.name ?? userId} mxc={me.profile?.avatar ?? null} onPicked={savePhoto} />
      {#if nameDraft !== null}
        <form class="name-row" onsubmit={(e) => (e.preventDefault(), saveName())}>
          <!-- svelte-ignore a11y_autofocus -->
          <input class="m-input" bind:value={nameDraft} maxlength="60" placeholder={tr("Your name")} autofocus />
          <button class="m-btn" disabled={nameBusy}>{tr("Save")}</button>
        </form>
      {:else}
        <button class="name" onclick={() => (nameDraft = me.profile?.name ?? "")} title={tr("Change your name")}>{me.profile?.name ?? userId}</button>
      {/if}
      <small>{userId}</small>
    </div>

    <button class="m-row" class:active={wide.current && section === "status"} onclick={() => (section = "status")}>
      <span class="m-icon"><Smile size={20} /></span><span class="m-text"><strong>{tr("Status")}</strong><small>{profiles.statusLabel(userId) ?? tr("Set a status")}</small></span><ChevronRight size={18} />
    </button>
    <button class="m-row" class:active={wide.current && section === "notifications"} onclick={() => (section = "notifications")}>
      <span class="m-icon"><Bell size={20} /></span><span class="m-text"><strong>{tr("Notifications")}</strong><small>{tr("{value} · mentions, keywords", { value: notificationsOn ? "On" : "Off" })}</small></span><ChevronRight size={18} />
    </button>
    <button class="m-row" class:active={wide.current && section === "voice"} onclick={() => (section = "voice")}>
      <span class="m-icon"><Mic size={20} /></span><span class="m-text"><strong>{tr("Voice & video")}</strong><small>{tr("Microphone, speakers, camera, push to talk")}</small></span><ChevronRight size={18} />
    </button>
    <button class="m-row" class:active={wide.current && section === "privacy"} onclick={() => (section = "privacy")}>
      <span class="m-icon"><Lock size={20} /></span><span class="m-text"><strong>{tr("Privacy & security")}</strong><small>{tr("Read receipts, app lock, sessions, blocked people")}</small></span><ChevronRight size={18} />
    </button>
    <button class="m-row" class:active={wide.current && section === "appearance"} onclick={() => (section = "appearance")}>
      <span class="m-icon"><Palette size={20} /></span><span class="m-text"><strong>{tr("Appearance")}</strong><small>{tr("Language, time format, theme, text size, effects")}</small></span><ChevronRight size={18} />
    </button>
    <button class="m-row" class:active={wide.current && section === "chats"} onclick={() => (section = "chats")}>
      <span class="m-icon"><MessagesSquare size={20} /></span><span class="m-text"><strong>{tr("Chats & media")}</strong><small>{tr("Spaces in Chats, folders, photo quality")}</small></span><ChevronRight size={18} />
    </button>
    <button class="m-row" class:active={wide.current && section === "about"} onclick={() => (section = "about")}>
      <span class="m-icon"><Info size={20} /></span><span class="m-text"><strong>{tr("About")}</strong><small>{version ? tr("AlwyChat {version}", { version: version }) : tr("AlwyChat")}</small></span><ChevronRight size={18} />
    </button>
    <button class="m-row" onclick={() => openAccount("profile")}>
      <span class="m-icon"><ExternalLink size={20} /></span><span class="m-text"><strong>{tr("Manage account")}</strong><small>{tr("Email, password, deactivation (your server's account page)")}</small></span>
    </button>
    <button class="m-row danger" onclick={onSignOut}>
      <span class="m-icon"><LogOut size={20} /></span><span class="m-text"><strong>{tr("Sign out")}</strong></span>
    </button>
  {:else if !error}
    <p class="muted">{tr("Loading…")}</p>
  {/if}
  {#if error}<p class="error">{error}</p>{/if}
</Page>

{#if section === "status"}
  <Page title={tr("Status")} onClose={() => (section = null)} layout="section">
    {#if settings}
    <div class="status-emoji" role="radiogroup" aria-label={tr("Status emoji")}>
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
        placeholder={tr("What's up?")}
        maxlength="80"
        bind:value={statusText}
        onkeydown={(e) => e.key === "Enter" && statusChanged && saveStatus()}
      />
      {#if statusChanged}
        <button class="status-save" disabled={statusBusy} onclick={() => saveStatus()}>{tr("Save")}</button>
      {:else if statusSaved}
        <button class="status-clear" disabled={statusBusy} onclick={() => saveStatus(true)}>{tr("Clear")}</button>
      {/if}
    </div>
    <small class="status-hint">{tr("Anyone who can see your profile sees it, in AlwyChat and other apps that support statuses.")}</small>
    <label class="toggle">
      <span>
        <strong>{tr("Share my local time")}</strong>
        <small>{tr("People see your time zone ({ourZone}) and the time it is for you.", { ourZone: ourZone() })}</small>
      </span>
      <input type="checkbox" role="switch" checked={shareZone} onchange={(e) => toggleZone(e.currentTarget.checked)} />
    </label>
    {/if}
    {#if error}<p class="error">{error}</p>{/if}
  </Page>
{/if}
{#if section === "notifications"}
  <Page title={tr("Notifications")} onClose={() => (section = null)} layout="section">
    {#if settings}
    <h3 class="section">{tr("Notifications")}</h3>
    <label class="toggle">
      <span>
        <strong>{tr("New messages")}</strong>
        <small>
          {tr("{value} Follows your mute and mention settings.", { value: isAndroid ? "While AlwyChat isn't on screen." : "While AlwyChat isn't in front." })}
          {#if settings.notifications && system && !system.notifications}
            <em>{tr("Turned off in {value} settings.", { value: isAndroid ? "Android" : "system" })}</em>
          {/if}
        </small>
      </span>
      <input type="checkbox" role="switch" checked={notificationsOn} onchange={toggleNotifications} />
    </label>
    {#if isAndroid && notificationsOn && system}
      <div class="check">
        <span class="mark" class:ok={system.batteryUnrestricted}>{system.batteryUnrestricted ? "✓" : "!"}</span>
        <span>
          <strong>{tr("Battery")}</strong>
          <small>
            {system.batteryUnrestricted
              ? tr("AlwyChat can keep running in the background.")
              : tr("Battery optimisation pauses AlwyChat in the background: notifications come late or not at all.")}
          </small>
        </span>
        {#if !system.batteryUnrestricted}<button onclick={() => fix("battery")}>{tr("Allow")}</button>{/if}
      </div>
      <div class="check">
        <span class="mark" class:ok={system.backgroundData}>{system.backgroundData ? "✓" : "!"}</span>
        <span>
          <strong>{tr("Background data")}</strong>
          <small>
            {system.backgroundData
              ? tr("AlwyChat can receive messages in the background.")
              : tr("Data saver blocks AlwyChat in the background, so new messages can't arrive.")}
          </small>
        </span>
        {#if !system.backgroundData}<button onclick={() => fix("backgroundData")}>{tr("Allow")}</button>{/if}
      </div>
    {/if}

    {#if rules}
      <label class="toggle">
        <span><strong>{tr("@room mentions")}</strong><small>{tr("Notify when someone alerts a whole group.")}</small></span>
        <input type="checkbox" role="switch" checked={rules.room_mentions} onchange={(e) => changeRule("room_mentions", e.currentTarget.checked)} />
      </label>
      <label class="toggle">
        <span><strong>{tr("Invites")}</strong><small>{tr("Notify when you're invited to a chat or space.")}</small></span>
        <input type="checkbox" role="switch" checked={rules.invites} onchange={(e) => changeRule("invites", e.currentTarget.checked)} />
      </label>
      <div class="keywords">
        <strong>{tr("Keywords")}</strong>
        <small>{tr("Messages containing these notify you, even in chats set to mentions only. Synced with your other apps.")}</small>
        <div class="m-chips">
          {#each rules.keywords as k (k)}
            <button class="m-chip kw" onclick={() => changeKeyword(k, false)} aria-label={tr("Remove {k}", { k: k })}>{k}<X size={14} /></button>
          {/each}
        </div>
        <form class="name-row" onsubmit={(e) => (e.preventDefault(), changeKeyword(keyword, true))}>
          <input class="m-input" bind:value={keyword} placeholder={tr("Add a keyword")} maxlength="40" />
          <button class="m-btn plain" disabled={!keyword.trim()}>{tr("Add")}</button>
        </form>
      </div>
    {/if}
    {#if isAndroid}
      <h3 class="section">{tr("Test")}</h3>
      <button class="m-btn plain wide-btn" disabled={pushTest === "running"} onclick={runPushTest}>
        {pushTest === "running" ? tr("Testing…") : tr("Send a test notification")}
      </button>
      {#if pushTest && pushTest !== "running"}
        <ul class="push-test">
          {#each pushSteps(pushTest) as step (step.label)}
            <li class:ok={step.ok} class:bad={step.ok === false}>
              <span class="mark">{step.ok ? "✓" : step.ok === false ? "!" : "·"}</span>
              <span><strong>{step.label}</strong>{#if step.hint}<small>{step.hint}</small>{/if}</span>
            </li>
          {/each}
        </ul>
      {/if}
    {/if}
    {/if}
    {#if error}<p class="error">{error}</p>{/if}
  </Page>
{/if}
{#if section === "voice"}
  <Page title={tr("Voice & video")} onClose={() => (section = null)} layout="section">
    <VoiceSettings />
  </Page>
{/if}
{#if section === "privacy"}
  <Page title={tr("Privacy & security")} onClose={() => (section = null)} layout="section">
    {#if settings}
    <h3 class="section">{tr("Privacy")}</h3>
    {#each privacy as t (t.key)}{@render row(t)}{/each}
    <h3 class="section">{tr("App lock")}</h3>
    <label class="toggle">
      <span>
        <strong>{tr("Lock with a PIN")}</strong>
        <small>{tr("Asked when AlwyChat opens and after you've been away. It hides the app; messages on this device aren't encrypted with it.")}</small>
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
        <span><strong>{tr("Lock again")}</strong></span>
        <select
          value={settings.app_lock_after_s}
          onchange={(e) => save({ app_lock_after_s: Number(e.currentTarget.value) })}
        >
          {#each LOCK_AFTER as o (o.s)}<option value={o.s}>{o.label}</option>{/each}
        </select>
      </label>
      {#if appLock.state.biometry}
        <label class="toggle">
          <span><strong>{tr("Unlock with {value}", { value: appLock.state.biometry === "face" ? "face" : "fingerprint" })}</strong></span>
          <input type="checkbox" role="switch" checked={settings.app_lock_biometric} onchange={() => toggle("app_lock_biometric")} />
        </label>
      {/if}
      <button class="link-row" onclick={() => (pinFlow = { mode: "change", step: "current" })}>{tr("Change PIN")}</button>
    {/if}
    {#if isAndroid}
      <label class="toggle">
        <span>
          <strong>{tr("Hide in app switcher")}</strong>
          <small>{tr("Recent apps shows a blank screen, and screenshots are blocked.")}</small>
        </span>
        <input type="checkbox" role="switch" checked={settings.hide_in_switcher} onchange={() => toggle("hide_in_switcher")} />
      </label>
    {/if}
    <h3 class="section">{tr("Account security")}</h3>
    <button class="m-row" onclick={() => (sub = "devices")}>
      <span class="m-icon"><Laptop size={20} /></span><span class="m-text"><strong>{tr("Sessions")}</strong><small>{tr("Where you're signed in")}</small></span><ChevronRight size={18} />
    </button>
    <button class="m-row" onclick={() => (sub = "security")}>
      <span class="m-icon"><KeyRound size={20} /></span><span class="m-text"><strong>{tr("Security & recovery")}</strong><small>{tr("Verification, key backup, recovery key")}</small></span><ChevronRight size={18} />
    </button>
    <button class="m-row" onclick={() => (sub = "blocked")}>
      <span class="m-icon"><UserX size={20} /></span><span class="m-text"><strong>{tr("Blocked people")}</strong></span><ChevronRight size={18} />
    </button>
    {/if}
    {#if error}<p class="error">{error}</p>{/if}
  </Page>
{/if}
{#if section === "appearance"}
  <Page title={tr("Appearance")} onClose={() => (section = null)} layout="section">
    {#if settings}
    <h3 class="section">{tr("Language")}</h3>
    <select class="m-input language" value={languageChoice} onchange={(e) => chooseLanguage(e.currentTarget.value)}>
      <option value="auto">{tr("Automatic ({name})", { name: languageName(locale) })}</option>
      {#each LANGUAGES as code (code)}<option value={code}>{languageName(code)}</option>{/each}
    </select>
    <h3 class="section">{tr("Message layout")}</h3>
    <div class="m-seg">
      {#each [["auto", tr("Automatic")], ["bubbles", tr("Mine on the right")], ["left", tr("Everyone on the left")]] as [value, label] (value)}
        <button class:on={(settings.message_layout ?? "auto") === value} onclick={() => save({ message_layout: value as MessageLayout })}>{label}</button>
      {/each}
    </div>
    <p class="m-hint">{tr("Automatic: everyone on the left on wide screens, your messages on the right elsewhere. Each chat can choose for itself in its settings.")}</p>
    <h3 class="section">{tr("Time format")}</h3>
    <div class="m-seg">
      <button class:on={clock.choice === "system"} onclick={() => clock.set("system")}>
        {tr("System ({format})", { format: clock.system12 ? tr("12-hour") : tr("24-hour") })}
      </button>
      <button class:on={clock.choice === "24"} onclick={() => clock.set("24")}>{tr("24-hour")}</button>
      <button class:on={clock.choice === "12"} onclick={() => clock.set("12")}>{tr("12-hour")}</button>
    </div>
    <h3 class="section">{tr("Appearance")}</h3>
    <div class="themes" role="radiogroup" aria-label={tr("Theme")}>
      {#each THEMES as t (t.id)}
        <button role="radio" aria-checked={settings.app_theme === t.id} class:on={settings.app_theme === t.id} onclick={() => save({ app_theme: t.id })}>
          <span class="swatch {t.id}"></span>{t.label}
        </button>
      {/each}
    </div>
    {#each appearance as t (t.key)}{@render row(t)}{/each}
    <h3 class="section">{tr("Text size")}</h3>
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
  <Page title={tr("Chats & media")} onClose={() => (section = null)} layout="section">
    {#if settings}
    <h3 class="section">{tr("Chats")}</h3>
    <label class="toggle">
      <span>
        <strong>{tr("Show chats from spaces in Chats")}</strong>
        <small>{tr("Off: they're only under Spaces. Each space and chat can choose for itself in its settings. Synced to your other devices.")}</small>
      </span>
      <input
        type="checkbox"
        role="switch"
        checked={spaces.prefs.show_space_chats}
        onchange={(e) => spaces.setGlobal(e.currentTarget.checked).catch((err) => (error = String(err)))}
      />
    </label>

    <h3 class="section">{tr("Folders")}</h3>
    <ul class="folders">
      {#each spaces.prefs.folders as f, i (f.id)}
        <li>
          <button class="folder-name" onclick={() => (folderEdit = f)}>{f.name}</button>
          <button class="icon" disabled={i === 0} onclick={() => move(f.id, -1)} aria-label={tr("Move up")}><ChevronUp size={18} /></button>
          <button class="icon" disabled={i === spaces.prefs.folders.length - 1} onclick={() => move(f.id, 1)} aria-label={tr("Move down")}><ChevronDown size={18} /></button>
        </li>
      {/each}
      <li><button class="folder-name new" onclick={() => (folderEdit = "new")}><Plus size={18} />{tr("New folder")}</button></li>
    </ul>
    <h3 class="section">{tr("Media")}</h3>
    {#each media as t (t.key)}{@render row(t)}{/each}
    {#if !isAndroid}
      <label class="toggle">
        <span><strong>{tr("Enter sends")}</strong><small>{settings.enter_sends ? tr("Shift+Enter adds a new line.") : tr("Enter adds a new line, Ctrl+Enter sends.")}</small></span>
        <input type="checkbox" role="switch" checked={settings.enter_sends} onchange={() => toggle("enter_sends")} />
      </label>
    {/if}
    {/if}
    {#if error}<p class="error">{error}</p>{/if}
  </Page>
{/if}
{#if section === "about"}
  <Page title={tr("About")} onClose={() => (section = null)} layout="section">
    {#if settings}
    <div class="about">
      <p><strong>{tr("AlwyChat")}</strong> {version}</p>
      <p>{tr("Server: {server}", { server: server })}</p>
      <p>{tr("Free software (AGPL-3.0-or-later).")} <button class="link" onclick={() => openExternal("https://github.com/arlexTech/alwychat")}>{tr("Source code")}</button></p>
    </div>
    {#if isTauri}
      <h3 class="section">{tr("Updates")}</h3>
      {#if updates.available}
        <button class="m-btn" disabled={updates.stage === "downloading" || updates.stage === "installing"} onclick={() => updates.install()}>
          {tr("Update to AlwyChat {version}", { version: updates.available.version })}
        </button>
      {:else}
        <button class="m-btn" disabled={updates.stage === "checking"} onclick={() => updates.check()}>
          {updates.stage === "checking" ? tr("Checking…") : tr("Check for updates")}
        </button>
        {#if updates.upToDate}<p class="m-hint">{tr("You have the latest version.")}</p>{/if}
      {/if}
      {#if updates.error}<p class="m-hint">{updates.error}</p>{/if}
      <p class="m-hint">{tr("AlwyChat installed from GitHub updates itself: it checks a few times a day and asks before installing.")}</p>
    {/if}
    {/if}
    {#if error}<p class="error">{error}</p>{/if}
  </Page>
{/if}
{#if pinFlow}
  <Page title={pinFlow.mode === "disable" ? tr("Turn off app lock") : pinFlow.mode === "change" ? tr("Change PIN") : tr("App lock")} onClose={() => (pinFlow = null)}>
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
  .wide-btn {
    width: 100%;
  }
  .push-test {
    margin: 10px 0 0;
    padding: 0;
    list-style: none;
  }
  .push-test li {
    display: flex;
    gap: 10px;
    padding: 6px 2px;
  }
  .push-test li > span:last-child {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .push-test small {
    color: var(--muted);
  }
  .push-test .mark {
    display: grid;
    flex: none;
    width: 22px;
    height: 22px;
    place-items: center;
    border-radius: 50%;
    background: var(--surface-2);
    font-size: 0.8rem;
  }
  .push-test .ok .mark {
    background: #31d158;
    color: white;
  }
  .push-test .bad .mark {
    background: var(--danger);
    color: white;
  }
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
