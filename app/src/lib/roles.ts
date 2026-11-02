import { tr } from "$lib/i18n/index.svelte";
// Matrix power levels as named roles (Element's names).

export const ROLES = [
  { power: 100, label: tr("Admin") },
  { power: 50, label: tr("Moderator") },
  { power: 0, label: tr("Member") },
] as const;

export function roleName(power: number): string {
  if (power >= 150) return tr("Owner");
  if (power === 100) return tr("Admin");
  if (power === 50) return tr("Moderator");
  if (power === 0) return tr("Member");
  return tr("Custom ({power})", { power: power });
}

/** The permissions page, in display order: key → label (keys from `RoomDetails.levels`). */
export const PERMISSION_LABELS: [string, string][] = [
  ["events_default", tr("Send messages")],
  ["invite", tr("Invite people")],
  ["m.room.pinned_events", tr("Pin messages")],
  ["org.matrix.msc3401.call.member", tr("Join calls")],
  ["redact", tr("Delete others' messages")],
  ["kick", tr("Remove people")],
  ["ban", tr("Ban people")],
  ["m.room.name", tr("Change the name")],
  ["m.room.avatar", tr("Change the photo")],
  ["m.room.topic", tr("Change the topic")],
  ["m.space.child", tr("Add or remove chats (spaces)")],
  ["m.room.join_rules", tr("Change who can join")],
  ["m.room.history_visibility", tr("Change who can read history")],
  ["m.room.encryption", tr("Turn on encryption")],
  ["m.room.power_levels", tr("Change roles and permissions")],
  ["state_default", tr("Change other settings")],
  ["users_default", tr("Role of new members")],
];

/** Can someone at `power` do what needs `key`? */
export const can = (levels: Record<string, number>, power: number, key: string) => power >= (levels[key] ?? 50);
