<script lang="ts">
  import { onMount } from "svelte";
  import {
    roomDetails,
    roomMembers,
    setNickname,
    setQuickReaction,
    setTheme,
    type Member,
    type RoomDetails,
  } from "$lib/backend";
  import RoomManage from "./RoomManage.svelte";
  import UserPage from "./UserPage.svelte";
  import UserRound from "@lucide/svelte/icons/user-round";
  import Users from "@lucide/svelte/icons/users";
  import CaseSensitive from "@lucide/svelte/icons/case-sensitive";
  import Palette from "@lucide/svelte/icons/palette";
  import ChartColumn from "@lucide/svelte/icons/chart-column";
  import ImageUp from "@lucide/svelte/icons/image-up";
  import LayoutGrid from "@lucide/svelte/icons/layout-grid";
  import ShieldCheck from "@lucide/svelte/icons/shield-check";
  import CustomizePage from "./CustomizePage.svelte";
  import MembersPage from "./MembersPage.svelte";
  import Search from "@lucide/svelte/icons/search";
  import { nav } from "$lib/stores/nav.svelte";
  import { onBack } from "$lib/back.svelte";
  import { chat } from "$lib/stores/chat.svelte";
  import { chatList } from "$lib/stores/chatList.svelte";
  import { spaces } from "$lib/stores/spaces.svelte";
  import { DEFAULT_QUICK_REACTION, themeById } from "$lib/themes";
  import Avatar from "./Avatar.svelte";
  import Page from "./Page.svelte";
  import Images from "@lucide/svelte/icons/images";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";

  let {
    roomId,
    hd,
    onChooseHd,
    onSharedMedia,
    onSearch,
    onHistory,
    onClose,
    onLeft,
  }: {
    roomId: string;
    /** This chat's photo quality: null = app default. */
    hd: boolean | null;
    onChooseHd: (hd: boolean | null) => void;
    onSharedMedia: () => void;
    onSearch: () => void;
    onHistory: () => void;
    onClose: () => void;
    /** We left the chat: close it. */
    onLeft: () => void;
  } = $props();

  let details = $state<RoomDetails | null>(null);
  let sub = $state<"profile" | "members" | "nicknames" | "customize" | "privacy" | null>(null);
  $effect(() => {
    if (sub) return onBack(() => (sub = null));
  });
  function loadDetails() {
    roomDetails(roomId)
      .then((d) => (details = d))
      .catch((e) => (error = String(e)));
  }

  const summary = $derived(chatList.rooms.find((r) => r.id === roomId) ?? null);

  const style = $derived(chat.style);
  const canEdit = $derived(style?.can_edit ?? false);
  const currentTheme = $derived(themeById(style?.theme));
  const quick = $derived(style?.quick_reaction ?? DEFAULT_QUICK_REACTION);

  let members = $state<Member[]>([]);
  /** User ID whose nickname is being edited, and the draft. */
  let editing = $state<{ userId: string; value: string } | null>(null);
  let error = $state<string | null>(null);

  onMount(() => {
    loadDetails();
    roomMembers(roomId)
      .then((list) => (members = list.sort((a, b) => Number(b.is_me) - Number(a.is_me))))
      .catch((e) => (error = String(e)));
  });

  /** Show the change at once; the server's copy comes back through the chat's style stream. */
  async function apply(change: Partial<NonNullable<typeof chat.style>>, save: () => Promise<void>) {
    const previous = chat.style;
    if (previous) chat.style = { ...previous, ...change };
    error = null;
    try {
      await save();
    } catch (e) {
      chat.style = previous;
      error = String(e);
    }
  }

  function saveNickname(member: Member) {
    if (!editing) return;
    const value = editing.value.trim() || null;
    editing = null;
    if (member.is_me) {
      // Our per-room display name: shows in the timeline once the member event syncs back.
      error = null;
      setNickname(roomId, member.user_id, value)
        .then(() => (member.name = value ?? member.name))
        .catch((e) => (error = String(e)));
      return;
    }
    const nicknames = { ...(chat.style?.nicknames ?? {}) };
    if (value) nicknames[member.user_id] = value;
    else delete nicknames[member.user_id];
    apply({ nicknames }, () => setNickname(roomId, member.user_id, value));
  }

  function shownName(member: Member) {
    return member.is_me ? member.name : (style?.nicknames[member.user_id] ?? member.name);
  }
</script>

<Page title="" {onClose}>
  {#if details}<RoomManage room={details} part="header" reload={loadDetails} />{/if}

  <div class="actions">
    {#if summary?.is_dm && summary.dm_user}
      <button onclick={() => (sub = "profile")}><span class="circle"><UserRound size={22} /></span>Profile</button>
    {:else if details}
      <button onclick={() => (sub = "members")}><span class="circle"><Users size={22} /></span>Members</button>
    {/if}
    <button onclick={() => (sub = "nicknames")}><span class="circle"><CaseSensitive size={24} /></span>Nicknames</button>
    <button onclick={onSearch}><span class="circle"><Search size={22} /></span>Search</button>
    <button onclick={() => (sub = "customize")}>
      <span class="circle themed" style:--a={currentTheme.a} style:--b={currentTheme.b}><Palette size={22} /></span>Customize
    </button>
  </div>

  <h3 class="m-section">Chat info</h3>
  <button class="m-row" onclick={onSharedMedia}>
    <span class="m-icon"><Images size={20} /></span><span class="m-text"><strong>Media, files & links</strong></span><ChevronRight size={18} />
  </button>
  <button class="m-row" onclick={onHistory}>
    <span class="m-icon"><ChartColumn size={20} /></span>
    <span class="m-text"><strong>History</strong><small>Messages over time; tap a moment to go there</small></span><ChevronRight size={18} />
  </button>
  <button class="m-row" onclick={() => (sub = "customize")}>
    <span class="m-icon"><Palette size={20} /></span>
    <span class="m-text"><strong>Theme</strong><small>{style?.wallpaper ? "Your photo" : `${currentTheme.name} ${currentTheme.emoji}`}</small></span>
    <span class="m-end quick">{quick}</span><ChevronRight size={18} />
  </button>
  <div class="m-row">
    <span class="m-icon"><ImageUp size={20} /></span>
    <span class="m-text"><strong>Photos you send here</strong></span>
    <select class="m-select" value={hd === null ? "default" : hd ? "hd" : "compressed"} onchange={(e) => onChooseHd(e.currentTarget.value === "default" ? null : e.currentTarget.value === "hd")}>
      <option value="default">App default</option>
      <option value="hd">Always HD</option>
      <option value="compressed">Compressed</option>
    </select>
  </div>
  {#if summary && !summary.is_dm && spaces.spacesOf(roomId).length}
    <label class="m-row">
      <span class="m-icon"><LayoutGrid size={20} /></span>
      <span class="m-text"><strong>Show in Chats</strong><small>It's in a space; on: also listed in the Chats tab</small></span>
      <input type="checkbox" role="switch" checked={spaces.inChats(summary)} onchange={(e) => spaces.setRoom(roomId, e.currentTarget.checked).catch(() => {})} />
    </label>
  {/if}

  {#if details && !details.is_dm}
    <h3 class="m-section">People</h3>
    <RoomManage room={details} part="people" reload={loadDetails} />
  {/if}

  {#if details}
    <h3 class="m-section">Notifications</h3>
    <RoomManage room={details} part="notify" reload={loadDetails} />

    <h3 class="m-section">Privacy & support</h3>
    <button class="m-row" onclick={() => (sub = "privacy")}>
      <span class="m-icon"><ShieldCheck size={20} /></span>
      <span class="m-text"><strong>Privacy & security</strong><small>{details.encrypted ? "End-to-end encrypted" : "Not encrypted"}{details.is_dm ? "" : " · who can join, roles"}</small></span>
      <ChevronRight size={18} />
    </button>
    <RoomManage room={details} part="leave" reload={loadDetails} {onLeft} />
  {/if}
  {#if error}<p class="m-error">{error}</p>{/if}
</Page>

{#if sub === "customize"}
  <CustomizePage {roomId} onClose={() => (sub = null)} />
{:else if sub === "privacy" && details}
  <Page title="Privacy & security" onClose={() => (sub = null)}>
    <RoomManage room={details} part="privacy" reload={loadDetails} />
  </Page>
{:else if sub === "members" && details}
  <MembersPage room={details} onClose={() => (sub = null)} onOpenChat={(id) => ((sub = null), onClose(), nav.openChat(id))} onChanged={loadDetails} />
{:else if sub === "nicknames"}
  <Page title="Nicknames" onClose={() => (sub = null)}>
    {#if style && !canEdit}<p class="m-hint">Only the chat's admins can give nicknames to others. Yours is yours to change.</p>{/if}
    <ul class="members">
      {#each members as m (m.user_id)}
        <li>
          <Avatar name={m.name} mxc={m.avatar} size={40} />
          {#if editing?.userId === m.user_id}
            <!-- svelte-ignore a11y_autofocus -->
            <input
              class="nick-input"
              bind:value={editing.value}
              placeholder={m.name}
              autofocus
              onkeydown={(e) => {
                if (e.key === "Enter") saveNickname(m);
                if (e.key === "Escape") editing = null;
              }}
              onblur={() => saveNickname(m)}
            />
          {:else}
            <button
              class="member"
              disabled={!canEdit && !m.is_me}
              onclick={() => (editing = { userId: m.user_id, value: m.is_me ? "" : (style?.nicknames[m.user_id] ?? "") })}
            >
              <strong>{shownName(m)}{m.is_me ? " (you)" : ""}</strong>
              <small>{style?.nicknames[m.user_id] || m.is_me ? m.user_id : "Set a nickname"}</small>
            </button>
          {/if}
        </li>
      {/each}
    </ul>
    {#if error}<p class="m-error">{error}</p>{/if}
  </Page>
{/if}
{#if sub === "profile" && summary?.dm_user}
  <UserPage userId={summary.dm_user} onClose={() => (sub = null)} onOpenChat={(id) => ((sub = null), onClose(), nav.openChat(id))} />
{/if}

<style>
  .actions {
    display: flex;
    justify-content: center;
    gap: 14px;
    margin: 6px 0 4px;
  }
  .actions button {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    width: 74px;
    padding: 0;
    border: none;
    background: none;
    color: var(--text);
    font: inherit;
    font-size: 0.82rem;
    cursor: pointer;
  }
  .circle {
    display: grid;
    width: 48px;
    height: 48px;
    place-items: center;
    border-radius: 50%;
    background: var(--surface-2);
  }
  .circle.themed {
    background: linear-gradient(135deg, var(--a), var(--b));
    color: white;
  }
  .actions button:active .circle {
    transform: scale(0.92);
  }
  .quick {
    font-size: 1.3rem;
  }
  .nick-input {
    flex: 1;
    min-width: 0;
    padding: 9px 14px;
    border: none;
    border-radius: 999px;
    background: var(--surface-2);
    outline: none;
  }
  .members {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .members li {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 6px 0;
  }
  .member {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    padding: 6px 10px;
    border: none;
    border-radius: 10px;
    background: none;
    text-align: left;
    cursor: pointer;
  }
  .member:not(:disabled):hover {
    background: var(--surface-hover);
  }
  .member small {
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .member small {
    color: var(--muted);
  }
</style>
