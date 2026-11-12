// Message layout: everyone on the left (Discord) or your bubbles on the right (Messenger).
// "auto" (the default) picks left on wide landscape chats and bubbles elsewhere. A general
// setting (Settings → Appearance, this device) and a per-chat override (chat settings, this
// device: the right choice depends on the screen).
import type { MessageLayout } from "$lib/backend";
import { stored } from "$lib/stored";
import { prefs } from "./prefs.svelte";

const KEY = "alwychat.chatLayout";

class Layouts {
  #perChat = $state<Record<string, MessageLayout>>(stored(KEY, {}));

  get general(): MessageLayout {
    return prefs.settings?.message_layout ?? "auto";
  }

  /** The chat's own choice, null when it follows the general setting. */
  chatChoice(roomId: string): MessageLayout | null {
    return this.#perChat[roomId] ?? null;
  }

  /** What applies in this chat. */
  of(roomId: string): MessageLayout {
    return this.#perChat[roomId] ?? this.general;
  }

  setChat(roomId: string, layout: MessageLayout | null) {
    const next = { ...this.#perChat };
    if (layout) next[roomId] = layout;
    else delete next[roomId];
    this.#perChat = next;
    localStorage.setItem(KEY, JSON.stringify(next));
  }
}

export const layouts = new Layouts();
