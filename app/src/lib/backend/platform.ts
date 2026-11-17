// Where the core runs: in the Tauri app (Rust next to the webview), or in the page itself as
// WebAssembly (the web version). Only `index.ts` uses this; the rest of the UI doesn't care.
import * as app from "@tauri-apps/api/app";
import * as core from "@tauri-apps/api/core";
import * as events from "@tauri-apps/api/event";
import * as opener from "@tauri-apps/plugin-opener";
import { version } from "../../../package.json";
import { tr } from "$lib/i18n/index.svelte";

export const isTauri = core.isTauri();

// The WebAssembly build of the core (crates/alwy-web, built by `pnpm build:web` into ./wasm),
// present only in web builds.
interface WebCore {
  default(): Promise<unknown>;
  invoke(cmd: string, args: unknown): Promise<unknown>;
  setEventHandler(handler: (name: string, payload: unknown) => void): void;
  finishLogin(url: string): boolean;
  commands(): string[];
  media(path: string, query: string): Promise<{ bytes: Uint8Array; mime: string }>;
}
// (Only web builds, `VITE_ALWY_WEB`, bundle it: the apps would carry its 18 MB for nothing.)
const wasmModules = import.meta.env.VITE_ALWY_WEB ? import.meta.glob<WebCore>("./wasm/alwy_web.js") : {};
const wasmLoader = Object.values(wasmModules)[0];
export const isWeb = !isTauri && !!wasmLoader;

type Handler = (event: { payload: unknown }) => void;
const listeners = new Map<string, Set<Handler>>();

/** Web: whether a signed-in session is saved in this browser (the core's session file lives in
 * localStorage as `alwychat:<folder>/session.json`). Without one, the sign-in screen can show
 * right away instead of waiting for the 18 MB core to download and start. */
export const webHasSession = () => webHasFile("session.json");

/** Web: whether the core has a file named `name` (its files live in localStorage as
 * `alwychat:<folder>/<name>`). Unknown counts as yes. */
export function webHasFile(name: string): boolean {
  try {
    return Object.keys(localStorage).some((k) => k.startsWith("alwychat:") && (k.endsWith("/" + name) || k === "alwychat:" + name));
  } catch {
    return true;
  }
}

/** Start downloading the core now (web), so it's ready by the time it's needed. */
export const warmUpWeb = () => void (isWeb && web().catch(() => {}));

let webCore: Promise<WebCore> | null = null;
function web(): Promise<WebCore> {
  webCore ??= oneTab().then(() => wasmLoader!()).then(async (mod) => {
    await mod.default();
    mod.setEventHandler((name: string, payload: unknown) => {
      for (const handler of listeners.get(name) ?? []) handler({ payload });
    });
    await startMediaWorker(mod);
    // For bug reports from the browser console: `await alwychatLog()` prints the recent log
    // (the same one "Report a problem" attaches).
    (globalThis as Record<string, unknown>).alwychatLog = () => mod.invoke("recent_log", {});
    return mod;
  });
  return webCore;
}

/**
 * Only one tab may run the core: two would share the browser's encryption store (IndexedDB),
 * which breaks encryption. The tab holding the "alwychat-core" lock runs it; another tab
 * shows "open in another tab" and can take over (the first one then reloads and waits).
 */
function oneTab(): Promise<void> {
  if (!navigator.locks) return Promise.resolve();
  const channel = new BroadcastChannel("alwychat-tab");
  return new Promise((ready) => {
    const hold = () =>
      navigator.locks.request("alwychat-core", () => {
        overlay?.remove();
        ready();
        // Asked to hand over: reloading ends this tab's core and frees the lock.
        channel.onmessage = (e) => e.data === "take-over" && location.reload();
        return new Promise<void>(() => {}); // held while the page lives
      });
    let overlay: HTMLElement | null = null;
    void navigator.locks.query().then(async (state) => {
      if (!state.held?.some((l) => l.name === "alwychat-core")) return void hold();
      // Right after a reload the page being left may still hold it for a moment: wait a little
      // before saying another tab has it.
      try {
        return await navigator.locks.request("alwychat-core", { signal: AbortSignal.timeout(2000) }, () => {
          ready();
          channel.onmessage = (e) => e.data === "take-over" && location.reload();
          return new Promise<void>(() => {});
        });
      } catch {
        // Still held: another tab really runs it.
      }
      overlay = elsewhere(() => {
        channel.postMessage("take-over");
        void hold();
      });
    });
  });
}

/** Full-page "AlwyChat is open in another tab", with "Use it here". */
function elsewhere(useHere: () => void): HTMLElement {
  const page = document.createElement("div");
  page.setAttribute("style", "position:fixed;inset:0;z-index:1000;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:16px;padding:24px;background:var(--bg,#0e1014);color:var(--text,#eee);font:16px system-ui,sans-serif;text-align:center");
  const title = document.createElement("strong");
  title.textContent = tr("AlwyChat is open in another tab");
  const hint = document.createElement("p");
  hint.setAttribute("style", "margin:0;max-width:380px;color:var(--muted,#999)");
  hint.textContent = tr("It can only run in one tab at a time, to keep your encryption keys safe.");
  const button = document.createElement("button");
  button.setAttribute("style", "padding:10px 20px;border:none;border-radius:999px;background:var(--accent-a,#5b5bd6);color:#fff;font:inherit;cursor:pointer");
  button.textContent = tr("Use it here");
  button.onclick = () => {
    button.disabled = true;
    useHere();
  };
  page.append(title, hint, button);
  document.body.append(page);
  return page;
}

/** media-sw.js serves /_media/… (the apps' media:// scheme) with bytes from the core. Media
 * URLs only work once it controls the page, so the first command waits for it. */
async function startMediaWorker(mod: WebCore) {
  if (!("serviceWorker" in navigator)) return;
  navigator.serviceWorker.addEventListener("message", (event: MessageEvent) => {
    const { data } = event;
    if (data?.type !== "alwychat-media") return;
    const port = event.ports[0];
    mod.media(data.path, data.query).then(
      (media) => port.postMessage(media, [media.bytes.buffer]),
      (e) => port.postMessage({ error: String(e) }),
    );
  });
  try {
    await navigator.serviceWorker.register(new URL("media-sw.js", location.href), { scope: "./" });
  } catch (e) {
    // Offline the browser can't re-check the worker: the installed one keeps working.
    if (!navigator.serviceWorker.controller) console.warn("service worker:", e);
    return;
  }
  if (navigator.serviceWorker.controller) return;
  await new Promise<void>((resolve) => {
    navigator.serviceWorker.addEventListener("controllerchange", () => resolve(), { once: true });
    setTimeout(resolve, 5000);
  });
}

/** Web: while a saved session is being restored (the app is already on screen), other commands
 * wait for it, so nothing reaches the core before it has an account. */
let restoring: Promise<unknown> | null = null;
const NO_WAIT = new Set(["restore_session", "login", "cancel_login", "discover_server", "recent_log"]);
export function holdUntilRestored(restore: Promise<unknown>) {
  restoring = restore.catch(() => {});
  restoring.then(() => (restoring = null));
}

export async function invoke<T>(cmd: string, args?: unknown, options?: core.InvokeOptions): Promise<T> {
  if (isTauri) return core.invoke<T>(cmd, args as core.InvokeArgs, options);
  if (restoring && !NO_WAIT.has(cmd)) await restoring;
  return (await web()).invoke(cmd, args ?? {}) as Promise<T>;
}

/** Same shape as Tauri's `Channel`: the core calls `onmessage` with each message. */
class WebChannel<T> {
  onmessage: (message: T) => void = () => {};
}
export const Channel = (isTauri ? core.Channel : WebChannel) as typeof core.Channel;
export type Channel<T> = core.Channel<T>;

export function listen<T>(name: string, handler: (event: { payload: T }) => void): Promise<() => void> {
  if (isTauri) return events.listen<T>(name, handler);
  const set = listeners.get(name) ?? new Set();
  listeners.set(name, set);
  set.add(handler as Handler);
  return Promise.resolve(() => void set.delete(handler as Handler));
}

export const getVersion = () => (isTauri ? app.getVersion() : Promise.resolve(version));

export async function onBackButtonPress(handler: () => void) {
  if (isTauri) return app.onBackButtonPress(handler);
  return { unregister: async () => {} };
}

export async function openUrl(url: string) {
  if (isTauri) return opener.openUrl(url);
  window.open(url, "_blank", "noopener,noreferrer");
}

