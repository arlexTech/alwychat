// The only module allowed to call Tauri's invoke(). Everything else imports from here.
import { getVersion, onBackButtonPress } from "@tauri-apps/api/app";
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

/** An incoming ring, or the answer to ours. */
export type CallSignal =
  | {
      kind: "ring";
      room_id: string;
      notification_id: string;
      caller_name: string;
      caller_avatar: string | null;
      video: boolean;
      /** ms since epoch. */
      expires_at: number;
    }
  | { kind: "declined"; room_id: string };

export const onCallSignal = (handler: (signal: CallSignal) => void) =>
  listen<CallSignal>("call-signal", (e) => handler(e.payload));
export const declineCall = (roomId: string, notificationId: string) =>
  invoke<void>("decline_call", { roomId, notificationId });

/** Unsent composer text, synced between our devices. */
export type Draft = { text: string; reply_to?: string | null; updated: number };
export const getDrafts = () => invoke<Record<string, Draft>>("drafts");
export const setDraft = (roomId: string, draft: Draft | null) => invoke<void>("set_draft", { roomId, draft });
/** A draft changed through sync (another device, or the echo of ours). */
export const onDraft = (handler: (change: { room_id: string; draft: Draft | null }) => void) =>
  listen<{ room_id: string; draft: Draft | null }>("draft", (e) => handler(e.payload));

/** A notification was tapped: open this chat. */
export const onOpenRoom = (handler: (roomId: string) => void) =>
  listen<string>("open-room", (e) => handler(e.payload));
/** The chat whose notification launched the app, if any (asked once at start). */
export const launchRoom = () => invoke<string | null>("launch_room");
/** Push on/off for this device (Android); resolves to whether push is in use. */
export const setupPush = (enabled: boolean) => invoke<boolean>("setup_push", { enabled });
export const clearNotifications = (roomId: string) => invoke<void>("clear_notifications", { roomId });

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
  /** People in the room's call right now. */
  in_call: CallMember[];
  /** A voice channel (Element Call video room): opening it joins the call. */
  is_voice: boolean;
  /** Only for invites (joined spaces aren't in the chat list). */
  is_space: boolean;
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
  | { kind: "text"; body: string; html: string | null; link: string | null; effect: Effect | null }
  | { kind: "emote"; body: string; html: string | null; effect: Effect | null }
  | { kind: "notice"; body: string; html: string | null; link: string | null }
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
  | { kind: "location"; geo_uri: string; description: string }
  | {
      kind: "live_location";
      geo_uri: string | null;
      description: string | null;
      is_live: boolean;
      updated: number | null;
      ends_at: number;
    }
  | {
      kind: "poll";
      question: string;
      answers: PollAnswer[];
      /** People who voted. */
      voters: number;
      max_selections: number;
      /** Results visible while the poll runs. */
      disclosed: boolean;
      ended: boolean;
      /** We started it (only the creator can end it). */
      is_mine: boolean;
    }
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
  /** Replies in the thread started on this message. */
  thread: ThreadInfo | null;
}

/** Element-compatible message effects. */
export type Effect = "confetti" | "fireworks" | "hearts" | "snowfall" | "rainfall" | "space_invaders";
export type AppTheme = "system" | "light" | "dark" | "amoled";

export type TimelineEntry = { key: string } & (
  | ({ type: "message" } & MessageEntry)
  | { type: "notice"; text: string; action?: "theme" }
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

export interface LinkPreview {
  url: string;
  site_name: string | null;
  title: string | null;
  description: string | null;
  /** mxc:// image. */
  image: string | null;
  image_width: number | null;
  image_height: number | null;
}

export const linkPreview = (url: string) => invoke<LinkPreview | null>("link_preview", { url });

/** What can keep notifications from arriving (always all true on desktop). */
export interface SystemStatus {
  notifications: boolean;
  batteryUnrestricted: boolean;
  backgroundData: boolean;
}

/** Read the status, or with `fix`, ask Android / open its settings page for that item. */
export const systemStatus = (fix?: "notifications" | "battery" | "backgroundData" | "appSettings") =>
  invoke<SystemStatus>("system_status", { fix: fix ?? null });

/** Shared look of a chat (room state `com.alwy.chat.room.*`). */
export interface ChatStyle {
  quick_reaction: string | null;
  /** Theme preset id from `$lib/themes`. */
  theme: string | null;
  /** User ID → nickname given in this chat. */
  nicknames: Record<string, string>;
  /** `mxc://` background image (over the theme's scenery). */
  wallpaper: string | null;
  can_edit: boolean;
}

// ── Extended profile: status, time zone ─────────────────────────────────────

/** MSC4426 status: emoji and/or short text. */
export interface Status {
  text: string;
  emoji: string;
}
export interface ProfileExtras {
  user_id: string;
  status: Status | null;
  /** IANA name, e.g. "Europe/Brussels". */
  time_zone: string | null;
}
export const profileExtras = (userIds: string[]) => invoke<ProfileExtras[]>("profile_extras", { userIds });
/** null clears it. */
export const setStatus = (status: Status | null) => invoke<void>("set_status", { status });
/** null stops sharing it. */
export const setTimeZone = (timeZone: string | null) => invoke<void>("set_time_zone", { timeZone });

// ── App lock ────────────────────────────────────────────────────────────────

export interface AppLockState {
  enabled: boolean;
  biometry: "fingerprint" | "face" | null;
}
export type UnlockResult = { result: "ok" } | { result: "wrong" } | { result: "wait"; seconds: number };
export const appLockState = () => invoke<AppLockState>("app_lock_state");
/** Turn on / change (4–12 digits), or off with null. */
export const setAppLock = (pin: string | null) => invoke<void>("set_app_lock", { pin });
export const unlockApp = (pin: string) => invoke<UnlockResult>("unlock_app", { pin });
/** The system fingerprint / face prompt; true once recognised. */
export const unlockBiometric = () => invoke<boolean>("unlock_biometric");
export const setSecureScreen = (secure: boolean) => invoke<void>("set_secure_screen", { secure });

export interface ThreadInfo {
  replies: number;
  latest_sender: string | null;
  latest_avatar: string | null;
  latest_text: string | null;
}

export function openChat(
  roomId: string,
  onDiffs: (diffs: ListDiff<TimelineEntry>[]) => void,
  onTyping: (names: string[]) => void,
  onStyle: (style: ChatStyle) => void,
  onPins: (pins: Pins) => void,
  /** Open on this message (a search result) instead of the latest ones. */
  focus: string | null = null,
) {
  const diffs = new Channel<ListDiff<TimelineEntry>[]>();
  diffs.onmessage = onDiffs;
  const typing = new Channel<string[]>();
  typing.onmessage = onTyping;
  const style = new Channel<ChatStyle>();
  style.onmessage = onStyle;
  const pins = new Channel<Pins>();
  pins.onmessage = onPins;
  return invoke<void>("open_chat", { roomId, onDiffs: diffs, onTyping: typing, onStyle: style, onPins: pins, focus });
}
/**
 * Open the thread started on `root` in the open chat. Until closed, passing `root` where a
 * command takes a room ID (send, react, edit, delete, load older, mark read…) acts on the thread.
 */
export function openThread(roomId: string, root: string, onDiffs: (diffs: ListDiff<TimelineEntry>[]) => void) {
  const diffs = new Channel<ListDiff<TimelineEntry>[]>();
  diffs.onmessage = onDiffs;
  return invoke<void>("open_thread", { roomId, root, onDiffs: diffs });
}
export const closeThread = () => invoke<void>("close_thread");
/** Newer messages of a chat opened on a search result; resolves to true at the newest. */
export const loadNewer = (roomId: string) => invoke<boolean>("load_newer", { roomId });

// ── Spaces ──────────────────────────────────────────────────────────────────

export interface SpaceInfo {
  id: string;
  name: string;
  avatar: string | null;
  /** Joined chats inside it, at any depth. */
  rooms: string[];
}
export interface CallMember {
  user_id: string;
  name: string;
  avatar: string | null;
}

export interface SpaceChild {
  id: string;
  name: string;
  avatar: string | null;
  topic: string | null;
  members: number;
  is_space: boolean;
  is_voice: boolean;
  joined: boolean;
  suggested: boolean;
  via: string[];
}
/** Which space chats also appear in "Chats" (synced between devices). */
export interface ChatListPrefs {
  show_space_chats: boolean;
  spaces: Record<string, boolean>;
  rooms: Record<string, boolean>;
  /** Custom chat-list folders, in display order. */
  folders: Folder[];
}

/** A chat-list folder: chats picked by type and/or by hand. */
export interface Folder {
  id: string;
  name: string;
  /** Every 1:1 chat. */
  people: boolean;
  /** Every group (space chats only when they show in "Chats"). */
  groups: boolean;
  /** Always in. */
  rooms: string[];
  /** Never in. */
  exclude: string[];
  unread_only: boolean;
  hide_muted: boolean;
}

export function subscribeSpaces(onSpaces: (spaces: SpaceInfo[]) => void) {
  const channel = new Channel<SpaceInfo[]>();
  channel.onmessage = onSpaces;
  return invoke<void>("subscribe_spaces", { onSpaces: channel });
}
export function openSpace(spaceId: string, onChildren: (children: SpaceChild[]) => void) {
  const channel = new Channel<SpaceChild[]>();
  channel.onmessage = onChildren;
  return invoke<void>("open_space", { spaceId, onChildren: channel });
}
export const closeSpace = () => invoke<void>("close_space");
export const joinRoom = (roomId: string, via: string[]) => invoke<void>("join_room", { roomId, via });
export const canAddToSpace = (spaceId: string) => invoke<boolean>("can_add_to_space", { spaceId });
export const createVoiceChannel = (spaceId: string, name: string) =>
  invoke<string>("create_voice_channel", { spaceId, name });
export const getChatListPrefs = () => invoke<ChatListPrefs>("chat_list_prefs");
export const setChatListPrefs = (prefs: ChatListPrefs) => invoke<void>("set_chat_list_prefs", { prefs });

// ── Search & shared media ───────────────────────────────────────────────────

/** Photos+videos, files (incl. audio/voice) or messages with links. */
export type KindFilter = "media" | "files" | "links";

export interface SearchHit {
  room_id: string;
  room_name: string;
  room_avatar: string | null;
  event_id: string;
  sender: string;
  sender_name: string;
  is_own: boolean;
  timestamp: number;
  kind: string;
  /** The matching part; matches are between \u0002 and \u0003. */
  snippet: string;
  media: string | null;
}

export interface SharedItem {
  event_id: string;
  sender_name: string;
  timestamp: number;
  kind: string;
  media: string | null;
  width: number | null;
  height: number | null;
  duration_ms: number | null;
  filename: string | null;
  size: number | null;
  text: string | null;
  url: string | null;
}

export interface SearchStatus {
  messages: number;
  chats_done: number;
  chats: number;
  indexing: boolean;
  chat_done: boolean | null;
}

export const searchMessages = (
  query: string,
  opts: { roomId?: string | null; sender?: string | null; kind?: KindFilter | null; before?: number | null } = {},
) =>
  invoke<SearchHit[]>("search_messages", {
    query,
    roomId: opts.roomId ?? null,
    sender: opts.sender ?? null,
    kind: opts.kind ?? null,
    before: opts.before ?? null,
  });
export const sharedMedia = (roomId: string, kind: KindFilter, before: number | null = null) =>
  invoke<SharedItem[]>("shared_media", { roomId, kind, before });
export const searchStatus = (roomId: string | null = null) => invoke<SearchStatus>("search_status", { roomId });
export const startSearchBackfill = () => invoke<void>("start_search_backfill");

export interface PinnedMessage {
  event_id: string;
  sender_name: string;
  preview: string;
}

/** The chat's pinned messages, most recently pinned first. */
export interface Pins {
  messages: PinnedMessage[];
  can_pin: boolean;
}

export interface PollAnswer {
  id: string;
  text: string;
  votes: number;
  mine: boolean;
}

export const createPoll = (roomId: string, question: string, answers: string[], disclosed: boolean, multiple: boolean) =>
  invoke<void>("create_poll", { roomId, question, answers, disclosed, multiple });
export const votePoll = (roomId: string, pollId: string, answers: string[]) =>
  invoke<void>("vote_poll", { roomId, pollId, answers });
export const endPoll = (roomId: string, pollId: string) => invoke<void>("end_poll", { roomId, pollId });

/** Copies of messages (event IDs) from `roomId`, sent to `targetRoomId`. Resolves to how many. */
export const forwardMessages = (roomId: string, eventIds: string[], targetRoomId: string) =>
  invoke<number>("forward_messages", { roomId, eventIds, targetRoomId });

export const pinMessage = (roomId: string, eventId: string, pin: boolean) =>
  invoke<void>("pin_message", { roomId, eventId, pin });

export const setQuickReaction = (roomId: string, emoji: string | null) =>
  invoke<void>("set_quick_reaction", { roomId, emoji });
export const setTheme = (roomId: string, preset: string | null, wallpaper: string | null = null) =>
  invoke<void>("set_theme", { roomId, preset, wallpaper });
/** A chat background image (base64 JPEG) → its `mxc://` URI, for `setTheme`. */
export const uploadWallpaper = (image: string) => invoke<string>("upload_wallpaper", { image });
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
/** Android: the phone's camera app; empty when cancelled. */
export const capture = (kind: "photo" | "video") => invoke<Picked[]>("capture", { kind });

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
/** Open a file message in another app, save it where the user picks (Android), or to Downloads. */
export const fileAction = (media: string, action: "open" | "save_as" | "downloads") =>
  invoke<string | null>("file_action", { media, action });
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
  /** Play message effects (confetti, snow…). */
  chat_effects: boolean;
  app_theme: AppTheme;
  /** App lock: ask again after this long in the background (0 = every time). */
  app_lock_after_s: number;
  app_lock_biometric: boolean;
  /** Android: blank in the app switcher, no screenshots. */
  hide_in_switcher: boolean;
  /** Enter sends (Shift+Enter: new line); off: Ctrl+Enter sends. */
  enter_sends: boolean;
  /** Text size in percent. */
  text_scale: number;
}
/** Android: dark system bar icons on a light theme, and the colour behind the bars. */
export const setSystemBars = (light: boolean, background: string) =>
  invoke<void>("set_system_bars", { light, background });
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

// ---- Managing chats, spaces and the account ----

export interface UserResult {
  user_id: string;
  name: string;
  avatar: string | null;
}

export interface InviteInfo {
  inviter_id: string;
  inviter_name: string | null;
  inviter_avatar: string | null;
  is_space: boolean;
  topic: string | null;
  members: number;
}

export interface NewRoom {
  kind: "group" | "space";
  name: string;
  topic: string;
  /** Invite only, members of `parent`, or anyone with the link. */
  access: "private" | "space" | "public";
  encrypted: boolean;
  parent: string | null;
  invite: string[];
}

export interface MemberInfo {
  user_id: string;
  name: string;
  avatar: string | null;
  is_me: boolean;
  power: number;
  membership: "join" | "invite" | "ban" | "knock" | "leave";
}

export interface RoomDetails {
  id: string;
  name: string;
  topic: string | null;
  avatar: string | null;
  alias: string | null;
  is_space: boolean;
  is_dm: boolean;
  encrypted: boolean;
  join_rule: string;
  allow_spaces: string[];
  history: string;
  members: number;
  notify: "all" | "mentions" | "mute";
  notify_custom: boolean;
  my_power: number;
  /** Level needed per permission key (power-levels key or event type). */
  levels: Record<string, number>;
  parent_spaces: string[];
}

export interface DeviceInfo {
  device_id: string;
  name: string | null;
  last_seen_ip: string | null;
  last_seen_ts: number | null;
  is_current: boolean;
  verified: boolean | null;
}

export interface SecurityStatus {
  recovery: string;
  backup: string;
  cross_signed: boolean;
}

export interface NotificationRules {
  keywords: string[];
  room_mentions: boolean;
  invites: boolean;
}

export const inviteInfo = (roomId: string) => invoke<InviteInfo>("invite_info", { roomId });
export const acceptInvite = (roomId: string) => invoke<void>("accept_invite", { roomId });
/** Decline an invite, or leave a chat or space. */
export const leaveRoom = (roomId: string) => invoke<void>("leave_room", { roomId });
export const searchUsers = (term: string) => invoke<UserResult[]>("search_users", { term });
export const openDm = (userId: string) => invoke<string>("open_dm", { userId });
/** Our "Saved messages" chat (only us in it), if it exists. */
export const savedMessagesId = () => invoke<string | null>("saved_messages_id");
/** Our "Saved messages" chat, created on first use. */
export const openSavedMessages = () => invoke<string>("open_saved_messages");
export const createRoom = (room: NewRoom) => invoke<string>("create_room", { room });
export const joinAddress = (address: string) => invoke<string>("join_address", { address });
export const spaceRoomsJoined = (spaceId: string) => invoke<string[]>("space_rooms_joined", { spaceId });
export const members = (roomId: string) => invoke<MemberInfo[]>("members", { roomId });
export const inviteUser = (roomId: string, userId: string) => invoke<void>("invite_user", { roomId, userId });
export const moderate = (roomId: string, userId: string, action: "kick" | "ban" | "unban", reason: string | null = null) =>
  invoke<void>("moderate", { roomId, userId, action, reason });
export const setUserPower = (roomId: string, userId: string, power: number) =>
  invoke<void>("set_user_power", { roomId, userId, power });
export const setPermission = (roomId: string, key: string, power: number) =>
  invoke<void>("set_permission", { roomId, key, power });
export const roomDetails = (roomId: string) => invoke<RoomDetails>("room_details", { roomId });
export const setRoomName = (roomId: string, name: string) => invoke<void>("set_room_name", { roomId, name });
export const setRoomTopic = (roomId: string, topic: string) => invoke<void>("set_room_topic", { roomId, topic });
export const setRoomAvatar = (roomId: string, image: string | null, mime = "image/jpeg") =>
  invoke<void>("set_room_avatar", { roomId, image, mime });
export const setJoinRule = (roomId: string, rule: string, spaces: string[] = []) =>
  invoke<void>("set_join_rule", { roomId, rule, spaces });
export const setHistoryVisibility = (roomId: string, history: string) =>
  invoke<void>("set_history_visibility", { roomId, history });
export const enableRoomEncryption = (roomId: string) => invoke<void>("enable_room_encryption", { roomId });
export const setNotifyMode = (roomId: string, mode: "all" | "mentions" | "mute" | "default") =>
  invoke<void>("set_notify_mode", { roomId, mode });
export const reportRoom = (roomId: string, reason: string) => invoke<void>("report_room", { roomId, reason });
export const addToSpace = (spaceId: string, roomId: string) => invoke<void>("add_to_space", { spaceId, roomId });
export const removeFromSpace = (spaceId: string, roomId: string) =>
  invoke<void>("remove_from_space", { spaceId, roomId });
export const setSuggested = (spaceId: string, roomId: string, suggested: boolean) =>
  invoke<void>("set_suggested", { spaceId, roomId, suggested });
export const ownProfile = () => invoke<UserResult>("own_profile");
export const setDisplayName = (name: string) => invoke<void>("set_display_name", { name });
export const setOwnAvatar = (image: string | null, mime = "image/jpeg") => invoke<void>("set_own_avatar", { image, mime });
export const userProfile = (userId: string) => invoke<UserResult>("user_profile", { userId });
export const devices = () => invoke<DeviceInfo[]>("devices");
export const renameDevice = (deviceId: string, name: string) => invoke<void>("rename_device", { deviceId, name });
/** The account page: "profile", "devices", "sign_out:<device>", "deactivate", "reset_identity". */
export const accountUrl = (action: string) => invoke<string>("account_url", { action });
export const securityStatus = () => invoke<SecurityStatus>("security_status");
export const newRecoveryKey = () => invoke<string>("new_recovery_key");
export const ignoredUsers = () => invoke<UserResult[]>("ignored_users");
export const setIgnored = (userId: string, ignored: boolean) => invoke<void>("set_ignored", { userId, ignored });
export const notificationRules = () => invoke<NotificationRules>("notification_rules");
export const setNotificationRule = (rule: "room_mentions" | "invites", enabled: boolean) =>
  invoke<void>("set_notification_rule", { rule, enabled });
export const setKeyword = (keyword: string, add: boolean) => invoke<void>("set_keyword", { keyword, add });
export const appVersion = () => getVersion();

/** What another app shared with us (Android "Share" → AlwyChat), taken once. */
export interface IncomingShare {
  text?: string | null;
  files: Picked[];
}
export const takeShare = () => invoke<IncomingShare>("take_share");
export const sendLocation = (roomId: string, geoUri: string, description = "") =>
  invoke<void>("send_location", { roomId, geoUri, description });

export type ConnectionStatus = "connecting" | "online" | "offline" | "error";
export function subscribeConnection(onStatus: (status: ConnectionStatus) => void) {
  const channel = new Channel<ConnectionStatus>();
  channel.onmessage = onStatus;
  return invoke<void>("subscribe_connection", { onStatus: channel });
}
/** Back in front: make sure the sync runs. */
export const resumeSync = () => invoke<void>("resume_sync");
/** Resolves to the share's ID, for `updateLiveLocation`. */
export const startLiveLocation = (roomId: string, durationMs: number, geoUri: string) =>
  invoke<string>("start_live_location", { roomId, durationMs, geoUri });
export const updateLiveLocation = (roomId: string, shareId: string, geoUri: string) =>
  invoke<void>("update_live_location", { roomId, shareId, geoUri });
export const stopLiveLocation = (roomId: string) => invoke<void>("stop_live_location", { roomId });
/** Send a message that failed again. */
export const retrySend = (roomId: string, itemId: string) => invoke<void>("retry_send", { roomId, itemId });

export interface PushTestResult {
  permission: boolean;
  token: boolean;
  registered?: boolean;
  gateway_ok?: boolean;
  rejected?: boolean;
  error?: string | null;
}
/** Android: check push end to end; a test notification should appear. */
export const testPush = () => invoke<PushTestResult>("test_push");

export interface DayCount {
  /** Start of the (UTC) day, ms. */
  day: number;
  mine: number;
  others: number;
}
export interface Activity {
  days: DayCount[];
  /** False while older messages are still being indexed. */
  complete: boolean;
}
export const messageActivity = (roomId: string) => invoke<Activity>("message_activity", { roomId });
/** The first message on or after `ts` (ms). */
export const messageAt = (roomId: string, ts: number) => invoke<string | null>("message_at", { roomId, ts });
