// Android chat bubbles (Bubbles.kt): float a chat as a chat head that opens over other apps.
import { isAndroid, openBubble, type RoomSummary } from "$lib/backend";
import { mediaUrl } from "$lib/media";
import { coreText } from "$lib/coreText";
import { contextMenu } from "$lib/stores/menu.svelte";
import { tr } from "$lib/i18n/index.svelte";

/** Bubbles need Android 11 (the app's bubble switch and Android's own setting decide the rest). */
export const canBubble = isAndroid && Number(/Android (\d+)/.exec(navigator.userAgent)?.[1] ?? 0) >= 11;

/** The chat's photo as base64 (the bubble's icon), if it has one. */
async function photo(mxc: string | null): Promise<string | null> {
  const url = mediaUrl(mxc, 128);
  if (!url) return null;
  try {
    const blob = await (await fetch(url)).blob();
    const data = await new Promise<string>((ok, fail) => {
      const reader = new FileReader();
      reader.onload = () => ok(String(reader.result));
      reader.onerror = () => fail(reader.error);
      reader.readAsDataURL(blob);
    });
    return data.slice(data.indexOf(",") + 1);
  } catch {
    return null;
  }
}

/** "Open as a bubble": false when it didn't float (Android's settings opened, or the phone
 *  doesn't let AlwyChat bubble: then a message says so). */
export async function floatChat(room: RoomSummary): Promise<boolean> {
  const { opened, blocked } = await openBubble({
    roomId: room.id,
    name: room.name,
    avatar: await photo(room.avatar),
    group: !room.is_dm,
    text: room.latest ? coreText(room.latest.text) : "",
    sender: room.latest?.sender_name ?? room.name,
  });
  if (blocked) contextMenu.flash(BLOCKED());
  return opened;
}

/** What to say when the phone throws bubbles away. */
export const BLOCKED = () => tr("This phone doesn't let AlwyChat show bubbles.");
