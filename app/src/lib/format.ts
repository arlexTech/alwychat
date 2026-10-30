const time = new Intl.DateTimeFormat(undefined, { hour: "2-digit", minute: "2-digit" });
const weekday = new Intl.DateTimeFormat(undefined, { weekday: "short" });
const date = new Intl.DateTimeFormat(undefined, { day: "numeric", month: "short" });
const fullDate = new Intl.DateTimeFormat(undefined, { day: "numeric", month: "short", year: "numeric" });

/** Chat-list style timestamp: 14:32 · Tue · 12 Mar · 12 Mar 2024. */
export function shortTime(ms: number | null | undefined, now = Date.now()): string {
  if (!ms) return "";
  const d = new Date(ms);
  const today = new Date(now);
  if (d.toDateString() === today.toDateString()) return time.format(d);
  const days = (now - ms) / 86_400_000;
  if (days < 6) return weekday.format(d);
  if (d.getFullYear() === today.getFullYear()) return date.format(d);
  return fullDate.format(d);
}

/** Two-letter initials for avatar fallbacks. */
export function initials(name: string): string {
  const words = name.replace(/^[@#!]/, "").trim().split(/\s+/).filter(Boolean);
  const letters = words.length > 1 ? words[0][0] + words[1][0] : (words[0] ?? "?").slice(0, 1);
  return letters.toUpperCase();
}
