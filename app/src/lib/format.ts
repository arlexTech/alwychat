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

/** File size: 830 B · 42 KB · 3.1 MB. */
export function formatSize(bytes: number | null | undefined): string {
  if (bytes == null) return "";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 ** 2) return `${(bytes / 1024).toFixed(0)} KB`;
  return `${(bytes / 1024 ** 2).toFixed(1)} MB`;
}

/** Media length: 0:42 · 12:05. */
export function formatDuration(ms: number | null | undefined): string {
  if (ms == null) return "";
  const s = Math.round(ms / 1000);
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
}
