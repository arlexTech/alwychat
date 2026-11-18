<script lang="ts">
  // Ring someone into the call we're in (voice channel, group call), from the call view: only
  // their devices ring. Lists the chat's members who aren't in the call.
  import { tr, trn } from "$lib/i18n/index.svelte";
  import { onBack } from "$lib/back.svelte";
  import { fly } from "svelte/transition";
  import X from "@lucide/svelte/icons/x";
  import PhoneCall from "@lucide/svelte/icons/phone-call";
  import { call, type Invite } from "$lib/stores/call.svelte";
  import Avatar from "./Avatar.svelte";

  let { onClose }: { onClose: () => void } = $props();
  $effect(() => onBack(onClose));

  let filter = $state("");
  const inCall = $derived(new Set(call.tiles.map((t) => t.userId)));
  const people = $derived(
    call.people
      .filter((m) => !m.is_me && !inCall.has(m.user_id))
      .filter((m) => !filter.trim() || `${m.name} ${m.user_id}`.toLowerCase().includes(filter.trim().toLowerCase()))
      .sort((a, b) => a.name.localeCompare(b.name)),
  );

  function statusOf(invite: Invite | undefined) {
    if (!invite) return null;
    switch (invite.status) {
      case "calling":
        return tr("Calling…");
      case "ringing":
        return invite.devices.length > 1 ? trn(invite.devices.length, "Ringing on {n} device", "Ringing on {n} devices") : tr("Ringing…");
      case "declined":
        return tr("Declined");
      case "no_answer":
        return tr("No answer");
      case "failed":
        return tr("Couldn't ring");
    }
  }
  const busy = (invite: Invite | undefined) => invite?.status === "calling" || invite?.status === "ringing";
</script>

<div class="panel" transition:fly={{ y: 24, duration: 160 }}>
  <header>
    <strong>{tr("Ring someone")}</strong>
    <button class="icon" onclick={onClose} aria-label={tr("Close")}><X size={20} /></button>
  </header>
  {#if call.people.length > 8}
    <input class="filter" type="search" placeholder={tr("Search people")} bind:value={filter} />
  {/if}
  {#each people as person (person.user_id)}
    {@const invite = call.invites[person.user_id]}
    {@const status = statusOf(invite)}
    <div class="person">
      <Avatar name={person.name} mxc={person.avatar} size={40} />
      <span class="text">
        <strong>{person.name}</strong>
        <small class:ringing={invite?.status === "ringing"}>{status ?? person.user_id}</small>
      </span>
      <button class="ring" class:busy={busy(invite)} disabled={busy(invite)} onclick={() => call.ringPerson(person.user_id)}>
        <PhoneCall size={16} />{invite && !busy(invite) ? tr("Ring again") : tr("Ring")}
      </button>
    </div>
  {:else}
    <p class="note">{call.people.length ? tr("Everyone in this chat is already in the call.") : tr("Loading…")}</p>
  {/each}
  <p class="hint">{tr("Only people in this chat can be rung. Apps other than AlwyChat don't say when they ring, so they stay at “Calling…”.")}</p>
</div>

<style>
  .panel {
    position: absolute;
    left: 12px;
    right: 12px;
    bottom: calc(110px + env(safe-area-inset-bottom));
    max-width: 520px;
    margin: 0 auto;
    max-height: min(60vh, 520px);
    overflow-y: auto;
    z-index: 5;
    padding: 12px;
    border-radius: 18px;
    background: var(--surface);
    color: var(--text);
    box-shadow: 0 10px 40px rgb(0 0 0 / 0.45);
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 8px;
  }
  /* Close: a plain round icon, like the soundboard's (it had the browser's grey box). */
  .icon {
    display: grid;
    place-items: center;
    width: 36px;
    height: 36px;
    border: none;
    border-radius: 50%;
    background: none;
    color: inherit;
    cursor: pointer;
  }
  .icon:hover,
  .icon:focus-visible {
    background: var(--surface-2);
  }
  .filter {
    width: 100%;
    box-sizing: border-box;
    margin-bottom: 8px;
    padding: 8px 12px;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--bg);
    color: inherit;
    font: inherit;
  }
  .person {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 2px;
  }
  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .text strong,
  .text small {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .text small {
    color: var(--muted);
  }
  .text small.ringing {
    color: #31d158;
  }
  .ring {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 7px 12px;
    border: none;
    border-radius: 999px;
    background: #31d158;
    color: white;
    font: inherit;
    font-size: 0.9rem;
    cursor: pointer;
    flex-shrink: 0;
  }
  .ring.busy {
    background: var(--surface-hover);
    color: var(--muted);
    cursor: default;
  }
  .note {
    margin: 8px 2px;
    color: var(--muted);
  }
  .hint {
    margin: 12px 2px 0;
    font-size: 0.8rem;
    color: var(--muted);
  }
</style>
