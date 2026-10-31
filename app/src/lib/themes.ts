// Chat themes (Messenger-style): the gradient of your bubbles, the send button and accents.
// Chosen per chat and shared with its members through `com.alwy.chat.room.theme` (just the id),
// so ids must never change. Colours are tuned for white text on both ends.

export interface Theme {
  id: string;
  name: string;
  a: string;
  b: string;
}

export const THEMES: Theme[] = [
  { id: "alwy", name: "AlwyChat", a: "#7b5cff", b: "#ff5c8a" },
  { id: "love", name: "Love", a: "#ff416c", b: "#ff7a59" },
  { id: "berry", name: "Berry", a: "#8e2de2", b: "#e8459c" },
  { id: "ocean", name: "Ocean", a: "#1e6fe0", b: "#22b2d8" },
  { id: "forest", name: "Forest", a: "#0f8f6f", b: "#3bb54a" },
  { id: "sunset", name: "Sunset", a: "#e2552d", b: "#f09a2b" },
  { id: "lavender", name: "Lavender", a: "#7f6fe0", b: "#c56fd6" },
  { id: "mint", name: "Mint", a: "#0d9488", b: "#14b8a6" },
  { id: "cherry", name: "Cherry", a: "#b3123c", b: "#e0375c" },
  { id: "night", name: "Night", a: "#3a3f5c", b: "#5a6385" },
];

export const DEFAULT_THEME = THEMES[0];

export const themeById = (id: string | null | undefined) => THEMES.find((t) => t.id === id) ?? DEFAULT_THEME;

/** Offered first in the quick reaction picker (any emoji can be typed too). */
export const QUICK_REACTIONS = ["❤️", "👍", "😂", "🔥", "😍", "🥰", "😘", "🙏", "🎉", "💜", "🌸", "🐱"];

export const DEFAULT_QUICK_REACTION = "❤️";
