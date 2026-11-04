// End-to-end encryption of call media in the webview's call engine (livekit-client), with the
// keys the core exchanges over Matrix (crates/alwy-core/src/call_keys.rs). Same settings as
// Element Call, so its frames and ours decrypt on both sides.
import { BaseKeyProvider, isE2EESupported } from "livekit-client";
import type { CallKey } from "$lib/backend";

export class MatrixKeyProvider extends BaseKeyProvider {
  constructor() {
    // Element Call's: 256 key slots (indexes 0–255), ratchet window 10. Never give up on a key
    // after failed frames: during a switch a new key may simply still be on its way.
    super({ ratchetWindowSize: 10, keyringSize: 256, failureTolerance: -1 });
  }

  /** Use `key` for each of its identities (raw 32 bytes → HKDF, as Element Call does). */
  async add(key: CallKey) {
    const raw = Uint8Array.from(atob(key.key), (c) => c.charCodeAt(0));
    const material = await crypto.subtle.importKey("raw", raw, "HKDF", false, ["deriveBits", "deriveKey"]);
    for (const identity of key.identities) this.onSetEncryptionKey(material, identity, key.index);
  }
}

/** livekit-client's encryption worker, or null where frames can't be encrypted (old WebView). */
export async function e2eeWorker(): Promise<Worker | null> {
  if (!isE2EESupported()) return null;
  try {
    const { default: E2EEWorker } = await import("livekit-client/e2ee-worker?worker");
    return new E2EEWorker();
  } catch (e) {
    console.warn("call encryption unavailable", e);
    return null;
  }
}
