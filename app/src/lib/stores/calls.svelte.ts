// The Calls tab: this device's call log (kept by the core, see call_history.rs).
import { callHistory, clearCallHistory, hasBackend, markCallsSeen, onCallHistory, type CallRecord } from "$lib/backend";

class Calls {
  records = $state<CallRecord[]>([]);
  /** Missed calls since the tab was last opened (its badge). */
  unseenMissed = $state(0);
  #listening = false;

  async start() {
    if (!hasBackend || this.#listening) return;
    this.#listening = true;
    await onCallHistory(() => void this.refresh());
    await this.refresh();
  }

  async refresh() {
    const history = await callHistory().catch(() => null);
    if (!history) return;
    this.records = history.records;
    this.unseenMissed = history.unseen_missed;
  }

  seen() {
    if (this.unseenMissed) void markCallsSeen().catch(() => {});
    this.unseenMissed = 0;
  }

  async remove(id: string | null) {
    await clearCallHistory(id);
  }
}

export const calls = new Calls();
