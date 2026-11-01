<script lang="ts">
  import Link from "@lucide/svelte/icons/link";
  import UserPlus from "@lucide/svelte/icons/user-plus";
  import { onBack } from "$lib/back.svelte";
  import { inviteUser, members as loadMembers, type MemberInfo, type RoomDetails, type UserResult } from "$lib/backend";
  import { presence } from "$lib/stores/presence.svelte";
  import { can, roleName } from "$lib/roles";
  import Avatar from "./Avatar.svelte";
  import Page from "./Page.svelte";
  import UserPage from "./UserPage.svelte";
  import UserPicker from "./UserPicker.svelte";

  /** Members of a chat or space by role, invited and banned ones, and inviting people. */
  let {
    room,
    startInviting = false,
    onClose,
    onOpenChat,
    onChanged,
  }: {
    room: RoomDetails;
    startInviting?: boolean;
    onClose: () => void;
    onOpenChat?: (roomId: string) => void;
    onChanged?: () => void;
  } = $props();

  let list = $state<MemberInfo[]>([]);
  let loaded = $state(false);
  let query = $state("");
  let open = $state<MemberInfo | null>(null);
  // svelte-ignore state_referenced_locally
  let inviting = $state(startInviting);
  let toInvite = $state<UserResult[]>([]);
  let busy = $state(false);
  let error = $state<string | null>(null);
  let notice = $state<string | null>(null);

  function reload() {
    loadMembers(room.id)
      .then((m) => ((list = m), (loaded = true)))
      .catch((e) => ((error = String(e)), (loaded = true)));
  }
  $effect(reload);
  $effect(() => {
    if (open) return onBack(() => (open = null));
  });
  $effect(() => {
    if (inviting && !startInviting) return onBack(() => (inviting = false));
  });

  const canInvite = $derived(can(room.levels, room.my_power, "invite"));
  const shown = $derived.by(() => {
    const q = query.trim().toLowerCase();
    return q ? list.filter((m) => m.name.toLowerCase().includes(q) || m.user_id.toLowerCase().includes(q)) : list;
  });
  const groups = $derived([
    { title: "Admins", people: shown.filter((m) => m.membership === "join" && m.power >= 100) },
    { title: "Moderators", people: shown.filter((m) => m.membership === "join" && m.power >= 50 && m.power < 100) },
    { title: "Members", people: shown.filter((m) => m.membership === "join" && m.power < 50) },
    { title: "Asking to join", people: shown.filter((m) => m.membership === "knock") },
    { title: "Invited", people: shown.filter((m) => m.membership === "invite") },
    { title: "Banned", people: shown.filter((m) => m.membership === "ban") },
  ]);
  const link = $derived(
    `https://matrix.to/#/${encodeURIComponent(room.alias ?? room.id)}${room.alias ? "" : `?via=${room.id.split(":")[1]}`}`,
  );

  async function sendInvites() {
    busy = true;
    error = null;
    const failed: string[] = [];
    for (const u of toInvite) {
      await inviteUser(room.id, u.user_id).catch((e) => failed.push(`${u.name}: ${e}`));
    }
    busy = false;
    if (failed.length) error = failed.join("\n");
    else notice = `Invited ${toInvite.map((u) => u.name).join(", ")}`;
    toInvite = [];
    if (!failed.length) inviting = false;
    reload();
    onChanged?.();
  }

  function copyLink() {
    navigator.clipboard?.writeText(link).then(() => (notice = "Link copied")).catch(() => {});
  }
</script>

{#snippet invitePanel()}
  <UserPicker multiple bind:picked={toInvite} exclude={list.filter((m) => m.membership === "join" || m.membership === "invite").map((m) => m.user_id)} />
  <div class="m-buttons sticky">
    <button class="m-btn" disabled={!toInvite.length || busy} onclick={sendInvites}>
      {busy ? "Inviting…" : toInvite.length ? `Invite ${toInvite.length}` : "Invite"}
    </button>
  </div>
{/snippet}

<Page title={inviting ? `Invite to ${room.name}` : `Members · ${room.members}`} onClose={inviting && !startInviting ? () => (inviting = false) : onClose}>
  {#if inviting}
    {@render invitePanel()}
  {:else}
    {#if canInvite}
      <button class="m-row" onclick={() => (inviting = true)}>
        <span class="m-icon"><UserPlus size={20} /></span><span class="m-text"><strong>Invite people</strong></span>
      </button>
    {/if}
    {#if room.join_rule === "public" || room.alias}
      <button class="m-row" onclick={copyLink}>
        <span class="m-icon"><Link size={20} /></span><span class="m-text"><strong>Copy link</strong><small>{link}</small></span>
      </button>
    {/if}
    {#if list.length > 8}<input class="m-input search" type="search" placeholder="Find a member" bind:value={query} />{/if}
    {#each groups as g (g.title)}
      {#if g.people.length}
        <h3 class="m-section">{g.title} · {g.people.length}</h3>
        {#each g.people as m (m.user_id)}
          <button class="m-row" onclick={() => (open = m)}>
            <span class="av">
              <Avatar name={m.name} mxc={m.avatar} size={40} />
              {#if presence.online(m.user_id)}<span class="dot"></span>{/if}
            </span>
            <span class="m-text"><strong>{m.name}{m.is_me ? " (you)" : ""}</strong><small>{m.user_id}</small></span>
            {#if m.power !== 0 && m.power !== 50 && m.power !== 100}<span class="m-end">{roleName(m.power)}</span>{/if}
          </button>
        {/each}
      {/if}
    {/each}
    {#if !loaded}<p class="m-hint">Loading…</p>{/if}
  {/if}
  {#if notice}<p class="m-hint">{notice}</p>{/if}
  {#if error}<p class="m-error">{error}</p>{/if}
</Page>

{#if open}
  <UserPage
    userId={open.user_id}
    member={open}
    {room}
    onClose={() => (open = null)}
    {onOpenChat}
    onChanged={() => (reload(), onChanged?.())}
  />
{/if}

<style>
  .search {
    margin: 8px 0;
  }
  .av {
    position: relative;
    display: flex;
  }
  .dot {
    position: absolute;
    right: 0;
    bottom: 0;
    width: 11px;
    height: 11px;
    border: 2px solid var(--bg);
    border-radius: 50%;
    background: #31d158;
  }
  .sticky {
    position: sticky;
    bottom: 0;
    padding: 12px 0 6px;
    background: var(--bg);
  }
</style>
