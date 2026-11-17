// Web version's sign-in, done by the page itself: looking up the server, registering the app,
// the server's sign-in page and the token exchange are a few small requests, so nobody waits for
// the 18 MB core to download before signing in. The core only receives the result
// (`login_with_tokens`), by when it has long finished loading in the background.
// The apps keep the core's own sign-in (`Core::login`): the same requests, from Rust.
import type { ServerInfo } from "./index";
import { tr } from "$lib/i18n/index.svelte";

interface AuthMetadata {
  issuer: string;
  authorization_endpoint: string;
  token_endpoint: string;
  registration_endpoint?: string;
}
interface Server {
  info: ServerInfo;
  auth: AuthMetadata | null;
}
/** One sign-in attempt, ready to open (single use). */
interface Attempt {
  server: Server;
  clientId: string;
  url: string;
  state: string;
  verifier: string;
  deviceId: string;
  redirect: string;
}
export interface SignedIn {
  homeserver: string;
  client_id: string;
  user_id: string;
  device_id: string;
  access_token: string;
  refresh_token: string | null;
}

const lookups = new Map<string, Promise<Server>>();

async function json<T>(url: string, init?: RequestInit): Promise<T> {
  const response = await fetch(url, init);
  if (!response.ok) throw new Error(`${new URL(url).host}: HTTP ${response.status}`);
  return response.json() as Promise<T>;
}

/** The connection dropped (phones switching networks, the browser resetting its connection to the
 * server when the tab comes back): the request may be repeated. */
const dropped = (e: unknown) => e instanceof TypeError || (e instanceof DOMException && e.name === "AbortError");

/** `json` for requests that are safe to repeat (lookups, whoami): tries 3 times on a dropped
 * connection. Never for the token exchange: its code works only once. */
async function again<T>(url: string, init?: RequestInit): Promise<T> {
  for (let attempt = 1; ; attempt++) {
    try {
      return await json<T>(url, init);
    } catch (e) {
      if (attempt >= 3 || !dropped(e)) throw e;
      await new Promise((r) => setTimeout(r, attempt * 500));
    }
  }
}

/** A message people understand for a dropped connection. */
export function signInError(e: unknown): Error {
  return dropped(e) ? new Error(tr("The connection dropped while signing in. Please try again.")) : (e as Error);
}

/** Resolve `name` (e.g. matrix.org) through .well-known and see what it supports. Same checks as
 * the core's `discover`. Remembered for the page's life. */
export function lookUpServer(name: string): Promise<Server> {
  const key = name.trim().toLowerCase();
  let found = lookups.get(key);
  if (!found) {
    found = (async () => {
      const base = /^https?:\/\//.test(key) ? key : `https://${key}`;
      let homeserver = base;
      if (!/^https?:\/\//.test(key)) {
        try {
          const wk = await again<{ "m.homeserver"?: { base_url?: string } }>(`${base}/.well-known/matrix/client`);
          if (wk["m.homeserver"]?.base_url) homeserver = wk["m.homeserver"].base_url;
        } catch {
          // No .well-known: the name is the homeserver itself.
        }
      }
      homeserver = homeserver.replace(/\/+$/, "");
      const versions = await again<{ versions: string[]; unstable_features?: Record<string, boolean> }>(
        `${homeserver}/_matrix/client/versions`,
      );
      let auth: AuthMetadata | null = null;
      for (const path of ["/_matrix/client/v1/auth_metadata", "/_matrix/client/unstable/org.matrix.msc2965/auth_metadata"]) {
        try {
          auth = await again<AuthMetadata>(homeserver + path);
          break;
        } catch {
          // Try the older address, else: no browser sign-in on this server.
        }
      }
      return {
        info: {
          homeserver: homeserver + "/",
          versions: versions.versions,
          oauth_issuer: auth?.issuer ?? null,
          sliding_sync: versions.unstable_features?.["org.matrix.simplified_msc3575"] === true,
        },
        auth,
      };
    })();
    lookups.set(key, found);
    found.catch(() => lookups.delete(key));
  }
  return found;
}

const redirectUri = () => new URL("oauth-callback.html", location.origin + "/").href;

/** The app's registration with the sign-in service: once per browser (the ID stays valid). */
async function clientId(auth: AuthMetadata): Promise<string> {
  const redirect = redirectUri();
  const key = `alwychat.oauthClient:${auth.issuer}|${redirect}`;
  try {
    const known = localStorage.getItem(key);
    if (known) return known;
  } catch {}
  if (!auth.registration_endpoint) throw new Error("This server doesn't let apps register for sign-in.");
  // Exactly what the core registers (session.rs `client_metadata`, web).
  const registered = await json<{ client_id: string }>(auth.registration_endpoint, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      redirect_uris: [redirect],
      token_endpoint_auth_method: "none",
      grant_types: ["authorization_code", "refresh_token"],
      response_types: ["code"],
      application_type: "web",
      client_uri: location.origin + "/",
      client_name: "AlwyChat",
    }),
  });
  try {
    localStorage.setItem(key, registered.client_id);
  } catch {}
  return registered.client_id;
}

function random(chars: string, length: number): string {
  const bytes = crypto.getRandomValues(new Uint8Array(length));
  return Array.from(bytes, (b) => chars[b % chars.length]).join("");
}
const base64url = (bytes: ArrayBuffer) =>
  btoa(String.fromCharCode(...new Uint8Array(bytes)))
    .replace(/\+/g, "-")
    .replace(/\//g, "_")
    .replace(/=+$/, "");

/** Everything for one attempt, so the click can open the server's page at once. */
async function prepare(name: string, invite: string | null): Promise<Attempt> {
  const server = await lookUpServer(name);
  if (!server.auth) throw new Error("This server doesn't support signing in through the browser.");
  const id = await clientId(server.auth);
  const verifier = random("ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~", 64);
  const challenge = base64url(await crypto.subtle.digest("SHA-256", new TextEncoder().encode(verifier)));
  const state = random("ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789", 24);
  // Like the SDK's DeviceId::new: 10 capital letters.
  const deviceId = random("ABCDEFGHIJKLMNOPQRSTUVWXYZ", 10);
  const redirect = redirectUri();
  const url = new URL(server.auth.authorization_endpoint);
  url.search = new URLSearchParams({
    response_type: "code",
    response_mode: "query",
    client_id: id,
    redirect_uri: redirect,
    scope: `urn:matrix:org.matrix.msc2967.client:api:* urn:matrix:org.matrix.msc2967.client:device:${deviceId}`,
    state,
    code_challenge: challenge,
    code_challenge_method: "S256",
    ...(invite !== null ? { prompt: "create" } : {}),
  }).toString();
  // An invite's registration token rides along as #invite=… (see Core::login).
  if (invite) url.hash = new URLSearchParams({ invite }).toString();
  return { server, clientId: id, url: url.href, state, verifier, deviceId, redirect };
}

// The next attempt, prepared while the sign-in screen is shown.
let next: { key: string; attempt: Promise<Attempt>; ready: Attempt | null } | null = null;

/** Get the next sign-in ready in the background (the sign-in screen calls it as soon as the
 * server is known), so the button opens the server's page without waiting. */
export function prepareSignIn(name: string, invite: string | null) {
  const key = `${name.trim().toLowerCase()}|${invite ?? "-"}`;
  if (next?.key === key) return;
  const entry: NonNullable<typeof next> = { key, attempt: prepare(name, invite), ready: null };
  entry.attempt.then(
    (a) => (entry.ready = a),
    () => next === entry && (next = null),
  );
  next = entry;
}

let cancelled: (() => void) | null = null;
export function cancelSignIn() {
  cancelled?.();
}

/** Sign in (or create an account with `invite`): opens the server's page in a popup straight
 * from the click, waits for it to come back, exchanges the code for tokens. */
export async function signIn(name: string, invite: string | null): Promise<SignedIn> {
  const key = `${name.trim().toLowerCase()}|${invite ?? "-"}`;
  const prepared = next?.key === key ? next : null;
  next = null;
  // Browsers only allow a popup straight from the click: open it now, with the page if it's ready.
  const features = "popup,width=480,height=720";
  const popup = prepared?.ready ? window.open(prepared.ready.url, "alwychat-login", features) : window.open("about:blank", "alwychat-login", features);
  const attempt = await (prepared?.attempt ?? prepare(name, invite)).catch((e) => {
    popup?.close();
    throw e;
  });
  if (!prepared?.ready) {
    if (popup && !popup.closed) popup.location.href = attempt.url;
    else window.open(attempt.url, "alwychat-login", features);
  }
  // Ready the one after, in case this one is abandoned.
  queueMicrotask(() => prepareSignIn(name, invite));

  const answer = await new Promise<URL>((resolve, reject) => {
    // The callback page (static/oauth-callback.js) answers on both: postMessage needs the opener,
    // which the server's page often cuts (Cross-Origin-Opener-Policy); the channel doesn't.
    const channel = new BroadcastChannel("alwychat-login");
    const done = () => {
      channel.close();
      window.removeEventListener("message", onMessage);
      cancelled = null;
    };
    const take = (data: unknown) => {
      const d = data as { type?: string; url?: string } | null;
      if (d?.type !== "alwychat-login" || !d.url) return;
      const url = new URL(String(d.url));
      if (url.searchParams.get("state") !== attempt.state) return; // another attempt's answer
      done();
      resolve(url);
    };
    const onMessage = (e: MessageEvent) => e.origin === location.origin && take(e.data);
    window.addEventListener("message", onMessage);
    channel.onmessage = (e) => take(e.data);
    cancelled = () => {
      done();
      reject(new Error("Sign-in cancelled"));
    };
  });
  const error = answer.searchParams.get("error");
  if (error) throw new Error(answer.searchParams.get("error_description") || error);
  const code = answer.searchParams.get("code");
  if (!code) throw new Error("The server's page came back without an answer.");

  const auth = attempt.server.auth!;
  const tokens = await json<{ access_token: string; refresh_token?: string }>(auth.token_endpoint, {
    method: "POST",
    headers: { "Content-Type": "application/x-www-form-urlencoded" },
    body: new URLSearchParams({
      grant_type: "authorization_code",
      code,
      redirect_uri: attempt.redirect,
      client_id: attempt.clientId,
      code_verifier: attempt.verifier,
    }),
  });
  const homeserver = attempt.server.info.homeserver;
  const me = await again<{ user_id: string; device_id?: string }>(`${homeserver}_matrix/client/v3/account/whoami`, {
    headers: { Authorization: `Bearer ${tokens.access_token}` },
  });
  return {
    homeserver,
    client_id: attempt.clientId,
    user_id: me.user_id,
    // The device asked for; the core checks the server agrees.
    device_id: attempt.deviceId,
    access_token: tokens.access_token,
    refresh_token: tokens.refresh_token ?? null,
  };
}
