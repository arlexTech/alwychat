/** What other apps need to show a video before playing it: its shape, length and a picture. */
export interface VideoMeta {
  width: number | null;
  height: number | null;
  duration_ms: number | null;
  /** A JPEG frame from near the start (none if the player can't hand frames over). */
  thumbnail: Blob | null;
}

const THUMBNAIL_MAX_SIDE = 800;

function once(target: EventTarget, events: string[], ms: number): Promise<boolean> {
  return new Promise((resolve) => {
    const done = (ok: boolean) => {
      clearTimeout(timer);
      for (const e of events) target.removeEventListener(e, yes);
      target.removeEventListener("error", no);
      resolve(ok);
    };
    const yes = () => done(true);
    const no = () => done(false);
    const timer = setTimeout(no, ms);
    for (const e of events) target.addEventListener(e, yes, { once: true });
    target.addEventListener("error", no, { once: true });
  });
}

/**
 * Read a video's size, duration and a preview frame with the webview's own player. Returns
 * null when it can't open the video at all (a format it doesn't know): it's then sent as is.
 */
export async function probeVideo(src: string): Promise<VideoMeta | null> {
  const video = document.createElement("video");
  video.muted = true;
  video.playsInline = true;
  video.preload = "auto";
  // media:// is another origin: without this the frame couldn't be read back from a canvas.
  if (!src.startsWith("blob:")) video.crossOrigin = "anonymous";
  try {
    video.src = src;
    if (!(await once(video, ["loadedmetadata"], 8000)) || !video.videoWidth) return null;
    const duration = Number.isFinite(video.duration) && video.duration > 0 ? video.duration : null;
    const meta: VideoMeta = {
      width: video.videoWidth,
      height: video.videoHeight,
      duration_ms: duration ? Math.round(duration * 1000) : null,
      thumbnail: null,
    };
    // A moment in (the very first frame is often black), never past the middle.
    const seeked = once(video, ["seeked"], 5000);
    video.currentTime = Math.min(1, (duration ?? 0) / 2);
    if (!(await seeked) && video.readyState < 2) return meta;
    if (video.readyState < 2 && !(await once(video, ["loadeddata", "canplay"], 3000))) return meta;
    const scale = Math.min(1, THUMBNAIL_MAX_SIDE / Math.max(video.videoWidth, video.videoHeight));
    const canvas = document.createElement("canvas");
    canvas.width = Math.max(1, Math.round(video.videoWidth * scale));
    canvas.height = Math.max(1, Math.round(video.videoHeight * scale));
    try {
      canvas.getContext("2d")!.drawImage(video, 0, 0, canvas.width, canvas.height);
      meta.thumbnail = await new Promise<Blob | null>((resolve) => canvas.toBlob(resolve, "image/jpeg", 0.8));
    } catch (e) {
      console.warn("video preview frame", e);
    }
    return meta;
  } catch (e) {
    console.warn("video details", e);
    return null;
  } finally {
    video.removeAttribute("src");
    video.load();
  }
}
