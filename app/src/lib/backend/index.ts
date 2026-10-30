// The only module allowed to call Tauri's invoke(). Everything else imports from here.
import { Channel, invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";

/** False when the UI is opened in a plain browser (e.g. localhost:1420): there is no Rust core there. */
export const hasBackend = isTauri();

// Copy uncaught interface errors to the Rust log (visible in the `tauri dev` terminal).
if (hasBackend) {
  const report = (message: string) => void invoke("log_ui_error", { message }).catch(() => {});
  window.addEventListener("error", (e) => report(`${e.message} at ${e.filename}:${e.lineno}`));
  window.addEventListener("unhandledrejection", (e) => report(`unhandled rejection: ${e.reason}`));
}

export interface ServerInfo {
  homeserver: string;
  versions: string[];
  oauth_issuer: string | null;
  sliding_sync: boolean;
}

export const discoverServer = (server: string) =>
  invoke<ServerInfo>("discover_server", { server });

export interface Account {
  user_id: string;
  device_id: string;
  display_name: string | null;
}

export const restoreSession = () => invoke<Account | null>("restore_session");
export const login = (server: string) => invoke<Account>("login", { server });
export const logout = () => invoke<void>("logout");
/** Stop using a session the server ended, keeping local data (unlike logout). */
export const endSession = () => invoke<void>("end_session");
/** The server ended our session (signed out elsewhere, revoked…). Returns an unsubscribe function. */
export const onSessionEnded = (handler: () => void) => listen("session-ended", handler);

export interface LatestMessage {
  sender_name: string | null;
  is_own: boolean;
  text: string;
  timestamp: number | null;
  is_sending: boolean;
}

export interface RoomSummary {
  id: string;
  name: string;
  avatar: string | null;
  is_dm: boolean;
  is_invite: boolean;
  is_favourite: boolean;
  /** Low-priority tag: shown under "Archived". */
  is_archived: boolean;
  is_muted: boolean;
  is_encrypted: boolean;
  unread_messages: number;
  unread_mentions: number;
  marked_unread: boolean;
  latest: LatestMessage | null;
}

/** Mirror of eyeball-im's VectorDiff, as sent by alwy-core. */
export type ListDiff<T> =
  | { op: "append"; values: T[] }
  | { op: "clear" }
  | { op: "push_front"; value: T }
  | { op: "push_back"; value: T }
  | { op: "pop_front" }
  | { op: "pop_back" }
  | { op: "insert"; index: number; value: T }
  | { op: "set"; index: number; value: T }
  | { op: "remove"; index: number }
  | { op: "truncate"; length: number }
  | { op: "reset"; values: T[] };

export function subscribeChatList(onDiffs: (diffs: ListDiff<RoomSummary>[]) => void) {
  const channel = new Channel<ListDiff<RoomSummary>[]>();
  channel.onmessage = onDiffs;
  return invoke<void>("subscribe_chat_list", { onDiffs: channel });
}

// ── Open chat ────────────────────────────────────────────────────────────────

export interface ReplyPreview {
  event_id: string;
  /** null while the replied-to message is loading. */
  sender_name: string | null;
  text: string | null;
}

export interface Reaction {
  key: string;
  count: number;
  mine: boolean;
}

export type MessageContent =
  /** `html` is sanitised by the core (Matrix allowlist): safe for {@html}. */
  | { kind: "text"; body: string; html: string | null }
  | { kind: "emote"; body: string; html: string | null }
  | { kind: "notice"; body: string; html: string | null }
  /** `media` is a key for eventMediaUrl() in $lib/media. */
  | {
      kind: "image";
      media: string;
      width: number | null;
      height: number | null;
      blurhash: string | null;
      caption: string | null;
    }
  | {
      kind: "video";
      media: string;
      width: number | null;
      height: number | null;
      blurhash: string | null;
      duration_ms: number | null;
      has_thumbnail: boolean;
      caption: string | null;
    }
  | { kind: "file"; media: string; filename: string; size: number | null; caption: string | null }
  | { kind: "sticker"; media: string; body: string; width: number | null; height: number | null }
  | { kind: "media"; label: string }
  | { kind: "poll"; question: string }
  | { kind: "redacted" }
  | { kind: "unable_to_decrypt" }
  | { kind: "unsupported" };

export interface MessageEntry {
  /** Event ID ($…) or local transaction ID: pass it back for actions. */
  item_id: string;
  sender: string;
  sender_name: string;
  sender_avatar: string | null;
  is_own: boolean;
  timestamp: number;
  content: MessageContent;
  is_edited: boolean;
  only_emojis: boolean;
  can_reply: boolean;
  can_edit: boolean;
  can_delete: boolean;
  reply_to: ReplyPreview | null;
  reactions: Reaction[];
  send_state: "sending" | "failed" | null;
  /** Others whose latest read receipt is this message. */
  seen_by: { name: string; avatar: string | null }[];
}

export type TimelineEntry = { key: string } & (
  | ({ type: "message" } & MessageEntry)
  | { type: "notice"; text: string }
  | { type: "date_divider"; timestamp: number }
  | { type: "read_marker" }
  | { type: "timeline_start" }
  | { type: "hidden" }
);

export function openChat(
  roomId: string,
  onDiffs: (diffs: ListDiff<TimelineEntry>[]) => void,
  onTyping: (names: string[]) => void,
) {
  const diffs = new Channel<ListDiff<TimelineEntry>[]>();
  diffs.onmessage = onDiffs;
  const typing = new Channel<string[]>();
  typing.onmessage = onTyping;
  return invoke<void>("open_chat", { roomId, onDiffs: diffs, onTyping: typing });
}

export const setTyping = (roomId: string, typing: boolean) =>
  invoke<void>("set_typing", { roomId, typing });
export const editMessage = (roomId: string, itemId: string, body: string) =>
  invoke<void>("edit_message", { roomId, itemId, body });
export const deleteMessage = (roomId: string, itemId: string) =>
  invoke<void>("delete_message", { roomId, itemId });

export interface Revision {
  text: string;
  timestamp: number | null;
}
export const editHistory = (roomId: string, eventId: string) =>
  invoke<Revision[]>("edit_history", { roomId, eventId });

/** Saves to ~/Pictures/AlwyChat (photos/videos) or Downloads; resolves to the file path. */
export const saveMedia = (media: string, toGallery: boolean) =>
  invoke<string>("save_media", { media, toGallery });

// ── Chat list actions ────────────────────────────────────────────────────────

export const setPinned = (roomId: string, pinned: boolean) =>
  invoke<void>("set_pinned", { roomId, pinned });
export const setArchived = (roomId: string, archived: boolean) =>
  invoke<void>("set_archived", { roomId, archived });
export const setMuted = (roomId: string, muted: boolean) =>
  invoke<void>("set_muted", { roomId, muted });
export const setMarkedUnread = (roomId: string, unread: boolean) =>
  invoke<void>("set_marked_unread", { roomId, unread });

// ── Settings ─────────────────────────────────────────────────────────────────

export interface Settings {
  send_read_receipts: boolean;
  send_typing: boolean;
  notifications: boolean;
}
export const getSettings = () => invoke<Settings>("get_settings");
export const setSettings = (settings: Settings) => invoke<void>("set_settings", { settings });

export const closeChat = () => invoke<void>("close_chat");
/** Resolves to true once the start of the room is reached. */
export const loadOlder = (roomId: string) => invoke<boolean>("load_older", { roomId });
export const sendMessage = (roomId: string, body: string, replyTo: string | null) =>
  invoke<void>("send_message", { roomId, body, replyTo });
export const toggleReaction = (roomId: string, itemId: string, key: string) =>
  invoke<void>("toggle_reaction", { roomId, itemId, key });
export const markRead = (roomId: string) => invoke<void>("mark_read", { roomId });
/** Open a link from a message in the system browser (never inside the app's webview). */
export const openExternal = (url: string) => openUrl(url);

// ── Encryption ───────────────────────────────────────────────────────────────

export interface EncryptionStatus {
  device_verified: boolean;
  recovery: "unknown" | "enabled" | "incomplete" | "disabled";
  can_verify_with_device: boolean;
}

export interface VerificationEmoji {
  symbol: string;
  description: string;
}

export type VerificationUpdate =
  | { state: "requested" }
  | { state: "incoming"; from: string }
  | { state: "ready" }
  | { state: "emojis"; emojis: VerificationEmoji[] }
  | { state: "confirmed" }
  | { state: "done" }
  | { state: "cancelled"; reason: string };

export function subscribeEncryption(
  onStatus: (status: EncryptionStatus) => void,
  onVerification: (update: VerificationUpdate) => void,
) {
  const status = new Channel<EncryptionStatus>();
  status.onmessage = onStatus;
  const verification = new Channel<VerificationUpdate>();
  verification.onmessage = onVerification;
  return invoke<void>("subscribe_encryption", { onStatus: status, onVerification: verification });
}

export const recover = (key: string) => invoke<void>("recover", { key });
export const verifyThisDevice = () => invoke<void>("verify_this_device");
export const acceptVerification = () => invoke<void>("accept_verification");
export const confirmEmojis = (matching: boolean) => invoke<void>("confirm_emojis", { matching });
export const cancelVerification = () => invoke<void>("cancel_verification");
