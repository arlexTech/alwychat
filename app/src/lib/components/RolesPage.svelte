<script lang="ts">
  import { tr } from "$lib/i18n/index.svelte";
  import { setPermission, type RoomDetails } from "$lib/backend";
  import { PERMISSION_LABELS, can } from "$lib/roles";
  import Page from "./Page.svelte";

  /** Who may do what in a chat or space (power levels), as Everyone / Moderators / Admins. */
  let { room, onClose, onChanged }: { room: RoomDetails; onClose: () => void; onChanged: () => void } = $props();

  // svelte-ignore state_referenced_locally
  let levels = $state({ ...room.levels });
  let busy = $state<string | null>(null);
  let error = $state<string | null>(null);

  const editable = $derived(can(room.levels, room.my_power, "m.room.power_levels"));
  const shown = $derived(
    PERMISSION_LABELS.filter(([key]) =>
      room.is_space ? !["org.matrix.msc3401.call.member", "m.room.pinned_events", "m.room.encryption"].includes(key) : key !== "m.space.child",
    ),
  );
  const options = (key: string) =>
    key === "users_default"
      ? [
          { power: 0, label: tr("Member") },
          { power: 50, label: tr("Moderator") },
        ]
      : [
          { power: 0, label: tr("Everyone") },
          { power: 50, label: tr("Moderators") },
          { power: 100, label: tr("Admins") },
        ];

  async function change(key: string, power: number) {
    const before = levels[key];
    levels[key] = power;
    busy = key;
    error = null;
    try {
      await setPermission(room.id, key, power);
      onChanged();
    } catch (e) {
      levels[key] = before;
      error = String(e);
    } finally {
      busy = null;
    }
  }
</script>

<Page title={tr("Roles & permissions")} {onClose}>
  <p class="m-hint">
    {editable ? tr("Admins can always do everything. Moderators have what you allow them below.") : tr("Only admins can change these.")}
  </p>
  {#each shown as [key, label] (key)}
    {@const value = levels[key] ?? 50}
    {@const opts = options(key)}
    <div class="m-row">
      <span class="m-text"><span>{label}</span></span>
      <select class="m-select" disabled={!editable || busy === key || value > room.my_power} value={value} onchange={(e) => change(key, Number(e.currentTarget.value))}>
        {#each opts as o (o.power)}<option value={o.power} disabled={o.power > room.my_power}>{o.label}</option>{/each}
        {#if !opts.some((o) => o.power === value)}<option value={value}>{tr("Level {value}", { value: value })}</option>{/if}
      </select>
    </div>
  {/each}
  {#if error}<p class="m-error">{error}</p>{/if}
</Page>
