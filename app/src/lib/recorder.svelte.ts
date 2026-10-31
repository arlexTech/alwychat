// Voice message recording in the webview: MediaRecorder for the Opus audio, an analyser for
// the waveform (loudness every SAMPLE_MS). The core turns WebM into Ogg/Opus before sending.

/** Opus first: it's what Matrix voice messages use. */
const TYPES = ["audio/ogg;codecs=opus", "audio/webm;codecs=opus", "audio/webm", "audio/mp4"];
const SAMPLE_MS = 50;
/** Element caps voice messages too; keeps uploads small. */
export const MAX_RECORDING_MS = 15 * 60 * 1000;

export interface Recording {
  blob: Blob;
  mime: string;
  durationMs: number;
  /** Loudness samples between 0 and 1. */
  waveform: number[];
}

export class VoiceRecorder {
  recording = $state(false);
  elapsedMs = $state(0);
  /** Recent loudness (0–1), newest last, for the live waveform. */
  levels = $state<number[]>([]);

  #recorder: MediaRecorder | null = null;
  #stream: MediaStream | null = null;
  #context: AudioContext | null = null;
  #timer: ReturnType<typeof setInterval> | null = null;
  #chunks: Blob[] = [];
  #samples: number[] = [];
  #startedAt = 0;
  #done: ((r: Recording | null) => void) | null = null;

  /** Ask for the microphone and start; throws if it's unavailable or refused. */
  async start() {
    if (this.recording) return;
    const stream = await navigator.mediaDevices.getUserMedia({
      audio: { echoCancellation: true, noiseSuppression: true, autoGainControl: true },
    });
    const mime = TYPES.find((t) => MediaRecorder.isTypeSupported(t)) ?? "";
    const recorder = new MediaRecorder(stream, mime ? { mimeType: mime, audioBitsPerSecond: 32_000 } : undefined);

    const context = new AudioContext();
    const analyser = context.createAnalyser();
    analyser.fftSize = 1024;
    context.createMediaStreamSource(stream).connect(analyser);
    const buffer = new Float32Array(analyser.fftSize);

    this.#stream = stream;
    this.#recorder = recorder;
    this.#context = context;
    this.#chunks = [];
    this.#samples = [];
    this.levels = [];
    this.elapsedMs = 0;
    this.#startedAt = performance.now();

    recorder.ondataavailable = (e) => e.data.size && this.#chunks.push(e.data);
    recorder.onstop = () => {
      const durationMs = Math.round(performance.now() - this.#startedAt);
      const type = recorder.mimeType || mime || "audio/webm";
      const result: Recording = {
        blob: new Blob(this.#chunks, { type }),
        mime: type,
        durationMs,
        waveform: this.#samples,
      };
      this.#done?.(this.#chunks.length ? result : null);
      this.#done = null;
    };
    recorder.start(250);
    this.recording = true;

    this.#timer = setInterval(() => {
      analyser.getFloatTimeDomainData(buffer);
      let sum = 0;
      for (const v of buffer) sum += v * v;
      // RMS, lifted so quiet speech still shows (perceived loudness is roughly logarithmic).
      const level = Math.min(1, Math.sqrt(Math.sqrt(sum / buffer.length)) * 1.6);
      this.#samples.push(level);
      this.levels = [...this.levels.slice(-59), level];
      this.elapsedMs = performance.now() - this.#startedAt;
      if (this.elapsedMs >= MAX_RECORDING_MS) void this.stop();
    }, SAMPLE_MS);
  }

  /** Finish and return the recording (null if nothing was captured). */
  stop(): Promise<Recording | null> {
    const recorder = this.#recorder;
    if (!recorder || recorder.state === "inactive") return Promise.resolve(null);
    const done = new Promise<Recording | null>((resolve) => (this.#done = resolve));
    recorder.stop();
    this.#release();
    return done;
  }

  /** Throw the recording away. */
  cancel() {
    this.#done = null;
    if (this.#recorder?.state === "recording") this.#recorder.stop();
    this.#release();
  }

  #release() {
    if (this.#timer) clearInterval(this.#timer);
    this.#timer = null;
    this.#stream?.getTracks().forEach((t) => t.stop());
    void this.#context?.close();
    this.#stream = null;
    this.#context = null;
    this.#recorder = null;
    this.recording = false;
  }
}
