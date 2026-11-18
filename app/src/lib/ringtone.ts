// A soft two-note ringtone made with WebAudio (no audio file to ship), plus vibration on phones.
// Android plays the phone's own ringtone instead: the webview's sound and vibration only work
// after a tap on the page, and the ringtone follows the ring / vibrate / silent mode.
import { isAndroid, isTauri, ringInApp } from "$lib/backend";

const nativeRing = isAndroid && isTauri;

let context: AudioContext | null = null;
let timer: ReturnType<typeof setInterval> | undefined;

function chime(ctx: AudioContext) {
  const now = ctx.currentTime;
  for (const [i, freq] of [659.25, 830.61, 659.25, 830.61].entries()) {
    const osc = ctx.createOscillator();
    const gain = ctx.createGain();
    osc.type = "sine";
    osc.frequency.value = freq;
    const start = now + i * 0.22;
    gain.gain.setValueAtTime(0, start);
    gain.gain.linearRampToValueAtTime(0.18, start + 0.02);
    gain.gain.exponentialRampToValueAtTime(0.001, start + 0.2);
    osc.connect(gain).connect(ctx.destination);
    osc.start(start);
    osc.stop(start + 0.22);
  }
}

/** Start ringing until `stopRinging()`. */
export function startRinging() {
  if (nativeRing) {
    ringInApp(true).catch((e) => console.warn("ring", e));
    return;
  }
  stopRinging();
  try {
    context ??= new AudioContext();
    void context.resume();
    chime(context);
  } catch {
    // No audio (autoplay rules): vibration still works.
  }
  navigator.vibrate?.([400, 200, 400]);
  timer = setInterval(() => {
    if (context) chime(context);
    navigator.vibrate?.([400, 200, 400]);
  }, 2500);
}

export function stopRinging() {
  if (nativeRing) ringInApp(false).catch(() => {});
  clearInterval(timer);
  timer = undefined;
  navigator.vibrate?.(0);
}
