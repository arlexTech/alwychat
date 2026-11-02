#!/usr/bin/env node
// Translation files (src/lib/i18n/locales/*.json) against the strings the code uses.
//   node scripts/i18n.mjs            report missing / unused strings per language
//   node scripts/i18n.mjs --sync     add missing strings (empty = still English) and drop unused ones
//   node scripts/i18n.mjs --add de   start a new language with every string to translate
// Strings are the English texts given to tr("…"), trc("context", "…") (key "context|…") and
// trn(n, "…", "…") (the plural's second text
// is the key; translate it as { "one": "…", "other": "…" } with the language's plural forms).
import fs from "node:fs";
import path from "node:path";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname), "..");
const src = path.join(root, "src");
const dir = path.join(src, "lib/i18n/locales");

function files(d) {
  return fs.readdirSync(d, { withFileTypes: true }).flatMap((e) => {
    const p = path.join(d, e.name);
    if (e.isDirectory()) return e.name === "locales" ? [] : files(p);
    return /\.(svelte|ts)$/.test(e.name) ? [p] : [];
  });
}

const STRING = String.raw`"((?:[^"\\]|\\.)*)"`;
const used = new Set();
for (const file of files(src)) {
  const text = fs.readFileSync(file, "utf8");
  for (const m of text.matchAll(new RegExp(String.raw`\btr\(\s*${STRING}`, "g"))) used.add(JSON.parse(`"${m[1]}"`));
  for (const m of text.matchAll(new RegExp(String.raw`\btrc\(\s*${STRING}\s*,\s*${STRING}`, "g"))) {
    used.add(`${JSON.parse(`"${m[1]}"`)}|${JSON.parse(`"${m[2]}"`)}`);
  }
  for (const m of text.matchAll(new RegExp(String.raw`\btrn\([^,]+,\s*${STRING}\s*,\s*${STRING}`, "g"))) {
    used.add(JSON.parse(`"${m[2]}"`));
  }
}

const args = process.argv.slice(2);
const add = args.includes("--add") ? args[args.indexOf("--add") + 1] : null;
if (add) {
  const file = path.join(dir, `${add}.json`);
  if (fs.existsSync(file)) throw new Error(`${file} exists`);
  fs.writeFileSync(file, JSON.stringify(Object.fromEntries([...used].sort().map((k) => [k, ""])), null, 2) + "\n");
  console.log(`${file}: ${used.size} strings to translate`);
  process.exit(0);
}

const sync = args.includes("--sync");
let incomplete = false;
for (const name of fs.readdirSync(dir).filter((f) => f.endsWith(".json")).sort()) {
  const file = path.join(dir, name);
  const catalog = JSON.parse(fs.readFileSync(file, "utf8"));
  const missing = [...used].filter((k) => !catalog[k]);
  const unused = Object.keys(catalog).filter((k) => !used.has(k));
  console.log(`${name}: ${used.size - missing.length}/${used.size} translated, ${unused.length} unused`);
  for (const k of missing.slice(0, 20)) console.log(`  missing: ${k}`);
  if (missing.length > 20) console.log(`  … ${missing.length - 20} more`);
  if (missing.length) incomplete = true;
  if (sync) {
    const next = Object.fromEntries([...used].sort().map((k) => [k, catalog[k] ?? ""]));
    fs.writeFileSync(file, JSON.stringify(next, null, 2) + "\n");
  }
}
process.exitCode = incomplete && !sync ? 1 : 0;
