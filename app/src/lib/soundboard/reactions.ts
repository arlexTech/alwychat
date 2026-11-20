// Sounds for in-call reactions, like Element Call: each person's app plays them locally when a
// reaction shows (nothing is sent as audio). Some reuse the soundboard's built-ins, the
// animals and the laugh are drawn here; all synthesised, nothing to license or download.
import { BUILTIN, render, type BuiltinSound } from "./builtin";

/** A voice-like note: a sawtooth whose pitch follows `pitch` ([time, Hz]…) through two
 * formant filters following `formants` ([time, f1, f2]…). */
function voiced(
  ctx: BaseAudioContext,
  out: AudioNode,
  at: number,
  length: number,
  pitch: [number, number][],
  formants: [number, number, number][],
  level: number,
  attack = 0.02,
) {
  const osc = ctx.createOscillator();
  osc.type = "sawtooth";
  osc.frequency.setValueAtTime(pitch[0][1], at);
  for (const [t, f] of pitch.slice(1)) osc.frequency.exponentialRampToValueAtTime(f, at + t);
  const gain = ctx.createGain();
  gain.gain.setValueAtTime(0, 0);
  gain.gain.setValueAtTime(0, at);
  gain.gain.linearRampToValueAtTime(level, at + attack);
  gain.gain.setValueAtTime(level, at + length * 0.6);
  gain.gain.exponentialRampToValueAtTime(0.0001, at + length);
  gain.connect(out);
  for (const which of [1, 2] as const) {
    const band = ctx.createBiquadFilter();
    band.type = "bandpass";
    band.Q.value = which === 1 ? 5 : 7;
    band.frequency.setValueAtTime(formants[0][which], at);
    for (const f of formants.slice(1)) band.frequency.linearRampToValueAtTime(f[which], at + f[0]);
    const mix = ctx.createGain();
    mix.gain.value = which === 1 ? 1 : 0.6;
    osc.connect(band).connect(mix).connect(gain);
  }
  osc.start(at);
  osc.stop(at + length + 0.05);
}

/** Breath: noise through a band, for the start of a laugh or a bark. */
function breath(ctx: BaseAudioContext, out: AudioNode, at: number, length: number, freq: number, level: number) {
  const buffer = ctx.createBuffer(1, Math.ceil(length * ctx.sampleRate), ctx.sampleRate);
  const data = buffer.getChannelData(0);
  for (let i = 0; i < data.length; i++) data[i] = Math.random() * 2 - 1;
  const n = ctx.createBufferSource();
  n.buffer = buffer;
  const band = ctx.createBiquadFilter();
  band.type = "bandpass";
  band.frequency.value = freq;
  band.Q.value = 1;
  const gain = ctx.createGain();
  gain.gain.setValueAtTime(0, 0);
  gain.gain.setValueAtTime(level, at);
  gain.gain.exponentialRampToValueAtTime(0.0001, at + length);
  n.connect(band).connect(gain).connect(out);
  n.start(at);
}

const builtin = (id: string) => BUILTIN.find((s) => s.id === id)!;
const own = (id: string, duration: number, draw: BuiltinSound["draw"]): BuiltinSound => ({ id, name: () => id, emoji: "", duration, draw });

const laugh = own("laugh", 1.1, (ctx, out) => {
  // "Ha ha ha ha", each a little lower.
  for (let k = 0; k < 4; k++) {
    const at = 0.02 + k * 0.17;
    const f = 300 - k * 22;
    breath(ctx, out, at, 0.05, 1500, 0.25);
    voiced(ctx, out, at + 0.02, 0.13, [[0, f * 1.08], [0.13, f * 0.9]], [[0, 750, 1250], [0.13, 700, 1150]], 0.5);
  }
});

const bark = own("dog", 0.8, (ctx, out) => {
  for (const at of [0.01, 0.34]) {
    breath(ctx, out, at, 0.08, 900, 0.5);
    voiced(ctx, out, at, 0.17, [[0, 520], [0.05, 600], [0.17, 300]], [[0, 650, 1500], [0.17, 500, 1100]], 0.7, 0.008);
  }
});

const meow = own("cat", 0.9, (ctx, out) => {
  // "Mi-a-ow": the pitch rises then falls, the mouth opens then rounds.
  voiced(
    ctx,
    out,
    0.02,
    0.8,
    [[0, 560], [0.25, 820], [0.8, 520]],
    [[0, 400, 2300], [0.25, 900, 1700], [0.6, 600, 1000], [0.8, 450, 850]],
    0.6,
    0.06,
  );
});

const deer = own("deer", 0.9, (ctx, out) => {
  // A short bleat: a high voice with a quick wobble.
  const pitch: [number, number][] = [[0, 700]];
  for (let k = 1; k <= 10; k++) pitch.push([k * 0.07, k % 2 ? 760 : 660]);
  voiced(ctx, out, 0.02, 0.75, pitch, [[0, 900, 2200], [0.75, 700, 1800]], 0.5, 0.03);
});

const rock = own("rock", 1.5, (ctx, out) => {
  // A distorted power chord (E5).
  const drive = ctx.createWaveShaper();
  const curve = new Float32Array(1024);
  for (let i = 0; i < curve.length; i++) {
    const x = (i / (curve.length - 1)) * 2 - 1;
    curve[i] = Math.tanh(x * 6);
  }
  drive.curve = curve;
  const tone = ctx.createBiquadFilter();
  tone.type = "lowpass";
  tone.frequency.value = 2800;
  const gain = ctx.createGain();
  gain.gain.setValueAtTime(0.5, 0);
  gain.gain.setValueAtTime(0.5, 0.9);
  gain.gain.exponentialRampToValueAtTime(0.0001, 1.45);
  drive.connect(tone).connect(gain).connect(out);
  for (const f of [82.4, 123.5, 164.8]) {
    const osc = ctx.createOscillator();
    osc.type = "sawtooth";
    osc.frequency.value = f;
    const level = ctx.createGain();
    level.gain.value = 0.35;
    osc.connect(level).connect(drive);
    osc.start(0);
    osc.stop(1.5);
  }
});

/** Reaction emoji → its sound (the reactions Element Call gives a sound). */
const SOUNDS: Record<string, BuiltinSound> = {
  "🎉": builtin("tada"),
  "👏": builtin("applause"),
  "😄": laugh,
  "🤘": rock,
  "🦗": builtin("crickets"),
  "🐶": bark,
  "🐱": meow,
  "🦌": deer,
  "🥁": builtin("badumtss"),
  "🛎": builtin("ding"),
};

/** Whether this reaction emoji plays a sound. */
export const hasReactionSound = (emoji: string) => emoji.replace(/\uFE0F/g, "") in SOUNDS;

/** The soundboard's built-ins a reaction already plays (the panel doesn't list them twice). */
export const REACTION_SOUND_IDS = new Set(
  Object.values(SOUNDS)
    .map((s) => s.id)
    .filter((id) => BUILTIN.some((b) => b.id === id)),
);

const rendered = new Map<string, Promise<AudioBuffer>>();
let context: AudioContext | null = null;
let playing: AudioBufferSourceNode | null = null;

/** Play a reaction's sound here, at `volume` (0–1, already perceptual). A new one replaces
 * the one playing, so a burst of reactions doesn't pile up. */
export async function playReactionSound(emoji: string, volume: number) {
  // Element writes some with the emoji variation selector, some without.
  const sound = SOUNDS[emoji.replace(/\uFE0F/g, "")];
  if (!sound || volume < 0.01) return;
  let buffer = rendered.get(sound.id);
  if (!buffer) {
    buffer = render(sound);
    rendered.set(sound.id, buffer);
  }
  context ??= new AudioContext();
  await context.resume().catch(() => {});
  const source = context.createBufferSource();
  source.buffer = await buffer;
  const gain = context.createGain();
  gain.gain.value = 0.6 * volume;
  source.connect(gain).connect(context.destination);
  playing?.stop();
  playing = source;
  source.onended = () => {
    if (playing === source) playing = null;
  };
  source.start();
}
