<script lang="ts">
  import logo from "$lib/assets/logo.svg";
  import { tr } from "$lib/i18n/index.svelte";
  import { onMount } from "svelte";
  import { fade, fly } from "svelte/transition";
  import {
    cancelLogin,
    discoverServer,
    endSession,
    hasBackend,
    isWeb,
    login,
    logout,
    prepareLogin,
    onSessionEnded,
    restoreSession,
    holdUntilRestored,
    warmUpWeb,
    webHasSession,
    type Account,
    type ServerInfo,
  } from "$lib/backend";
  // The app itself (most of the code) loads only once someone is signed in: the sign-in and
  // invite screens show without waiting for it.
  const loadApp = () => import("$lib/components/ChatsScreen.svelte");
  let appModule = $state<Awaited<ReturnType<typeof loadApp>> | null>(null);
  $effect(() => {
    if (view === "signed-in" && !appModule) loadApp().then((m) => (appModule = m));
  });
  import { dropInvite, pendingInvite, takeInviteFromAddress } from "$lib/invite";
  import { afterNavigate, replaceState } from "$app/navigation";
  import { hideSplash } from "$lib/splash";
  import { appLock } from "$lib/stores/applock.svelte";

  type View = "loading" | "signed-out" | "waiting" | "signed-in";

  let view = $state<View>("loading");
  // Keep the logo up while there's nothing to show yet (restoring a session without a kept
  // account, the app lock check); a few seconds at most, then the "Opening your chats…" text.
  $effect(() => {
    if ((view !== "loading" || slow) && !(view === "signed-in" && !appModule) && !appLock.checking) hideSplash();
  });
  /** The local restore is quick: only say something if it isn't. */
  let slow = $state(false);
  setTimeout(() => (slow = true), 1200);
  // Builds may preset another server (VITE_ALWY_DEFAULT_SERVER, e.g. in a gitignored .env.local).
  let server = $state(import.meta.env.VITE_ALWY_DEFAULT_SERVER || "matrix.org");
  let account = $state<Account | null>(null);
  let error = $state<string | null>(null);
  /** From an invite link (/register?token=…): offer to create an account on this app's server. */
  const fromLink = takeInviteFromAddress();
  // The token is kept: no need to show it in the address bar (once the router is ready).
  afterNavigate(() => {
    if (fromLink && location.pathname === "/register") replaceState("/", {});
  });
  let invite = $state(pendingInvite());

  // The server is looked up while typing (after a pause): sign-in only starts on one that
  // exists and supports what AlwyChat needs (browser sign-in, sliding sync).
  type Check =
    | { state: "checking"; for: string }
    | { state: "ok"; for: string; info: ServerInfo }
    | { state: "missing" | "unsupported"; for: string; reason: string };
  let check = $state<Check | null>(null);
  let checkTimer: ReturnType<typeof setTimeout> | undefined;
  let firstCheck = true;
  async function lookUp(name: string): Promise<Check> {
    try {
      const info = await discoverServer(name);
      if (!info.oauth_issuer) {
        return { state: "unsupported", for: name, reason: tr("This server doesn't support signing in through the browser (it needs Matrix Authentication Service).") };
      }
      if (!info.sliding_sync) {
        return { state: "unsupported", for: name, reason: tr("This server is too old for AlwyChat (no sliding sync).") };
      }
      return { state: "ok", for: name, info };
    } catch {
      return { state: "missing", for: name, reason: tr("No Matrix server answers at {name}.", { name }) };
    }
  }
  $effect(() => {
    const name = server.trim();
    clearTimeout(checkTimer);
    if (!hasBackend || !/\S\.\S|localhost/.test(name)) return void (check = null);
    check = { state: "checking", for: name };
    // The preset server is looked up at once; typed ones after a pause.
    checkTimer = setTimeout(async () => {
      const result = await lookUp(name);
      if (server.trim() !== name) return;
      check = result;
      // Web: register the app and prepare the attempt now, so the button opens the server's
      // page without any wait.
      if (result.state === "ok") prepareLogin(name, invite);
    }, firstCheck ? 0 : 600);
    firstCheck = false;
  });
  // Usable while the server is still being looked up: signing in then just finishes that first.
  const ready = $derived(!!check && check.for === server.trim() && (check.state === "ok" || check.state === "checking"));

  onMount(async () => {
    if (!hasBackend) return void (view = "signed-out");
    // Never a full sign-out here: that would delete this device's keys on a false alarm.
    onSessionEnded(async () => {
      await endSession().catch(() => {});
      keepAccount(null);
      account = null;
      view = "signed-out";
      error = tr("Your session ended. Please sign in again.");
    });
    // Someone is (probably) signed in: fetch the app's code while the session is restored.
    if (!isWeb || webHasSession()) loadApp().then((m) => (appModule = m));
    // Web, nobody signed in here: show sign-in now, the core keeps loading behind it.
    if (isWeb && !webHasSession()) {
      view = "signed-out";
      warmUpWeb();
      return;
    }
    // Web: the core takes seconds to download and start. The account we had last time goes on
    // screen right away (chat list from what was kept) and the app's commands wait for the restore.
    const kept = isWeb ? keptAccount() : null;
    const restoring = restoreSession();
    if (kept) {
      holdUntilRestored(restoring);
      account = kept;
      view = "signed-in";
    }
    let restored: Account | null = null;
    try {
      restored = await restoring;
    } catch (e) {
      error = String(e);
    }
    const same = kept && restored && JSON.stringify(kept) === JSON.stringify(restored);
    if (!same) account = restored;
    keepAccount(restored);
    view = restored ? "signed-in" : "signed-out";
    // Already signed in: the invite was for someone new.
    if (restored) forgetInvite();
  });

  const ACCOUNT_KEY = "alwychat.account";
  function keptAccount(): Account | null {
    try {
      const a = JSON.parse(localStorage.getItem(ACCOUNT_KEY) ?? "null");
      return a && typeof a.user_id === "string" && typeof a.device_id === "string" ? a : null;
    } catch {
      return null;
    }
  }
  function keepAccount(a: Account | null) {
    try {
      if (a) localStorage.setItem(ACCOUNT_KEY, JSON.stringify(a));
      else localStorage.removeItem(ACCOUNT_KEY);
    } catch {}
  }

  function forgetInvite() {
    dropInvite();
    invite = null;
  }

  async function signIn(event: SubmitEvent) {
    event.preventDefault();
    error = null;
    if (!ready) return;
    view = "waiting";
    try {
      account = await login(server.trim(), invite ?? undefined);
      keepAccount(account);
      forgetInvite();
      view = "signed-in";
    } catch (e) {
      if (view === "waiting") error = e instanceof Error ? e.message : String(e);
      view = "signed-out";
    }
  }

  function cancelSignIn() {
    view = "signed-out";
    cancelLogin().catch(() => {});
  }

  async function signOut() {
    error = null;
    try {
      await logout();
    } catch (e) {
      error = String(e);
    }
    // What was kept to open things instantly goes with the account.
    const [{ chatList }, { chat }] = await Promise.all([import("$lib/stores/chatList.svelte"), import("$lib/stores/chat.svelte")]);
    chatList.reset(true);
    chat.forget();
    keepAccount(null);
    account = null;
    view = "signed-out";
  }
</script>

{#if view === "signed-in" && account && appModule}
  <appModule.default {account} onSignOut={signOut} />
{:else if view === "signed-in"}
  <main class="hidden"></main>
{:else}
<main class:hidden={view === "loading" && !slow}>
  <div class="brand">
    <img src={logo} alt="" width="56" height="56" />
    <h1>{tr("AlwyChat")}</h1>
  </div>

  {#if !hasBackend}
    <p class="error">
      {tr("This page is the app's interface only. Run")} <code>pnpm tauri dev</code> {tr("and use the AlwyChat window (a web version comes later).")}
    </p>
  {/if}

  {#key view}
    <section in:fly={{ y: 12, duration: 250 }}>
      {#if view === "loading"}
        {#if slow}<p class="muted" in:fade>{tr("Opening your chats…")}</p>{/if}
      {:else if view === "signed-out" && invite !== null}
        <form class="invite" onsubmit={signIn}>
          <h2>{tr("You're invited")}</h2>
          <p>{tr("Create your account on {server} to chat with your friends.", { server: server.trim() })}</p>
          <button disabled={!hasBackend || !ready}>{tr("Create my account")}</button>
          {#if check && check.state !== "ok" && check.state !== "checking"}<p class="check {check.state}">{check.reason}</p>{/if}
          <p class="muted">{tr("Your browser will open your server's page, then bring you back here.")}</p>
          <button type="button" class="ghost" onclick={forgetInvite}>{tr("I already have an account")}</button>
        </form>
      {:else if view === "signed-out"}
        <form onsubmit={signIn}>
          <label for="server">{tr("Your server")}</label>
          <div class="row">
            <input id="server" bind:value={server} autocapitalize="off" spellcheck="false" />
            <button disabled={!hasBackend || !ready}>{tr("Sign in")}</button>
          </div>
          {#if check}
            <p class="check {check.state}" aria-live="polite">
              {#if check.state === "checking"}{tr("Looking for the server…")}
              {:else if check.state === "ok"}✓ {tr("Found {host}", { host: new URL(check.info.homeserver).host })}
              {:else}{check.reason}{/if}
            </p>
          {/if}
          <p class="muted">{tr("Your browser will open your server's sign-in page.")}</p>
        </form>
      {:else if view === "waiting"}
        <div class="waiting">
          <div class="spinner" aria-hidden="true"></div>
          <p>{tr("Finish signing in in your browser…")}</p>
          <p class="muted">{tr("This window will update by itself.")}</p>
          <button class="ghost" onclick={cancelSignIn}>{tr("Cancel")}</button>
        </div>
      {/if}
    </section>
  {/key}

  {#if error}<p class="error" transition:fade>{error}</p>{/if}
</main>
{/if}

<style>
  main {
    max-width: 420px;
    margin: 12vh auto;
    padding: 0 1.5rem;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 14px;
  }
  .brand img {
    border-radius: 14px;
    box-shadow: 0 8px 24px rgb(91 91 214 / 0.35);
  }
  h1 {
    font-size: 2rem;
    background: linear-gradient(90deg, #7b5cff, #ff5c8a);
    background-clip: text;
    color: transparent;
  }
  .check {
    margin: 8px 2px 0;
    font-size: 0.85rem;
    color: var(--muted);
  }
  .check.ok {
    color: #3ba55d;
  }
  .check.missing,
  .check.unsupported {
    color: var(--danger);
  }
  label {
    display: block;
    margin-bottom: 0.5rem;
    color: var(--muted);
  }
  .row {
    display: flex;
    gap: 0.5rem;
  }
  input,
  button {
    font: inherit;
    padding: 0.65rem 1rem;
    border-radius: 999px;
    border: 1px solid var(--border);
    background: var(--surface);
    color: inherit;
  }
  input {
    flex: 1;
    min-width: 0;
  }
  button {
    background: linear-gradient(90deg, #7b5cff, #ff5c8a);
    border: none;
    cursor: pointer;
    transition: transform 0.12s ease, opacity 0.12s ease;
  }
  button:active {
    transform: scale(0.96);
  }
  button:disabled {
    opacity: 0.5;
  }
  .muted {
    color: var(--muted);
    margin: 0.4rem 0 0;
  }
  .waiting {
    text-align: center;
  }
  .invite {
    display: grid;
    gap: 0.6rem;
  }
  .invite h2 {
    margin: 0.5rem 0 0;
    font-size: 1.3rem;
  }
  .invite p {
    margin: 0;
  }
  .invite .ghost {
    margin-top: 0.8rem;
  }
  .ghost {
    margin-top: 1.5rem;
    background: var(--surface);
    border: 1px solid var(--border);
  }
  .spinner {
    width: 36px;
    height: 36px;
    margin: 1rem auto;
    border-radius: 50%;
    border: 3px solid var(--border);
    border-top-color: #ff5c8a;
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .error {
    color: var(--danger);
  }
  main.hidden {
    visibility: hidden;
  }
</style>
