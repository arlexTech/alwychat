// Web version: the session (tokens, the encryption store's passphrase) is sealed with AES-GCM
// under a key the browser keeps non-extractable in IndexedDB, as Element Web does with its
// pickle key. The page can use the key but no script can read it out, so copying localStorage
// or IndexedDB values elsewhere gives nothing usable. (Code running in the page itself can
// still unseal: that is the limit of any web client.)

const DB = "alwychat-keys";
const STORE = "keys";
const ID = "session";
const PREFIX = "sealed1:";

function openDb() {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(DB, 1);
    request.onupgradeneeded = () => request.result.createObjectStore(STORE);
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });
}

/** The key, made on first use. Two tabs starting at once agree on one (first one stored wins). */
async function loadKey() {
  const db = await openDb();
  try {
    const existing = await new Promise((resolve, reject) => {
      const request = db.transaction(STORE, "readonly").objectStore(STORE).get(ID);
      request.onsuccess = () => resolve(request.result);
      request.onerror = () => reject(request.error);
    });
    if (existing) return existing;
    const fresh = await crypto.subtle.generateKey({ name: "AES-GCM", length: 256 }, false, ["encrypt", "decrypt"]);
    return await new Promise((resolve, reject) => {
      // One transaction: keep a key another tab stored meanwhile.
      const tx = db.transaction(STORE, "readwrite");
      const store = tx.objectStore(STORE);
      let chosen = fresh;
      const get = store.get(ID);
      get.onsuccess = () => {
        if (get.result) chosen = get.result;
        else store.put(fresh, ID);
      };
      tx.oncomplete = () => resolve(chosen);
      tx.onerror = () => reject(tx.error);
    });
  } finally {
    db.close();
  }
}

/** @type {Promise<CryptoKey> | null} */
let key = null;
function sessionKey() {
  key ??= loadKey().catch((e) => {
    key = null;
    throw e;
  });
  return key;
}

/** @param {Uint8Array} bytes */
function toBase64(bytes) {
  let text = "";
  for (let i = 0; i < bytes.length; i += 0x8000) text += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return btoa(text);
}

/** Text → "sealed1:" + base64(iv ‖ ciphertext). @param {string} text */
export async function seal(text) {
  const iv = crypto.getRandomValues(new Uint8Array(12));
  const data = new Uint8Array(
    await crypto.subtle.encrypt({ name: "AES-GCM", iv }, await sessionKey(), new TextEncoder().encode(text)),
  );
  const out = new Uint8Array(iv.length + data.length);
  out.set(iv);
  out.set(data, iv.length);
  return PREFIX + toBase64(out);
}

/** The text `seal` was given; throws if it can't be opened (other key, damaged). @param {string} sealed */
export async function unseal(sealed) {
  if (!sealed.startsWith(PREFIX)) throw new Error("not sealed");
  const bytes = Uint8Array.from(atob(sealed.slice(PREFIX.length)), (c) => c.charCodeAt(0));
  const plain = await crypto.subtle.decrypt({ name: "AES-GCM", iv: bytes.subarray(0, 12) }, await sessionKey(), bytes.subarray(12));
  return new TextDecoder().decode(plain);
}

/** @param {string} text */
export function isSealed(text) {
  return text.startsWith(PREFIX);
}
