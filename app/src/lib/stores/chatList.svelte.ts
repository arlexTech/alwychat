import { subscribeChatList, type RoomSummary } from "$lib/backend";
import { applyDiffs } from "./list";

/** Live chat list, kept in sync with the core. */
class ChatList {
  rooms = $state<RoomSummary[]>([]);
  loaded = $state(false);
  error = $state<string | null>(null);

  async start() {
    this.error = null;
    try {
      await subscribeChatList((diffs) => {
        applyDiffs(this.rooms, diffs);
        this.loaded = true;
      });
    } catch (e) {
      this.error = String(e);
    }
  }

  reset() {
    this.rooms = [];
    this.loaded = false;
  }
}

export const chatList = new ChatList();
