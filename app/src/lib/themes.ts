// Chat themes (Messenger-style): the gradient flowing across your bubbles, the accents, a
// matching emoji and, for most, an illustrated background (see ChatScene.svelte). Chosen per
// chat and shared with its members through `com.alwy.chat.room.theme` ({ preset: id, wallpaper? }),
// so ids must never change. Colours are tuned for white text.

export type Scene = "ocean" | "space" | "aurora" | "sakura" | "forest" | "sunset" | "love" | "night" | "lagoon";

export interface Theme {
  id: string;
  name: string;
  /** Your bubbles go from `a` (top of the screen) to `b` (bottom). */
  a: string;
  b: string;
  /** Offered as the chat's quick reaction when the theme is picked. */
  emoji: string;
  /** Illustrated background; plain app background without one. */
  scene?: Scene;
  /** Others' bubbles over the scenery. */
  incoming?: string;
}

export const THEMES: Theme[] = [
  { id: "alwy", name: "AlwyChat", a: "#7b5cff", b: "#ff5c8a", emoji: "❤️" },
  { id: "ocean", name: "Ocean", a: "#16d4e8", b: "#1a5cf0", emoji: "🐳", scene: "ocean", incoming: "rgb(16 42 92 / 0.82)" },
  { id: "space", name: "Space", a: "#a35cff", b: "#4b3cf0", emoji: "🚀", scene: "space", incoming: "rgb(34 26 72 / 0.82)" },
  { id: "aurora", name: "Aurora", a: "#3ee6a8", b: "#2a8cf0", emoji: "🌌", scene: "aurora", incoming: "rgb(14 40 54 / 0.82)" },
  { id: "sakura", name: "Sakura", a: "#ff8fb8", b: "#d94c8a", emoji: "🌸", scene: "sakura", incoming: "rgb(74 34 64 / 0.8)" },
  { id: "forest", name: "Forest", a: "#4cc27a", b: "#0f7a55", emoji: "🌲", scene: "forest", incoming: "rgb(18 46 36 / 0.84)" },
  { id: "sunset", name: "Sunset", a: "#ffb03a", b: "#e8486c", emoji: "🌅", scene: "sunset", incoming: "rgb(70 30 52 / 0.8)" },
  { id: "love", name: "Love", a: "#ff6a88", b: "#e3194f", emoji: "💖", scene: "love", incoming: "rgb(78 18 42 / 0.82)" },
  { id: "night", name: "Night", a: "#7a8cff", b: "#4a3fbf", emoji: "🌙", scene: "night", incoming: "rgb(26 30 58 / 0.84)" },
  { id: "lagoon", name: "Lagoon", a: "#38e0c0", b: "#0aa0b8", emoji: "🐠", scene: "lagoon", incoming: "rgb(8 56 66 / 0.8)" },
  { id: "berry", name: "Berry", a: "#c04cf0", b: "#e8459c", emoji: "🍇" },
  { id: "lavender", name: "Lavender", a: "#a08cff", b: "#c56fd6", emoji: "💜" },
  { id: "mint", name: "Mint", a: "#2dd4bf", b: "#0d9488", emoji: "🍃" },
  { id: "cherry", name: "Cherry", a: "#ff4d6d", b: "#b3123c", emoji: "🍒" },
];

export const DEFAULT_THEME = THEMES[0];

export const themeById = (id: string | null | undefined) => THEMES.find((t) => t.id === id) ?? DEFAULT_THEME;

/** Offered first in the quick reaction picker (any emoji can be typed too). */
export const QUICK_REACTIONS = ["❤️", "👍", "😂", "🔥", "😍", "🥰", "😘", "🙏", "🎉", "💜", "🌸", "🐱", "🐳", "🚀"];

export const DEFAULT_QUICK_REACTION = "🔥";
