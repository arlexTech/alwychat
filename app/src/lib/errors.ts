import { tr } from "$lib/i18n/index.svelte";

/** An error for people: network failures in plain words, and no server URLs (the core's
 * HTTP errors quote the whole request). */
export function friendlyError(e: unknown): string {
  const text = String(e).replace(/^Error: /, "");
  if (/error sending request|connection (refused|reset|closed)|timed? ?out|dns|network|unreachable/i.test(text)) {
    return tr("Can't reach your server. Check your connection.");
  }
  return text.replace(/\s*\(?https?:\/\/\S+\)?/g, "").trim();
}
