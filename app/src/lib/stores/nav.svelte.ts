import type { RoomSummary } from "$lib/backend";

// Opening a chat or space from anywhere (a profile's "Message", a new chat, an accepted invite).
// ChatsScreen performs it; SpacesPane picks up `space`.

class Nav {
  /** A space to show in the Spaces tab (consumed by SpacesPane). */
  space = $state<string | null>(null);
  #openChat: ((roomId: string) => void) | null = null;
  #showSpaces: (() => void) | null = null;
  #chatMenu: ((event: MouseEvent, room: RoomSummary, openSettings?: () => void) => void) | null = null;

  /** Called once by ChatsScreen. */
  attach(openChat: (roomId: string) => void, showSpaces: () => void) {
    this.#openChat = openChat;
    this.#showSpaces = showSpaces;
  }

  #mute: ((room: RoomSummary) => void) | null = null;
  /** Called once by ChatsScreen: "Mute for…" for any chat. */
  attachMute(mute: (room: RoomSummary) => void) {
    this.#mute = mute;
  }
  mute(room: RoomSummary) {
    this.#mute?.(room);
  }

  /** Called once by ChatsScreen: the chat list's right-click menu. */
  attachChatMenu(open: (event: MouseEvent, room: RoomSummary, openSettings?: () => void) => void) {
    this.#chatMenu = open;
  }

  /** The chat list's menu for this chat, from elsewhere (the open chat's header). */
  chatMenu(event: MouseEvent, room: RoomSummary, openSettings?: () => void) {
    event.preventDefault();
    this.#chatMenu?.(event, room, openSettings);
  }

  openChat(roomId: string) {
    this.#openChat?.(roomId);
  }

  openSpace(spaceId: string) {
    this.space = spaceId;
    this.#showSpaces?.();
  }
}

export const nav = new Nav();
