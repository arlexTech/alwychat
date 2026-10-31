// The only module allowed to call Tauri's invoke(). Everything else imports from here.
import { onBackButtonPress } from "@tauri-apps/api/app";
import { Channel, invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";

/** False when the UI is opened in a plain browser (e.g. localhost:1420): there is no Rust core there. */
export const hasBackend = isTauri();

export const isAndroid = /Android/.test(navigator.userAgent);

/** Take over Android's back gesture until the returned function is called (then the default runs). */
export async function interceptBack(handler: () => void): Promise<() => Promise<void>> {
  const listener = await onBackButtonPress(handler);
  return () => listener.unregister();
}

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
export const cancelLogin = () => invoke<void>("cancel_login");
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
  dm_user: string | null;
  /** Users in the room's call right now. */
  in_call: string[];
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
      /** Album (com.alwy.chat.gallery) this photo was sent in. */
      gallery: GalleryRef | null;
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
  | {
      kind: "audio";
      media: string;
      duration_ms: number | null;
      /** Loudness bars, 0–1024. */
      waveform: number[];
      is_voice: boolean;
      filename: string;
      size: number | null;
    }
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
  /** Mentions us (or @room). */
  mentions_me: boolean;
}

export type TimelineEntry = { key: string } & (
  | ({ type: "message" } & MessageEntry)
  | { type: "notice"; text: string }
  | { type: "date_divider"; timestamp: number }
  | { type: "read_marker" }
  | { type: "timeline_start" }
  | { type: "hidden" }
);

/** Join (or start) the room's call: returns where to connect with livekit-client. */
export const joinCall = (roomId: string, video: boolean) =>
  invoke<{ url: string; jwt: string }>("join_call", { roomId, video });
export const leaveCall = () => invoke<void>("leave_call");
export const callParticipants = (roomId: string) => invoke<Member[]>("call_participants", { roomId });

export interface Presence {
  user_id: string;
  online: boolean;
  last_active_ago_ms: number | null;
}

/** Online / last active of these people (unknown ones are left out). */
export const getPresence = (userIds: string[]) => invoke<Presence[]>("presence", { userIds });
/** Tell the server whether we're using the app (ignored when "Show when I'm active" is off). */
export const reportActivity = (active: boolean) => invoke<void>("report_activity", { active });

/** What can keep notifications from arriving (always all true on desktop). */
export interface SystemStatus {
  notifications: boolean;
  batteryUnrestricted: boolean;
  backgroundData: boolean;
}

/** Read the status, or with `fix`, ask Android / open its settings page for that item. */
export const systemStatus = (fix?: "notifications" | "battery" | "backgroundData") =>
  invoke<SystemStatus>("system_status", { fix: fix ?? null });

/** Shared look of a chat (room state `com.alwy.chat.room.*`). */
export interface ChatStyle {
  quick_reaction: string | null;
  /** Theme preset id from `$lib/themes`. */
  theme: string | null;
  /** User ID → nickname given in this chat. */
  nicknames: Record<string, string>;
  can_edit: boolean;
}

export function openChat(
  roomId: string,
  onDiffs: (diffs: ListDiff<TimelineEntry>[]) => void,
  onTyping: (names: string[]) => void,
  onStyle: (style: ChatStyle) => void,
) {
  const diffs = new Channel<ListDiff<TimelineEntry>[]>();
  diffs.onmessage = onDiffs;
  const typing = new Channel<string[]>();
  typing.onmessage = onTyping;
  const style = new Channel<ChatStyle>();
  style.onmessage = onStyle;
  return invoke<void>("open_chat", { roomId, onDiffs: diffs, onTyping: typing, onStyle: style });
}

export const setQuickReaction = (roomId: string, emoji: string | null) =>
  invoke<void>("set_quick_reaction", { roomId, emoji });
export const setTheme = (roomId: string, preset: string | null) => invoke<void>("set_theme", { roomId, preset });
export const setNickname = (roomId: string, userId: string, nickname: string | null) =>
  invoke<void>("set_nickname", { roomId, userId, nickname });

export const setTyping = (roomId: string, typing: boolean) =>
  invoke<void>("set_typing", { roomId, typing });
export const editMessage = (roomId: string, itemId: string, body: string, mentions: MentionInput[]) =>
  invoke<void>("edit_message", { roomId, itemId, body, mentions });
export const deleteMessage = (roomId: string, itemId: string) =>
  invoke<void>("delete_message", { roomId, itemId });

export interface Revision {
  text: string;
  timestamp: number | null;
}
export const editHistory = (roomId: string, eventId: string) =>
  invoke<Revision[]>("edit_history", { roomId, eventId });

export interface GalleryRef {
  id: string;
  index: number;
  count: number;
}

/** Send a photo/video/file to the open chat (bytes as a raw IPC body, metadata in headers). */
export async function sendFile(
  roomId: string,
  file: File,
  meta: { caption?: string | null; gallery?: GalleryRef | null } = {},
) {
  const bytes = new Uint8Array(await file.arrayBuffer());
  const info = { filename: file.name || "file", mime: file.type || null, caption: meta.caption ?? null, gallery: meta.gallery ?? null };
  return invoke<void>("send_file", bytes, {
    headers: { "x-room": encodeURIComponent(roomId), "x-file": encodeURIComponent(JSON.stringify(info)) },
  });
}

/** A file chosen in Android's system picker, copied into the app's cache until it's sent. */
export interface Picked {
  path: string;
  name: string;
  mime: string;
  size: number;
}

/** Send a recorded voice message (base64: the Android webview can't pass raw bytes). */
export async function sendVoice(
  roomId: string,
  recording: { blob: Blob; mime: string; durationMs: number; waveform: number[] },
) {
  const audio = await new Promise<string>((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result).split(",", 2)[1] ?? "");
    reader.onerror = () => reject(reader.error);
    reader.readAsDataURL(recording.blob);
  });
  return invoke<void>("send_voice", {
    roomId,
    audio,
    mime: recording.mime,
    durationMs: recording.durationMs,
    waveform: recording.waveform,
  });
}

/** Android: open the photo picker ("media") or the document picker ("files"). */
export const pickFiles = (kind: "media" | "files") => invoke<Picked[]>("pick_files", { kind });

/** Android: send a picked file (the webview can't hand raw bytes to Rust there). */
export function sendPicked(
  roomId: string,
  picked: Picked,
  meta: { caption?: string | null; gallery?: GalleryRef | null } = {},
) {
  const file = { filename: picked.name, mime: picked.mime, caption: meta.caption ?? null, gallery: meta.gallery ?? null };
  return invoke<void>("send_picked", { roomId, path: picked.path, file });
}

export const discardPicked = (picked: Picked) =>
  invoke<void>("discard_picked", { path: picked.path }).catch(() => {});

/** The chat's own HD choice; null = follow the app default. */
export const chatMediaHd = (roomId: string) => invoke<boolean | null>("chat_media_hd", { roomId });
export const setChatMediaHd = (roomId: string, hd: boolean | null) =>
  invoke<void>("set_chat_media_hd", { roomId, hd });

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
  share_presence: boolean;
  notifications: boolean;
  /** Photos in original quality by default (chats can override). */
  hd_media: boolean;
}
export const getSettings = () => invoke<Settings>("get_settings");
export const setSettings = (settings: Settings) => invoke<void>("set_settings", { settings });

export const closeChat = () => invoke<void>("close_chat");
/** Resolves to true once the start of the room is reached. */
export const loadOlder = (roomId: string) => invoke<boolean>("load_older", { roomId });
/** A mention picked in the composer; `text` is what the draft shows (e.g. "@Léa"). */
export interface MentionInput {
  user_id: string;
  text: string;
}

export interface Member {
  user_id: string;
  name: string;
  avatar: string | null;
  is_me: boolean;
}

export const sendMessage = (
  roomId: string,
  body: string,
  replyTo: string | null,
  mentions: MentionInput[] = [],
) => invoke<void>("send_message", { roomId, body, mentions, replyTo });
export const roomMembers = (roomId: string) => invoke<Member[]>("room_members", { roomId });
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
