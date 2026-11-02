// How many side panels (chat / space settings on a wide screen, Page layout "panel") are open:
// while any is, the chats layout leaves their width free instead of being covered.
class SidePanels {
  open = $state(0);
}

export const sidePanels = new SidePanels();
