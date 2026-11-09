import {
  isAndroid,
  setCanScanQr,
  subscribeEncryption,
  type EncryptionStatus,
  type VerificationUpdate,
} from "$lib/backend";

const NUDGE_KEY = "alwychat.recoveryNudgeAfter";

/** Whether this device can read encrypted history, and any emoji verification in progress. */
class Encryption {
  status = $state<EncryptionStatus | null>(null);
  verification = $state<VerificationUpdate | null>(null);
  /** The verification on screen was asked for by another device: we're vouching for it (we
   * were already verified), not being verified. */
  helping = $state(false);
  /** The unlock sheet is open. */
  sheetOpen = $state(false);

  /** This device isn't verified: encrypted messages may be unreadable (the chat list banner). */
  get locked() {
    const s = this.status;
    return !!s && !s.device_verified;
  }

  /** Verified, but the key backup secret didn't come along (some apps don't share it when
   * verifying): new messages are fine, old ones may need the recovery key. Shown in Settings →
   * Security only, not as the banner (it looked like the unlock had failed). */
  get backupMissing() {
    const s = this.status;
    return !!s && s.device_verified && s.recovery === "incomplete";
  }

  /** The account has no recovery at all (a new account): offer to set it up, at most once a
   * week once dismissed. Without it, losing every device loses the encrypted history. */
  get noRecovery() {
    const s = this.status;
    if (!s || !s.device_verified || s.recovery !== "disabled") return false;
    void this.#nudgeTick; // depend on dismissals
    return Date.now() > Number(localStorage.getItem(NUDGE_KEY) ?? 0);
  }
  /** Bumped to recompute `noRecovery` after a dismissal (localStorage isn't reactive). */
  #nudgeTick = $state(0);

  /** "Not now": ask again in a week. */
  dismissRecovery() {
    localStorage.setItem(NUDGE_KEY, String(Date.now() + 7 * 24 * 3600 * 1000));
    this.#nudgeTick++;
  }

  async start() {
    await subscribeEncryption(
      (status) => (this.status = status),
      (update) => {
        this.verification = update;
        if (update.state === "incoming") {
          // Another device asked to verify: bring the sheet up.
          this.helping = !this.locked;
          this.sheetOpen = true;
        } else if (update.state === "requested") this.helping = false;
      },
    );
    // Phones have a camera to scan the other device's code; computers rarely, they show theirs.
    await setCanScanQr(isAndroid).catch(() => {});
  }

  reset() {
    this.status = null;
    this.verification = null;
    this.sheetOpen = false;
  }
}

export const encryption = new Encryption();
