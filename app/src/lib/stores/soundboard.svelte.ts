// The soundboard: built-in sounds plus the chat's own (room state, see core sounds.rs).
// Sounds are decoded once into 48 kHz mono buffers, the format the call plays them in.
import {
  canEditSounds,
  discardPicked,
  pickFiles,
  roomSounds,
  setRoomSounds,
  uploadSound,
  type Sound,
} from "$lib/backend";
import { pickedMediaUrl, plainFileUrl } from "$lib/media";
import { BUILTIN, RATE, render, type BuiltinSound } from "$lib/soundboard/builtin";
import { call } from "./call.svelte";

/** Longest custom sound (the core refuses more). */
export const MAX_SECONDS = 6;
/** Between two sounds from us. */
const COOLDOWN_MS = 400;

export type AnySound = { kind: "builtin"; sound: BuiltinSound } | { kind: "room"; sound: Sound };

const toBase64 = (bytes: Uint8Array) => {
  let binary = "";
  for (let k = 0; k < bytes.length; k += 0x8000) binary += String.fromCharCode(...bytes.subarray(k, k + 0x8000));
  return btoa(binary);
};

/** Any audio file → 48 kHz mono, at most `maxSeconds`. */
export async function decodeMono(data: ArrayBuffer, maxSeconds = MAX_SECONDS): Promise<AudioBuffer> {
  const decoded = await new OfflineAudioContext(1, 1, RATE).decodeAudioData(data);
  const length = Math.min(decoded.length, Math.round(maxSeconds * decoded.sampleRate));
  const mono = new OfflineAudioContext(1, length, RATE);
  const source = mono.createBufferSource();
  source.buffer = decoded;
  // Mixing down to one channel is done by the context (channelCount 1 destination).
  source.connect(mono.destination);
  source.start();
  return mono.startRendering();
}

/** 16-bit little-endian PCM of a mono buffer. */
export function pcm16(buffer: AudioBuffer): Uint8Array {
  const data = buffer.getChannelData(0);
  const out = new DataView(new ArrayBuffer(data.length * 2));
  for (let i = 0; i < data.length; i++) out.setInt16(i * 2, Math.round(Math.max(-1, Math.min(1, data[i])) * 32767), true);
  return new Uint8Array(out.buffer);
}

/** A mono buffer as a WAV file. */
function wav(buffer: AudioBuffer): Uint8Array {
  const pcm = pcm16(buffer);
  const header = new DataView(new ArrayBuffer(44));
  const text = (at: number, s: string) => [...s].forEach((c, i) => header.setUint8(at + i, c.charCodeAt(0)));
  text(0, "RIFF");
  header.setUint32(4, 36 + pcm.length, true);
  text(8, "WAVE");
  text(12, "fmt ");
  header.setUint32(16, 16, true);
  header.setUint16(20, 1, true); // PCM
  header.setUint16(22, 1, true); // mono
  header.setUint32(24, RATE, true);
  header.setUint32(28, RATE * 2, true);
  header.setUint16(32, 2, true);
  header.setUint16(34, 16, true);
  text(36, "data");
  header.setUint32(40, pcm.length, true);
  const file = new Uint8Array(44 + pcm.length);
  file.set(new Uint8Array(header.buffer), 0);
  file.set(pcm, 44);
  return file;
}

class Soundboard {
  roomId = $state<string | null>(null);
  sounds = $state<Sound[]>([]);
  canEdit = $state(false);
  /** The sound playing from us (its key), for the button's glow. */
  playing = $state<string | null>(null);
  #buffers = new Map<string, Promise<AudioBuffer>>();
  #last = 0;

  async load(roomId: string) {
    this.roomId = roomId;
    const [sounds, canEdit] = await Promise.all([roomSounds(roomId).catch(() => []), canEditSounds(roomId).catch(() => false)]);
    if (this.roomId !== roomId) return;
    this.sounds = sounds;
    this.canEdit = canEdit;
  }

  key(s: AnySound) {
    return s.kind === "builtin" ? `builtin:${s.sound.id}` : `room:${s.sound.url}`;
  }

  #buffer(s: AnySound): Promise<AudioBuffer> {
    const key = this.key(s);
    let buffer = this.#buffers.get(key);
    if (!buffer) {
      buffer =
        s.kind === "builtin"
          ? render(s.sound)
          : fetch(plainFileUrl(s.sound.url)!)
              .then((r) => (r.ok ? r.arrayBuffer() : Promise.reject(new Error(`sound ${r.status}`))))
              .then((data) => decodeMono(data));
      buffer.catch(() => this.#buffers.delete(key));
      this.#buffers.set(key, buffer);
    }
    return buffer;
  }

  /** Play for everyone in the call. */
  async play(s: AnySound) {
    const now = performance.now();
    if (now - this.#last < COOLDOWN_MS) return;
    this.#last = now;
    const buffer = await this.#buffer(s);
    const key = this.key(s);
    this.playing = key;
    setTimeout(() => this.playing === key && (this.playing = null), buffer.duration * 1000);
    await call.playSound(buffer);
  }

  /** Preview a file before adding it: decoded, trimmed. */
  async decodeFile(file: Blob) {
    return decodeMono(await file.arrayBuffer());
  }

  /** Pick an audio file (Android: the system picker; elsewhere the file input's file). */
  async pickAndroid(): Promise<Blob | null> {
    const [picked] = await pickFiles("files");
    if (!picked) return null;
    try {
      const response = await fetch(pickedMediaUrl(picked.path));
      return await response.blob();
    } finally {
      void discardPicked(picked);
    }
  }

  async add(roomId: string, buffer: AudioBuffer, name: string, emoji: string | null) {
    const url = await uploadSound(toBase64(wav(buffer)), "audio/wav");
    const current = await roomSounds(roomId);
    const sound: Sound = {
      id: crypto.randomUUID(),
      name: name.trim(),
      emoji: emoji?.trim() || null,
      url,
      mime: "audio/wav",
      duration_ms: Math.max(1, Math.round(buffer.duration * 1000)),
      by: null,
    };
    await setRoomSounds(roomId, [...current, sound]);
    if (this.roomId === roomId) this.sounds = [...current, sound];
  }

  async remove(roomId: string, id: string) {
    const current = await roomSounds(roomId);
    const next = current.filter((s) => s.id !== id);
    await setRoomSounds(roomId, next);
    if (this.roomId === roomId) this.sounds = next;
  }
}

export const soundboard = new Soundboard();
export const builtinSounds = BUILTIN;
