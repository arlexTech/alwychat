<script lang="ts">
  import { onMount } from "svelte";
  import { fade } from "svelte/transition";
  import { unlockApp, unlockBiometric } from "$lib/backend";
  import { appLock } from "$lib/stores/applock.svelte";
  import { prefs } from "$lib/stores/prefs.svelte";
  import PinPad from "./PinPad.svelte";

  /** Covers everything until the PIN (or fingerprint / face) is given. */
  const biometry = $derived(prefs.settings?.app_lock_biometric !== false ? appLock.state.biometry : null);

  async function biometric() {
    if (await unlockBiometric().catch(() => false)) appLock.unlock();
  }

  async function submit(pin: string) {
    const r = await unlockApp(pin);
    if (r.result === "ok") {
      appLock.unlock();
      return null;
    }
    if (r.result === "wait") return `Too many tries. Wait ${r.seconds} s`;
    return "Wrong PIN";
  }

  onMount(() => {
    if (biometry) void biometric();
  });
</script>

<div class="lock" out:fade={{ duration: 160 }}>
  <PinPad title="AlwyChat is locked" hint="Enter your PIN" {biometry} onBiometric={biometric} onSubmit={submit} />
</div>

<style>
  .lock {
    position: fixed;
    inset: 0;
    z-index: 1000;
    display: grid;
    place-items: center;
    padding: env(safe-area-inset-top) 0 env(safe-area-inset-bottom);
    background: var(--bg);
  }
</style>
