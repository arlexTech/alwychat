// Opening a chat or space from anywhere (a profile's "Message", a new chat, an accepted invite).
// ChatsScreen performs it; SpacesPane picks up `space`.

class Nav {
  /** A space to show in the Spaces tab (consumed by SpacesPane). */
  space = $state<string | null>(null);
  #openChat: ((roomId: string) => void) | null = null;
  #showSpaces: (() => void) | null = null;

  /** Called once by ChatsScreen. */
  attach(openChat: (roomId: string) => void, showSpaces: () => void) {
    this.#openChat = openChat;
    this.#showSpaces = showSpaces;
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
