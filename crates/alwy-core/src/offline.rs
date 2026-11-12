//! Working offline.
//!
//! - Reads of room state and account data that the SDK doesn't keep (our custom chat settings,
//!   power levels, pins…) go to the server; without a connection they answer with the last
//!   value we saw (kept on disk), else the local store, instead of failing.
//! - Writes (chat settings, preferences, drafts, tags…) that can't reach the server wait in an
//!   outbox (on disk) and go out when the connection is back. Each target keeps only its
//!   latest value (these are "set to this" writes, not messages), and reads see the waiting
//!   value at once.
//!
//! Messages, reactions and edits don't come here: the SDK's send queue already holds them.

use std::{
    collections::HashMap,
    future::Future,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

use matrix_sdk::{
    Client, Room,
    ruma::{
        OwnedRoomId,
        api::client::state::get_state_event_for_key,
        events::{GlobalAccountDataEventType, RoomAccountDataEventType, StateEventType},
        serde::Raw,
    },
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{info, warn};

use crate::{Error, Result};

/// How long a read waits for the server before answering from what we know.
const READ_WAIT: Duration = Duration::from_secs(6);
/// Most remembered reads (oldest dropped).
const MAX_CACHED: usize = 2000;
/// How often a waiting outbox is retried while nothing says we're back online.
const RETRY_EVERY: Duration = Duration::from_secs(20);

/// A write waiting for the connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Pending {
    State {
        room: String,
        event_type: String,
        key: String,
        content: Value,
    },
    RoomData {
        room: String,
        event_type: String,
        content: Value,
    },
    GlobalData {
        event_type: String,
        content: Value,
    },
    Tag {
        room: String,
        tag: String,
        on: bool,
    },
    Unread {
        room: String,
        on: bool,
    },
    /// A chat's notifications muted (push rule) or back to default.
    Mute {
        room: String,
        on: bool,
    },
    /// Read up to the newest message (sent as an unthreaded receipt).
    Read {
        room: String,
        private: bool,
    },
    /// Delete one of the messages.
    Redact {
        room: String,
        event: String,
    },
}

impl Pending {
    /// What it sets: a newer write to the same target replaces it.
    fn target(&self) -> String {
        match self {
            Self::State {
                room,
                event_type,
                key,
                ..
            } => state_key(room, event_type, key),
            Self::RoomData {
                room, event_type, ..
            } => format!("room-data|{room}|{event_type}"),
            Self::GlobalData { event_type, .. } => format!("global|{event_type}"),
            Self::Tag { room, tag, .. } => format!("tag|{room}|{tag}"),
            Self::Unread { room, .. } => format!("unread|{room}"),
            Self::Mute { room, .. } => format!("mute|{room}"),
            Self::Read { room, .. } => format!("read|{room}"),
            Self::Redact { room, event } => format!("redact|{room}|{event}"),
        }
    }
}

fn state_key(room: &str, event_type: &str, key: &str) -> String {
    format!("state|{room}|{event_type}|{key}")
}

#[derive(Default, Serialize, Deserialize)]
struct Cache {
    /// Last value seen per target (`state|…`, `global|…`, `room-data|…`), oldest first.
    values: Vec<(String, Value)>,
}

struct Store {
    cache_path: PathBuf,
    outbox_path: PathBuf,
    cache: HashMap<String, Value>,
    order: Vec<String>,
    outbox: Vec<Pending>,
}

static STORE: Mutex<Option<Store>> = Mutex::new(None);
static WAKE: std::sync::LazyLock<Arc<tokio::sync::Notify>> =
    std::sync::LazyLock::new(|| Arc::new(tokio::sync::Notify::new()));

/// Load what was kept for this session (`dir` = its store folder) and start sending the outbox
/// whenever the connection allows. Once per signed-in client.
pub(crate) fn install(client: &Client, dir: PathBuf) -> crate::rt::AbortHandle {
    let cache_path = dir.join("offline_cache.json");
    let outbox_path = dir.join("offline_outbox.json");
    let cache: Cache = crate::files::read_now(&cache_path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    let outbox: Vec<Pending> = crate::files::read_now(&outbox_path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    let order = cache.values.iter().map(|(k, _)| k.clone()).collect();
    *STORE.lock().unwrap() = Some(Store {
        cache_path,
        outbox_path,
        cache: cache.values.into_iter().collect(),
        order,
        outbox,
    });
    let client = client.clone();
    crate::rt::spawn(async move {
        loop {
            flush(&client).await;
            let sleep = crate::rt::sleep(RETRY_EVERY);
            let woken = WAKE.notified();
            futures_util::pin_mut!(sleep, woken);
            futures_util::future::select(sleep, woken).await;
        }
    })
    .abort_handle()
}

/// The connection is back (or the app came to the front): send what's waiting now.
pub(crate) fn kick() {
    WAKE.notify_one();
}

static ONLINE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

/// What the sync says about the connection (for errors that don't tell network from refusal).
pub(crate) fn set_online(online: bool) {
    ONLINE.store(online, std::sync::atomic::Ordering::Relaxed);
    if online {
        kick();
    }
}

pub(crate) fn online() -> bool {
    ONLINE.load(std::sync::atomic::Ordering::Relaxed)
}

/// A chat flag changed offline and not sent yet (`kind`: "tag|m.favourite", "unread", "mute"),
/// so the chat list shows the change rather than the server's old value.
pub(crate) fn waiting_flag(room_id: &str, kind: &str) -> Option<bool> {
    let target = match kind.split_once('|') {
        Some((head, tag)) => format!("{head}|{room_id}|{tag}"),
        None => format!("{kind}|{room_id}"),
    };
    let store = STORE.lock().unwrap();
    store.as_ref()?.outbox.iter().rev().find_map(|p| match p {
        Pending::Tag { on, .. } | Pending::Unread { on, .. } | Pending::Mute { on, .. }
            if p.target() == target =>
        {
            Some(*on)
        }
        _ => None,
    })
}

/// Keep a chat mute for later (the push-rule API hides network errors: called when offline).
pub(crate) fn later_mute(room: &Room, on: bool) {
    enqueue(Pending::Mute {
        room: room.room_id().to_string(),
        on,
    });
}

/// Keep "read up to here" for later.
pub(crate) fn later_read(room: &Room, private: bool) {
    enqueue(Pending::Read {
        room: room.room_id().to_string(),
        private,
    });
}

/// Keep a message deletion for later.
pub(crate) fn later_redact(room: &Room, event: &str) {
    enqueue(Pending::Redact {
        room: room.room_id().to_string(),
        event: event.to_owned(),
    });
}

/// Writes still waiting for the connection.
pub fn waiting() -> usize {
    STORE.lock().unwrap().as_ref().map_or(0, |s| s.outbox.len())
}

/// No answer because there's no connection (not a refusal by the server).
pub(crate) fn is_network(e: &Error) -> bool {
    match e {
        Error::Http(h) => http_is_network(h),
        Error::Matrix(e) => sdk_is_network(e),
        Error::Offline => true,
        _ => false,
    }
}

fn sdk_is_network(e: &matrix_sdk::Error) -> bool {
    matches!(e, matrix_sdk::Error::Http(h) if http_is_network(h))
}

fn http_is_network(e: &matrix_sdk::HttpError) -> bool {
    match e {
        matrix_sdk::HttpError::Reqwest(_) => true,
        // Refreshing the access token needs the network too.
        matrix_sdk::HttpError::RefreshToken(_) => true,
        matrix_sdk::HttpError::Cached(inner) => http_is_network(inner),
        _ => false,
    }
}

/// `fut`, or `None` once `wait` has passed.
async fn within<T>(wait: Duration, fut: impl Future<Output = T>) -> Option<T> {
    let sleep = crate::rt::sleep(wait);
    futures_util::pin_mut!(fut, sleep);
    match futures_util::future::select(fut, sleep).await {
        futures_util::future::Either::Left((value, _)) => Some(value),
        futures_util::future::Either::Right(_) => None,
    }
}

// ---- Reads ------------------------------------------------------------------------------------

fn waiting_value(target: &str) -> Option<Value> {
    let store = STORE.lock().unwrap();
    let store = store.as_ref()?;
    store.outbox.iter().rev().find_map(|p| match p {
        Pending::State { content, .. }
        | Pending::RoomData { content, .. }
        | Pending::GlobalData { content, .. }
            if p.target() == target =>
        {
            Some(content.clone())
        }
        _ => None,
    })
}

fn cached(target: &str) -> Option<Value> {
    STORE.lock().unwrap().as_ref()?.cache.get(target).cloned()
}

fn remember(target: String, value: Option<Value>) {
    let saved = {
        let mut store = STORE.lock().unwrap();
        let Some(store) = store.as_mut() else { return };
        store.order.retain(|k| *k != target);
        match value {
            Some(value) => {
                if store.cache.get(&target) == Some(&value) {
                    store.order.push(target);
                    return;
                }
                store.cache.insert(target.clone(), value);
                store.order.push(target);
            }
            None => {
                if store.cache.remove(&target).is_none() {
                    return;
                }
            }
        }
        while store.order.len() > MAX_CACHED {
            let old = store.order.remove(0);
            store.cache.remove(&old);
        }
        let values: Vec<(String, Value)> = store
            .order
            .iter()
            .filter_map(|k| Some((k.clone(), store.cache.get(k)?.clone())))
            .collect();
        (store.cache_path.clone(), Cache { values })
    };
    save(saved.0, &saved.1);
}

fn save(path: PathBuf, value: &impl Serialize) {
    match serde_json::to_vec(value) {
        Ok(bytes) => {
            crate::rt::spawn(async move {
                if let Err(e) = crate::files::write(&path, bytes).await {
                    warn!("offline store: {e}");
                }
            });
        }
        Err(e) => warn!("offline store: {e}"),
    }
}

/// A state event's content (`None`: there is none), from the server when it answers, else
/// from what we last saw or the local store.
pub(crate) async fn read_state(room: &Room, event_type: &str, key: &str) -> Result<Option<Value>> {
    let target = state_key(room.room_id().as_str(), event_type, key);
    if let Some(value) = waiting_value(&target) {
        return Ok(Some(value));
    }
    let request = get_state_event_for_key::v3::Request::new(
        room.room_id().to_owned(),
        StateEventType::from(event_type),
        key.to_owned(),
    );
    let client = room.client();
    let failure = match within(READ_WAIT, async { client.send(request).await }).await {
        Some(Ok(response)) => {
            let value: Option<Value> = serde_json::from_str(response.event_or_content.get()).ok();
            remember(target, value.clone());
            return Ok(value);
        }
        Some(Err(e)) if is_not_found(&e) => {
            remember(target, None);
            return Ok(None);
        }
        Some(Err(e)) if !http_is_network(&e) => return Err(e.into()),
        Some(Err(e)) => Error::from(e),
        None => Error::Offline,
    };
    if let Some(value) = cached(&target) {
        return Ok(Some(value));
    }
    // The SDK's own copy (it keeps the standard state types).
    if let Ok(Some(raw)) = room
        .get_state_event(StateEventType::from(event_type), key)
        .await
    {
        let json = match raw {
            matrix_sdk::deserialized_responses::RawAnySyncOrStrippedState::Sync(r) => {
                r.json().get().to_owned()
            }
            matrix_sdk::deserialized_responses::RawAnySyncOrStrippedState::Stripped(r) => {
                r.json().get().to_owned()
            }
        };
        if let Ok(event) = serde_json::from_str::<Value>(&json) {
            return Ok(event.get("content").cloned());
        }
    }
    Err(failure)
}

fn is_not_found(e: &matrix_sdk::HttpError) -> bool {
    matches!(
        e.client_api_error_kind(),
        Some(matrix_sdk::ruma::api::error::ErrorKind::NotFound)
    )
}

/// Global account data, from the server when it answers, else the local copy.
pub(crate) async fn read_global(client: &Client, event_type: &str) -> Result<Option<Value>> {
    let target = format!("global|{event_type}");
    if let Some(value) = waiting_value(&target) {
        return Ok(Some(value));
    }
    let account = client.account();
    let kind = GlobalAccountDataEventType::from(event_type);
    match within(READ_WAIT, account.fetch_account_data(kind.clone())).await {
        Some(Ok(raw)) => {
            return Ok(raw.and_then(|r| serde_json::from_str(r.json().get()).ok()));
        }
        // Offline, slow, or refused: the local copy is the best we have.
        Some(Err(e)) => warn!("{event_type} from the server: {e}"),
        None => {}
    }
    Ok(account
        .account_data_raw(kind)
        .await?
        .and_then(|r| serde_json::from_str(r.json().get()).ok()))
}

/// A room's members: the server's list when it answers (the SDK fetches it once), else the ones
/// the store knows.
pub(crate) async fn members(
    room: &Room,
    which: matrix_sdk::RoomMemberships,
) -> Result<Vec<matrix_sdk::room::RoomMember>> {
    match within(READ_WAIT, room.members(which)).await {
        Some(Ok(list)) => Ok(list),
        Some(Err(e)) if !sdk_is_network(&e) => Err(e.into()),
        _ => Ok(room.members_no_sync(which).await?),
    }
}

/// Remember any value under `key` (kept on disk), for `last_seen` when offline.
pub(crate) fn remember_value(key: &str, value: Value) {
    remember(format!("value|{key}"), Some(value));
}

pub(crate) fn last_seen(key: &str) -> Option<Value> {
    cached(&format!("value|{key}"))
}

// ---- Writes -----------------------------------------------------------------------------------

/// Set a state event; without a connection it waits in the outbox (and reads see it).
pub(crate) async fn write_state(
    room: &Room,
    event_type: &str,
    key: &str,
    content: Value,
) -> Result<()> {
    let pending = Pending::State {
        room: room.room_id().to_string(),
        event_type: event_type.to_owned(),
        key: key.to_owned(),
        content,
    };
    send_or_wait(&room.client(), pending).await
}

/// Set room account data; waits in the outbox without a connection.
pub(crate) async fn write_room_data(room: &Room, event_type: &str, content: Value) -> Result<()> {
    let pending = Pending::RoomData {
        room: room.room_id().to_string(),
        event_type: event_type.to_owned(),
        content,
    };
    send_or_wait(&room.client(), pending).await
}

/// Set global account data; waits in the outbox without a connection.
pub(crate) async fn write_global(client: &Client, event_type: &str, content: Value) -> Result<()> {
    let pending = Pending::GlobalData {
        event_type: event_type.to_owned(),
        content,
    };
    send_or_wait(client, pending).await
}

/// Add or remove a room tag (`m.favourite`, `m.lowpriority`); waits without a connection.
pub(crate) async fn write_tag(room: &Room, tag: &str, on: bool) -> Result<()> {
    let pending = Pending::Tag {
        room: room.room_id().to_string(),
        tag: tag.to_owned(),
        on,
    };
    send_or_wait(&room.client(), pending).await
}

/// Mark a chat unread (or not); waits without a connection.
pub(crate) async fn write_unread(room: &Room, on: bool) -> Result<()> {
    let pending = Pending::Unread {
        room: room.room_id().to_string(),
        on,
    };
    send_or_wait(&room.client(), pending).await
}

async fn send_or_wait(client: &Client, pending: Pending) -> Result<()> {
    // Something for the same target already waits: queue behind it, keeping the order.
    let queued_target = {
        let store = STORE.lock().unwrap();
        store
            .as_ref()
            .is_some_and(|s| s.outbox.iter().any(|p| p.target() == pending.target()))
    };
    if !queued_target {
        match send(client, &pending).await {
            Ok(()) => return Ok(()),
            Err(e) if !is_network(&e) => return Err(e),
            Err(e) => info!("offline, kept for later: {} ({e})", pending.target()),
        }
    }
    enqueue(pending);
    Ok(())
}

fn enqueue(pending: Pending) {
    let saved = {
        let mut store = STORE.lock().unwrap();
        let Some(store) = store.as_mut() else { return };
        let target = pending.target();
        store.outbox.retain(|p| p.target() != target);
        // What reads would get from the server once it's sent.
        if let Pending::State { content, .. }
        | Pending::RoomData { content, .. }
        | Pending::GlobalData { content, .. } = &pending
        {
            store.cache.insert(target.clone(), content.clone());
        }
        store.outbox.push(pending);
        (store.outbox_path.clone(), store.outbox.clone())
    };
    save(saved.0, &saved.1);
}

async fn send(client: &Client, pending: &Pending) -> Result<()> {
    let room = |id: &str| -> Result<Room> {
        let id = OwnedRoomId::try_from(id).map_err(|_| Error::UnknownRoom)?;
        client.get_room(&id).ok_or(Error::UnknownRoom)
    };
    match pending {
        Pending::State {
            room: id,
            event_type,
            key,
            content,
        } => {
            room(id)?
                .send_state_event_raw(event_type, key, content.clone())
                .await?;
        }
        Pending::RoomData {
            room: id,
            event_type,
            content,
        } => {
            room(id)?
                .set_account_data_raw(
                    RoomAccountDataEventType::from(event_type.as_str()),
                    Raw::new(content)?.cast_unchecked(),
                )
                .await?;
        }
        Pending::GlobalData {
            event_type,
            content,
        } => {
            client
                .account()
                .set_account_data_raw(
                    GlobalAccountDataEventType::from(event_type.as_str()),
                    Raw::new(content)?.cast_unchecked(),
                )
                .await?;
        }
        Pending::Tag { room: id, tag, on } => {
            let room = room(id)?;
            let tag = matrix_sdk::ruma::events::tag::TagName::from(tag.as_str());
            if *on {
                room.set_tag(tag, Default::default()).await?;
            } else {
                room.remove_tag(tag).await?;
            }
        }
        Pending::Unread { room: id, on } => {
            room(id)?.set_unread_flag(*on).await?;
        }
        Pending::Mute { room: id, on } => {
            use matrix_sdk::notification_settings::RoomNotificationMode;
            let room = room(id)?;
            let settings = client.notification_settings().await;
            let result = if *on {
                settings
                    .set_room_notification_mode(room.room_id(), RoomNotificationMode::Mute)
                    .await
            } else {
                settings
                    .delete_user_defined_room_rules(room.room_id())
                    .await
            };
            if let Err(e) = result {
                // It hides the cause: still offline means try again later.
                return Err(if online() {
                    Error::Sync(e.to_string())
                } else {
                    Error::Offline
                });
            }
        }
        Pending::Read { room: id, private } => {
            use matrix_sdk::ruma::{
                api::client::receipt::create_receipt::v3::ReceiptType,
                events::receipt::ReceiptThread,
            };
            let room = room(id)?;
            let (cache, _handles) = room
                .event_cache()
                .await
                .map_err(|e| Error::Timeline(e.to_string()))?;
            let latest = cache
                .rfind_map_event_in_memory_by(|e| e.event_id().map(|id| id.to_owned()))
                .await
                .map_err(|e| Error::Timeline(e.to_string()))?;
            if let Some(latest) = latest {
                let kind = if *private {
                    ReceiptType::ReadPrivate
                } else {
                    ReceiptType::Read
                };
                room.send_single_receipt(kind, ReceiptThread::Unthreaded, latest)
                    .await?;
            }
        }
        Pending::Redact { room: id, event } => {
            let event = matrix_sdk::ruma::OwnedEventId::try_from(event.as_str())
                .map_err(|_| Error::UnknownEvent)?;
            room(id)?.redact(&event, None, None).await?;
        }
    }
    Ok(())
}

/// Send what's waiting, oldest first; stop at the first network failure (still offline).
async fn flush(client: &Client) {
    loop {
        let next = STORE
            .lock()
            .unwrap()
            .as_ref()
            .and_then(|s| s.outbox.first().cloned());
        let Some(pending) = next else { return };
        let result = send(client, &pending).await;
        if let Err(e) = &result
            && is_network(e)
        {
            return;
        }
        match result {
            Ok(()) => info!("sent after being offline: {}", pending.target()),
            Err(e) => warn!("dropped a change made offline ({}): {e}", pending.target()),
        }
        let saved =
            {
                let mut store = STORE.lock().unwrap();
                let Some(store) = store.as_mut() else { return };
                // Only this exact write: a newer one for the same target may have replaced it.
                if let Some(i) = store.outbox.iter().position(|p| {
                    serde_json::to_value(p).ok() == serde_json::to_value(&pending).ok()
                }) {
                    store.outbox.remove(i);
                }
                (store.outbox_path.clone(), store.outbox.clone())
            };
        save(saved.0, &saved.1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_newer_write_replaces_the_waiting_one() {
        let dir = std::env::temp_dir().join(format!("alwy-offline-{}", std::process::id()));
        *STORE.lock().unwrap() = Some(Store {
            cache_path: dir.join("c.json"),
            outbox_path: dir.join("o.json"),
            cache: HashMap::new(),
            order: Vec::new(),
            outbox: Vec::new(),
        });
        let theme = |name: &str| Pending::State {
            room: "!r:x".into(),
            event_type: "com.alwy.chat.room.theme".into(),
            key: String::new(),
            content: serde_json::json!({ "theme": name }),
        };
        enqueue(theme("ocean"));
        enqueue(Pending::Tag {
            room: "!r:x".into(),
            tag: "m.favourite".into(),
            on: true,
        });
        enqueue(theme("sunset"));
        assert_eq!(waiting(), 2);
        assert_eq!(waiting_flag("!r:x", "tag|m.favourite"), Some(true));
        assert_eq!(waiting_flag("!r:x", "tag|m.lowpriority"), None);
        assert_eq!(
            waiting_value(&state_key("!r:x", "com.alwy.chat.room.theme", "")),
            Some(serde_json::json!({ "theme": "sunset" }))
        );
        // Reads see it in the cache too.
        assert_eq!(
            cached(&state_key("!r:x", "com.alwy.chat.room.theme", "")),
            Some(serde_json::json!({ "theme": "sunset" }))
        );
        *STORE.lock().unwrap() = None;
    }
}
