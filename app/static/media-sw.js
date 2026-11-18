// Web version's service worker, two jobs:
// 1. The app itself, kept on the device: every file of a build (engine included) is stored once,
//    and the app starts from that copy with no network at all; a new build is fetched in the
//    background (the browser re-checks this file) and used from the next start. The previous
//    build's copy stays, so a page that is still open can load its remaining pieces.
// 2. Media: answers the UI's /_media/… requests (the apps' media:// scheme) by asking the
//    AlwyChat page, whose WebAssembly core downloads and decrypts the file. Nothing is cached
//    for that: decrypted media only lives in memory (the core's store keeps the encrypted copy).

// Filled in by scripts/build-web.sh: the build's id and its files (paths relative to the scope).
const BUILD = /*BUILD*/ { version: "dev", files: [] } /*END*/;
const CACHE = `alwychat-app-${BUILD.version}`;
const scope = new URL(self.registration.scope);

self.addEventListener("install", (event) => {
  event.waitUntil(
    (async () => {
      if (!BUILD.files.length) return;
      const cache = await caches.open(CACHE);
      // Files with a content hash ("_app/immutable/") never change: "force-cache" takes what the
      // page just downloaded from the browser's cache instead of the network again (this worker
      // is only registered once the page has loaded the engine). The others (index.html…) keep
      // their name across builds: "no-cache" asks the server, or a new build would be stored
      // with the previous build's index.html and the app would never update.
      // All or nothing: a half-stored build would start broken offline, so a failure keeps the
      // previous build and the browser tries again later.
      await Promise.all(
        BUILD.files.map(async (path) => {
          const url = new URL(path, scope);
          const hashed = url.pathname.startsWith(new URL("_app/immutable/", scope).pathname);
          const response = await fetch(url, { cache: hashed ? "force-cache" : "no-cache" });
          if (!response.ok) throw new Error(`${path}: ${response.status}`);
          await cache.put(url, response);
        }),
      );
    })().then(() => self.skipWaiting()),
  );
});

self.addEventListener("activate", (event) =>
  event.waitUntil(
    (async () => {
      // Keep this build and the one before it (builds are named in time order).
      const names = (await caches.keys()).filter((n) => n.startsWith("alwychat-app-")).sort();
      const keep = new Set([CACHE, ...names.filter((n) => n < CACHE).slice(-1)]);
      await Promise.all(names.filter((n) => !keep.has(n)).map((n) => caches.delete(n)));
      await self.clients.claim();
    })(),
  ),
);

/** A stored copy of `url`, from this build or the previous one. */
async function stored(url) {
  for (const name of (await caches.keys()).filter((n) => n.startsWith("alwychat-app-")).sort().reverse()) {
    const hit = await (await caches.open(name)).match(url);
    if (hit) return hit;
  }
  return undefined;
}

async function app(request, url) {
  if (request.mode === "navigate") {
    // Any page of the app is index.html (single-page app), except real files like the
    // sign-in callback page.
    const own = await stored(url.origin + url.pathname);
    if (own) return own;
    const index = await stored(new URL("index.html", scope).href);
    if (index && !/\.[a-z0-9]+$/i.test(url.pathname)) return index;
    return fetch(request);
  }
  const hit = await stored(url.origin + url.pathname);
  if (hit) return hit;
  const response = await fetch(request);
  // Files with a content hash never change (emoji fonts, pieces of older builds): keep them.
  if (response.ok && url.pathname.startsWith(new URL("_app/immutable/", scope).pathname)) {
    const copy = response.clone();
    caches.open(CACHE).then((cache) => cache.put(url.origin + url.pathname, copy));
  }
  return response;
}

const PREFIX = new URL("_media/", self.registration.scope).pathname;

self.addEventListener("fetch", (event) => {
  const url = new URL(event.request.url);
  if (url.origin !== location.origin || event.request.method !== "GET") return;
  if (url.pathname.startsWith(PREFIX)) return event.respondWith(serve(event, url));
  // (Pages are the same whatever their query, e.g. /register?token=…; other requests with a
  // query aren't app files.)
  if (BUILD.files.length && !url.pathname.endsWith("/media-sw.js") && (event.request.mode === "navigate" || !url.search)) {
    event.respondWith(app(event.request, url));
  }
});

async function serve(event, url) {
  const client = (event.clientId && (await self.clients.get(event.clientId))) || (await anyPage());
  if (!client) return new Response(null, { status: 503 });
  const path = url.pathname.slice(PREFIX.length);
  let media;
  try {
    media = await ask(client, { path, query: url.search });
  } catch {
    return new Response(null, { status: 404 });
  }
  return respond(event.request, media.bytes, media.mime);
}

async function anyPage() {
  const pages = await self.clients.matchAll({ type: "window" });
  return pages[0];
}

function ask(client, request) {
  return new Promise((resolve, reject) => {
    const channel = new MessageChannel();
    const timer = setTimeout(() => reject(new Error("timeout")), 120_000);
    channel.port1.onmessage = ({ data }) => {
      clearTimeout(timer);
      data.error ? reject(new Error(data.error)) : resolve(data);
    };
    client.postMessage({ type: "alwychat-media", ...request }, [channel.port2]);
  });
}

/** Full body, or the requested Range (video players need it to play and seek). */
function respond(request, bytes, mime) {
  // Media comes from other people: never let it run as a page or be sniffed as another type.
  const safeType = /html|xml|javascript/i.test(mime) ? "application/octet-stream" : mime;
  const headers = {
    "Content-Type": safeType,
    "Accept-Ranges": "bytes",
    "X-Content-Type-Options": "nosniff",
    "Content-Security-Policy": "default-src 'none'; sandbox",
  };
  const total = bytes.byteLength;
  const range = /^bytes=(\d+)-(\d*)$/.exec(request.headers.get("Range") ?? "");
  if (range) {
    const start = Number(range[1]);
    const end = range[2] ? Math.min(Number(range[2]), total - 1) : total - 1;
    if (start <= end && start < total) {
      return new Response(bytes.slice(start, end + 1), {
        status: 206,
        headers: { ...headers, "Content-Range": `bytes ${start}-${end}/${total}` },
      });
    }
  }
  return new Response(bytes, { headers });
}
