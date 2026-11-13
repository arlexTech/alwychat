// Short cues while in a call or a voice channel: we join or leave, someone else joins or leaves,
// mute / unmute, deafen / undeafen, someone starts sharing their screen.
// Made with WebAudio (no files to ship) and played on this device only, never into the call.
//
// Built on what was measured in the call sounds people already like (Discord's, 2026-11-13):
// - everything sits LOW-MID: notes between ~165 and 600 Hz and nothing above ~1.2 kHz, so the
//   sound never pierces and stays out of the range where voices are understood (1–4 kHz);
// - the tone is hollow and round: the note plus its octave almost as loud, a hint of the third
//   harmonic, no "bell" or "wood" overtones (those are what sounds like a toy);
// - notes are very short (instant attack, fading with a ~90 ms time constant) and come right
//   after each other (~75 ms): the first is a grace note, cut off as the next one starts;
// - the steps are open intervals (fifths and fourths), which sound neutral: up = someone is
//   here / on, down = gone / off. No major-chord "jingle";
// - a faint echo (~130 ms, quiet) gives space instead of a reverb wash.
// Ours use the same recipe with their own notes (around A and E).

export type CallCue = "join" | "leave" | "peerJoin" | "peerLeave" | "mute" | "unmute" | "deafen" | "undeafen" | "screen";

interface Cue {
  /** Note frequencies (Hz), in order. */
  notes: number[];
  /** Time between notes (s). */
  step: number;
  /** How fast a note fades (time constant, s). */
  fade: number;
  gain: number;
  /** Each note slides to this multiple of its pitch while it fades (a "tap" that dips or lifts,
   *  instead of a melody). */
  bend?: number;
}

// Notes (Hz).
const A3 = 220, B3 = 246.94, D4 = 293.66, E4 = 329.63, A4 = 440, D5 = 587.33;

const CUES: Record<CallCue, Cue> = {
  // Us: three steps over an octave (fifth, then fourth).
  join: { notes: [A3, E4, A4], step: 0.075, fade: 0.095, gain: 1 },
  leave: { notes: [A4, E4, A3], step: 0.075, fade: 0.095, gain: 1 },
  // Someone else: two notes a fifth apart.
  peerJoin: { notes: [D4, A4], step: 0.075, fade: 0.095, gain: 0.95 },
  peerLeave: { notes: [E4, A3], step: 0.08, fade: 0.095, gain: 0.95 },
  // Our own buttons are a different kind of sound from people coming and going: no melody, just
  // a short dry tap whose pitch dips (off) or lifts (on). Mute = one tap, deafen = two.
  mute: { notes: [B3], step: 0, fade: 0.04, gain: 0.95, bend: 0.72 },
  unmute: { notes: [B3], step: 0, fade: 0.04, gain: 0.95, bend: 1.4 },
  deafen: { notes: [A3, A3], step: 0.1, fade: 0.035, gain: 0.95, bend: 0.72 },
  undeafen: { notes: [A3, A3], step: 0.1, fade: 0.035, gain: 0.95, bend: 1.4 },
  // Someone shares their screen.
  screen: { notes: [A4, D5], step: 0.07, fade: 0.08, gain: 0.7 },
};

/** How loud a slider position sounds: hearing is logarithmic, so sliders follow a curve
 *  (50 % on the slider = 40 % of full power). Used for call sounds and people's volumes; a
 *  person can be boosted up to 200 % (2.5× the power). */
export const loudness = (slider: number) => Math.pow(Math.max(0, Math.min(2, slider)), 1.32);

let context: AudioContext | null = null;
let output: AudioNode | null = null;
let level: GainNode | null = null;

/** Master chain, built once: a soft low-pass, the faint echo, a limiter, THEN the volume (so
 *  every cue scales alike with the slider). */
function chain(ctx: AudioContext): AudioNode {
  if (output) return output;
  const input = ctx.createBiquadFilter();
  input.type = "lowpass";
  input.frequency.value = 2200;
  input.Q.value = 0.4;
  const delay = ctx.createDelay(0.5);
  delay.delayTime.value = 0.13;
  const feedback = ctx.createGain();
  feedback.gain.value = 0.3;
  const wet = ctx.createGain();
  wet.gain.value = 0.14;
  const limiter = ctx.createDynamicsCompressor();
  limiter.threshold.value = -4;
  limiter.knee.value = 4;
  limiter.ratio.value = 12;
  limiter.attack.value = 0.002;
  limiter.release.value = 0.1;
  level = ctx.createGain();
  input.connect(limiter);
  input.connect(delay).connect(wet).connect(limiter);
  delay.connect(feedback).connect(delay);
  limiter.connect(level).connect(ctx.destination);
  return (output = input);
}

/** One note: fundamental + octave + a little third harmonic, struck and fading; cut short at
 *  `until` when another note follows. */
function note(ctx: AudioContext, out: AudioNode, freq: number, start: number, until: number | null, cue: Cue) {
  for (const [ratio, share] of [[1, 0.4], [2, 0.31], [3, 0.04]] as const) {
    const osc = ctx.createOscillator();
    const env = ctx.createGain();
    osc.type = "sine";
    osc.frequency.setValueAtTime(freq * ratio, start);
    if (cue.bend) osc.frequency.exponentialRampToValueAtTime(freq * ratio * cue.bend, start + cue.fade * 2.5);
    env.gain.setValueAtTime(0, start);
    env.gain.linearRampToValueAtTime(share * cue.gain, start + 0.006);
    env.gain.setTargetAtTime(0, start + 0.006, cue.fade);
    // The next note takes over: this one drops away quickly (but without a click).
    if (until !== null && !cue.bend) env.gain.setTargetAtTime(0, until, 0.012);
    osc.connect(env).connect(out);
    osc.start(start);
    osc.stop((until ?? start) + cue.fade * 8);
  }
}

/** Play a cue at `volume` (the "Call sounds" slider, 0–1). */
export function playCue(name: CallCue, volume = 1) {
  if (volume <= 0.001) return;
  try {
    context ??= new AudioContext();
    const ctx = context;
    void ctx.resume();
    const out = chain(ctx);
    const now = ctx.currentTime + 0.02;
    level!.gain.setValueAtTime(loudness(volume), now);
    const cue = CUES[name];
    cue.notes.forEach((freq, i) =>
      note(ctx, out, freq, now + i * cue.step, i < cue.notes.length - 1 ? now + (i + 1) * cue.step : null, cue),
    );
  } catch {
    // No audio output available: cues are a nicety.
  }
}
