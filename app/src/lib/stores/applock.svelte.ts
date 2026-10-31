import { appLockState, hasBackend, type AppLockState } from "$lib/backend";
import { prefs } from "./prefs.svelte";

/** The optional app lock: whether the lock screen is up, and when to bring it back. */
class AppLock {
  state = $state<AppLockState>({ enabled: false, biometry: null });
  /** Until the state is known at start-up, the app stays covered. */
  checking = $state(hasBackend);
  locked = $state(false);
  #hiddenAt: number | null = null;

  async start() {
    if (!hasBackend) return;
    try {
      this.state = await appLockState();
      this.locked = this.state.enabled;
    } finally {
      this.checking = false;
    }
    document.addEventListener("visibilitychange", () => {
      if (document.visibilityState === "hidden") {
        this.#hiddenAt ??= Date.now();
        return;
      }
      const away = this.#hiddenAt === null ? 0 : Date.now() - this.#hiddenAt;
      this.#hiddenAt = null;
      const after = (prefs.settings?.app_lock_after_s ?? 60) * 1000;
      if (this.state.enabled && away >= after) this.locked = true;
    });
  }

  async refresh() {
    this.state = await appLockState();
  }

  unlock() {
    this.locked = false;
  }
}

export const appLock = new AppLock();
