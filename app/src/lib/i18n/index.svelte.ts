// Translations. The English text is the key: `tr("Mute for me")`, `tr("Seen by {names}", { names })`.
// Each language is one JSON file in ./locales (`fr.json`: { "Mute for me": "Couper pour moi" }),
// found automatically: adding a language = adding its file (`node scripts/i18n.mjs --add de`
// creates it with every string to translate). Missing entries fall back to English.
// Same English, different meanings: `trc("chat", "Mute")` (key "chat|Mute" in locale files).
// Plurals: `trn(count, "{n} reply", "{n} replies")`; in a locale file, an object keyed by the
// language's plural forms (Intl.PluralRules): { "one": "{n} réponse", "other": "{n} réponses" }.
// Files are bundled (small) and changing the language reloads the app, so `tr` can be used
// anywhere, even in constants.

type Entry = string | Partial<Record<Intl.LDMLPluralRule, string>>;
type Catalog = Record<string, Entry>;
type Vars = Record<string, unknown>;

const files = import.meta.glob<Catalog>("./locales/*.json", { eager: true, import: "default" });
const catalogs: Record<string, Catalog> = Object.fromEntries(
  Object.entries(files).map(([path, catalog]) => [path.match(/([\w-]+)\.json$/)![1], catalog]),
);
/** Languages with a translation file (English is the source and always there). */
export const LANGUAGES = ["en", ...Object.keys(catalogs)].sort();

const STORAGE = "alwychat.language";

/** "fr-BE" → "fr" when only "fr" exists. */
function supported(tag: string): string | null {
  const lower = tag.toLowerCase();
  return LANGUAGES.find((l) => l.toLowerCase() === lower) ?? LANGUAGES.find((l) => l === lower.split("-")[0]) ?? null;
}

function systemLanguage(): string {
  for (const tag of typeof navigator === "undefined" ? [] : (navigator.languages ?? [navigator.language])) {
    const found = supported(tag);
    if (found) return found;
  }
  return "en";
}

/** "auto" (follow the system) or a language code. */
export const languageChoice: string = (typeof localStorage !== "undefined" && localStorage.getItem(STORAGE)) || "auto";
/** The language in use: a code of LANGUAGES, also for dates and numbers. */
export const locale: string = languageChoice === "auto" ? systemLanguage() : (supported(languageChoice) ?? "en");
const catalog: Catalog = catalogs[locale] ?? {};
const plurals = new Intl.PluralRules(locale);
if (typeof document !== "undefined") document.documentElement.lang = locale;

/** Pick a language ("auto": the system's), saved on this device; the app reloads in it. */
export function chooseLanguage(choice: string) {
  localStorage.setItem(STORAGE, choice);
  location.reload();
}

/** Name of a language in that language ("Français"), for the picker. */
export function languageName(code: string): string {
  try {
    const name = new Intl.DisplayNames([code], { type: "language" }).of(code) ?? code;
    return name.charAt(0).toLocaleUpperCase(code) + name.slice(1);
  } catch {
    return code;
  }
}

function fill(text: string, vars?: Vars): string {
  if (!vars) return text;
  return text.replace(/\{(\w+)\}/g, (match, name: string) => (name in vars ? String(vars[name] ?? "") : match));
}

/** Translate (English text as the key). */
export function tr(text: string, vars?: Vars): string {
  const entry = catalog[text];
  const out = typeof entry === "string" && entry ? entry : typeof entry === "object" ? (entry.other ?? text) : text;
  return fill(out, vars);
}

/** Translate with a count, `{n}` in the text: `trn(n, "{n} reply", "{n} replies")`. */
export function trn(count: number, one: string, other: string, vars?: Vars): string {
  const entry = catalog[other];
  const all = { n: count, ...vars };
  if (typeof entry === "object") {
    const form = entry[plurals.select(count)] ?? entry.other;
    if (form) return fill(form, all);
  } else if (typeof entry === "string" && entry) {
    return fill(entry, all);
  }
  return fill(count === 1 ? one : other, all);
}

/** Translate text whose meaning depends on where it is: `trc("chat", "Mute")` vs the call's "Mute". */
export function trc(context: string, text: string, vars?: Vars): string {
  const entry = catalog[`${context}|${text}`];
  return typeof entry === "string" && entry ? fill(entry, vars) : tr(text, vars);
}
