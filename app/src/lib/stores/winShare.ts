// Windows screen sharing done by the app (see app/src-tauri/src/win_share.rs): the capture can
// follow an app from one window to another (a game and its launcher), which the browser's
// getDisplayMedia can't. Frames arrive in a WebView2 shared buffer (3 slots) with a note per
// frame; sound arrives as 10 ms pieces of 48 kHz stereo f32. Both become MediaStreamTracks
// (track generators) that the call's LiveKit room publishes like any screen share.

import { winScreenStart, winScreenStop, type WinFrameNote, type WinShareEvent, type WinSound } from "$lib/backend";

type Generator = MediaStreamTrack & { writable: WritableStream };
type GeneratorCtor = new (init: { kind: "video" | "audio" }) => Generator;
interface SharedBufferEvent extends Event {
  additionalData?: { alwyScreen?: number; slot?: number };
  getBuffer(): ArrayBuffer;
}
interface WebView2 {
  addEventListener(type: "sharedbufferreceived", listener: (e: SharedBufferEvent) => void): void;
  removeEventListener(type: "sharedbufferreceived", listener: (e: SharedBufferEvent) => void): void;
  releaseBuffer?(buffer: ArrayBuffer): void;
}

const webview = () => (globalThis as { chrome?: { webview?: WebView2 } }).chrome?.webview;
const Generator = () => (globalThis as { MediaStreamTrackGenerator?: GeneratorCtor }).MediaStreamTrackGenerator;

/** This webview can take app-captured frames (WebView2 with shared buffers and generators). */
export const winShareSupported = () => !!webview() && !!Generator() && typeof VideoFrame !== "undefined" && typeof AudioData !== "undefined";

export class WinShare {
  video: MediaStreamTrack;
  audio: MediaStreamTrack;
  #videoWriter: WritableStreamDefaultWriter<VideoFrame>;
  #audioWriter: WritableStreamDefaultWriter<AudioData>;
  #buffers = new Map<number, { buffer: ArrayBuffer; slot: number }>();
  #onBuffer = (e: SharedBufferEvent) => {
    const gen = e.additionalData?.alwyScreen;
    if (!gen || !e.additionalData?.slot) return;
    // Older generations are closed by the app; keep only the newest.
    for (const [g, b] of this.#buffers) if (g < gen) (webview()?.releaseBuffer?.(b.buffer), this.#buffers.delete(g));
    this.#buffers.set(gen, { buffer: e.getBuffer(), slot: e.additionalData.slot });
  };
  #lastSeq = -1;
  #size: [number, number] | null = null;
  #writing = false;
  #samples = 0;
  #start = performance.now();
  #firstFrame: () => void = () => {};
  /** Resolves at the first frame (LiveKit wants the picture's size when publishing). */
  firstFrame = new Promise<void>((resolve) => (this.#firstFrame = resolve));
  #stopped = false;

  constructor() {
    const Gen = Generator()!;
    const video = new Gen({ kind: "video" });
    const audio = new Gen({ kind: "audio" });
    this.video = video;
    this.audio = audio;
    // A generator's settings carry no size, and LiveKit sizes its layers from them (else it
    // assumes 720p): report the latest frame's.
    const settings = video.getSettings.bind(video);
    video.getSettings = () => ({ ...settings(), ...(this.#size ? { width: this.#size[0], height: this.#size[1] } : {}) });
    this.#videoWriter = video.writable.getWriter();
    this.#audioWriter = audio.writable.getWriter();
    webview()!.addEventListener("sharedbufferreceived", this.#onBuffer);
  }

  start(source: string, q: { fps: number; width: number; height: number }, sound: WinSound, onEvent: (e: WinShareEvent) => void) {
    return winScreenStart(source, q, sound, (n) => this.#frame(n), (pcm) => this.#sound(pcm), onEvent);
  }

  #frame(n: WinFrameNote) {
    if (this.#stopped || n.seq <= this.#lastSeq) return;
    const b = this.#buffers.get(n.generation);
    // Still busy with the previous frame: skip this one rather than queue up (lag).
    if (!b || b.buffer.byteLength === 0 || this.#writing) return;
    this.#lastSeq = n.seq;
    this.#size = [n.width, n.height];
    let frame: VideoFrame;
    try {
      // Copied out of the slot at once (the app reuses it 3 frames later).
      frame = new VideoFrame(new Uint8Array(b.buffer, n.slot * b.slot, n.stride * n.height), {
        format: "BGRX",
        codedWidth: n.stride / 4,
        codedHeight: n.height,
        visibleRect: { x: 0, y: 0, width: n.width, height: n.height },
        timestamp: Math.round((performance.now() - this.#start) * 1000),
      });
    } catch (e) {
      console.warn("screen frame", e);
      return;
    }
    this.#writing = true;
    this.#videoWriter
      .write(frame)
      .catch(() => frame.close())
      .finally(() => {
        this.#writing = false;
        this.#firstFrame();
      });
  }

  #sound(pcm: ArrayBuffer) {
    if (this.#stopped || !pcm.byteLength) return;
    const frames = pcm.byteLength / 8;
    const data = new AudioData({
      format: "f32",
      sampleRate: 48_000,
      numberOfChannels: 2,
      numberOfFrames: frames,
      timestamp: Math.round((this.#samples / 48_000) * 1e6),
      data: pcm,
    });
    this.#samples += frames;
    void this.#audioWriter.write(data).catch(() => data.close());
  }

  async stop() {
    if (this.#stopped) return;
    this.#stopped = true;
    this.#firstFrame();
    await winScreenStop().catch(() => {});
    webview()?.removeEventListener("sharedbufferreceived", this.#onBuffer);
    for (const b of this.#buffers.values()) webview()?.releaseBuffer?.(b.buffer);
    this.#buffers.clear();
    await this.#videoWriter.close().catch(() => {});
    await this.#audioWriter.close().catch(() => {});
    this.video.stop();
    this.audio.stop();
  }
}
