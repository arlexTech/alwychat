// Pick a photo for an avatar (ours, a chat's or a space's): the system picker on Android, a
// file input elsewhere; resized to a square-ish JPEG small enough to travel base64 over IPC.
import { discardPicked, isAndroid, pickFiles } from "$lib/backend";
import { pickedMediaUrl } from "$lib/media";
import { mount, unmount } from "svelte";
import ImageCropper, { type Crop } from "$lib/components/ImageCropper.svelte";

const AVATAR_MAX = 512;

export interface PickedImage {
  /** Base64 JPEG, for `setOwnAvatar` / `setRoomAvatar`. */
  data: string;
  /** Object URL for a preview (revoke when done). */
  preview: string;
}

async function pickBlob(): Promise<Blob | null> {
  if (isAndroid) {
    // One photo only: the picker doesn't let several be ticked.
    const [file] = await pickFiles("photo");
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

/** Let the person move and zoom the photo inside a frame of `aspect`; null when cancelled. */
function cropImage(blob: Blob, bitmap: ImageBitmap, aspect: number, round: boolean): Promise<Crop | null> {
  const url = URL.createObjectURL(blob);
  return new Promise((resolve) => {
    const cropper = mount(ImageCropper, {
      target: document.body,
      props: {
        bitmap,
        url,
        aspect,
        round,
        onDone: (crop: Crop | null) => {
          void unmount(cropper, { outro: true });
          setTimeout(() => URL.revokeObjectURL(url), 1000);
          resolve(crop);
        },
      },
    });
  });
}

async function jpegBase64(canvas: HTMLCanvasElement, quality: number): Promise<PickedImage> {
  const jpeg = await new Promise<Blob>((resolve, reject) =>
    canvas.toBlob((b) => (b ? resolve(b) : reject(new Error("couldn't encode the photo"))), "image/jpeg", quality),
  );
  return { data: await toBase64(jpeg), preview: URL.createObjectURL(jpeg) };
}

/** Pick a photo, choose its part in the cropper, draw it at `width`×`height`. */
async function pickCropped(width: number, height: number, round: boolean, quality: number): Promise<PickedImage | null> {
  const blob = await pickBlob();
  if (!blob) return null;
  const bitmap = await createImageBitmap(blob, { imageOrientation: "from-image" });
  try {
    const crop = await cropImage(blob, bitmap, width / height, round);
    if (!crop) return null;
    // Never upscale a small photo beyond its own pixels.
    const scale = Math.min(1, crop.sw / width);
    const canvas = document.createElement("canvas");
    canvas.width = Math.max(1, Math.round(width * scale));
    canvas.height = Math.max(1, Math.round(height * scale));
    const ctx = canvas.getContext("2d")!;
    ctx.imageSmoothingQuality = "high";
    ctx.drawImage(bitmap, crop.sx, crop.sy, crop.sw, crop.sh, 0, 0, canvas.width, canvas.height);
    return await jpegBase64(canvas, quality);
  } finally {
    bitmap.close();
  }
}

/** A square avatar (the part chosen in the cropper, ≤`max` px); null when cancelled. `round`:
 * the frame is drawn as a circle, like people's avatars (spaces are rounded squares). */
export const pickImage = (max = AVATAR_MAX, round = true) => pickCropped(max, max, round, 0.88);

/** A wide banner (3:1, the part chosen in the cropper, 1500×500 JPEG); null when cancelled. */
export const pickBanner = () => pickCropped(1500, 500, false, 0.85);

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
