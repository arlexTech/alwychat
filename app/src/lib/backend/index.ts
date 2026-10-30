// The only module allowed to call Tauri's invoke(). Everything else imports from here.
import { Channel, invoke, isTauri } from "@tauri-apps/api/core";

/** False when the UI is opened in a plain browser (e.g. localhost:1420): there is no Rust core there. */
export const hasBackend = isTauri();

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
