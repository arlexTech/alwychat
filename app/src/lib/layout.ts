// Phones drawn edge to edge (iPhone, Android): the page covers the whole screen and the body is
// the area between the status bar and the navigation / home bar (app.css, `html.edges`). Fixed
// layers (menus, pages, the viewer) are placed in the body, so screen positions need moving.

/** A pointer's screen position in the body's coordinates, and the body's size. */
export function inBody(event: { clientX: number; clientY: number }) {
  const r = document.body.getBoundingClientRect();
  return { x: event.clientX - r.left, y: event.clientY - r.top, width: r.width, height: r.height };
}

/** The body's size (the window's, where the page isn't drawn edge to edge). */
export function bodySize() {
  const r = document.body.getBoundingClientRect();
  return { width: r.width || innerWidth, height: r.height || innerHeight };
}
