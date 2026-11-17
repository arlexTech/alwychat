// After the web build: (1) gives the service worker (media-sw.js) this build's id and file list,
// so the app is stored on the device and starts without network; (2) writes Brotli copies
// (.br, best level) next to the files, served by Caddy's `precompressed` (docker/web/Caddyfile):
// about 30 % smaller than compressing on the fly, for the 18 MB engine most of all.
import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import zlib from "node:zlib";

const root = path.resolve(process.argv[2] ?? "build-web");
const all = [];
(function walk(dir) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) walk(full);
    else all.push(path.relative(root, full).split(path.sep).join("/"));
  }
})(root);
const files = all.filter((f) => !/\.(br|gz|zst)$/.test(f));

// (0) index.html only preloads the layout's code; the page's own pieces were found one round
// trip at a time (HTML → layout → page → its imports), which on a slow network is most of the
// wait. List every page's static imports in index.html so they all come at once.
{
  const immutable = path.join(root, "_app/immutable");
  const imports = (file) =>
    [...fs.readFileSync(file, "utf8").matchAll(/(?:from|import)\s*["'`](\.{1,2}\/[^"'`]+\.js)["'`]/g)].map((m) =>
      path.resolve(path.dirname(file), m[1]),
    );
  const wanted = new Set();
  const visit = (file) => {
    if (wanted.has(file) || !fs.existsSync(file)) return;
    wanted.add(file);
    imports(file).forEach(visit);
  };
  for (const node of fs.readdirSync(path.join(immutable, "nodes"))) visit(path.join(immutable, "nodes", node));
  const indexPath = path.join(root, "index.html");
  let index = fs.readFileSync(indexPath, "utf8");
  const links = [...wanted]
    .map((f) => "/" + path.relative(root, f).split(path.sep).join("/"))
    .filter((href) => !index.includes(`"${href}"`))
    .map((href) => `<link rel="modulepreload" href="${href}">`);
  index = index.replace("</head>", `\t\t${links.join("\n\t\t")}\n\t</head>`);
  fs.writeFileSync(indexPath, index);
  console.log(`index.html: ${links.length} more pieces announced up front`);
}

// Stored for offline start: everything but this worker and the fonts (emoji fonts are big and
// loaded only when used; the worker keeps those it sees).
const stored = files.filter((f) => f !== "media-sw.js" && !/\.(woff2?|ttf|otf)$/.test(f)).sort();
const hash = createHash("sha256");
for (const f of stored) hash.update(f).update(fs.readFileSync(path.join(root, f)));
const stamp = new Date().toISOString().replace(/\D/g, "").slice(0, 14);
const build = { version: `${stamp}-${hash.digest("hex").slice(0, 10)}`, files: stored };
const sw = path.join(root, "media-sw.js");
const source = fs.readFileSync(sw, "utf8");
const filled = source.replace(/\/\*BUILD\*\/[\s\S]*?\/\*END\*\//, `/*BUILD*/ ${JSON.stringify(build)} /*END*/`);
if (filled === source) throw new Error("media-sw.js: build placeholder not found");
fs.writeFileSync(sw, filled);

let before = 0, after = 0;
for (const f of files) {
  if (!/\.(js|mjs|css|html|json|svg|wasm|webmanifest|txt|ttf)$/.test(f)) continue;
  const data = fs.readFileSync(path.join(root, f));
  if (data.length < 1024) continue;
  const br = zlib.brotliCompressSync(data, {
    params: {
      [zlib.constants.BROTLI_PARAM_QUALITY]: 11,
      [zlib.constants.BROTLI_PARAM_LGWIN]: 24,
      [zlib.constants.BROTLI_PARAM_SIZE_HINT]: data.length,
    },
  });
  if (br.length < data.length * 0.9) {
    fs.writeFileSync(path.join(root, f) + ".br", br);
    before += data.length;
    after += br.length;
  }
}
console.log(`service worker: build ${build.version}, ${stored.length} files stored for offline start`);
console.log(`brotli: ${(before / 1e6).toFixed(1)} MB → ${(after / 1e6).toFixed(1)} MB`);
