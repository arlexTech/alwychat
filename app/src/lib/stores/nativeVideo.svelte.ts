// Pictures of native calls (Linux desktop): the Rust engine sends each camera / screen as JPEG
// frames under a key; `NativeVideo.svelte` draws the latest one of its key on a canvas.

type Draw = (frame: ImageBitmap) => void;

class NativeVideoFrames {
  #latest = new Map<string, ImageBitmap>();
  #viewers = new Map<string, Set<Draw>>();
  /** Keys being decoded: a frame arriving meanwhile replaces nothing (frames are dropped). */
  #busy = new Set<string>();

  push(key: string, jpeg: Uint8Array) {
    if (this.#busy.has(key)) return;
    this.#busy.add(key);
    createImageBitmap(new Blob([jpeg as BlobPart], { type: "image/jpeg" }))
      .then((bitmap) => {
        this.#latest.get(key)?.close();
        this.#latest.set(key, bitmap);
        for (const draw of this.#viewers.get(key) ?? []) draw(bitmap);
      })
      .catch(() => {})
      .finally(() => this.#busy.delete(key));
  }

  /** Draw `key`'s frames with `draw` (the latest one right away) until the returned stop. */
  watch(key: string, draw: Draw): () => void {
    const viewers = this.#viewers.get(key) ?? new Set();
    this.#viewers.set(key, viewers);
    viewers.add(draw);
    const latest = this.#latest.get(key);
    if (latest) draw(latest);
    return () => viewers.delete(draw);
  }

  clear() {
    for (const bitmap of this.#latest.values()) bitmap.close();
    this.#latest.clear();
  }
}

export const nativeVideo = new NativeVideoFrames();
