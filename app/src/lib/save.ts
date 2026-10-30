import { saveMedia } from "$lib/backend";

const home = (path: string) => path.replace(/^\/home\/[^/]+/, "~");
const folder = (path: string) => home(path.slice(0, path.lastIndexOf("/")));

/** Save photos/videos to the gallery folder, one after the other; returns a message for a toast. */
export async function saveToGallery(media: string[]): Promise<string> {
  let lastPath = "";
  let failed = 0;
  let error = "";
  for (const m of media) {
    try {
      lastPath = await saveMedia(m, true);
    } catch (e) {
      failed++;
      error = String(e);
    }
  }
  const saved = media.length - failed;
  if (saved === 0) return `Couldn't save: ${error}`;
  if (media.length === 1) return `Saved to ${home(lastPath)}`;
  const done = `Saved ${saved} ${saved === 1 ? "item" : "items"} to ${folder(lastPath)}`;
  return failed ? `${done} (${failed} failed: ${error})` : done;
}
