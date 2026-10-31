<script lang="ts">
  import "../app.css";
  import { prefs } from "$lib/stores/prefs.svelte";
  import { appLock } from "$lib/stores/applock.svelte";
  import LockScreen from "$lib/components/LockScreen.svelte";

  prefs.load().catch(() => {});
  appLock.start().catch(() => {});

  let { children } = $props();
</script>

{@render children()}
{#if appLock.checking}
  <div class="cover"></div>
{:else if appLock.locked}
  <LockScreen />
{/if}

<style>
  /* Until we know whether the app is locked, nothing shows. */
  .cover {
    position: fixed;
    inset: 0;
    z-index: 1000;
    background: var(--bg);
  }
</style>
