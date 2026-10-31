<script lang="ts">
  import Delete from "@lucide/svelte/icons/delete";
  import Fingerprint from "@lucide/svelte/icons/fingerprint";
  import ScanFace from "@lucide/svelte/icons/scan-face";
  import ArrowRight from "@lucide/svelte/icons/arrow-right";

  /**
   * A PIN keypad (also typed on a keyboard). `onSubmit` returns an error to show, or null when
   * accepted; the dots clear either way.
   */
  let {
    title,
    hint = null,
    biometry = null,
    onBiometric,
    onSubmit,
  }: {
    title: string;
    hint?: string | null;
    /** Shows the fingerprint / face key. */
    biometry?: "fingerprint" | "face" | null;
    onBiometric?: () => void;
    onSubmit: (pin: string) => Promise<string | null> | string | null;
  } = $props();

  const MAX = 12;
  let pin = $state("");
  let error = $state<string | null>(null);
  let busy = $state(false);
  let shake = $state(false);

  function press(digit: string) {
    if (busy || pin.length >= MAX) return;
    error = null;
    pin += digit;
  }

  async function submit() {
    if (busy || pin.length < 4) return;
    busy = true;
    const tried = pin;
    pin = "";
    try {
      error = await onSubmit(tried);
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
    if (error) {
      shake = true;
      setTimeout(() => (shake = false), 400);
      navigator.vibrate?.(60);
    }
  }

  function onKeydown(e: KeyboardEvent) {
    if (/^\d$/.test(e.key)) press(e.key);
    else if (e.key === "Backspace") pin = pin.slice(0, -1);
    else if (e.key === "Enter") void submit();
    else return;
    e.preventDefault();
  }
</script>

<svelte:window onkeydown={onKeydown} />

<div class="pad">
  <h2>{title}</h2>
  <p class="hint" class:error={!!error}>{error ?? hint ?? " "}</p>
  <div class="dots" class:shake aria-label="{pin.length} digits entered">
    {#each Array(Math.max(4, pin.length)) as _, i (i)}
      <span class:on={i < pin.length}></span>
    {/each}
  </div>
  <div class="keys">
    {#each ["1", "2", "3", "4", "5", "6", "7", "8", "9"] as d (d)}
      <button onclick={() => press(d)}>{d}</button>
    {/each}
    {#if biometry && onBiometric}
      <button class="icon" onclick={onBiometric} aria-label="Use {biometry}">
        {#if biometry === "face"}<ScanFace size={26} />{:else}<Fingerprint size={26} />{/if}
      </button>
    {:else}
      <button class="icon" onclick={() => (pin = pin.slice(0, -1))} aria-label="Delete" disabled={!pin}><Delete size={24} /></button>
    {/if}
    <button onclick={() => press("0")}>0</button>
    {#if pin.length >= 4}
      <button class="icon go" onclick={submit} aria-label="OK" disabled={busy}><ArrowRight size={24} /></button>
    {:else if biometry && onBiometric}
      <button class="icon" onclick={() => (pin = pin.slice(0, -1))} aria-label="Delete" disabled={!pin}><Delete size={24} /></button>
    {:else}
      <span></span>
    {/if}
  </div>
</div>

<style>
  .pad {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 14px;
    width: min(320px, 90vw);
  }
  h2 {
    margin: 0;
    font-size: 1.3rem;
    font-weight: 600;
  }
  .hint {
    margin: 0;
    min-height: 1.3em;
    color: var(--muted);
    font-size: 0.9rem;
    text-align: center;
  }
  .hint.error {
    color: var(--danger);
  }
  .dots {
    display: flex;
    gap: 14px;
    margin: 6px 0 18px;
  }
  .dots span {
    width: 14px;
    height: 14px;
    border: 2px solid var(--muted);
    border-radius: 50%;
    transition: background 0.12s var(--ease), border-color 0.12s var(--ease);
  }
  .dots span.on {
    border-color: transparent;
    background: var(--accent);
  }
  .shake {
    animation: shake 0.35s;
  }
  @keyframes shake {
    20%,
    60% {
      transform: translateX(-8px);
    }
    40%,
    80% {
      transform: translateX(8px);
    }
  }
  .keys {
    display: grid;
    grid-template-columns: repeat(3, 72px);
    gap: 16px 28px;
  }
  .keys button,
  .keys span {
    width: 72px;
    height: 72px;
  }
  .keys button {
    display: grid;
    place-items: center;
    border: none;
    border-radius: 50%;
    background: var(--surface);
    color: inherit;
    font-size: 1.6rem;
    cursor: pointer;
    transition: transform 0.1s var(--ease), background 0.15s var(--ease);
  }
  .keys button:active {
    transform: scale(0.92);
    background: var(--surface-2);
  }
  .keys button.icon {
    background: none;
  }
  .keys button.go {
    background: var(--accent);
    color: white;
  }
  .keys button:disabled {
    opacity: 0.35;
  }
</style>
