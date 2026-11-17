import {
  isAndroid,
  setCanScanQr,
  subscribeEncryption,
  type EncryptionStatus,
  type VerificationUpdate,
} from "$lib/backend";

const PENDING_KEY = "alwychat.recoveryUnconfirmed";

function stored(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}
function store(key: string, value: string | null) {
  try {
    if (value === null) localStorage.removeItem(key);
    else localStorage.setItem(key, value);
  } catch {}
}

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

  /** The recovery key screen (`RecoverySetup`) must be on screen: the account has no recovery
   * yet (a new account: set up right away, it can't be skipped), or a key was made here but its
   * owner never showed they saved it (the app was closed on the key: a new one is made). */
  get mustSaveRecovery() {
    const s = this.status;
    if (!s || !s.device_verified) return false;
    return s.recovery === "disabled" || this.#unconfirmed;
  }
  /** Opened by hand (Settings → Security: "Make a new recovery key"). */
  recoveryOpen = $state(false);
  #unconfirmed = $state(stored(PENDING_KEY) === "1");

  /** About to make a key: until `recoverySaved`, the screen comes back (even after a restart). */
  recoveryPending() {
    this.#unconfirmed = true;
    store(PENDING_KEY, "1");
  }
  /** The key was typed back: it's saved somewhere. */
  recoverySaved() {
    this.#unconfirmed = false;
    store(PENDING_KEY, null);
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
    this.recoveryOpen = false;
    // Signed out: the next account starts clean.
    this.#unconfirmed = false;
    store(PENDING_KEY, null);
  }
}

export const encryption = new Encryption();
