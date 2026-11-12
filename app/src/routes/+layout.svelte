<script lang="ts">
  import "../app.css";
  import { prefs } from "$lib/stores/prefs.svelte";
  import { emojiStyle } from "$lib/stores/emojiStyle.svelte";
  import { appLock } from "$lib/stores/applock.svelte";
  import LockScreen from "$lib/components/LockScreen.svelte";
  import TitleBar from "$lib/components/TitleBar.svelte";
  import { hasTitlebar } from "$lib/backend";

  prefs.load().catch(() => {});
  emojiStyle.start();
  appLock.start().catch(() => {});

  let { children } = $props();
</script>

{@render children()}
{#if appLock.checking}
  <div class="cover"></div>
{:else if appLock.locked}
  <LockScreen />
{/if}
{#if hasTitlebar}<TitleBar />{/if}

<style>
  /* Until we know whether the app is locked, nothing shows. */
  .cover {
    position: fixed;
    inset: 0;
    z-index: 1000;
    background: var(--bg);
  }
</style>
