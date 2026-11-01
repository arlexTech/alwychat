// Web version: answers the UI's /_media/… requests (the apps' media:// scheme) by asking the
// AlwyChat page, whose WebAssembly core downloads and decrypts the file. Nothing is cached
// here: decrypted media only lives in memory (the core's own store keeps the encrypted copy).

self.addEventListener("install", () => self.skipWaiting());
self.addEventListener("activate", (event) => event.waitUntil(self.clients.claim()));

const PREFIX = new URL("_media/", self.registration.scope).pathname;

self.addEventListener("fetch", (event) => {
  const url = new URL(event.request.url);
  if (url.origin !== location.origin || !url.pathname.startsWith(PREFIX)) return;
  event.respondWith(serve(event, url));
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
