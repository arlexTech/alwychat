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

/** A wide banner (3:1, centre-cropped, 1500×500 JPEG); null when cancelled. */
export async function pickBanner(): Promise<PickedImage | null> {
  const blob = await pickBlob();
  if (!blob) return null;
  const bitmap = await createImageBitmap(blob, { imageOrientation: "from-image" });
  const [W, H] = [1500, 500];
  // The largest 3:1 area in the middle of the photo.
  const sw = Math.min(bitmap.width, bitmap.height * 3);
  const sh = sw / 3;
  const canvas = document.createElement("canvas");
  canvas.width = W;
  canvas.height = H;
  canvas.getContext("2d")!.drawImage(bitmap, (bitmap.width - sw) / 2, (bitmap.height - sh) / 2, sw, sh, 0, 0, W, H);
  bitmap.close();
  const jpeg = await new Promise<Blob>((resolve, reject) =>
    canvas.toBlob((b) => (b ? resolve(b) : reject(new Error("couldn't encode the photo"))), "image/jpeg", 0.85),
  );
  const bytes = new Uint8Array(await jpeg.arrayBuffer());
  let binary = "";
  for (let i = 0; i < bytes.length; i += 0x8000) binary += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return { data: btoa(binary), preview: URL.createObjectURL(jpeg) };
}

export interface PickedEmote {
  /** Base64 image, for `uploadEmote`. */
  data: string;
  mime: string;
  width: number;
  height: number;
  size: number;
  /** Object URL for a preview (revoke when done). */
  preview: string;
}

const toBase64 = async (blob: Blob) => {
  const bytes = new Uint8Array(await blob.arrayBuffer());
  let binary = "";
  for (let i = 0; i < bytes.length; i += 0x8000) binary += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return btoa(binary);
};

/** Animated GIFs up to this size are kept as they are (resizing would stop the animation). */
const GIF_KEEP = 1024 * 1024;

/**
 * Pick an image for a custom emoji (`max` 128) or sticker (512): PNG, keeping transparency;
 * small GIFs untouched so they stay animated. Null when cancelled.
 */
export async function pickEmoteImage(max: number): Promise<PickedEmote | null> {
  const blob = await pickBlob();
  if (!blob) return null;
  const bitmap = await createImageBitmap(blob, { imageOrientation: "from-image" });
  const { width, height } = bitmap;
  if (blob.type === "image/gif" && blob.size <= GIF_KEEP) {
    bitmap.close();
    return { data: await toBase64(blob), mime: "image/gif", width, height, size: blob.size, preview: URL.createObjectURL(blob) };
  }
  const scale = Math.min(1, max / Math.max(width, height));
  const canvas = document.createElement("canvas");
  canvas.width = Math.max(1, Math.round(width * scale));
  canvas.height = Math.max(1, Math.round(height * scale));
  canvas.getContext("2d")!.drawImage(bitmap, 0, 0, canvas.width, canvas.height);
  bitmap.close();
  const png = await new Promise<Blob>((resolve, reject) =>
    canvas.toBlob((b) => (b ? resolve(b) : reject(new Error("couldn't encode the image"))), "image/png"),
  );
  return { data: await toBase64(png), mime: "image/png", width: canvas.width, height: canvas.height, size: png.size, preview: URL.createObjectURL(png) };
}
