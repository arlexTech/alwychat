// The soundboard's built-in sounds, synthesised with Web Audio (no recordings: nothing to
// license, nothing to download). Each one draws itself into an offline context; `render`
// turns it into a 48 kHz mono buffer, the format calls carry.
import { tr } from "$lib/i18n/index.svelte";

export const RATE = 48_000;

export interface BuiltinSound {
  id: string;
  name: () => string;
  emoji: string;
  /** Seconds. */
  duration: number;
  draw: (ctx: BaseAudioContext, out: AudioNode) => void;
}

/** White noise, `seconds` long. */
function noise(ctx: BaseAudioContext, seconds: number) {
  const buffer = ctx.createBuffer(1, Math.ceil(seconds * ctx.sampleRate), ctx.sampleRate);
  const data = buffer.getChannelData(0);
  for (let i = 0; i < data.length; i++) data[i] = Math.random() * 2 - 1;
  const source = ctx.createBufferSource();
  source.buffer = buffer;
  return source;
}

/** A gain that opens at `at` in `attack` s, stays at `level` for `hold` s, closes in `release` s. */
function envelope(ctx: BaseAudioContext, at: number, attack: number, hold: number, release: number, level = 1) {
  const gain = ctx.createGain();
  gain.gain.setValueAtTime(0, at);
  gain.gain.linearRampToValueAtTime(level, at + attack);
  gain.gain.setValueAtTime(level, at + attack + hold);
  gain.gain.exponentialRampToValueAtTime(0.0001, at + attack + hold + release);
  return gain;
}

/** A struck sound: instant attack, exponential decay. */
function hit(ctx: BaseAudioContext, at: number, decay: number, level = 1) {
  const gain = ctx.createGain();
  // Silent until the hit (a gain starts at 1).
  gain.gain.setValueAtTime(0, 0);
  gain.gain.setValueAtTime(level, at);
  gain.gain.exponentialRampToValueAtTime(0.0001, at + decay);
  return gain;
}

/** Brass-like note (saw through a low-pass), with optional vibrato. */
function brass(ctx: BaseAudioContext, out: AudioNode, freq: number, at: number, length: number, level: number, vibrato = 0) {
  const osc = ctx.createOscillator();
  osc.type = "sawtooth";
  osc.frequency.setValueAtTime(freq, at);
  if (vibrato) {
    const lfo = ctx.createOscillator();
    const depth = ctx.createGain();
    lfo.frequency.value = 5.5;
    depth.gain.setValueAtTime(0, at);
    depth.gain.linearRampToValueAtTime(vibrato, at + length * 0.6);
    lfo.connect(depth).connect(osc.frequency);
    lfo.start(at);
    lfo.stop(at + length + 0.1);
  }
  const filter = ctx.createBiquadFilter();
  filter.type = "lowpass";
  filter.Q.value = 4;
  filter.frequency.setValueAtTime(freq * 1.5, at);
  filter.frequency.linearRampToValueAtTime(freq * 6, at + 0.08);
  filter.frequency.linearRampToValueAtTime(freq * 3, at + length);
  const env = envelope(ctx, at, 0.03, Math.max(0, length - 0.1), 0.12, level);
  osc.connect(filter).connect(env).connect(out);
  osc.start(at);
  osc.stop(at + length + 0.2);
}

function snare(ctx: BaseAudioContext, out: AudioNode, at: number, level = 1) {
  const n = noise(ctx, 0.25);
  const hp = ctx.createBiquadFilter();
  hp.type = "highpass";
  hp.frequency.value = 1400;
  n.connect(hp).connect(hit(ctx, at, 0.16, 0.7 * level)).connect(out);
  n.start(at);
  const body = ctx.createOscillator();
  body.frequency.setValueAtTime(220, at);
  body.frequency.exponentialRampToValueAtTime(140, at + 0.1);
  body.connect(hit(ctx, at, 0.12, 0.5 * level)).connect(out);
  body.start(at);
  body.stop(at + 0.2);
}

function tom(ctx: BaseAudioContext, out: AudioNode, at: number, from: number, level = 1) {
  const osc = ctx.createOscillator();
  osc.frequency.setValueAtTime(from, at);
  osc.frequency.exponentialRampToValueAtTime(from / 2, at + 0.3);
  osc.connect(hit(ctx, at, 0.35, level)).connect(out);
  osc.start(at);
  osc.stop(at + 0.4);
}

function cymbal(ctx: BaseAudioContext, out: AudioNode, at: number, decay: number, level = 0.5) {
  const n = noise(ctx, decay + 0.1);
  const hp = ctx.createBiquadFilter();
  hp.type = "highpass";
  hp.frequency.value = 6500;
  const ring = ctx.createBiquadFilter();
  ring.type = "peaking";
  ring.frequency.value = 9000;
  ring.gain.value = 8;
  n.connect(hp).connect(ring).connect(hit(ctx, at, decay, level)).connect(out);
  n.start(at);
}

export const BUILTIN: BuiltinSound[] = [
  {
    id: "airhorn",
    name: () => tr("Air horn"),
    emoji: "📯",
    duration: 1.7,
    draw(ctx, out) {
      const blast = (at: number, length: number) => {
        for (const [freq, detune] of [
          [370, -6],
          [466, 4],
          [554, -3],
          [740, 7],
        ]) {
          const osc = ctx.createOscillator();
          osc.type = "sawtooth";
          osc.frequency.value = freq;
          osc.detune.value = detune;
          const filter = ctx.createBiquadFilter();
          filter.type = "lowpass";
          filter.frequency.value = 3200;
          osc.connect(filter).connect(envelope(ctx, at, 0.015, length, 0.06, 0.16)).connect(out);
          osc.start(at);
          osc.stop(at + length + 0.1);
        }
      };
      blast(0, 0.16);
      blast(0.24, 0.16);
      blast(0.48, 1.05);
    },
  },
  {
    id: "applause",
    name: () => tr("Applause"),
    emoji: "👏",
    duration: 2.6,
    draw(ctx, out) {
      for (let i = 0; i < 110; i++) {
        // Denser in the middle, fading out.
        const at = Math.pow(Math.random(), 0.8) * 2.3;
        const level = 0.25 + Math.random() * 0.35 * (1 - at / 2.6);
        const n = noise(ctx, 0.08);
        const band = ctx.createBiquadFilter();
        band.type = "bandpass";
        band.frequency.value = 900 + Math.random() * 1800;
        band.Q.value = 1.2;
        n.connect(band).connect(hit(ctx, at, 0.05 + Math.random() * 0.03, level)).connect(out);
        n.start(at);
      }
    },
  },
  {
    id: "badumtss",
    name: () => tr("Ba dum tss"),
    emoji: "🥁",
    duration: 1.7,
    draw(ctx, out) {
      snare(ctx, out, 0.02, 0.9);
      tom(ctx, out, 0.22, 150, 0.9);
      snare(ctx, out, 0.46, 0.8);
      cymbal(ctx, out, 0.46, 1.2, 0.45);
    },
  },
  {
    id: "sadtrombone",
    name: () => tr("Sad trombone"),
    emoji: "🎺",
    duration: 2.9,
    draw(ctx, out) {
      brass(ctx, out, 196, 0, 0.42, 0.35);
      brass(ctx, out, 185, 0.48, 0.42, 0.35);
      brass(ctx, out, 175, 0.96, 0.42, 0.35);
      brass(ctx, out, 165, 1.44, 1.3, 0.35, 7);
    },
  },
  {
    id: "tada",
    name: () => tr("Ta-da"),
    emoji: "🎉",
    duration: 1.9,
    draw(ctx, out) {
      brass(ctx, out, 523, 0, 0.1, 0.22);
      brass(ctx, out, 659, 0.1, 0.1, 0.22);
      for (const f of [523, 659, 784, 1047]) brass(ctx, out, f, 0.22, 1.4, 0.13, 4);
      cymbal(ctx, out, 0.22, 1.4, 0.25);
    },
  },
  {
    id: "ding",
    name: () => tr("Ding"),
    emoji: "🔔",
    duration: 1.8,
    draw(ctx, out) {
      for (const [ratio, level, decay] of [
        [1, 0.45, 1.7],
        [2, 0.2, 1.1],
        [2.76, 0.15, 0.8],
        [5.4, 0.08, 0.4],
      ]) {
        const osc = ctx.createOscillator();
        osc.frequency.value = 1046 * ratio;
        osc.connect(hit(ctx, 0.005, decay, level)).connect(out);
        osc.start(0);
        osc.stop(decay + 0.1);
      }
    },
  },
  {
    id: "boing",
    name: () => tr("Boing"),
    emoji: "🦘",
    duration: 1,
    draw(ctx, out) {
      const osc = ctx.createOscillator();
      osc.frequency.setValueAtTime(110, 0);
      osc.frequency.exponentialRampToValueAtTime(420, 0.12);
      osc.frequency.exponentialRampToValueAtTime(300, 0.9);
      const lfo = ctx.createOscillator();
      const depth = ctx.createGain();
      lfo.frequency.value = 14;
      depth.gain.setValueAtTime(70, 0.1);
      depth.gain.exponentialRampToValueAtTime(1, 0.9);
      lfo.connect(depth).connect(osc.frequency);
      osc.connect(hit(ctx, 0.01, 0.95, 0.5)).connect(out);
      lfo.start(0);
      osc.start(0);
      lfo.stop(1);
      osc.stop(1);
    },
  },
  {
    id: "crickets",
    name: () => tr("Crickets"),
    emoji: "🦗",
    duration: 2.4,
    draw(ctx, out) {
      for (const start of [0.1, 0.75, 1.4]) {
        for (let k = 0; k < 3; k++) {
          const at = start + k * 0.09;
          const osc = ctx.createOscillator();
          osc.frequency.value = 4300 + Math.random() * 150;
          const trem = ctx.createGain();
          const lfo = ctx.createOscillator();
          lfo.type = "square";
          lfo.frequency.value = 45;
          const lfoDepth = ctx.createGain();
          lfoDepth.gain.value = 0.5;
          trem.gain.value = 0.5;
          lfo.connect(lfoDepth).connect(trem.gain);
          osc.connect(trem).connect(envelope(ctx, at, 0.005, 0.05, 0.02, 0.18)).connect(out);
          osc.start(at);
          lfo.start(at);
          osc.stop(at + 0.1);
          lfo.stop(at + 0.1);
        }
      }
    },
  },
  {
    id: "drumroll",
    name: () => tr("Drum roll"),
    emoji: "🪘",
    duration: 2.8,
    draw(ctx, out) {
      let at = 0;
      let step = 0.09;
      while (at < 1.8) {
        snare(ctx, out, at, 0.35 + at * 0.25);
        at += step;
        step = Math.max(0.035, step * 0.96);
      }
      tom(ctx, out, 1.85, 90, 1);
      cymbal(ctx, out, 1.85, 0.9, 0.55);
    },
  },
  {
    id: "whoosh",
    name: () => tr("Whoosh"),
    emoji: "💨",
    duration: 1.1,
    draw(ctx, out) {
      const n = noise(ctx, 1.1);
      const band = ctx.createBiquadFilter();
      band.type = "bandpass";
      band.Q.value = 2;
      band.frequency.setValueAtTime(300, 0);
      band.frequency.exponentialRampToValueAtTime(3500, 0.55);
      band.frequency.exponentialRampToValueAtTime(600, 1.05);
      n.connect(band).connect(envelope(ctx, 0, 0.45, 0.05, 0.55, 0.9)).connect(out);
      n.start(0);
    },
  },
];

/** Every sound peaks here: as loud as each other, never clipping. */
const PEAK = 0.85;

/** A built-in sound as a 48 kHz mono buffer. */
export async function render(sound: BuiltinSound): Promise<AudioBuffer> {
  const ctx = new OfflineAudioContext(1, Math.ceil(sound.duration * RATE), RATE);
  sound.draw(ctx, ctx.destination);
  const buffer = await ctx.startRendering();
  const data = buffer.getChannelData(0);
  let peak = 0;
  for (const v of data) peak = Math.max(peak, Math.abs(v));
  if (peak > 0) {
    const scale = PEAK / peak;
    for (let i = 0; i < data.length; i++) data[i] *= scale;
  }
  return buffer;
}
