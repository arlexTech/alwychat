<script lang="ts">
  import "../app.css";
  import { prefs } from "$lib/stores/prefs.svelte";

  prefs.load().catch(() => {});

  let { children } = $props();
</script>

{@render children()}
