<script lang="ts">
  import { tr } from "$lib/i18n/index.svelte";
  // Discord-style menu for someone in a call or a voice channel: right-click (or click) on a
  // computer opens it at the pointer, a tap on a phone opens it as a bottom sheet.
  import Copy from "@lucide/svelte/icons/copy";
  import Headphones from "@lucide/svelte/icons/headphones";
  import HeadphoneOff from "@lucide/svelte/icons/headphone-off";
  import MessageCircle from "@lucide/svelte/icons/message-circle";
  import Mic from "@lucide/svelte/icons/mic";
  import MicOff from "@lucide/svelte/icons/mic-off";
  import UserRound from "@lucide/svelte/icons/user-round";
  import Volume2 from "@lucide/svelte/icons/volume-2";
  import VolumeX from "@lucide/svelte/icons/volume-x";
  import { fade, fly } from "svelte/transition";
  import { openDm } from "$lib/backend";
  import { onBack } from "$lib/back.svelte";
  import { call } from "$lib/stores/call.svelte";
  import { nav } from "$lib/stores/nav.svelte";
  import Avatar from "./Avatar.svelte";

  const menu = $derived(call.menu!);
  const person = $derived(menu.person);
  const close = () => (call.menu = null);
  $effect(() => onBack(close));

  // The slider keeps its own value while dragged; the store follows it.
  let volume = $state(Math.round(call.volumeOf(call.menu!.person.userId) * 100));
  const mutedForMe = $derived(call.isMutedForMe(person.userId));
  let copied = $state(false);
  let error = $state<string | null>(null);

  function setVolume(value: number) {
    volume = value;
    call.setVolume(person.userId, value / 100);
    // Moving the slider means they should be heard again.
    if (value > 0 && mutedForMe) call.toggleMuteForMe(person.userId);
  }

  function showProfile() {
    call.profile = person.userId;
    call.minimized = true;
    close();
  }

  async function message() {
    try {
      const roomId = await openDm(person.userId);
      call.minimized = true;
      close();
      nav.openChat(roomId);
    } catch (e) {
      error = String(e);
    }
  }

  function copyId() {
    void navigator.clipboard?.writeText(person.userId);
    copied = true;
    setTimeout(close, 600);
  }

  // Kept inside the window at the pointer.
  let box = $state<HTMLElement>();
  const left = $derived(Math.max(8, Math.min(menu.x, innerWidth - (box?.offsetWidth ?? 260) - 8)));
  const top = $derived(Math.max(8, Math.min(menu.y, innerHeight - (box?.offsetHeight ?? 320) - 8)));
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div
  class="catcher"
  class:dim={menu.sheet}
  onclick={close}
  oncontextmenu={(e) => (e.preventDefault(), close())}
  transition:fade={{ duration: 120 }}
></div>
<div
  bind:this={box}
  class="menu"
  class:sheet={menu.sheet}
  style:left={menu.sheet ? null : `${left}px`}
  style:top={menu.sheet ? null : `${top}px`}
  role="menu"
  tabindex="-1"
  aria-label={person.name}
  oncontextmenu={(e) => e.preventDefault()}
  transition:fly={menu.sheet ? { y: 40, duration: 200 } : { y: -4, duration: 110 }}
>
  {#if menu.sheet}<div class="grip"></div>{/if}
  <div class="who">
    <Avatar name={person.name} mxc={person.avatar} size={menu.sheet ? 44 : 32} />
    <span>
      <strong>{person.name}</strong>
      <small>{person.userId}</small>
    </span>
  </div>

  <button role="menuitem" onclick={showProfile}><UserRound size={18} />{tr("Profile")}</button>
  {#if !person.isMe}
    <button role="menuitem" onclick={message}><MessageCircle size={18} />{tr("Message")}</button>
  {/if}

  {#if person.live}
    <hr />
    {#if person.isMe}
      <button role="menuitemcheckbox" aria-checked={!call.micOn} onclick={() => call.toggleMic()}>
        {#if call.micOn}<Mic size={18} />{tr("Mute")}{:else}<MicOff size={18} />{tr("Unmute")}{/if}
      </button>
      <button role="menuitemcheckbox" aria-checked={call.deafened} onclick={() => call.toggleDeafen()}>
        {#if call.deafened}<HeadphoneOff size={18} />{tr("Undeafen")}{:else}<Headphones size={18} />{tr("Deafen")}{/if}
      </button>
    {:else}
      <button role="menuitemcheckbox" aria-checked={mutedForMe} onclick={() => call.toggleMuteForMe(person.userId)}>
        {#if mutedForMe}<Volume2 size={18} />{tr("Unmute for me")}{:else}<VolumeX size={18} />{tr("Mute for me")}{/if}
        {#if mutedForMe}<span class="check" aria-hidden="true"></span>{/if}
      </button>
      <!-- Native calls (Linux desktop) can only silence someone for now. -->
      {#if !call.native}
      <div class="volume">
        <span class="label"><Volume2 size={18} />{tr("User volume")} <b>{mutedForMe ? 0 : volume}%</b></span>
        <input
          type="range"
          min="0"
          max="100"
          step="1"
          value={mutedForMe ? 0 : volume}
          oninput={(e) => setVolume(Number(e.currentTarget.value))}
          aria-label={tr("Volume of {name}", { name: person.name })}
        />
      </div>
      {/if}
    {/if}
  {/if}

  <hr />
  <button role="menuitem" onclick={copyId}><Copy size={18} />{copied ? tr("Copied") : tr("Copy user ID")}</button>
  {#if error}<p class="error">{error}</p>{/if}
</div>

<style>
  .catcher {
    position: fixed;
    inset: 0;
    z-index: 70;
  }
  .catcher.dim {
    background: rgb(0 0 0 / 0.45);
  }
  .menu {
    position: fixed;
    z-index: 71;
    display: flex;
    flex-direction: column;
    width: 260px;
    padding: 6px;
    border: 1px solid var(--border);
    border-radius: 12px;
    background: var(--surface-2);
    color: var(--text);
    box-shadow: 0 12px 32px rgb(0 0 0 / 0.45);
  }
  .menu.sheet {
    left: 50%;
    bottom: 0;
    width: min(520px, 100%);
    padding: 8px 12px calc(16px + env(safe-area-inset-bottom));
    border: none;
    border-radius: 22px 22px 0 0;
    background: var(--surface);
    box-shadow: 0 -12px 40px rgb(0 0 0 / 0.4);
    translate: -50% 0;
  }
  .grip {
    width: 36px;
    height: 4px;
    margin: 0 auto 10px;
    border-radius: 2px;
    background: var(--border);
  }
  .who {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 8px 8px;
  }
  .who span {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .who strong,
  .who small {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .who small {
    color: var(--muted);
    font-size: 0.78rem;
  }
  button {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 9px 10px;
    border: none;
    border-radius: 8px;
    background: none;
    color: inherit;
    font-size: 0.92rem;
    text-align: left;
    cursor: pointer;
  }
  .sheet button {
    padding: 13px 10px;
    font-size: 1rem;
  }
  button:hover,
  button:active {
    background: var(--surface-hover);
  }
  button :global(svg) {
    flex: none;
    color: var(--muted);
  }
  .check {
    width: 8px;
    height: 8px;
    margin-left: auto;
    border-radius: 50%;
    background: var(--danger);
  }
  hr {
    width: 100%;
    margin: 4px 0;
    border: none;
    border-top: 1px solid var(--border);
  }
  .volume {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 8px 10px 10px;
  }
  .label {
    display: flex;
    align-items: center;
    gap: 12px;
    font-size: 0.92rem;
  }
  .label :global(svg) {
    color: var(--muted);
  }
  .label b {
    margin-left: auto;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }
  /* The browser must not take a horizontal drag on it for a scroll or a swipe. */
  input[type="range"] {
    width: 100%;
    height: 28px;
    margin: 0;
    accent-color: var(--accent);
    touch-action: none;
    cursor: pointer;
  }
  .error {
    margin: 4px 10px;
    color: var(--danger);
    font-size: 0.85rem;
  }
</style>
