// Custom emoji and sticker packs (MSC2545) for the open chat: ours, the chat's, and those
// turned on everywhere. Loaded when the emoji panel opens, kept a couple of minutes.
import { emotePacks, type Emote, type EmotePack } from "$lib/backend";

const FRESH_MS = 120_000;

class Emotes {
  packs = $state<EmotePack[]>([]);
  #room: string | null = null;
  #at = 0;

  async load(roomId: string | null, force = false) {
    if (!force && roomId === this.#room && Date.now() - this.#at < FRESH_MS) return;
    try {
      const packs = await emotePacks(roomId);
      this.packs = packs;
      this.#room = roomId;
      this.#at = Date.now();
    } catch (e) {
      console.warn("emote packs", e);
    }
  }

  /** Every inline emoji by shortcode (the first pack wins on a clash: ours). */
  get byCode(): Map<string, Emote> {
    const map = new Map<string, Emote>();
    for (const pack of this.packs) for (const e of pack.emotes) if (e.emoticon && !map.has(e.shortcode)) map.set(e.shortcode, e);
    return map;
  }

  /** The custom emoji written as `:code:` in `text`. */
  usedIn(text: string) {
    const out: { shortcode: string; url: string }[] = [];
    for (const [code, e] of this.byCode) if (text.includes(`:${code}:`)) out.push({ shortcode: code, url: e.url });
    return out;
  }
}

export const emotes = new Emotes();
