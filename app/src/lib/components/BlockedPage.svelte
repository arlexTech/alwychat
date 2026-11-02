<script lang="ts">
  import { tr } from "$lib/i18n/index.svelte";
  import { ignoredUsers, setIgnored, type UserResult } from "$lib/backend";
  import Avatar from "./Avatar.svelte";
  import Page from "./Page.svelte";
  import UserPicker from "./UserPicker.svelte";

  /** Blocked (ignored) people: their messages and invites are hidden, on every device. */
  let { onClose }: { onClose: () => void } = $props();

  let list = $state<UserResult[]>([]);
  let loaded = $state(false);
  let adding = $state(false);
  let busy = $state<string | null>(null);
  let error = $state<string | null>(null);

  const load = () =>
    ignoredUsers()
      .then((l) => ((list = l), (loaded = true)))
      .catch((e) => ((error = String(e)), (loaded = true)));
  $effect(() => void load());

  async function set(user: UserResult, blocked: boolean) {
    busy = user.user_id;
    error = null;
    try {
      await setIgnored(user.user_id, blocked);
      adding = false;
      await load();
    } catch (e) {
      error = String(e);
    } finally {
      busy = null;
    }
  }
</script>

<Page title={tr("Blocked people")} {onClose}>
  <p class="m-hint">{tr("You won't see their messages or invites. They aren't told.")}</p>
  {#each list as u (u.user_id)}
    <div class="m-row">
      <Avatar name={u.name} mxc={u.avatar} size={40} />
      <span class="m-text"><strong>{u.name}</strong><small>{u.user_id}</small></span>
      <button class="m-btn plain" disabled={busy === u.user_id} onclick={() => set(u, false)}>{tr("Unblock")}</button>
    </div>
  {:else}
    {#if loaded}<p class="m-hint">{tr("Nobody is blocked.")}</p>{/if}
  {/each}
  {#if adding}
    <h3 class="m-section">{tr("Block someone")}</h3>
    <UserPicker exclude={list.map((u) => u.user_id)} onPick={(u) => set(u, true)} />
  {:else}
    <button class="m-btn plain wide" onclick={() => (adding = true)}>{tr("Block someone")}</button>
  {/if}
  {#if error}<p class="m-error">{error}</p>{/if}
</Page>

<style>
  .wide {
    width: 100%;
    margin-top: 12px;
  }
</style>
