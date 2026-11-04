// The web version offers the app for the visitor's system (notifications with the tab closed,
// calls, saving to the gallery…): once as a banner in the chat list, and always in Settings → About.
// Files of the GitHub "beta" release (the only one published so far), versions that update themselves.
import { isWeb } from "$lib/backend";

const BASE = "https://github.com/arlexTech/alwychat/releases/download/beta/";
/** Every download, with install steps (the README). */
export const ALL_DOWNLOADS = "https://github.com/arlexTech/alwychat#which-file-do-i-download";

export interface NativeApp {
  /** Shown as "Get the AlwyChat app for {system}". */
  system: string;
  url: string;
}

/** The app for this browser's system, or null (iOS has no download: SideStore only). */
export function nativeApp(): NativeApp | null {
  const ua = navigator.userAgent;
  if (/Android/.test(ua)) return { system: "Android", url: `${BASE}AlwyChat-android-autoupdate.apk` };
  // iPadOS reports itself as a Mac: a touch screen gives it away.
  if (/iPhone|iPad|iPod/.test(ua) || (/Macintosh/.test(ua) && navigator.maxTouchPoints > 1)) return null;
  if (/Windows/.test(ua)) return { system: "Windows", url: `${BASE}AlwyChat-windows-x64-autoupdate-setup.exe` };
  if (/Macintosh/.test(ua)) return { system: "macOS", url: `${BASE}AlwyChat-macos-autoupdate.dmg` };
  if (/Linux|X11/.test(ua)) return { system: "Linux", url: `${BASE}AlwyChat-linux-x86_64-autoupdate.AppImage` };
  return null;
}

const OFFERED_KEY = "alwychat.appOffered";

/** The one-time banner: on the web, for a system with an app, until downloaded or dismissed. */
class AppOffer {
  shown = $state(isWeb && !!nativeApp() && localStorage.getItem(OFFERED_KEY) === null);
  dismiss() {
    this.shown = false;
    localStorage.setItem(OFFERED_KEY, "1");
  }
}
export const appOffer = new AppOffer();
