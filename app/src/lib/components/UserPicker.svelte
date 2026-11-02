<script lang="ts">
  import { tr } from "$lib/i18n/index.svelte";
  import Check from "@lucide/svelte/icons/check";
  import X from "@lucide/svelte/icons/x";
  import { searchUsers, type UserResult } from "$lib/backend";
  import { chatList } from "$lib/stores/chatList.svelte";
  import Avatar from "./Avatar.svelte";

  /**
   * Find people by name or Matrix ID (the server's user directory, plus the people of our 1:1
   * chats while the search is empty). `multiple`: picks collect in `picked`; otherwise
   * `onPick` fires on tap.
   */
  let {
    multiple = false,
    picked = $bindable([]),
    exclude = [],
    onPick,
  }: {
    multiple?: boolean;
    picked?: UserResult[];
    /** Already in the chat: shown, but not pickable. */
    exclude?: string[];
    onPick?: (user: UserResult) => void;
  } = $props();

  let query = $state("");
  let results = $state<UserResult[]>([]);
  let searching = $state(false);
  let error = $state<string | null>(null);

  // People we already talk to, while nothing is typed.
  const recent = $derived(
    chatList.rooms
      .filter((r) => r.is_dm && r.dm_user && !r.is_invite)
      .slice(0, 12)
      .map((r) => ({ user_id: r.dm_user!, name: r.name, avatar: r.avatar })),
  );

  let timer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    const term = query.trim();
    clearTimeout(timer);
    error = null;
    if (!term) {
      results = [];
      searching = false;
      return;
    }
    searching = true;
    timer = setTimeout(() => {
      searchUsers(term)
        .then((list) => query.trim() === term && (results = list))
        .catch((e) => (error = String(e)))
        .finally(() => query.trim() === term && (searching = false));
    }, 300);
  });

  const shown = $derived(query.trim() ? results : recent);
  const isPicked = (u: UserResult) => picked.some((p) => p.user_id === u.user_id);

  function tap(user: UserResult) {
    if (exclude.includes(user.user_id)) return;
    if (!multiple) return onPick?.(user);
    picked = isPicked(user) ? picked.filter((p) => p.user_id !== user.user_id) : [...picked, user];
  }
</script>

{#if multiple && picked.length}
  <div class="m-chips">
    {#each picked as p (p.user_id)}
      <button class="m-chip" onclick={() => tap(p)} aria-label={tr("Remove {name}", { name: p.name })}>
        <Avatar name={p.name} mxc={p.avatar} size={22} />{p.name}<X size={14} />
      </button>
    {/each}
  </div>
{/if}
<input class="m-input" type="search" placeholder={tr("Name or @user:server")} bind:value={query} autocapitalize="off" autocomplete="off" spellcheck="false" />
{#if !query.trim() && recent.length}<h3 class="m-section">{tr("People you talk to")}</h3>{/if}
<ul>
  {#each shown as u (u.user_id)}
    {@const already = exclude.includes(u.user_id)}
    <li>
      <button class="m-row" onclick={() => tap(u)} disabled={already}>
        <Avatar name={u.name} mxc={u.avatar} size={40} />
        <span class="m-text"><strong>{u.name}</strong><small>{already ? tr("Already here") : u.user_id}</small></span>
        {#if multiple}<span class="mark" class:on={isPicked(u)}>{#if isPicked(u)}<Check size={15} />{/if}</span>{/if}
      </button>
    </li>
  {/each}
</ul>
{#if query.trim() && !searching && !results.length && !error}
  <p class="m-hint">{tr("Nobody found. Type the full address, like @name:server.")}</p>
{/if}
{#if error}<p class="m-error">{error}</p>{/if}

<style>
  ul {
    margin: 6px 0 0;
    padding: 0;
    list-style: none;
  }
  .mark {
    display: grid;
    width: 22px;
    height: 22px;
    place-items: center;
    border: 2px solid var(--border);
    border-radius: 50%;
    color: white;
  }
  .mark.on {
    border-color: transparent;
    background: var(--accent);
  }
</style>
