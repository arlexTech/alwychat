// BlurHash (https://blurha.sh): the blurry preview most Matrix clients send with a photo
// (`info.blurhash`), drawn while the photo loads. Decoded once per hash to a tiny image.

const DIGITS = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz#$%*+,-.:;=?@[]^_{|}~";

function decode83(str: string): number {
  let value = 0;
  for (const c of str) {
    const digit = DIGITS.indexOf(c);
    if (digit < 0) throw new Error("bad blurhash");
    value = value * 83 + digit;
  }
  return value;
}

const toLinear = (v: number) => {
  const x = v / 255;
  return x <= 0.04045 ? x / 12.92 : Math.pow((x + 0.055) / 1.055, 2.4);
};
const toSrgb = (v: number) => {
  const x = Math.max(0, Math.min(1, v));
  return Math.round((x <= 0.0031308 ? x * 12.92 : 1.055 * Math.pow(x, 1 / 2.4) - 0.055) * 255);
};
const signPow = (v: number, e: number) => Math.sign(v) * Math.pow(Math.abs(v), e);

/** RGBA pixels of `hash` at `width`×`height`. Throws on a malformed hash. */
export function decodeBlurhash(hash: string, width: number, height: number): Uint8ClampedArray<ArrayBuffer> {
  if (hash.length < 6) throw new Error("bad blurhash");
  const size = decode83(hash[0]);
  const nx = (size % 9) + 1;
  const ny = Math.floor(size / 9) + 1;
  if (hash.length !== 4 + 2 * nx * ny) throw new Error("bad blurhash");
  const maxAc = (decode83(hash[1]) + 1) / 166;
  const colors: [number, number, number][] = [];
  const dc = decode83(hash.slice(2, 6));
  colors.push([toLinear(dc >> 16), toLinear((dc >> 8) & 255), toLinear(dc & 255)]);
  for (let i = 1; i < nx * ny; i++) {
    const v = decode83(hash.slice(4 + i * 2, 6 + i * 2));
    const q = (n: number) => signPow((n - 9) / 9, 2) * maxAc;
    colors.push([q(Math.floor(v / 361)), q(Math.floor(v / 19) % 19), q(v % 19)]);
  }
  const pixels = new Uint8ClampedArray(new ArrayBuffer(width * height * 4));
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      let r = 0;
      let g = 0;
      let b = 0;
      for (let j = 0; j < ny; j++) {
        for (let i = 0; i < nx; i++) {
          const basis = Math.cos((Math.PI * x * i) / width) * Math.cos((Math.PI * y * j) / height);
          const c = colors[i + j * nx];
          r += c[0] * basis;
          g += c[1] * basis;
          b += c[2] * basis;
        }
      }
      const p = 4 * (x + y * width);
      pixels[p] = toSrgb(r);
      pixels[p + 1] = toSrgb(g);
      pixels[p + 2] = toSrgb(b);
      pixels[p + 3] = 255;
    }
  }
  return pixels;
}

const cache = new Map<string, string | null>();

/** A small image (data URL) of `hash`, to stretch under the photo; null if it can't be decoded. */
export function blurhashUrl(hash: string | null | undefined): string | null {
  if (!hash) return null;
  if (cache.has(hash)) return cache.get(hash)!;
  let url: string | null = null;
  try {
    const w = 32;
    const h = 32;
    const canvas = document.createElement("canvas");
    canvas.width = w;
    canvas.height = h;
    const ctx = canvas.getContext("2d");
    if (ctx) {
      ctx.putImageData(new ImageData(decodeBlurhash(hash, w, h), w, h), 0, 0);
      url = canvas.toDataURL();
    }
  } catch {
    url = null;
  }
  if (cache.size > 500) cache.clear();
  cache.set(hash, url);
  return url;
}
