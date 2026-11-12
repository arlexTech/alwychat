import { isWeb } from "$lib/backend/platform";

// Matrix media goes through the app's `media://` protocol (served by the Rust core, cached on disk).
// Windows and Android webviews expose custom protocols as http://<scheme>.localhost instead.
// The web version serves the same paths under /_media/ from a service worker (static/media-sw.js).
const usesHttpScheme = /Windows|Android/.test(navigator.userAgent);
const base = isWeb
  ? new URL("_media/", location.href).href
  : usesHttpScheme
    ? "http://media.localhost/"
    : "media://localhost/";

/** `mxc://server/id` → URL usable in <img src>. `size` is in CSS px; we fetch for the device's pixel ratio. */
export function mediaUrl(mxc: string | null | undefined, size: number): string | null {
  if (!mxc?.startsWith("mxc://")) return null;
  const path = mxc.slice("mxc://".length);
  const px = Math.round(size * (window.devicePixelRatio || 1));
  return `${base}${path}?size=${px}`;
}

/** The whole (unencrypted) file behind `mxc://server/id`, e.g. a soundboard sound. */
export function plainFileUrl(mxc: string): string | null {
  if (!mxc.startsWith("mxc://")) return null;
  return `${base}${mxc.slice("mxc://".length)}?file=1`;
}

/** Preview of a file picked in Android's picker (served from the app cache by the core). */
export function pickedMediaUrl(path: string): string {
  const name = encodeURIComponent(path.slice(path.lastIndexOf("/") + 1));
  return `${base}_picked/${name}`;
}

/** URL of a timeline message's media (decrypted by the core). `thumb` asks for a preview size. */
export function eventMediaUrl(key: string, thumb = false): string {
  const path = `_event/${encodeURIComponent(key)}${thumb ? "?thumb=1" : ""}`;
  return `${base}${path}`;
}

/** Desktop Linux: WebKitGTK's audio/video player can't stream from the media:// scheme. */
const needsBlob = !isWeb && /Linux/.test(navigator.userAgent) && !/Android/.test(navigator.userAgent);
const blobs = new Map<string, Promise<string>>();
/** Big ones (videos), oldest use first: they're let go once they add up to too much memory.
 * Small ones (voice messages) stay for the session, their players keep the URL. */
const bigBlobs = new Map<string, { size: number; blobUrl: string }>();
const BIG_BLOB = 4 * 1024 * 1024;
const BIG_BLOBS_MAX = 256 * 1024 * 1024;
/** The latest ones are never let go: one may be playing. */
const BIG_BLOBS_KEEP = 3;

/**
 * A URL an <audio>/<video> element can play: the media URL itself, or on desktop Linux a
 * blob URL of its bytes (kept so replaying doesn't download again).
 */
export function playableUrl(url: string): Promise<string> {
  if (!needsBlob) return Promise.resolve(url);
  let blob = blobs.get(url);
  const known = bigBlobs.get(url);
  if (known) {
    // Used again: newest.
    bigBlobs.delete(url);
    bigBlobs.set(url, known);
  }
  if (!blob) {
    blob = fetch(url)
      .then((r) => (r.ok ? r.blob() : Promise.reject(new Error(`media ${r.status}`))))
      .then((b) => {
        const blobUrl = URL.createObjectURL(b);
        if (b.size >= BIG_BLOB) {
          bigBlobs.set(url, { size: b.size, blobUrl });
          dropOldBlobs();
        }
        return blobUrl;
      });
    blob.catch(() => blobs.delete(url));
    blobs.set(url, blob);
  }
  return blob;
}

function dropOldBlobs() {
  let total = 0;
  for (const b of bigBlobs.values()) total += b.size;
  for (const [url, b] of bigBlobs) {
    if (total <= BIG_BLOBS_MAX || bigBlobs.size <= BIG_BLOBS_KEEP) break;
    URL.revokeObjectURL(b.blobUrl);
    bigBlobs.delete(url);
    blobs.delete(url);
    total -= b.size;
  }
}
