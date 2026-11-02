<script lang="ts">
  import { tr, trn } from "$lib/i18n/index.svelte";
  import ArrowLeft from "@lucide/svelte/icons/arrow-left";
  import { acceptInvite, inviteInfo, leaveRoom, setIgnored, type InviteInfo, type RoomSummary } from "$lib/backend";
  import Avatar from "./Avatar.svelte";

  /** An invite opened from the chat list: who invited us, and Accept / Decline. */
  let {
    room,
    onBack,
    onAccepted,
    onDeclined,
  }: {
    room: RoomSummary;
    onBack?: () => void;
    onAccepted: () => void;
    onDeclined: () => void;
  } = $props();

  let info = $state<InviteInfo | null>(null);
  let busy = $state<"accept" | "decline" | "block" | null>(null);
  let error = $state<string | null>(null);

  $effect(() => {
    const id = room.id;
    info = null;
    inviteInfo(id)
      .then((i) => room.id === id && (info = i))
      .catch(() => {});
  });

  const inviter = $derived(info?.inviter_name ?? info?.inviter_id ?? tr("Someone"));
  const what = $derived(room.is_space || info?.is_space ? "space" : room.is_dm ? "chat" : "group");

  async function run(kind: "accept" | "decline" | "block") {
    busy = kind;
    error = null;
    try {
      if (kind === "accept") {
        await acceptInvite(room.id);
        onAccepted();
      } else {
        await leaveRoom(room.id);
        if (kind === "block" && info) await setIgnored(info.inviter_id, true);
        onDeclined();
      }
    } catch (e) {
      error = String(e);
    } finally {
      busy = null;
    }
  }
</script>

<div class="invite">
  {#if onBack}<button class="back" onclick={onBack} aria-label={tr("Back")}><ArrowLeft size={22} /></button>{/if}
  <div class="card">
    <Avatar name={room.name} mxc={room.avatar} size={96} square={room.is_space} />
    <h2>{room.name}</h2>
    {#if info?.topic}<p class="topic">{info.topic}</p>{/if}
    <p class="who">
      {#if info?.inviter_avatar !== undefined}<Avatar name={inviter} mxc={info?.inviter_avatar ?? null} size={22} />{/if}
      <span><strong>{inviter}</strong> {tr("invited you to {value}", { value: room.is_dm ? "chat" : `this ${what}` })}</span>
    </p>
    {#if info && !room.is_dm && info.members}<p class="members">{trn(info.members, "{n} member", "{n} members")}</p>{/if}
    {#if error}<p class="m-error">{error}</p>{/if}
    <div class="m-buttons">
      <button class="m-btn plain" disabled={!!busy} onclick={() => run("decline")}>{busy === "decline" ? tr("Declining…") : tr("Decline")}</button>
      <button class="m-btn" disabled={!!busy} onclick={() => run("accept")}>{busy === "accept" ? tr("Joining…") : tr("Accept")}</button>
    </div>
    {#if info}
      <button class="block" disabled={!!busy} onclick={() => run("block")}>{tr("Decline and block {inviter}", { inviter: inviter })}</button>
    {/if}
  </div>
</div>

<style>
  .invite {
    position: relative;
    display: grid;
    height: 100%;
    place-items: center;
    padding: 24px;
    box-sizing: border-box;
  }
  .back {
    position: absolute;
    top: 10px;
    left: 10px;
    display: grid;
    width: 42px;
    height: 42px;
    place-items: center;
    border: none;
    border-radius: 50%;
    background: none;
    color: var(--text);
    cursor: pointer;
  }
  .card {
    display: flex;
    flex-direction: column;
    align-items: center;
    width: min(420px, 100%);
    text-align: center;
  }
  h2 {
    margin: 16px 0 4px;
  }
  .topic {
    margin: 0 0 8px;
    color: var(--muted);
  }
  .who {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 8px 0 0;
  }
  .members {
    margin: 4px 0 0;
    color: var(--muted);
    font-size: 0.9rem;
  }
  .m-buttons {
    width: 100%;
    margin-top: 24px;
  }
  .block {
    margin-top: 14px;
    border: none;
    background: none;
    color: var(--danger);
    font: inherit;
    font-size: 0.9rem;
    cursor: pointer;
  }
</style>
