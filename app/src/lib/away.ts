// "Nobody is looking": the window is hidden, another window has the keyboard, or nothing was
// touched for a while. Decoration that moves for ever (animated chat themes) then stands still:
// left open on such a chat, the app redrew the whole window at the screen's rate day and night
// (measured: a quarter of a processor core with a good graphics card, far more without one).
// Marked as `<html data-away>` for the styles.
import { touchKeyboard } from "$lib/backend";

/** Untouched for this long = nobody is looking. */
const IDLE_MS = 2 * 60 * 1000;

let started = false;
export function watchAway() {
  if (started) return;
  started = true;
  let focused = true;
  let idle = false;
  let timer: ReturnType<typeof setTimeout> | undefined;
  const apply = () => document.documentElement.toggleAttribute("data-away", document.hidden || !focused || idle);
  const active = () => {
    clearTimeout(timer);
    timer = setTimeout(() => ((idle = true), apply()), IDLE_MS);
    if (idle) ((idle = false), apply());
  };
  for (const type of ["pointerdown", "pointermove", "keydown", "wheel", "touchstart"]) {
    window.addEventListener(type, active, { capture: true, passive: true });
  }
  // Computers only: on phones the app in front may be without "focus" (system dialogs, the
  // keyboard), and being hidden already covers the rest.
  if (!touchKeyboard) {
    window.addEventListener("blur", () => ((focused = false), apply()));
    window.addEventListener("focus", () => ((focused = true), active(), apply()));
  }
  document.addEventListener("visibilitychange", apply);
  active();
  apply();
}
