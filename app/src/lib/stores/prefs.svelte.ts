import { getSettings, hasBackend, isAndroid, setSettings, setSystemBars, type AppTheme, type Settings } from "$lib/backend";

/** This device's settings, shared by the settings page and what they affect (theme, effects). */
class Prefs {
  settings = $state<Settings | null>(null);
  #dark = window.matchMedia("(prefers-color-scheme: dark)");

  async load() {
    this.#dark.addEventListener("change", () => this.#applyTheme());
    if (!hasBackend) return;
    this.settings = await getSettings();
    this.#applyTheme();
  }

  /** Optimistic: shown at once, rolled back (and thrown) if it can't be saved. */
  async save(change: Partial<Settings>) {
    if (!this.settings) return;
    const previous = this.settings;
    this.settings = { ...this.settings, ...change };
    if ("app_theme" in change) this.#applyTheme();
    try {
      await setSettings(this.settings);
    } catch (e) {
      this.settings = previous;
      this.#applyTheme();
      throw e;
    }
  }

  get effects() {
    return this.settings?.chat_effects ?? true;
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
