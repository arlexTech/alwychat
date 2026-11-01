import {
  getSettings,
  hasBackend,
  isAndroid,
  setSecureScreen,
  setSettings,
  setSystemBars,
  type AppTheme,
  type Settings,
} from "$lib/backend";

/** This device's settings, shared by the settings page and what they affect (theme, effects). */
class Prefs {
  settings = $state<Settings | null>(null);
  #dark = window.matchMedia("(prefers-color-scheme: dark)");

  async load() {
    this.#dark.addEventListener("change", () => this.#applyTheme());
    if (!hasBackend) return;
    this.settings = await getSettings();
    this.#applyTheme();
    this.#applyTextScale();
    if (this.settings.hide_in_switcher) setSecureScreen(true).catch(() => {});
  }

  /** Optimistic: shown at once, rolled back (and thrown) if it can't be saved. */
  async save(change: Partial<Settings>) {
    if (!this.settings) return;
    const previous = this.settings;
    this.settings = { ...this.settings, ...change };
    if ("app_theme" in change) this.#applyTheme();
    if ("text_scale" in change) this.#applyTextScale();
    if ("hide_in_switcher" in change) setSecureScreen(!!change.hide_in_switcher).catch(() => {});
    try {
      await setSettings(this.settings);
    } catch (e) {
      this.settings = previous;
      this.#applyTheme();
      this.#applyTextScale();
      throw e;
    }
  }

  get effects() {
    return this.settings?.chat_effects ?? true;
  }

  /** Text sized in rem/em (messages, most of the UI) follows the root font size. */
  #applyTextScale() {
    const scale = this.settings?.text_scale ?? 100;
    document.documentElement.style.fontSize = scale === 100 ? "" : `${scale}%`;
  }

  #applyTheme() {
    const chosen: AppTheme = this.settings?.app_theme ?? "system";
    const theme = chosen === "system" ? (this.#dark.matches ? "dark" : "light") : chosen;
    document.documentElement.dataset.theme = theme;
    if (isAndroid) {
      const background = getComputedStyle(document.documentElement).getPropertyValue("--bg").trim();
      setSystemBars(theme === "light", background).catch(() => {});
    }
  }
}

export const prefs = new Prefs();
