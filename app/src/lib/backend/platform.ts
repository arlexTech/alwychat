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
const wasmModules = import.meta.glob<WebCore>("./wasm/alwy_web.js");
const wasmLoader = Object.values(wasmModules)[0];
export const isWeb = !isTauri && !!wasmLoader;

type Handler = (event: { payload: unknown }) => void;
const listeners = new Map<string, Set<Handler>>();

let webCore: Promise<WebCore> | null = null;
function web(): Promise<WebCore> {
  webCore ??= oneTab().then(() => wasmLoader!()).then(async (mod) => {
    await mod.default();
    mod.setEventHandler((name: string, payload: unknown) => {
      for (const handler of listeners.get(name) ?? []) handler({ payload });
    });
    await startMediaWorker(mod);
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
    void navigator.locks.query().then((state) => {
      if (!state.held?.some((l) => l.name === "alwychat-core")) return void hold();
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
  await navigator.serviceWorker.register(new URL("media-sw.js", location.href), { scope: "./" });
  if (navigator.serviceWorker.controller) return;
  await new Promise<void>((resolve) => {
    navigator.serviceWorker.addEventListener("controllerchange", () => resolve(), { once: true });
    setTimeout(resolve, 5000);
  });
}

export async function invoke<T>(cmd: string, args?: unknown, options?: core.InvokeOptions): Promise<T> {
  if (isTauri) return core.invoke<T>(cmd, args as core.InvokeArgs, options);
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

/** Web sign-in: the server's page opens in a popup, which comes back to oauth-callback.html. */
export const loginRedirect = () => new URL("oauth-callback.html", location.href).href;

/** Open the sign-in popup right away (browsers only allow popups straight from a click), then
 * point it at the server's page once the core has it. */
export async function webLogin(server: string): Promise<unknown> {
  const popup = window.open("about:blank", "alwychat-login", "popup,width=480,height=720");
  const mod = await web();
  const unlisten = await listen<string>("login-url", ({ payload }) => {
    if (popup && !popup.closed) popup.location.href = payload;
    else window.open(payload, "alwychat-login", "popup,width=480,height=720");
  });
  // The callback page (static/oauth-callback.js) answers on both: postMessage needs the opener,
  // which the server's sign-in page often cuts (Cross-Origin-Opener-Policy); the channel doesn't.
  let done = false;
  const finish = (data: unknown) => {
    const d = data as { type?: string; url?: string } | null;
    if (done || d?.type !== "alwychat-login" || !d.url) return;
    done = true;
    mod.finishLogin(String(d.url));
  };
  const onMessage = (e: MessageEvent) => e.origin === location.origin && finish(e.data);
  window.addEventListener("message", onMessage);
  const channel = new BroadcastChannel("alwychat-login");
  channel.onmessage = (e) => finish(e.data);
  // No cancel when the popup looks closed: once the opener link is cut, `closed` reads true while
  // the sign-in is still going on. The waiting screen has its own Cancel.
  try {
    return await mod.invoke("login", { server, redirect: loginRedirect() });
  } finally {
    channel.close();
    unlisten();
    window.removeEventListener("message", onMessage);
    if (popup && !popup.closed) popup.close();
  }
}
