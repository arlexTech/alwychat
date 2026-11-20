// Deleted messages: shown as "Message deleted" or left out. A general setting (Settings →
// Chats & media, this device) and a per-chat override (chat settings, this device), like the
// message layout. Off by default in both.
import { stored } from "$lib/stored";
import { prefs } from "./prefs.svelte";

const KEY = "alwychat.chatHideDeleted";

class DeletedPrefs {
  #perChat = $state<Record<string, boolean>>(stored(KEY, {}));

  get general(): boolean {
    return prefs.settings?.hide_deleted ?? false;
  }

  /** The chat's own choice, null when it follows the general setting. */
  chatChoice(roomId: string): boolean | null {
    return this.#perChat[roomId] ?? null;
  }

  /** Whether this chat leaves deleted messages out. */
  hides(roomId: string): boolean {
    return this.#perChat[roomId] ?? this.general;
  }

  setChat(roomId: string, hide: boolean | null) {
    const next = { ...this.#perChat };
    if (hide === null) delete next[roomId];
    else next[roomId] = hide;
    this.#perChat = next;
    try {
      localStorage.setItem(KEY, JSON.stringify(next));
    } catch {
      /* kept for this run */
    }
  }
}

export const deletedPrefs = new DeletedPrefs();
