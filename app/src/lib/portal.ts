/** `use:portal`: move an element to <body>, so a parent's filter or transform can't trap a
 * fixed-position overlay (sheets, menus) inside itself. */
export function portal(node: HTMLElement) {
  document.body.appendChild(node);
  return { destroy: () => node.remove() };
}
