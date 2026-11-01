// Our "Saved messages" chat: a chat with only ourselves, found through account data.
import { openSavedMessages, savedMessagesId } from "$lib/backend";

class SavedStore {
  id = $state<string | null>(null);

  start() {
    savedMessagesId()
      .then((id) => (this.id = id))
      .catch((e) => console.warn("saved messages unavailable", e));
  }

  is(roomId: string | null | undefined) {
    return !!roomId && roomId === this.id;
  }

  /** Its room ID, creating the chat the first time. */
  async open() {
    this.id = await openSavedMessages();
    return this.id;
  }
}

export const saved = new SavedStore();
