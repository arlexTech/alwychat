// Pick a photo for an avatar (ours, a chat's or a space's): the system picker on Android, a
// file input elsewhere; resized to a square-ish JPEG small enough to travel base64 over IPC.
import { discardPicked, isAndroid, pickFiles } from "$lib/backend";
import { pickedMediaUrl } from "$lib/media";

const AVATAR_MAX = 512;

export interface PickedImage {
  /** Base64 JPEG, for `setOwnAvatar` / `setRoomAvatar`. */
  data: string;
  /** Object URL for a preview (revoke when done). */
  preview: string;
}

async function pickBlob(): Promise<Blob | null> {
  if (isAndroid) {
    const [file] = await pickFiles("media");
    if (!file) return null;
    try {
      return await (await fetch(pickedMediaUrl(file.path))).blob();
    } finally {
      discardPicked(file).catch(() => {});
    }
  }
  return new Promise((resolve) => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = "image/*";
    input.onchange = () => resolve(input.files?.[0] ?? null);
    input.oncancel = () => resolve(null);
    input.click();
  });
}

/** Null when cancelled. */
export async function pickImage(max = AVATAR_MAX): Promise<PickedImage | null> {
  const blob = await pickBlob();
  if (!blob) return null;
  const bitmap = await createImageBitmap(blob, { imageOrientation: "from-image" });
  const scale = Math.min(1, max / Math.max(bitmap.width, bitmap.height));
  const canvas = document.createElement("canvas");
  canvas.width = Math.round(bitmap.width * scale);
  canvas.height = Math.round(bitmap.height * scale);
  canvas.getContext("2d")!.drawImage(bitmap, 0, 0, canvas.width, canvas.height);
  bitmap.close();
  const jpeg = await new Promise<Blob>((resolve, reject) =>
    canvas.toBlob((b) => (b ? resolve(b) : reject(new Error("couldn't encode the photo"))), "image/jpeg", 0.88),
  );
  const bytes = new Uint8Array(await jpeg.arrayBuffer());
  let binary = "";
  for (let i = 0; i < bytes.length; i += 0x8000) binary += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return { data: btoa(binary), preview: URL.createObjectURL(jpeg) };
}
