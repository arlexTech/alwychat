// How emoji look in the app, per device: the system's, Google's or Twitter's. Their fonts are
// only downloaded/decoded when chosen. Google's is ~10 MB, so phone apps don't get it.
import { isAndroid } from "$lib/backend";

export type EmojiStyle = "system" | "google" | "twitter";

const STORAGE = "alwychat.emojiStyle";
const android = import.meta.env.TAURI_ENV_PLATFORM === "android";
const load: Partial<Record<EmojiStyle, () => Promise<unknown>>> = {
  twitter: () => import("$lib/emoji/twemoji.css"),
  ...(android ? {} : { google: () => import("$lib/emoji/noto.css") }),
};

// Google's font needs COLRv1, which WebKitGTK (the Linux app, where the system's emoji are
// already Google's) draws blank.
const colorFonts = !isAndroid && /Chrome\/|Edg\/|Firefox\//.test(navigator.userAgent);

class EmojiStyles {
  /** What this device can offer. */
  readonly choices: EmojiStyle[] = colorFonts ? ["system", "google", "twitter"] : ["system", "twitter"];
  current = $state<EmojiStyle>("system");

  start() {
    let saved: string | null = null;
    try {
      saved = localStorage.getItem(STORAGE);
    } catch {
      // Private window: the system's emoji.
    }
    if (saved && this.choices.includes(saved as EmojiStyle)) this.#apply(saved as EmojiStyle);
  }

  set(style: EmojiStyle) {
    try {
      localStorage.setItem(STORAGE, style);
    } catch {
      // Only lasts until the app is closed.
    }
    this.#apply(style);
  }

  #apply(style: EmojiStyle) {
    this.current = style;
    void load[style]?.();
    if (style === "system") delete document.documentElement.dataset.emoji;
    else document.documentElement.dataset.emoji = style;
  }
}

export const emojiStyle = new EmojiStyles();
