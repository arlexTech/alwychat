//! Calls and voice channels over MatrixRTC + LiveKit, compatible with Element Call / Element X.
//!
//! Signalling lives here; audio/video run in the webview with `livekit-client`.
//! Joining a call in a room:
//! 1. find the LiveKit service in the homeserver's well-known (`org.matrix.msc4143.rtc_foci`);
//! 2. trade an OpenID token for a LiveKit JWT at `<service>/sfu/get` (lk-jwt-service);
//! 3. publish our `org.matrix.msc3401.call.member` state event, plus a *delayed* empty one
//!    (MSC4140) that the server sends by itself if we stop refreshing it: a crashed or killed
//!    app leaves the call after `LEAVE_DELAY` instead of haunting it for hours.
//!
//! Media end-to-end encryption (Element Call's per-participant keys): `call_keys.rs`.

use std::time::Duration;

use matrix_sdk::{
    Room,
    ruma::{
        OwnedRoomId,
        api::client::{
            delayed_events::{
                DelayParameters, delayed_state_event,
                update_delayed_event::{UpdateAction, unstable_v1 as update_delayed},
            },
            rtc::RtcTransport,
        },
        events::{AnyStateEventContent, StateEventType},
        serde::Raw,
    },
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{info, warn};

use crate::{Error, Result};

pub(crate) const CALL_MEMBER: &str = "org.matrix.msc3401.call.member";
/// "Ring" (MSC4075), as Element Call / Element X send it; the stable name is accepted too.
const RTC_NOTIFICATION: &str = "org.matrix.msc4075.rtc.notification";
const RTC_NOTIFICATION_STABLE: &str = "m.rtc.notification";
/// Older Element versions ring with this one.
const CALL_NOTIFY: &str = "org.matrix.msc4075.call.notify";
/// "Declined" (MSC4310), relating to the notification.
const RTC_DECLINE: &str = "org.matrix.msc4310.rtc.decline";
const RTC_DECLINE_STABLE: &str = "m.rtc.decline";
/// How long a ring lasts if nobody answers.
const RING_LIFETIME: Duration = Duration::from_secs(30);
/// The longest a ring from someone else is allowed to last.
const MAX_RING_LIFETIME: Duration = Duration::from_secs(120);

/// Incoming ring or the answer to ours, for the UI.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CallSignal {
    /// Someone calls us in a 1:1 chat.
    Ring {
        room_id: String,
        /// Pass back to `decline_call`.
        notification_id: String,
        caller_name: String,
        caller_avatar: Option<String>,
        video: bool,
        /// When the ring stops on its own (ms since epoch).
        expires_at: u64,
    },
    /// Our ring was declined.
    Declined { room_id: String },
}

pub(crate) type CallSignalHandler = Box<dyn Fn(CallSignal) + Send + Sync>;
/// How long after our last heartbeat the server drops us from the call.
const LEAVE_DELAY: Duration = Duration::from_secs(20);
const HEARTBEAT: Duration = Duration::from_secs(8);

/// What the webview needs to connect to the call's LiveKit room.
#[derive(Debug, Clone, Serialize)]
pub struct CallConnection {
    pub url: String,
    pub jwt: String,
    /// Encrypt our media (encrypted room); others' is decrypted whenever they send a key.
    pub encrypted: bool,
}

/// The call we're in (one at a time).
/// The ID of our scheduled leave (None: the server takes no delayed events). Shared with the
/// heartbeat, which schedules a new one after the server sent the old one.
type DelayId = std::sync::Arc<std::sync::Mutex<Option<String>>>;

pub(crate) struct ActiveCall {
    room: Room,
    state_key: String,
    delay_id: DelayId,
    heartbeat: crate::rt::AbortHandle,
    /// Sharing our media key (encrypted rooms).
    keys: Option<crate::rt::AbortHandle>,
}

impl Drop for ActiveCall {
    fn drop(&mut self) {
        self.heartbeat.abort();
        if let Some(keys) = &self.keys {
            keys.abort();
        }
        crate::call_keys::forget(&self.room.room_id().to_owned());
    }
}

#[derive(Deserialize)]
struct SfuResponse {
    url: String,
    jwt: String,
}

impl crate::Core {
    /// Join (or start) the call in `room_id`. `video` only sets the advertised intent.
    pub async fn join_call(&self, room_id: &str, video: bool) -> Result<CallConnection> {
        self.leave_call().await?;
        let room = self.room(room_id)?;
        let client = room.client();
        let user = client.user_id().ok_or(Error::NotLoggedIn)?.to_owned();
        let device = client.device_id().ok_or(Error::NotLoggedIn)?.to_owned();

        let service = client
            .well_known_rtc_transports()
            .await?
            .into_iter()
            .find_map(|t| match t {
                RtcTransport::LiveKit(lk) => Some(lk.service_url),
                _ => None,
            })
            .ok_or_else(|| Error::Call("this server has no call service (LiveKit)".into()))?;
        let service = service.trim_end_matches('/').to_owned();

        // OpenID token → LiveKit JWT.
        let openid = client.account().request_openid_token().await?;
        let body = json!({
            "room": room.room_id(),
            "device_id": device,
            "openid_token": {
                "access_token": openid.access_token,
                "token_type": openid.token_type,
                "matrix_server_name": openid.matrix_server_name,
                "expires_in": openid.expires_in.as_secs(),
            },
        });
        let response = reqwest::Client::new()
            .post(format!("{service}/sfu/get"))
            .timeout(Duration::from_secs(15))
            .json(&body)
            .send()
            .await
            .map_err(|e| Error::Call(format!("call service unreachable: {e}")))?;
        if !response.status().is_success() {
            return Err(Error::Call(format!(
                "call service refused ({})",
                response.status()
            )));
        }
        let sfu: SfuResponse = response
            .json()
            .await
            .map_err(|e| Error::Call(format!("unexpected call service answer: {e}")))?;

        // Membership, as Element Call writes it (one state key per device).
        let state_key = format!("_{user}_{device}");
        let membership = json!({
            "application": "m.call",
            "call_id": "",
            "scope": "m.room",
            "device_id": device,
            "expires": 4 * 3600 * 1000,
            "focus_active": { "type": "livekit", "focus_selection": "oldest_membership" },
            "foci_preferred": [{
                "type": "livekit",
                "livekit_alias": room.room_id(),
                "livekit_service_url": service,
            }],
            "m.call.intent": if video { "video" } else { "audio" },
        });

        // The delayed leave goes first: if anything below fails, we still end up out.
        let delay_id: DelayId = std::sync::Arc::new(std::sync::Mutex::new(
            schedule_leave(&room, &state_key).await,
        ));
        let member_event = room
            .send_state_event_raw(CALL_MEMBER, &state_key, membership.clone())
            .await?
            .event_id;
        info!(room = %room.room_id(), "joined call");

        // Starting a call in a 1:1 chat rings the other person.
        let others_in_call = room
            .active_room_call_participants()
            .iter()
            .any(|u| *u != user);
        if !others_in_call && room.is_direct().await.unwrap_or(false) {
            ring(&room, &member_event, video).await;
        }
        let heartbeat = crate::rt::spawn(heartbeat(
            room.clone(),
            state_key.clone(),
            membership,
            delay_id.clone(),
        ))
        .abort_handle();
        let encrypted = room
            .latest_encryption_state()
            .await
            .is_ok_and(|s| s.is_encrypted());
        let keys =
            encrypted.then(|| crate::rt::spawn(crate::call_keys::run(room.clone())).abort_handle());
        *self.call.lock().await = Some(ActiveCall {
            room,
            state_key,
            delay_id,
            heartbeat,
            keys,
        });
        Ok(CallConnection {
            url: sfu.url,
            jwt: sfu.jwt,
            encrypted,
        })
    }

    /// Leave the current call, if any.
    pub async fn leave_call(&self) -> Result<()> {
        let Some(call) = self.call.lock().await.take() else {
            return Ok(());
        };
        // Fire the delayed leave now; if that fails, send the empty membership ourselves.
        call.heartbeat.abort();
        let delay_id = call.delay_id.lock().unwrap().clone();
        let fired = match delay_id {
            Some(id) => call
                .room
                .client()
                .send(update_delayed::Request::new(id, UpdateAction::Send))
                .await
                .is_ok(),
            None => false,
        };
        if !fired {
            call.room
                .send_state_event_raw(CALL_MEMBER, &call.state_key, json!({}))
                .await?;
        }
        info!(room = %call.room.room_id(), "left call");
        Ok(())
    }

    /// `handler` gets incoming rings (and answers to ours) while signed in.
    pub fn on_call_signal(&self, handler: impl Fn(CallSignal) + Send + Sync + 'static) {
        *self.on_call_signal.lock().unwrap() = Some(Box::new(handler));
    }

    /// Say no to a ring: the caller's app stops calling.
    pub async fn decline_call(&self, room_id: &str, notification_id: &str) -> Result<()> {
        let room = self.room(room_id)?;
        let content = json!({
            "m.relates_to": { "rel_type": "m.reference", "event_id": notification_id },
        });
        room.send_raw(RTC_DECLINE, content).await?;
        Ok(())
    }

    /// The room of the call we're in.
    pub async fn current_call(&self) -> Option<OwnedRoomId> {
        self.call
            .lock()
            .await
            .as_ref()
            .map(|c| c.room.room_id().to_owned())
    }
}

/// Ring the other side (new and older Element formats). Best effort: the call still works.
async fn ring(room: &Room, member_event: &matrix_sdk::ruma::EventId, video: bool) {
    let now = matrix_sdk::ruma::MilliSecondsSinceUnixEpoch::now();
    let intent = if video { "video" } else { "audio" };
    let notification = json!({
        "notification_type": "ring",
        "sender_ts": now,
        "lifetime": RING_LIFETIME.as_millis() as u64,
        "m.mentions": { "user_ids": [], "room": true },
        "m.relates_to": { "rel_type": "m.reference", "event_id": member_event },
        "m.call.intent": intent,
    });
    let legacy = json!({
        "application": "m.call",
        "call_id": "",
        "notify_type": "ring",
        "m.mentions": { "user_ids": [], "room": true },
    });
    for (kind, content) in [(RTC_NOTIFICATION, notification), (CALL_NOTIFY, legacy)] {
        if let Err(e) = room.send_raw(kind, content).await {
            warn!("couldn't ring ({kind}): {e}");
        }
    }
}

#[derive(Deserialize)]
struct NotificationContent {
    #[serde(default)]
    notification_type: Option<String>,
    #[serde(default)]
    notify_type: Option<String>,
    #[serde(default)]
    sender_ts: Option<u64>,
    #[serde(default)]
    lifetime: Option<u64>,
    #[serde(rename = "m.call.intent", default)]
    intent: Option<String>,
}

#[derive(Deserialize)]
struct DeclineContent {
    #[serde(rename = "m.relates_to")]
    relates_to: Reference,
}

#[derive(Deserialize)]
struct Reference {
    event_id: String,
}

/// Rings sent by this device: a decline relating to one of them is for us.
static OUR_RINGS: std::sync::LazyLock<std::sync::Mutex<std::collections::HashSet<String>>> =
    std::sync::LazyLock::new(Default::default);

/// Watch for rings and declines (called once per signed-in client).
pub(crate) fn install(
    client: &matrix_sdk::Client,
    handler: std::sync::Arc<std::sync::Mutex<Option<CallSignalHandler>>>,
) {
    client.add_event_handler(
        move |event: Raw<matrix_sdk::ruma::events::AnySyncTimelineEvent>, room: Room| {
            let handler = handler.clone();
            async move {
                let Some(signal) = call_signal(&event, &room).await else {
                    return;
                };
                if let Some(handler) = handler.lock().unwrap().as_ref() {
                    handler(signal);
                }
            }
        },
    );
}

async fn call_signal(
    event: &Raw<matrix_sdk::ruma::events::AnySyncTimelineEvent>,
    room: &Room,
) -> Option<CallSignal> {
    let kind = event.get_field::<String>("type").ok()??;
    let sender = event
        .get_field::<matrix_sdk::ruma::OwnedUserId>("sender")
        .ok()??;
    let own = sender == room.own_user_id();
    let event_id = event.get_field::<String>("event_id").ok()??;
    match kind.as_str() {
        RTC_NOTIFICATION | RTC_NOTIFICATION_STABLE | CALL_NOTIFY => {
            if own {
                OUR_RINGS.lock().unwrap().insert(event_id);
                return None;
            }
            let (video, expires_at) = fresh_ring(event)?;
            // Ringing with both formats: the legacy one is ignored if the other came first.
            if !RECENT_RINGS
                .lock()
                .unwrap()
                .insert(room.room_id().to_string(), expires_at)
            {
                return None;
            }
            // A muted chat doesn't ring.
            if room.cached_user_defined_notification_mode()
                == Some(matrix_sdk::notification_settings::RoomNotificationMode::Mute)
            {
                return None;
            }
            let member = room.get_member_no_sync(&sender).await.ok().flatten();
            Some(CallSignal::Ring {
                room_id: room.room_id().to_string(),
                notification_id: event_id,
                caller_name: member
                    .as_ref()
                    .and_then(|m| m.display_name().map(ToOwned::to_owned))
                    .unwrap_or_else(|| sender.localpart().to_owned()),
                caller_avatar: member.and_then(|m| m.avatar_url().map(ToString::to_string)),
                video,
                expires_at,
            })
        }
        RTC_DECLINE | RTC_DECLINE_STABLE if !own => {
            let content: DeclineContent = event.get_field("content").ok()??;
            OUR_RINGS
                .lock()
                .unwrap()
                .contains(&content.relates_to.event_id)
                .then(|| CallSignal::Declined {
                    room_id: room.room_id().to_string(),
                })
        }
        _ => None,
    }
}

/// A ring that is still ringing: (video call, expiry in ms since epoch). Old rings come in with
/// the backlog after a restart, and a push can arrive late: only fresh ones count.
pub(crate) fn fresh_ring(
    event: &Raw<matrix_sdk::ruma::events::AnySyncTimelineEvent>,
) -> Option<(bool, u64)> {
    let kind = event.get_field::<String>("type").ok()??;
    if !matches!(
        kind.as_str(),
        RTC_NOTIFICATION | RTC_NOTIFICATION_STABLE | CALL_NOTIFY
    ) {
        return None;
    }
    let content: NotificationContent = event.get_field("content").ok()??;
    if content.notification_type.or(content.notify_type)? != "ring" {
        return None;
    }
    let sent = event
        .get_field::<u64>("origin_server_ts")
        .ok()
        .flatten()
        .or(content.sender_ts)?;
    // The caller chooses how long it rings, within reason.
    let lifetime = content
        .lifetime
        .unwrap_or(RING_LIFETIME.as_millis() as u64)
        .min(MAX_RING_LIFETIME.as_millis() as u64);
    let expires_at = sent.saturating_add(lifetime);
    let now: u64 = matrix_sdk::ruma::MilliSecondsSinceUnixEpoch::now()
        .get()
        .into();
    (expires_at > now).then_some((content.intent.as_deref() == Some("video"), expires_at))
}

/// Room → expiry of the ring we last showed, so one call rings once.
struct RecentRings(std::collections::HashMap<String, u64>);

impl RecentRings {
    /// False if a ring for this room is still showing.
    fn insert(&mut self, room: String, expires_at: u64) -> bool {
        let now: u64 = matrix_sdk::ruma::MilliSecondsSinceUnixEpoch::now()
            .get()
            .into();
        if self.0.get(&room).is_some_and(|&until| until > now) {
            return false;
        }
        self.0.insert(room, expires_at);
        true
    }
}

static RECENT_RINGS: std::sync::LazyLock<std::sync::Mutex<RecentRings>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(RecentRings(Default::default())));

/// Schedule the empty membership the server sends if our heartbeats stop.
async fn schedule_leave(room: &Room, state_key: &str) -> Option<String> {
    let request = delayed_state_event::unstable::Request::new_raw(
        room.room_id().to_owned(),
        state_key.to_owned(),
        StateEventType::from(CALL_MEMBER),
        DelayParameters::Timeout {
            timeout: LEAVE_DELAY,
        },
        Raw::<AnyStateEventContent>::from_json_string("{}".into()).expect("valid JSON"),
    );
    match room.client().send(request).await {
        Ok(r) => Some(r.delay_id),
        Err(e) => {
            warn!("server doesn't take delayed events, a crash would leave us in the call: {e}");
            None
        }
    }
}

/// Keep pushing the delayed leave back while we're in the call. If the server sent it anyway
/// (the computer slept, or the network was away for longer than the delay), everyone saw us
/// leave while we're still connected: schedule a new one and publish our membership again.
async fn heartbeat(room: Room, state_key: String, membership: serde_json::Value, delay: DelayId) {
    if delay.lock().unwrap().is_none() {
        return;
    }
    // The server sent our leave and our membership isn't back yet.
    let mut absent = false;
    loop {
        crate::rt::sleep(HEARTBEAT).await;
        let Some(id) = delay.lock().unwrap().clone() else {
            return;
        };
        let request = update_delayed::Request::new(id, UpdateAction::Restart);
        if let Err(e) = room.client().send(request).await {
            warn!("call heartbeat failed: {e}");
            // Only when the server says that leave no longer exists (it was sent).
            if e.client_api_error_kind() != Some(&matrix_sdk::ruma::api::error::ErrorKind::NotFound)
            {
                continue;
            }
            let Some(new_id) = schedule_leave(&room, &state_key).await else {
                continue;
            };
            *delay.lock().unwrap() = Some(new_id);
            absent = true;
        }
        if absent {
            match room
                .send_state_event_raw(CALL_MEMBER, &state_key, membership.clone())
                .await
            {
                Ok(_) => {
                    absent = false;
                    info!(room = %room.room_id(), "back in the call's member list");
                }
                Err(e) => warn!("couldn't publish the call membership again: {e}"),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fresh_ring;
    use matrix_sdk::ruma::serde::Raw;

    fn event(
        kind: &str,
        ts: u64,
        content: serde_json::Value,
    ) -> Raw<matrix_sdk::ruma::events::AnySyncTimelineEvent> {
        Raw::new(&serde_json::json!({
            "type": kind, "event_id": "$e", "sender": "@a:x", "origin_server_ts": ts, "content": content,
        }))
        .unwrap()
        .cast_unchecked()
    }

    fn now() -> u64 {
        matrix_sdk::ruma::MilliSecondsSinceUnixEpoch::now()
            .get()
            .into()
    }

    #[test]
    fn rings() {
        let t = now() - 1_000;
        let ring = serde_json::json!({ "notification_type": "ring", "lifetime": 30_000, "m.call.intent": "video" });
        assert_eq!(
            fresh_ring(&event(
                "org.matrix.msc4075.rtc.notification",
                t,
                ring.clone()
            )),
            Some((true, t + 30_000))
        );
        assert_eq!(
            fresh_ring(&event("m.rtc.notification", t, ring.clone())),
            Some((true, t + 30_000))
        );
        // The legacy event, voice.
        let legacy = serde_json::json!({ "notify_type": "ring", "lifetime": 30_000 });
        assert_eq!(
            fresh_ring(&event("org.matrix.msc4075.call.notify", t, legacy)),
            Some((false, t + 30_000))
        );
    }

    #[test]
    fn a_ring_cannot_last_for_ever() {
        let t = now() - 1_000;
        let long = serde_json::json!({ "notification_type": "ring", "lifetime": u64::MAX });
        assert_eq!(
            fresh_ring(&event("m.rtc.notification", t, long)),
            Some((false, t + 120_000))
        );
    }

    #[test]
    fn not_rings() {
        let ring = serde_json::json!({ "notification_type": "ring", "lifetime": 30_000 });
        // Too old: the ring is over.
        assert_eq!(
            fresh_ring(&event(
                "org.matrix.msc4075.rtc.notification",
                now() - 60_000,
                ring.clone()
            )),
            None
        );
        // A group call notice, not a ring.
        let notice = serde_json::json!({ "notification_type": "notification", "lifetime": 30_000 });
        assert_eq!(
            fresh_ring(&event("org.matrix.msc4075.rtc.notification", now(), notice)),
            None
        );
        // Another event type.
        assert_eq!(fresh_ring(&event("m.room.message", now(), ring)), None);
    }
}
