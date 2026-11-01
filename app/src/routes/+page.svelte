<script lang="ts">
  import { onMount } from "svelte";
  import { fade, fly } from "svelte/transition";
  import { cancelLogin, endSession, hasBackend, login, logout, onSessionEnded, restoreSession, type Account } from "$lib/backend";
  import ChatsScreen from "$lib/components/ChatsScreen.svelte";

  type View = "loading" | "signed-out" | "waiting" | "signed-in";

  let view = $state<View>("loading");
  /** The local restore is quick: only say something if it isn't. */
  let slow = $state(false);
  setTimeout(() => (slow = true), 1200);
  let server = $state("example.org");
  let account = $state<Account | null>(null);
  let error = $state<string | null>(null);

  onMount(async () => {
    if (!hasBackend) return void (view = "signed-out");
    // Never a full sign-out here: that would delete this device's keys on a false alarm.
    onSessionEnded(async () => {
      await endSession().catch(() => {});
      account = null;
      view = "signed-out";
      error = "Your session ended. Please sign in again.";
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
  <h1>AlwyChat</h1>

  {#if !hasBackend}
    <p class="error">
      This page is the app's interface only. Run <code>pnpm tauri dev</code> and use the AlwyChat
      window (a web version comes later).
    </p>
  {/if}

  {#key view}
    <section in:fly={{ y: 12, duration: 250 }}>
      {#if view === "loading"}
        {#if slow}<p class="muted" in:fade>Opening your chats…</p>{/if}
      {:else if view === "signed-out"}
        <form onsubmit={signIn}>
          <label for="server">Your server</label>
          <div class="row">
            <input id="server" bind:value={server} autocapitalize="off" spellcheck="false" />
            <button disabled={!hasBackend || !server.trim()}>Sign in</button>
          </div>
          <p class="muted">Your browser will open your server's sign-in page.</p>
        </form>
      {:else if view === "waiting"}
        <div class="waiting">
          <div class="spinner" aria-hidden="true"></div>
          <p>Finish signing in in your browser…</p>
          <p class="muted">This window will update by itself.</p>
          <button class="ghost" onclick={cancelSignIn}>Cancel</button>
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
  h1 {
    font-size: 2rem;
    background: linear-gradient(90deg, #7b5cff, #ff5c8a);
    background-clip: text;
    color: transparent;
  }
  label {
    display: block;
    margin-bottom: 0.5rem;
    color: #8a90a0;
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
    border: 1px solid #2a2f3a;
    background: #181b22;
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
    color: #8a90a0;
    margin: 0.4rem 0 0;
  }
  .waiting {
    text-align: center;
  }
  .ghost {
    margin-top: 1.5rem;
    background: #181b22;
    border: 1px solid #2a2f3a;
  }
  .spinner {
    width: 36px;
    height: 36px;
    margin: 1rem auto;
    border-radius: 50%;
    border: 3px solid #2a2f3a;
    border-top-color: #ff5c8a;
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .error {
    color: #ff6b6b;
  }
  main.hidden {
    visibility: hidden;
  }
</style>
