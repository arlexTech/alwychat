// How emoji look in the app, per device: the system's, Google's or Twitter's. Their fonts are
// only downloaded/decoded when chosen.
import { isAndroid } from "$lib/backend";

export type EmojiStyle = "system" | "google" | "twitter";

const STORAGE = "alwychat.emojiStyle";
// Google's font (4 MB, COLRv1) only ships where it can be drawn: the web (downloaded only when
// chosen; Chrome draws it) and the Windows app (WebView2 = Chromium). Android, Linux (WebKitGTK)
// and Apple's WebKit draw it blank. A build for several apps at once sets VITE_GOOGLE_EMOJI=0
// when Windows isn't one of them (scripts/release-local.sh).
const platform = import.meta.env.TAURI_ENV_PLATFORM as string | undefined;
const googleShipped = import.meta.env.VITE_GOOGLE_EMOJI !== "0" && (!platform || platform === "windows");
const load: Partial<Record<EmojiStyle, () => Promise<unknown>>> = {
  twitter: () => import("$lib/emoji/twemoji.css"),
  ...(googleShipped ? { google: () => import("$lib/emoji/noto.css") } : {}),
};

// Google's font needs COLRv1, which WebKitGTK (the Linux app, where the system's emoji are
// already Google's) draws blank.
const colorFonts = googleShipped && !isAndroid && /Chrome\/|Edg\/|Firefox\//.test(navigator.userAgent);

const BROKEN = "alwychat.emojiBroken";
const FAMILY: Partial<Record<EmojiStyle, string>> = { google: "Noto Color Emoji", twitter: "Twemoji" };

/** Whether this browser draws `family`'s emoji (some draw COLRv1 fonts blank: Chromium on
 * Linux, WebKitGTK): an emoji drawn off screen must leave colored pixels. */
async function draws(family: string): Promise<boolean> {
  try {
    await document.fonts.load(`32px "${family}"`, "😀");
    const canvas = document.createElement("canvas");
    canvas.width = canvas.height = 40;
    const ctx = canvas.getContext("2d");
    if (!ctx) return true;
    ctx.font = `32px "${family}"`;
    ctx.textBaseline = "top";
    ctx.fillText("😀", 2, 2);
    const data = ctx.getImageData(0, 0, 40, 40).data;
    for (let i = 3; i < data.length; i += 4) if (data[i] > 0) return true;
    return false;
  } catch {
    return true; // Can't tell: keep it.
  }
}

class EmojiStyles {
  /** Styles this device turned out not to draw (remembered). */
  #broken = $state<EmojiStyle[]>([]);
  /** What this device can offer. */
  get choices(): EmojiStyle[] {
    const all: EmojiStyle[] = colorFonts ? ["system", "google", "twitter"] : ["system", "twitter"];
    return all.filter((s) => !this.#broken.includes(s));
  }
  current = $state<EmojiStyle>("system");
  /** Set when a chosen style couldn't be drawn here (shown under the choices). */
  notice = $state<string | null>(null);

  start() {
    let saved: string | null = null;
    try {
      this.#broken = JSON.parse(localStorage.getItem(BROKEN) ?? "[]");
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
    const before = this.current;
    this.current = style;
    this.notice = null;
    if (style === "system") delete document.documentElement.dataset.emoji;
    else document.documentElement.dataset.emoji = style;
    const family = FAMILY[style];
    void load[style]?.()
      .then(() => (family ? draws(family) : true))
      .then((ok) => {
        if (ok || this.current !== style) return;
        // Blank here: back to what was there, and not offered again on this device.
        this.#broken = [...this.#broken, style];
        try {
          localStorage.setItem(BROKEN, JSON.stringify(this.#broken));
        } catch {
          // Only for this run.
        }
        this.set(before !== style ? before : "system");
        this.notice = style;
      });
  }
}

export const emojiStyle = new EmojiStyles();
