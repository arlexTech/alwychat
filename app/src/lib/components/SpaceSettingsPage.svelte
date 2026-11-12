<script lang="ts">
  import { tr } from "$lib/i18n/index.svelte";
  import Hash from "@lucide/svelte/icons/hash";
  import MessageSquarePlus from "@lucide/svelte/icons/message-square-plus";
  import Plus from "@lucide/svelte/icons/plus";
  import Star from "@lucide/svelte/icons/star";
  import Volume2 from "@lucide/svelte/icons/volume-2";
  import ArrowUpDown from "@lucide/svelte/icons/arrow-up-down";
  import SpaceOrderPage from "./SpaceOrderPage.svelte";
  import { peek, remember } from "$lib/cache";
  import X from "@lucide/svelte/icons/x";
  import { onBack } from "$lib/back.svelte";
  import {
    addToSpace,
    removeFromSpace,
    roomDetails,
    setSuggested,
    type RoomDetails,
    type SpaceChild,
  } from "$lib/backend";
  import { chatList } from "$lib/stores/chatList.svelte";
  import { spaces } from "$lib/stores/spaces.svelte";
  import { can } from "$lib/roles";
  import Avatar from "./Avatar.svelte";
  import CreateRoomPage from "./CreateRoomPage.svelte";
  import Page, { setPageLayout } from "./Page.svelte";
  import RoomManage from "./RoomManage.svelte";

  /** A space's settings: its profile, members, roles, privacy, its rooms, and leaving it. */
  let {
    spaceId,
    children,
    onClose,
    onLeft,
    onNewVoiceChannel,
  }: {
    spaceId: string;
    /** The space's rooms as listed in it (from the hierarchy). */
    children: SpaceChild[];
    onClose: () => void;
    onLeft: () => void;
    onNewVoiceChannel: () => void;
  } = $props();

  // svelte-ignore state_referenced_locally
  let details = $state<RoomDetails | null>(peek<RoomDetails>(`details:${spaceId}`) ?? null);
  let ordering = $state(false);
  let error = $state<string | null>(null);
  let adding = $state(false);
  let creating = $state(false);
  let busy = $state<string | null>(null);

  function load() {
    roomDetails(spaceId)
      .then((d) => (details = remember(`details:${spaceId}`, d)))
      .catch((e) => (error = String(e)));
  }
  $effect(load);
  $effect(() => {
    if (adding) return onBack(() => (adding = false));
  });
  $effect(() => {
    if (ordering) return onBack(() => (ordering = false));
  });
  $effect(() => {
    if (creating) return onBack(() => (creating = false));
  });

  const manage = $derived(!!details && can(details.levels, details.my_power, "m.space.child"));
  const rooms = $derived(children);
  const listed = $derived(new Set(children.map((c) => c.id)));
  // Our chats (not DMs) that aren't in this space yet.
  const addable = $derived(chatList.rooms.filter((r) => !r.is_dm && !r.is_invite && !r.is_space && !listed.has(r.id)));

  async function run(id: string, what: () => Promise<void>) {
    busy = id;
    error = null;
    try {
      await what();
    } catch (e) {
      error = String(e);
    } finally {
      busy = null;
    }
  }

  // Wide screens: a column on the right (Messenger), the chat stays visible; pages opened
  // from here stay in it.
  setPageLayout("panel");
</script>

<Page title={details?.name ?? tr("Space")} {onClose}>
  {#if details}
    <RoomManage room={details} part="header" reload={load} />

    <label class="m-row">
      <span class="m-text"><strong>{tr("Show its chats in Chats")}</strong><small>{tr("Each chat can override this in its settings.")}</small></span>
      <input
        type="checkbox"
        role="switch"
        checked={spaces.spaceShown(spaceId)}
        onchange={(e) => spaces.setSpace(spaceId, e.currentTarget.checked).catch(() => {})}
      />
    </label>

    <h3 class="m-section">{tr("Rooms · {length}", { length: rooms.length })}</h3>
    {#if manage}
      <button class="m-row" onclick={() => (creating = true)}>
        <span class="m-icon"><MessageSquarePlus size={20} /></span><span class="m-text"><strong>{tr("New chat in this space")}</strong></span>
      </button>
      <button class="m-row" onclick={onNewVoiceChannel}>
        <span class="m-icon"><Volume2 size={20} /></span><span class="m-text"><strong>{tr("New voice channel")}</strong></span>
      </button>
      <button class="m-row" onclick={() => (adding = true)}>
        <span class="m-icon"><Plus size={20} /></span><span class="m-text"><strong>{tr("Add an existing chat")}</strong></span>
      </button>
      {#if rooms.length > 1}
        <button class="m-row" onclick={() => (ordering = true)}>
          <span class="m-icon"><ArrowUpDown size={20} /></span><span class="m-text"><strong>{tr("Change the order")}</strong><small>{tr("Chats, voice channels and categories")}</small></span>
        </button>
      {/if}
    {/if}
    {#each rooms as c (c.id)}
      <div class="m-row">
        <Avatar name={c.name} mxc={c.avatar} size={36} square={c.is_space} />
        <span class="m-text">
          <span>{#if c.is_voice}<Volume2 size={13} />{:else if !c.is_space}<Hash size={13} />{/if} {c.name}</span>
          <small>{c.is_space ? tr("Space") : c.is_voice ? tr("Voice channel") : tr("Chat")}{c.suggested ? tr(" · suggested") : ""}{c.joined ? "" : tr(" · not joined")}</small>
        </span>
        {#if manage}
          <button
            class="icon"
            class:on={c.suggested}
            disabled={busy === c.id}
            onclick={() => run(c.id, () => setSuggested(spaceId, c.id, !c.suggested))}
            title={c.suggested ? tr("Suggested to members (tap to stop)") : tr("Suggest to members")}
            aria-label={tr("Suggested")}><Star size={18} /></button
          >
          <button class="icon" disabled={busy === c.id} onclick={() => run(c.id, () => removeFromSpace(spaceId, c.id))} title={tr("Remove from the space")} aria-label={tr("Remove from the space")}><X size={18} /></button>
        {/if}
      </div>
    {/each}
    {#if manage}<p class="m-hint">{tr("Removing a room from the space doesn't delete it: its members stay in it.")}</p>{/if}

    <RoomManage room={details} part="body" reload={load} {onLeft} />
  {:else if !error}
    <p class="m-hint">{tr("Loading…")}</p>
  {/if}
  {#if error}<p class="m-error">{error}</p>{/if}
</Page>

{#if ordering}
  <SpaceOrderPage {spaceId} {children} onClose={() => (ordering = false)} />
{/if}

{#if adding}
  <Page title={tr("Add to {value}", { value: details?.name ?? 'the space' })} onClose={() => (adding = false)}>
    {#each addable as r (r.id)}
      <button class="m-row" disabled={busy === r.id} onclick={() => run(r.id, async () => { await addToSpace(spaceId, r.id); adding = false; })}>
        <Avatar name={r.name} mxc={r.avatar} size={36} />
        <span class="m-text"><strong>{r.name}</strong></span>
        <Plus size={18} />
      </button>
    {:else}
      <p class="m-hint">{tr("All your group chats are already in this space.")}</p>
    {/each}
    {#if error}<p class="m-error">{error}</p>{/if}
  </Page>
{/if}

{#if creating && details}
  <CreateRoomPage kind="group" parent={spaceId} parentName={details.name} onClose={() => (creating = false)} onCreated={() => (creating = false)} />
{/if}

<style>
  .icon {
    display: grid;
    flex: none;
    width: 36px;
    height: 36px;
    place-items: center;
    border: none;
    border-radius: 50%;
    background: none;
    color: var(--muted);
    cursor: pointer;
  }
  .icon:hover {
    background: var(--surface-hover);
  }
  .icon.on {
    color: #ffc531;
  }
  .m-text :global(svg) {
    vertical-align: -1px;
    color: var(--muted);
  }
</style>
