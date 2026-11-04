//! Call history (the Calls tab): calls this device rang for, answered, declined, started or
//! joined, kept on this device in `calls.json` next to the store (Matrix keeps no call log).
//! Voice channels aren't logged, as on Discord. Rings that came by push while the app was
//! closed are logged too, so missed calls show.

use std::{
    path::PathBuf,
    sync::{LazyLock, Mutex},
};

use matrix_sdk::ruma::MilliSecondsSinceUnixEpoch;
use serde::{Deserialize, Serialize};
use tracing::warn;

/// Oldest records are dropped beyond this.
const KEEP: usize = 300;
/// Joining a call this soon after its ring answers that ring.
const ANSWER_WINDOW_MS: u64 = 90_000;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CallKind {
    /// We called (a 1:1 or group chat).
    Outgoing,
    /// Someone called and we answered here.
    Incoming,
    /// Someone called; not answered here.
    Missed,
    /// Someone called and we said no.
    Declined,
    /// We joined a call already going on.
    Joined,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CallRecord {
    /// The ring's event ID, or a local one.
    pub id: String,
    pub room_id: String,
    pub kind: CallKind,
    pub video: bool,
    /// When it rang or started (ms).
    pub at: u64,
    /// How long we were in it (calls we were in, once over).
    #[serde(default)]
    pub duration_s: Option<u64>,
    /// Who called (incoming, missed, declined).
    #[serde(default)]
    pub caller: Option<String>,
    /// Someone else was in the call with us (an outgoing call without: "No answer").
    #[serde(default)]
    pub connected: bool,
    /// When we joined (ms), while we're in it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    joined_at: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CallHistory {
    /// Newest first.
    pub records: Vec<CallRecord>,
    /// Missed calls since the Calls tab was last looked at.
    pub unseen_missed: usize,
}

#[derive(Default, Serialize, Deserialize)]
struct Log {
    records: Vec<CallRecord>,
    /// Missed calls up to this (ms) were seen.
    #[serde(default)]
    seen_until: u64,
}

struct Store {
    path: PathBuf,
    log: Log,
}

static STORE: LazyLock<Mutex<Option<Store>>> = LazyLock::new(Default::default);
pub(crate) type CallHistoryHandler = Box<dyn Fn() + Send + Sync>;
static CHANGED: LazyLock<Mutex<Option<CallHistoryHandler>>> = LazyLock::new(Default::default);

fn now_ms() -> u64 {
    MilliSecondsSinceUnixEpoch::now().get().into()
}

/// Load this account's log (once per signed-in client).
pub(crate) fn install(path: PathBuf) {
    let log = crate::files::read_now(&path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    *STORE.lock().unwrap() = Some(Store { path, log });
}

/// Change the log, save it, tell the UI.
fn update(change: impl FnOnce(&mut Log)) {
    let saved = {
        let mut store = STORE.lock().unwrap();
        let Some(store) = store.as_mut() else { return };
        change(&mut store.log);
        store.log.records.sort_by_key(|r| std::cmp::Reverse(r.at));
        store.log.records.truncate(KEEP);
        serde_json::to_vec(&store.log).map(|b| (store.path.clone(), b))
    };
    match saved {
        // Small: written at once (works without a runtime, e.g. from a push).
        Ok((path, bytes)) => {
            if let Err(e) = crate::files::write_private_now(&path, &bytes) {
                warn!("call history: {e}");
            }
        }
        Err(e) => warn!("call history: {e}"),
    }
    if let Some(handler) = CHANGED.lock().unwrap().as_ref() {
        handler();
    }
}

/// Someone rang (seen live or by push): missed until answered here.
pub(crate) fn rang(room_id: &str, ring_id: &str, caller: &str, video: bool, at: u64) {
    update(|log| {
        if log.records.iter().any(|r| r.id == ring_id) {
            return;
        }
        log.records.push(CallRecord {
            id: ring_id.to_owned(),
            room_id: room_id.to_owned(),
            kind: CallKind::Missed,
            video,
            at,
            duration_s: None,
            caller: Some(caller.to_owned()),
            connected: false,
            joined_at: None,
        });
    });
}

/// The latest unanswered ring in the room, if recent.
fn recent_ring<'a>(log: &'a mut Log, room_id: &str, now: u64) -> Option<&'a mut CallRecord> {
    log.records.iter_mut().find(|r| {
        r.room_id == room_id
            && r.kind == CallKind::Missed
            && now.saturating_sub(r.at) < ANSWER_WINDOW_MS
    })
}

/// We said no to the room's ring.
pub(crate) fn declined(room_id: &str) {
    let now = now_ms();
    update(|log| {
        if let Some(r) = recent_ring(log, room_id, now) {
            r.kind = CallKind::Declined;
        }
    });
}

/// We're in the room's call now: answering its ring, starting it, or joining it.
pub(crate) fn joined(room_id: &str, video: bool, others_there: bool) {
    let now = now_ms();
    update(|log| {
        if let Some(r) = recent_ring(log, room_id, now) {
            r.kind = CallKind::Incoming;
            r.joined_at = Some(now);
            r.connected = true;
            return;
        }
        log.records.push(CallRecord {
            id: format!("local-{now}"),
            room_id: room_id.to_owned(),
            kind: if others_there {
                CallKind::Joined
            } else {
                CallKind::Outgoing
            },
            video,
            at: now,
            duration_s: None,
            caller: None,
            connected: others_there,
            joined_at: Some(now),
        });
    });
}

/// Someone else is in the call with us (seen while in it).
pub(crate) fn connected(room_id: &str) {
    let needed = STORE.lock().unwrap().as_ref().is_some_and(|s| {
        s.log
            .records
            .iter()
            .any(|r| r.room_id == room_id && r.joined_at.is_some() && !r.connected)
    });
    if needed {
        update(|log| {
            for r in log
                .records
                .iter_mut()
                .filter(|r| r.room_id == room_id && r.joined_at.is_some())
            {
                r.connected = true;
            }
        });
    }
}

/// We left the room's call: how long it lasted for us.
pub(crate) fn left(room_id: &str) {
    let now = now_ms();
    update(|log| {
        for r in log.records.iter_mut().filter(|r| r.room_id == room_id) {
            if let Some(joined) = r.joined_at.take() {
                r.duration_s = Some(now.saturating_sub(joined) / 1000);
            }
        }
    });
}

impl crate::Core {
    /// The call log of this device, newest first.
    pub fn call_history(&self) -> CallHistory {
        let store = STORE.lock().unwrap();
        let Some(store) = store.as_ref() else {
            return CallHistory {
                records: Vec::new(),
                unseen_missed: 0,
            };
        };
        CallHistory {
            unseen_missed: store
                .log
                .records
                .iter()
                .filter(|r| r.kind == CallKind::Missed && r.at > store.log.seen_until)
                .count(),
            records: store.log.records.clone(),
        }
    }

    /// The Calls tab was looked at: missed calls so far aren't new any more.
    pub fn mark_calls_seen(&self) {
        let now = now_ms();
        update(|log| log.seen_until = now);
    }

    /// Forget one call (`Some(id)`) or all of them.
    pub fn clear_call_history(&self, id: Option<&str>) {
        update(|log| match id {
            Some(id) => log.records.retain(|r| r.id != id),
            None => log.records.clear(),
        });
    }

    /// `handler` is called whenever the log changes.
    pub fn on_call_history(&self, handler: impl Fn() + Send + Sync + 'static) {
        *CHANGED.lock().unwrap() = Some(Box::new(handler));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh() {
        let dir = std::env::temp_dir().join(format!("alwy-calls-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        install(dir.join("calls.json"));
        update(|log| *log = Log::default());
    }

    fn kinds() -> Vec<CallKind> {
        STORE
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .log
            .records
            .iter()
            .map(|r| r.kind)
            .collect()
    }

    // One test: the log is process-wide.
    #[test]
    fn flows() {
        fresh();
        let now = now_ms();
        // A ring, then answered here.
        rang("!a:x", "$ring1", "@b:x", true, now - 5_000);
        assert_eq!(kinds(), [CallKind::Missed]);
        joined("!a:x", true, true);
        assert_eq!(kinds(), [CallKind::Incoming]);
        left("!a:x");
        let r = STORE.lock().unwrap().as_ref().unwrap().log.records[0].clone();
        assert_eq!(r.duration_s, Some(0));
        assert!(r.connected);
        // The same ring again (both formats, or push + live): once.
        rang("!a:x", "$ring1", "@b:x", true, now - 5_000);
        assert_eq!(kinds().len(), 1);
        // A ring declined.
        rang("!c:x", "$ring2", "@d:x", false, now - 1_000);
        declined("!c:x");
        assert_eq!(kinds()[0], CallKind::Declined);
        // An old ring isn't answered by joining later: a new outgoing call.
        rang("!e:x", "$ring3", "@f:x", false, now - 10 * 60_000);
        joined("!e:x", false, false);
        assert!(kinds().contains(&CallKind::Missed));
        assert_eq!(kinds()[0], CallKind::Outgoing);
        connected("!e:x");
        left("!e:x");
        assert!(STORE.lock().unwrap().as_ref().unwrap().log.records[0].connected);
        // Unseen missed calls, then seen.
        let core = crate::Core::new(std::env::temp_dir());
        assert_eq!(core.call_history().unseen_missed, 1);
        core.mark_calls_seen();
        assert_eq!(core.call_history().unseen_missed, 0);
        core.clear_call_history(None);
        assert!(core.call_history().records.is_empty());
    }
}
