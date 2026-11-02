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

  async install() {
    if (!this.available) return;
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
