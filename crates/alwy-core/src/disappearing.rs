//! Disappearing messages: a chat-wide timer (room state `com.alwy.chat.room.disappearing`,
//! `{timer_ms, since}`) after which each message sent from then on is deleted (redacted).
//!
//! - The sender's device deletes its own messages. In room version 11+ the deletion is handed to
//!   the server right away as a delayed redaction (MSC4140), so it happens even if every phone
//!   is off (Synapse caps delays, often at 24 h). Older rooms reject a delayed redaction (its
//!   target must sit outside the content there), and longer timers exceed the cap: those wait in
//!   a queue that this app works through while it runs (kept across restarts).
//! - Every AlwyChat hides an expired message at once (UI), deleted or not yet.

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

use matrix_sdk::{
    Client, Room,
    ruma::{
        MilliSecondsSinceUnixEpoch, OwnedEventId, OwnedRoomId, TransactionId,
        api::client::delayed_events::{DelayParameters, delayed_message_event},
        events::{AnyMessageLikeEventContent, AnySyncTimelineEvent, MessageLikeEventType},
        serde::Raw,
    },
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{info, warn};

use crate::{Result, names::DISAPPEARING};

/// Longest delay we ask the server for (Synapse's `max_delay`, often 24 h).
const SERVER_MAX_DELAY: Duration = Duration::from_secs(24 * 3600);
/// How long a chat's timer is trusted before being read again.
const TIMER_FRESH: Duration = Duration::from_secs(60);
const REASON: &str = "Disappearing message";

/// A chat's timer: messages sent since `since` (ms) disappear `timer_ms` after being sent.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Disappearing {
    pub timer_ms: u64,
    pub since: u64,
}

fn now_ms() -> u64 {
    MilliSecondsSinceUnixEpoch::now().get().into()
}

/// The chat's timer, if one is on.
pub(crate) async fn read(room: &Room) -> Result<Option<Disappearing>> {
    Ok(crate::custom::fetch::<Disappearing>(room, DISAPPEARING, "")
        .await?
        .filter(|d| d.timer_ms > 0))
}

/// Room version 11 or later: a delayed redaction works there.
fn takes_delayed_redactions(room: &Room) -> bool {
    room.version()
        .and_then(|v| v.as_str().parse::<u32>().ok())
        .is_some_and(|v| v >= 11)
}

/// One of our messages to delete at `at` (ms).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Pending {
    room: OwnedRoomId,
    event: OwnedEventId,
    at: u64,
}

struct Queue {
    path: PathBuf,
    items: Mutex<Vec<Pending>>,
    /// Woken when something earlier than the current wait is added.
    changed: tokio::sync::Notify,
    timers: Mutex<HashMap<OwnedRoomId, (Option<Disappearing>, u64)>>,
}

impl Queue {
    fn save(&self) {
        let items = self.items.lock().unwrap().clone();
        let path = self.path.clone();
        crate::rt::spawn(async move {
            match serde_json::to_vec(&items) {
                Ok(bytes) => {
                    if let Err(e) = crate::files::write(&path, bytes).await {
                        warn!("disappearing queue: {e}");
                    }
                }
                Err(e) => warn!("disappearing queue: {e}"),
            }
        });
    }

    async fn timer(&self, room: &Room) -> Option<Disappearing> {
        let now = now_ms();
        if let Some((timer, at)) = self.timers.lock().unwrap().get(room.room_id())
            && now.saturating_sub(*at) < TIMER_FRESH.as_millis() as u64
        {
            return *timer;
        }
        let timer = read(room).await.ok().flatten();
        self.timers
            .lock()
            .unwrap()
            .insert(room.room_id().to_owned(), (timer, now));
        timer
    }
}

/// Watch our own new messages in chats with a timer, and work through the deletion queue
/// (`path`: where it's kept). Called once per signed-in client.
pub(crate) fn install(client: &Client, path: PathBuf) -> crate::rt::AbortHandle {
    let items: Vec<Pending> = crate::files::read_now(&path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    let queue = Arc::new(Queue {
        path,
        items: Mutex::new(items),
        changed: tokio::sync::Notify::new(),
        timers: Mutex::new(HashMap::new()),
    });

    let watched = queue.clone();
    client.add_event_handler(move |event: Raw<AnySyncTimelineEvent>, room: Room| {
        let queue = watched.clone();
        async move { schedule(&queue, &event, &room).await }
    });

    let worker = queue.clone();
    let client = client.clone();
    crate::rt::spawn(async move { work(worker, client).await }).abort_handle()
}

/// Message types that disappear (not reactions, edits' targets stay with their message).
fn disappears(kind: &str) -> bool {
    matches!(
        kind,
        "m.room.message" | "m.sticker" | "org.matrix.msc3381.poll.start" | "m.poll.start"
    )
}

async fn schedule(queue: &Queue, event: &Raw<AnySyncTimelineEvent>, room: &Room) {
    // Only messages sent by this device (the server tells only the sender their transaction
    // ID): our other devices handle their own.
    let mine = event
        .get_field::<String>("sender")
        .ok()
        .flatten()
        .is_some_and(|s| s == room.own_user_id().as_str());
    let sent_here = event
        .get_field::<serde_json::Value>("unsigned")
        .ok()
        .flatten()
        .is_some_and(|u| u.get("transaction_id").is_some());
    let kind = event
        .get_field::<String>("type")
        .ok()
        .flatten()
        .unwrap_or_default();
    if !mine || !sent_here || !disappears(&kind) {
        return;
    }
    let Some(timer) = queue.timer(room).await else {
        return;
    };
    let (Some(id), Some(ts)) = (
        event.get_field::<OwnedEventId>("event_id").ok().flatten(),
        event.get_field::<u64>("origin_server_ts").ok().flatten(),
    ) else {
        return;
    };
    if ts < timer.since {
        return;
    }
    let at = ts.saturating_add(timer.timer_ms);
    let wait = at.saturating_sub(now_ms());
    if takes_delayed_redactions(room) && wait <= SERVER_MAX_DELAY.as_millis() as u64 {
        match delayed_redaction(room, &id, Duration::from_millis(wait)).await {
            Ok(()) => return,
            Err(e) => warn!("server won't delete {id} later, this app will: {e}"),
        }
    }
    queue.items.lock().unwrap().push(Pending {
        room: room.room_id().to_owned(),
        event: id,
        at,
    });
    queue.save();
    queue.changed.notify_one();
}

async fn delayed_redaction(room: &Room, event: &OwnedEventId, after: Duration) -> Result<()> {
    let content = json!({ "redacts": event, "reason": REASON });
    let request = delayed_message_event::unstable::Request::new_raw(
        room.room_id().to_owned(),
        TransactionId::new(),
        MessageLikeEventType::from("m.room.redaction"),
        DelayParameters::Timeout { timeout: after },
        Raw::new(&content)?.cast_unchecked::<AnyMessageLikeEventContent>(),
    );
    room.client().send(request).await?;
    Ok(())
}

/// Delete what's due, then sleep until the next one (or a new one arrives).
async fn work(queue: Arc<Queue>, client: Client) {
    loop {
        let now = now_ms();
        let due: Vec<Pending> = {
            let mut items = queue.items.lock().unwrap();
            let (due, later): (Vec<_>, Vec<_>) = items.drain(..).partition(|p| p.at <= now);
            *items = later;
            due
        };
        if !due.is_empty() {
            let mut retry = Vec::new();
            for p in due {
                let Some(room) = client.get_room(&p.room) else {
                    continue;
                };
                match room.redact(&p.event, Some(REASON), None).await {
                    Ok(_) => info!("disappeared: {}", p.event),
                    // Offline: try again later; gone or not allowed: drop it.
                    Err(e) if is_network(&e) => retry.push(Pending {
                        at: now + 60_000,
                        ..p
                    }),
                    Err(e) => warn!("couldn't delete {}: {e}", p.event),
                }
            }
            queue.items.lock().unwrap().extend(retry);
            queue.save();
        }
        let next = queue.items.lock().unwrap().iter().map(|p| p.at).min();
        let wait = next
            .map(|at| at.saturating_sub(now_ms()).min(60_000))
            .unwrap_or(60_000);
        let sleep = crate::rt::sleep(Duration::from_millis(wait.max(500)));
        let woken = queue.changed.notified();
        futures_util::pin_mut!(sleep, woken);
        futures_util::future::select(sleep, woken).await;
    }
}

fn is_network(e: &matrix_sdk::HttpError) -> bool {
    e.client_api_error_kind().is_none()
}

impl crate::Core {
    /// Turn the chat's timer on (`timer_ms`) or off (`None`), for everyone in it.
    pub async fn set_disappearing(&self, room_id: &str, timer_ms: Option<u64>) -> Result<()> {
        let room = self.room(room_id)?;
        let content = match timer_ms.filter(|t| *t > 0) {
            Some(timer_ms) => json!({ "timer_ms": timer_ms, "since": now_ms() }),
            None => json!({}),
        };
        crate::offline::write_state(&room, DISAPPEARING, "", content).await?;
        Ok(())
    }

    /// Whether the server deletes this chat's messages on time (room version 11+), or this app.
    pub fn disappearing_by_server(&self, room_id: &str) -> Result<bool> {
        Ok(takes_delayed_redactions(&self.room(room_id)?))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn what_disappears() {
        assert!(super::disappears("m.room.message"));
        assert!(super::disappears("m.sticker"));
        assert!(!super::disappears("m.reaction"));
        assert!(!super::disappears("m.room.redaction"));
    }
}
