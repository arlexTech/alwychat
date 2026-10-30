// Matrix media goes through the app's `media://` protocol (served by the Rust core, cached on disk).
// Windows and Android webviews expose custom protocols as http://<scheme>.localhost instead.
const usesHttpScheme = /Windows|Android/.test(navigator.userAgent);

/** `mxc://server/id` → URL usable in <img src>. `size` is in CSS px; we fetch for the device's pixel ratio. */
export function mediaUrl(mxc: string | null | undefined, size: number): string | null {
  if (!mxc?.startsWith("mxc://")) return null;
  const path = mxc.slice("mxc://".length);
  const px = Math.round(size * (window.devicePixelRatio || 1));
  return usesHttpScheme
    ? `http://media.localhost/${path}?size=${px}`
    : `media://localhost/${path}?size=${px}`;
}

/** URL of a timeline message's media (decrypted by the core). `thumb` asks for a preview size. */
export function eventMediaUrl(key: string, thumb = false): string {
  const path = `_event/${encodeURIComponent(key)}${thumb ? "?thumb=1" : ""}`;
  return usesHttpScheme ? `http://media.localhost/${path}` : `media://localhost/${path}`;
}
