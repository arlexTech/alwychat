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

/** Bubble timestamp: always just the time. */
export function clockTime(ms: number): string {
  return time.format(new Date(ms));
}

const longDate = new Intl.DateTimeFormat(undefined, { weekday: "long", day: "numeric", month: "long" });
const longDateYear = new Intl.DateTimeFormat(undefined, { day: "numeric", month: "long", year: "numeric" });

/** Date divider label: Today · Yesterday · Tuesday 12 March · 12 March 2024. */
export function dayLabel(ms: number, now = Date.now()): string {
  const d = new Date(ms);
  const today = new Date(now);
  if (d.toDateString() === today.toDateString()) return "Today";
  const yesterday = new Date(now - 86_400_000);
  if (d.toDateString() === yesterday.toDateString()) return "Yesterday";
  return d.getFullYear() === today.getFullYear() ? longDate.format(d) : longDateYear.format(d);
}
