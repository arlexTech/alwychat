import { saveMedia } from "$lib/backend";
import { tr, trn } from "$lib/i18n/index.svelte";

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
  if (saved === 0) return tr("Couldn't save: {error}", { error: error });
  if (media.length === 1) return tr("Saved to {value}", { value: home(lastPath) });
  const done = trn(saved, "Saved {n} item to {folder}", "Saved {n} items to {folder}", { folder: folder(lastPath) });
  return failed ? `${done} (${failed} failed: ${error})` : done;
}
