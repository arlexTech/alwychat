// AlwyChat updating itself (desktop and Android apps built by CI, see src-tauri/src/updates.rs):
// checked a little after start and every few hours; the chat list shows a banner, Settings →
// About has "Check for updates".
import { updateCheck, updateInstall, type UpdateInfo } from "$lib/backend";

type Stage = "idle" | "checking" | "downloading" | "installing" | "needs-permission";

class Updates {
  available = $state<UpdateInfo | null>(null);
  stage = $state<Stage>("idle");
  /** 0–1 while downloading (null when the size isn't known). */
  progress = $state<number | null>(null);
  error = $state<string | null>(null);
  /** Last check ended with nothing new (for "You're up to date"). */
  upToDate = $state(false);
  #timer: ReturnType<typeof setInterval> | undefined;

  start() {
    if (this.#timer) return;
    // Back from Android's "install unknown apps" setting: carry on (the APK is already there).
    document.addEventListener("visibilitychange", () => {
      if (document.visibilityState === "visible" && this.stage === "needs-permission" && !this.#resumed) {
        this.#resumed = true;
        void this.install(true);
      }
    });
    setTimeout(() => void this.check(), 15_000);
    this.#timer = setInterval(() => void this.check(), 6 * 3600_000);
  }

  async check() {
    if (this.stage === "downloading" || this.stage === "installing") return;
    this.stage = "checking";
    this.error = null;
    try {
      this.available = await updateCheck();
      this.upToDate = !this.available;
    } catch (e) {
      this.error = String(e);
    } finally {
      this.stage = "idle";
    }
  }

  /** The automatic retry after the permission page was used (once, so a refusal doesn't loop). */
  #resumed = false;

  async install(auto = false) {
    if (!this.available) return;
    if (!auto) this.#resumed = false;
    this.error = null;
    this.stage = "downloading";
    this.progress = null;
    try {
      await updateInstall((p) => {
        if (p.kind === "downloading") {
          this.progress = p.total ? p.done / p.total : null;
        } else if (p.kind === "installing") {
          this.stage = "installing";
        } else {
          this.stage = "needs-permission";
        }
      });
    } catch (e) {
      this.error = String(e);
      this.stage = "idle";
    }
  }
}

export const updates = new Updates();
