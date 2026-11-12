import { locale, tr } from "$lib/i18n/index.svelte";
import { clock } from "$lib/stores/clock.svelte";

// Times follow the clock setting (24 h / 12 h, the system's by default): formatters per choice.
const timeFormats = new Map<string, Intl.DateTimeFormat>();
function timeFormat(timeZone?: string): Intl.DateTimeFormat {
  const hour12 = clock.hour12;
  const key = `${hour12}|${timeZone ?? ""}`;
  let format = timeFormats.get(key);
  if (!format) {
    format = new Intl.DateTimeFormat(locale, { hour: "2-digit", minute: "2-digit", hour12, timeZone });
    timeFormats.set(key, format);
  }
  return format;
}
const time = { format: (d: Date) => timeFormat().format(d) };

/** A time of day in another time zone (people's local time). */
export function timeInZone(at: Date, timeZone: string): string {
  return timeFormat(timeZone).format(at);
}
const weekday = new Intl.DateTimeFormat(locale, { weekday: "short" });
const date = new Intl.DateTimeFormat(locale, { day: "numeric", month: "short" });
const fullDate = new Intl.DateTimeFormat(locale, { day: "numeric", month: "short", year: "numeric" });

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

/** When something ends (a mute): 14:30 · Fri 14:30 · 12 Mar 14:30. */
export function laterTime(ms: number, now = Date.now()): string {
  const d = new Date(ms);
  const t = time.format(d);
  if (d.toDateString() === new Date(now).toDateString()) return t;
  if (ms - now < 6 * 86_400_000) return `${weekday.format(d)} ${t}`;
  return `${date.format(d)} ${t}`;
}

const graphemes = new Intl.Segmenter(undefined, { granularity: "grapheme" });

/** First visible character of a word, whole (an emoji or a flag is several code units). */
function firstChar(word: string): string {
  for (const { segment } of graphemes.segment(word)) return segment;
  return "";
}

/** Two-letter initials for avatar fallbacks (one emoji if the name starts with one). */
export function initials(name: string): string {
  const words = name.replace(/^[@#!]/, "").trim().split(/\s+/).filter(Boolean);
  if (!words.length) return "?";
  const first = firstChar(words[0]);
  if (/\p{Extended_Pictographic}|\p{Regional_Indicator}/u.test(first)) return first;
  const second = words.length > 1 ? firstChar(words[1]) : "";
  return (/\p{L}|\p{N}/u.test(second) ? first + second : first).toUpperCase();
}

/** Bubble timestamp: always just the time. */
export function clockTime(ms: number): string {
  return time.format(new Date(ms));
}

const longDate = new Intl.DateTimeFormat(locale, { weekday: "long", day: "numeric", month: "long" });
const longDateYear = new Intl.DateTimeFormat(locale, { day: "numeric", month: "long", year: "numeric" });

/** Date divider label: Today · Yesterday · Tuesday 12 March · 12 March 2024. */
export function dayLabel(ms: number, now = Date.now()): string {
  const d = new Date(ms);
  const today = new Date(now);
  if (d.toDateString() === today.toDateString()) return tr("Today");
  const yesterday = new Date(now - 86_400_000);
  if (d.toDateString() === yesterday.toDateString()) return tr("Yesterday");
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

/** Full time of a message (tap on it): Today, 14:03 · Tuesday 12 March, 14:03 · 12 March 2024, 14:03. */
export function messageStamp(ms: number, now = Date.now()): string {
  return `${dayLabel(ms, now)}, ${clockTime(ms)}`;
}
