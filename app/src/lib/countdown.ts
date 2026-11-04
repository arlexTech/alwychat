// Shared dates (countdowns, "days together"): what to show today. Dates are calendar days in
// the viewer's time zone, so "in 3 days" means the same thing everywhere in a day.
import type { Countdown } from "$lib/backend";
import { locale, tr, trn } from "$lib/i18n/index.svelte";

/** Days since 1970-01-01 of a local calendar day (no DST surprises: noon UTC of that date). */
function dayNumber(y: number, m: number, d: number) {
  return Math.round(Date.UTC(y, m - 1, d, 12) / 86_400_000);
}

function parse(date: string) {
  const [y, m, d] = date.split("-").map(Number);
  return { y, m, d };
}

/** Today as a local calendar day. */
export function today(now = new Date()) {
  return { y: now.getFullYear(), m: now.getMonth() + 1, d: now.getDate() };
}

/** `month/day` in `year`; 29 February falls on the 28th in other years. */
function inYear(year: number, m: number, d: number) {
  const leap = (year % 4 === 0 && year % 100 !== 0) || year % 400 === 0;
  return m === 2 && d === 29 && !leap ? dayNumber(year, 2, 28) : dayNumber(year, m, d);
}

export interface DateView {
  /** What the strip says, e.g. "Together · 1 234 days". */
  text: string;
  emoji: string;
  /** It's today (an anniversary, the countdown's day): celebrate. */
  isToday: boolean;
  /** Days until it (0 today, negative when past), for picking the most relevant one. */
  daysUntil: number;
}

const fmt = (n: number) => new Intl.NumberFormat(locale).format(n);

export function describe(c: Countdown, now = today()): DateView {
  const t = dayNumber(now.y, now.m, now.d);
  const { y, m, d } = parse(c.date);
  const emoji = c.emoji || (c.kind === "since" ? "💞" : "⏳");
  if (c.kind === "until") {
    let target = dayNumber(y, m, d);
    if (c.yearly) {
      target = inYear(now.y, m, d);
      if (target < t) target = inYear(now.y + 1, m, d);
    }
    const days = target - t;
    const when =
      days === 0 ? tr("today 🎉") : days === 1 ? tr("tomorrow") : days > 0 ? trn(days, "in {n} day", "in {n} days", { n: fmt(days) }) : tr("passed");
    return { text: `${c.title} · ${when}`, emoji, isToday: days === 0, daysUntil: days };
  }
  // "since": days so far; on the yearly anniversary, the years.
  const start = dayNumber(y, m, d);
  const days = t - start;
  const anniversary = c.yearly && days > 0 && inYear(now.y, m, d) === t;
  if (anniversary) {
    const years = now.y - y;
    return { text: `${c.title} · ${trn(years, "{n} year today 🎉", "{n} years today 🎉", { n: fmt(years) })}`, emoji, isToday: true, daysUntil: 0 };
  }
  const text = days < 0 ? `${c.title} · ${trn(-days, "in {n} day", "in {n} days", { n: fmt(-days) })}` : `${c.title} · ${trn(days, "{n} day", "{n} days", { n: fmt(days) })}`;
  // Ordered after upcoming countdowns, but before ones that passed.
  return { text, emoji, isToday: false, daysUntil: days < 0 ? -days : 10_000 };
}

/** The dates in the order the strip shows them: today's first, then the nearest coming up. */
export function ordered(items: Countdown[], now = today()) {
  return items
    .map((c) => ({ c, view: describe(c, now) }))
    .sort((a, b) => Number(b.view.isToday) - Number(a.view.isToday) || rank(a.view) - rank(b.view));
}

const rank = (v: DateView) => (v.daysUntil < 0 ? 1_000_000 - v.daysUntil : v.daysUntil);
