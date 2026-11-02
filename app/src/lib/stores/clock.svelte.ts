// 24-hour or 12-hour clock: the system's by default (asked natively: the webview only knows the
// language, so an English interface would show AM/PM on a 24-hour system), or the user's choice
// in Settings → Appearance. Saved on this device.
import { systemClock } from "$lib/backend";

export type ClockChoice = "system" | "24" | "12";
const CHOICE = "alwychat.clock";
const SYSTEM = "alwychat.systemClock";

/** Whether a locale writes times with 12 hours ("en-US") or 24 ("fr-BE"). */
function localeIs12h(locale?: string): boolean {
  try {
    return new Intl.DateTimeFormat(locale, { hour: "numeric" }).resolvedOptions().hour12 ?? false;
  } catch {
    return false;
  }
}

class Clock {
  choice = $state<ClockChoice>((localStorage.getItem(CHOICE) as ClockChoice) || "system");
  /** What the system uses, once known (remembered between launches). */
  #system12 = $state<boolean | null>(
    localStorage.getItem(SYSTEM) === null ? null : localStorage.getItem(SYSTEM) === "12",
  );

  /** Ask the system (desktop / Android apps); browsers keep their own locale's habit. */
  async detect() {
    try {
      const { h24, locale } = await systemClock();
      const is12 = h24 !== null ? !h24 : locale ? localeIs12h(locale) : null;
      if (is12 === null) return;
      this.#system12 = is12;
      localStorage.setItem(SYSTEM, is12 ? "12" : "24");
    } catch {
      // Not available here (browser): the browser's locale decides.
    }
  }

  get system12(): boolean {
    return this.#system12 ?? localeIs12h();
  }

  get hour12(): boolean {
    return this.choice === "system" ? this.system12 : this.choice === "12";
  }

  set(choice: ClockChoice) {
    this.choice = choice;
    localStorage.setItem(CHOICE, choice);
  }
}

export const clock = new Clock();
void clock.detect();
