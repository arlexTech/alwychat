<script lang="ts">
  import Ban from "@lucide/svelte/icons/ban";
  import Copy from "@lucide/svelte/icons/copy";
  import MessageCircle from "@lucide/svelte/icons/message-circle";
  import ShieldOff from "@lucide/svelte/icons/shield-off";
  import UserMinus from "@lucide/svelte/icons/user-minus";
  import UserX from "@lucide/svelte/icons/user-x";
  import {
    ignoredUsers,
    moderate,
    openDm,
    setIgnored,
    setUserPower,
    userProfile,
    type MemberInfo,
    type RoomDetails,
    type UserResult,
  } from "$lib/backend";
  import { presence } from "$lib/stores/presence.svelte";
  import { profiles } from "$lib/stores/profiles.svelte";
  import { can, roleName, ROLES } from "$lib/roles";
  import Avatar from "./Avatar.svelte";
  import Page from "./Page.svelte";

  /** Someone's profile; with `room`, also their role there and moderation. */
  let {
    userId,
    member = null,
    room = null,
    onClose,
    onOpenChat,
    onChanged,
  }: {
    userId: string;
    member?: MemberInfo | null;
    room?: RoomDetails | null;
    onClose: () => void;
    onOpenChat?: (roomId: string) => void;
    onChanged?: () => void;
  } = $props();

  let profile = $state<UserResult | null>(null);
  let blocked = $state(false);
  let busy = $state(false);
  let error = $state<string | null>(null);
  let notice = $state<string | null>(null);
  let confirm = $state<"kick" | "ban" | null>(null);
  let reason = $state("");

  $effect(() => {
    userProfile(userId).then((p) => (profile = p)).catch(() => {});
    ignoredUsers().then((list) => (blocked = list.some((u) => u.user_id === userId))).catch(() => {});
    profiles.refresh([userId], true);
  });

  const name = $derived(member?.name ?? profile?.name ?? userId);
  const avatar = $derived(member?.avatar ?? profile?.avatar ?? null);
  const isMe = $derived(member?.is_me ?? false);
  const myPower = $derived(room?.my_power ?? 0);
  // Moderation needs a higher level than theirs (and the permission).
  const above = $derived(!!room && !!member && myPower > member.power);
  const canRole = $derived(!!room && above && can(room.levels, myPower, "m.room.power_levels"));
  const canKick = $derived(!!room && above && member?.membership !== "ban" && can(room.levels, myPower, "kick"));
  const canBan = $derived(!!room && above && can(room.levels, myPower, "ban"));

  async function act(what: () => Promise<unknown>, done?: string) {
    busy = true;
    error = null;
    try {
      await what();
      if (done) notice = done;
      onChanged?.();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  const changeRole = (power: number) =>
    act(() => setUserPower(room!.id, userId, power), `${name} is now ${roleName(power).toLowerCase()}`);

  async function message() {
    await act(async () => onOpenChat?.(await openDm(userId)));
  }

  function runConfirm() {
    const kind = confirm!;
    confirm = null;
    void act(
      () => moderate(room!.id, userId, kind, reason || null),
      kind === "kick" ? `${name} was removed` : `${name} is banned`,
    );
    reason = "";
  }
</script>

<Page title={name} {onClose}>
  <div class="head">
    <Avatar {name} mxc={avatar} size={96} />
    <h2>{name}</h2>
    <button class="id" onclick={() => navigator.clipboard?.writeText(userId)} title="Copy">{userId} <Copy size={13} /></button>
    {#if profiles.statusLabel(userId)}<p class="status">{profiles.statusLabel(userId)}</p>{/if}
    {#if presence.online(userId)}<p class="online">Online</p>{/if}
    {#if room && member}<p class="role">{roleName(member.power)}{member.membership === "invite" ? " · invited" : member.membership === "ban" ? " · banned" : ""}</p>{/if}
  </div>

  {#if !isMe && onOpenChat}
    <button class="m-row" onclick={message} disabled={busy}>
      <span class="m-icon"><MessageCircle size={20} /></span><span class="m-text"><strong>Message</strong></span>
    </button>
  {/if}

  {#if canRole && member}
    <h3 class="m-section">Role in {room?.name}</h3>
    <div class="m-seg">
      {#each ROLES as r (r.power)}
        <button class:on={member.power === r.power} disabled={busy || r.power > myPower} onclick={() => changeRole(r.power)}>{r.label}</button>
      {/each}
    </div>
  {/if}

  {#if canKick || canBan}
    <h3 class="m-section">Moderation</h3>
    {#if canKick && member?.membership !== "ban"}
      <button class="m-row danger" onclick={() => (confirm = "kick")} disabled={busy}>
        <span class="m-icon"><UserMinus size={20} /></span><span class="m-text"><strong>{member?.membership === "invite" ? "Cancel invite" : "Remove from chat"}</strong><small>They can come back if invited (or if anyone can join)</small></span>
      </button>
    {/if}
    {#if canBan}
      {#if member?.membership === "ban"}
        <button class="m-row" onclick={() => act(() => moderate(room!.id, userId, "unban"), `${name} is unbanned`)} disabled={busy}>
          <span class="m-icon"><ShieldOff size={20} /></span><span class="m-text"><strong>Unban</strong></span>
        </button>
      {:else}
        <button class="m-row danger" onclick={() => (confirm = "ban")} disabled={busy}>
          <span class="m-icon"><Ban size={20} /></span><span class="m-text"><strong>Ban</strong><small>Removed, and can't come back</small></span>
        </button>
      {/if}
    {/if}
    {#if confirm}
      <div class="confirm">
        <p>{confirm === "kick" ? `Remove ${name}?` : `Ban ${name}?`}</p>
        <input class="m-input" bind:value={reason} placeholder="Reason (optional, they'll see it)" />
        <div class="m-buttons">
          <button class="m-btn plain" onclick={() => (confirm = null)}>Cancel</button>
          <button class="m-btn danger" onclick={runConfirm}>{confirm === "kick" ? "Remove" : "Ban"}</button>
        </div>
      </div>
    {/if}
  {/if}

  {#if !isMe}
    <h3 class="m-section">Privacy</h3>
    <button class="m-row" class:danger={!blocked} onclick={() => act(async () => { await setIgnored(userId, !blocked); blocked = !blocked; }, blocked ? `${name} is unblocked` : `${name} is blocked`)} disabled={busy}>
      <span class="m-icon"><UserX size={20} /></span>
      <span class="m-text"><strong>{blocked ? "Unblock" : "Block"}</strong><small>{blocked ? "See their messages and invites again" : "Hide their messages and invites, everywhere"}</small></span>
    </button>
  {/if}

  {#if notice}<p class="m-hint">{notice}</p>{/if}
  {#if error}<p class="m-error">{error}</p>{/if}
</Page>

<style>
  .head {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    margin: 8px 0 18px;
    text-align: center;
  }
  h2 {
    margin: 10px 0 0;
  }
  .id {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    border: none;
    background: none;
    color: var(--muted);
    font: inherit;
    font-size: 0.9rem;
    cursor: pointer;
  }
  p {
    margin: 0;
  }
  .status {
    color: var(--text);
  }
  .online {
    color: #31d158;
    font-size: 0.9rem;
  }
  .role {
    color: var(--muted);
    font-size: 0.9rem;
  }
  .confirm {
    margin: 8px 0;
    padding: 14px;
    border-radius: 14px;
    background: var(--surface);
  }
  .confirm p {
    margin-bottom: 10px;
    font-weight: 600;
  }
</style>
