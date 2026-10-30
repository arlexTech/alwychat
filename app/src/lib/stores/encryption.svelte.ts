import {
  subscribeEncryption,
  type EncryptionStatus,
  type VerificationUpdate,
} from "$lib/backend";

/** Whether this device can read encrypted history, and any emoji verification in progress. */
class Encryption {
  status = $state<EncryptionStatus | null>(null);
  verification = $state<VerificationUpdate | null>(null);
  /** The unlock sheet is open. */
  sheetOpen = $state(false);

  /** True when old encrypted messages may be unreadable on this device. */
  get locked() {
    const s = this.status;
    return !!s && (!s.device_verified || s.recovery === "incomplete");
  }

  async start() {
    await subscribeEncryption(
      (status) => (this.status = status),
      (update) => {
        this.verification = update;
        // Another device asked to verify us: bring the sheet up.
        if (update.state === "incoming") this.sheetOpen = true;
      },
    );
  }

  reset() {
    this.status = null;
    this.verification = null;
    this.sheetOpen = false;
  }
}

export const encryption = new Encryption();
