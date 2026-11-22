// The app's right-click menu (computers): any component opens it at the pointer with a list of
// items; `ContextMenu.svelte` (in ChatsScreen) draws it, and also forwards messages and shows a
// short message after an action ("Saved to …", "Copied").

import type { Component } from "svelte";
import { tr } from "$lib/i18n/index.svelte";

export interface MenuItem {
  label: string;
  // Lucide icons (any props).
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  icon?: Component<any>;
  /** May return a message to show briefly. */
  run: () => unknown;
  danger?: boolean;
}
/** "-" = a separator; "", false, null, undefined are left out (`cond && item`). */
export type MenuEntry = MenuItem | "-" | "" | false | null | undefined;

class ContextMenu {
  open = $state<{ x: number; y: number; title?: string; items: (MenuItem | "-")[] } | null>(null);
  toast = $state<string | null>(null);
  /** Messages to forward (opens the chat picker). */
  forwarding = $state<{ roomId: string; eventIds: string[] } | null>(null);

  show(event: MouseEvent, entries: MenuEntry[], title?: string) {
    event.preventDefault();
    event.stopPropagation();
    const items: (MenuItem | "-")[] = [];
    for (const e of entries) {
      if (!e) continue;
      if (e === "-" && (!items.length || items[items.length - 1] === "-")) continue;
      items.push(e);
    }
    while (items[items.length - 1] === "-") items.pop();
    if (!items.length) return;
    this.open = { x: event.clientX, y: event.clientY, title, items };
  }

  close() {
    this.open = null;
  }

  async run(item: MenuItem) {
    this.open = null;
    try {
      const result = await item.run();
      if (typeof result === "string" && result) this.flash(result);
    } catch (e) {
      this.flash(tr("Something went wrong: {e}", { e: String(e) }));
    }
  }

  flash(message: string) {
    this.toast = message;
    setTimeout(() => this.toast === message && (this.toast = null), 3500);
  }

  forward(roomId: string, eventIds: string[]) {
    this.forwarding = { roomId, eventIds };
  }

  async copy(text: string) {
    await navigator.clipboard.writeText(text);
    return tr("Copied");
  }
}

export const contextMenu = new ContextMenu();
