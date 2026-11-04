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
    login,
    logout,
    onSessionEnded,
    restoreSession,
    type Account,
    type ServerInfo,
  } from "$lib/backend";
  import ChatsScreen from "$lib/components/ChatsScreen.svelte";

  type View = "loading" | "signed-out" | "waiting" | "signed-in";

  let view = $state<View>("loading");
  /** The local restore is quick: only say something if it isn't. */
  let slow = $state(false);
  setTimeout(() => (slow = true), 1200);
  // Builds may preset another server (VITE_ALWY_DEFAULT_SERVER, e.g. in a gitignored .env.local).
  let server = $state(import.meta.env.VITE_ALWY_DEFAULT_SERVER || "matrix.org");
  let account = $state<Account | null>(null);
  let error = $state<string | null>(null);

  // The server is looked up while typing (after a pause): sign-in only starts on one that
  // exists and supports what AlwyChat needs (browser sign-in, sliding sync).
  type Check =
    | { state: "checking"; for: string }
    | { state: "ok"; for: string; info: ServerInfo }
    | { state: "missing" | "unsupported"; for: string; reason: string };
  let check = $state<Check | null>(null);
  let checkTimer: ReturnType<typeof setTimeout> | undefined;
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
    checkTimer = setTimeout(async () => {
      const result = await lookUp(name);
      if (server.trim() === name) check = result;
    }, 600);
  });
  const ready = $derived(check?.state === "ok" && check.for === server.trim());

  onMount(async () => {
    if (!hasBackend) return void (view = "signed-out");
    // Never a full sign-out here: that would delete this device's keys on a false alarm.
    onSessionEnded(async () => {
      await endSession().catch(() => {});
      account = null;
      view = "signed-out";
      error = tr("Your session ended. Please sign in again.");
    });
    try {
      account = await restoreSession();
    } catch (e) {
      error = String(e);
    }
    view = account ? "signed-in" : "signed-out";
  });

  async function signIn(event: SubmitEvent) {
    event.preventDefault();
    error = null;
    if (!ready) return;
    view = "waiting";
    try {
      account = await login(server.trim());
      view = "signed-in";
    } catch (e) {
      if (view === "waiting") error = String(e);
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
    account = null;
    view = "signed-out";
  }
</script>

{#if view === "signed-in" && account}
  <ChatsScreen {account} onSignOut={signOut} />
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
