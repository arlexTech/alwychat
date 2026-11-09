// Fixed English texts the Rust core puts in chat previews and notifications
// (`crates/alwy-core/src/rooms.rs` `preview` / `latest_message`, `push.rs`), in the app's
// language. Written out one by one so `pnpm i18n` sees them.
import { tr } from "$lib/i18n/index.svelte";

export function coreTexts(): Record<string, string> {
  return {
    Someone: tr("Someone"),
    "Invited you": tr("Invited you"),
    "Message deleted": tr("Message deleted"),
    "🔒 New message": tr("🔒 New message"),
    "📷 Photo": tr("📷 Photo"),
    "🎬 Video": tr("🎬 Video"),
    "🎤 Voice message": tr("🎤 Voice message"),
    "🎵 Audio": tr("🎵 Audio"),
    "📍 Location": tr("📍 Location"),
    Sticker: tr("Sticker"),
    "📊 Poll": tr("📊 Poll"),
  };
}

let cache: Record<string, string> | undefined;

/** A preview from the core, translated when it's one of its fixed texts. */
export function coreText(text: string): string {
  cache ??= coreTexts();
  return cache[text] ?? text;
}
