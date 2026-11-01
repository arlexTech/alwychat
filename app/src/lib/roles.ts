// Matrix power levels as named roles (Element's names).

export const ROLES = [
  { power: 100, label: "Admin" },
  { power: 50, label: "Moderator" },
  { power: 0, label: "Member" },
] as const;

export function roleName(power: number): string {
  if (power >= 150) return "Owner";
  if (power === 100) return "Admin";
  if (power === 50) return "Moderator";
  if (power === 0) return "Member";
  return `Custom (${power})`;
}

/** The permissions page, in display order: key → label (keys from `RoomDetails.levels`). */
export const PERMISSION_LABELS: [string, string][] = [
  ["events_default", "Send messages"],
  ["invite", "Invite people"],
  ["m.room.pinned_events", "Pin messages"],
  ["org.matrix.msc3401.call.member", "Join calls"],
  ["redact", "Delete others' messages"],
  ["kick", "Remove people"],
  ["ban", "Ban people"],
  ["m.room.name", "Change the name"],
  ["m.room.avatar", "Change the photo"],
  ["m.room.topic", "Change the topic"],
  ["m.space.child", "Add or remove chats (spaces)"],
  ["m.room.join_rules", "Change who can join"],
  ["m.room.history_visibility", "Change who can read history"],
  ["m.room.encryption", "Turn on encryption"],
  ["m.room.power_levels", "Change roles and permissions"],
  ["state_default", "Change other settings"],
  ["users_default", "Role of new members"],
];

/** Can someone at `power` do what needs `key`? */
export const can = (levels: Record<string, number>, power: number, key: string) => power >= (levels[key] ?? 50);
