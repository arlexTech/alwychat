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
        OwnedEventId, OwnedRoomId, OwnedUserId, UserId,
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
    /// Our ring was declined (by `user_id`).
    Declined { room_id: String, user_id: String },
    /// Our ring is ringing on one of `user_id`'s devices (only AlwyChat says so).
    Ringing {
        room_id: String,
        user_id: String,
        device_id: String,
    },
    /// Someone in a call raised or lowered their hand (Element Call's format: a "🖐️" reaction
    /// on their call membership, redacted to lower it).
    Hand {
        room_id: String,
        user_id: String,
        device_id: Option<String>,
        raised: bool,
    },
    /// Someone in a call reacted (`io.element.call.reaction`, shown a few seconds).
    Reaction {
        room_id: String,
        user_id: String,
        device_id: Option<String>,
        emoji: String,
    },
}

/// Element Call's in-call reaction event.
const CALL_REACTION: &str = "io.element.call.reaction";
/// The reaction key of a raised hand (Element Call's).
const RAISED_HAND: &str = "🖐️";

/// A raised hand's room, user and device.
type HandOf = (String, String, Option<String>);
/// Raised hands seen, by their reaction event: a redaction of one lowers that hand.
static HANDS: std::sync::LazyLock<std::sync::Mutex<std::collections::HashMap<String, HandOf>>> =
    std::sync::LazyLock::new(Default::default);

/// Whose call membership `event_id` is (user, device), from the room's current call state.
async fn membership_of(room: &Room, event_id: &str) -> Option<(OwnedUserId, Option<String>)> {
    let events = room
        .get_state_events(StateEventType::from(CALL_MEMBER))
        .await
        .ok()?;
    for event in events {
        let matrix_sdk::deserialized_responses::RawAnySyncOrStrippedState::Sync(raw) = event else {
            continue;
        };
        if raw
            .get_field::<String>("event_id")
            .ok()
            .flatten()
            .as_deref()
            != Some(event_id)
        {
            continue;
        }
        let sender = raw.get_field::<OwnedUserId>("sender").ok().flatten()?;
        let device = raw
            .get_field::<serde_json::Value>("content")
            .ok()
            .flatten()
            .and_then(|c| {
                c.get("device_id")
                    .and_then(|d| d.as_str())
                    .map(ToOwned::to_owned)
            });
        return Some((sender, device));
    }
    None
}

#[derive(Deserialize)]
struct RelatesTo {
    #[serde(default)]
    rel_type: Option<String>,
    event_id: String,
    #[serde(default)]
    key: Option<String>,
}

#[derive(Deserialize)]
struct CallReactionContent {
    #[serde(rename = "m.relates_to")]
    relates_to: RelatesTo,
    emoji: String,
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
    /// We rang the other person (a 1:1 call nobody else was in, by the server): "Calling…".
    pub rang: bool,
}

/// The call we're in (one at a time).
/// The ID of our scheduled leave (None: the server takes no delayed events). Shared with the
/// heartbeat, which schedules a new one after the server sent the old one.
type DelayId = std::sync::Arc<std::sync::Mutex<Option<String>>>;

pub(crate) struct ActiveCall {
    room: Room,
    state_key: String,
    /// Our membership event: rings relate to it.
    member_event: OwnedEventId,
    video: bool,
    delay_id: DelayId,
    heartbeat: crate::rt::AbortHandle,
    /// Sharing our media key (encrypted rooms).
    keys: Option<crate::rt::AbortHandle>,
    /// Our raised hand's reaction event (redacted to lower it).
    hand: Option<OwnedEventId>,
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

        // Membership, as current Element Call writes it: one state key per device, ending
        // with the application (`_@user_DEVICE_m.call`), and a `membershipID`. Without the
        // membership ID FluffyChat never sent us its media key: we saw no picture (tested
        // 2026-11-18, with and without it).
        let state_key = format!("_{user}_{device}_m.call");
        let membership = json!({
            "application": "m.call",
            "call_id": "",
            "scope": "m.room",
            "device_id": device,
            "membershipID": format!("{user}:{device}"),
            "created_ts": matrix_sdk::ruma::MilliSecondsSinceUnixEpoch::now(),
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
        // Asked of the server: right after the other person left, the local copy may still
        // have them in the call (calling back at once then didn't ring).
        let others_in_call = match call_senders_on_server(&room).await {
            Some(senders) => senders.iter().any(|u| *u != user),
            None => room
                .active_room_call_participants()
                .iter()
                .any(|u| *u != user),
        };
        let rang = !others_in_call && room.is_direct().await.unwrap_or(false);
        if rang {
            ring(&room, &member_event, video, None).await;
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
            member_event,
            video,
            delay_id,
            heartbeat,
            keys,
            hand: None,
        });
        Ok(CallConnection {
            url: sfu.url,
            jwt: sfu.jwt,
            encrypted,
            rang,
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

    /// Ring one person into the call we're in (a voice channel or group call): only their
    /// devices ring. They must be in the room to get it.
    pub async fn ring_user(&self, user_id: &str) -> Result<()> {
        let user = OwnedUserId::try_from(user_id).map_err(|_| Error::Call("bad user ID".into()))?;
        let (room, member_event, video) = {
            let call = self.call.lock().await;
            let call = call
                .as_ref()
                .ok_or_else(|| Error::Call("not in a call".into()))?;
            (call.room.clone(), call.member_event.clone(), call.video)
        };
        if !ring(&room, &member_event, video, Some(&user)).await {
            return Err(Error::Call("couldn't ring".into()));
        }
        Ok(())
    }

    /// When the call in this chat began, as far as its current members tell (ms since epoch):
    /// the oldest join among the people in it now. None: nobody is in a call there.
    pub async fn call_started_at(&self, room_id: &str) -> Option<u64> {
        let room = self.room(room_id).ok()?;
        let now = matrix_sdk::ruma::MilliSecondsSinceUnixEpoch::now()
            .get()
            .into();
        let events = room
            .get_state_events(StateEventType::from(CALL_MEMBER))
            .await
            .ok()?;
        events
            .into_iter()
            .filter_map(|event| {
                let matrix_sdk::deserialized_responses::RawAnySyncOrStrippedState::Sync(raw) =
                    event
                else {
                    return None;
                };
                let content = raw.get_field::<serde_json::Value>("content").ok()??;
                let sent = raw.get_field::<u64>("origin_server_ts").ok()??;
                joined_at(&content, sent, now)
            })
            .min()
    }

    /// Whether a ring in `room_id` should go on ringing, asked of the server (a phone woken by a
    /// push doesn't sync): Some(false) once nobody else is in the call any more (the caller hung
    /// up) or one of our own devices joined it (answered elsewhere); None if the server can't be
    /// asked (keep ringing).
    pub async fn ring_still_on(&self, room_id: &str) -> Option<bool> {
        let room = self.room(room_id).ok()?;
        let own = room.client().user_id()?.to_owned();
        let senders = call_senders_on_server(&room).await?;
        if senders.contains(&own) {
            return Some(false);
        }
        Some(!senders.is_empty())
    }

    /// Raise or lower our hand in the call we're in (Element Call's format, so its users see
    /// it too).
    pub async fn raise_hand(&self, raised: bool) -> Result<()> {
        let mut call = self.call.lock().await;
        let Some(call) = call.as_mut() else {
            return Err(Error::Call("not in a call".into()));
        };
        if raised && call.hand.is_none() {
            let content = json!({
                "m.relates_to": {
                    "rel_type": "m.annotation",
                    "event_id": call.member_event,
                    "key": RAISED_HAND,
                },
            });
            call.hand = Some(
                call.room
                    .send_raw("m.reaction", content)
                    .await?
                    .response
                    .event_id,
            );
        } else if !raised && let Some(id) = call.hand.take() {
            call.room.redact(&id, None, None).await?;
        }
        Ok(())
    }

    /// A reaction in the call we're in (Element Call's `io.element.call.reaction`).
    pub async fn call_react(&self, emoji: &str, name: &str) -> Result<()> {
        let (room, member_event) = {
            let call = self.call.lock().await;
            let Some(call) = call.as_ref() else {
                return Err(Error::Call("not in a call".into()));
            };
            (call.room.clone(), call.member_event.clone())
        };
        let content = json!({
            "m.relates_to": { "rel_type": "m.reference", "event_id": member_event },
            "emoji": emoji,
            "name": name,
        });
        room.send_raw(CALL_REACTION, content).await?;
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

/// Who is in the room's call according to the server now (the local store can lag a few
/// seconds behind someone leaving). None: the server couldn't be asked.
async fn call_senders_on_server(room: &Room) -> Option<Vec<OwnedUserId>> {
    use matrix_sdk::ruma::api::client::state::get_state_events::v3 as state;
    let response = room
        .client()
        .send(state::Request::new(room.room_id().to_owned()))
        .await
        .ok()?;
    let mut senders = Vec::new();
    for event in &response.room_state {
        if event.get_field::<String>("type").ok().flatten().as_deref() != Some(CALL_MEMBER) {
            continue;
        }
        let Ok(Some(content)) = event.get_field::<serde_json::Value>("content") else {
            continue;
        };
        if !in_call(&content) {
            continue;
        }
        if let Some(sender) = event.get_field::<OwnedUserId>("sender").ok().flatten() {
            senders.push(sender);
        }
    }
    Some(senders)
}

/// Ring the other side (MSC4075 `rtc.notification`): everyone in the room, or only `target`
/// (its mention also makes the homeserver push it to them). Best effort: the call still works.
/// True if the ring went out.
async fn ring(
    room: &Room,
    member_event: &matrix_sdk::ruma::EventId,
    video: bool,
    target: Option<&UserId>,
) -> bool {
    let now = matrix_sdk::ruma::MilliSecondsSinceUnixEpoch::now();
    let intent = if video { "video" } else { "audio" };
    let mentions = match target {
        Some(user) => json!({ "user_ids": [user], "room": false }),
        None => json!({ "user_ids": [], "room": true }),
    };
    let notification = json!({
        "notification_type": "ring",
        "sender_ts": now,
        "lifetime": RING_LIFETIME.as_millis() as u64,
        "m.mentions": mentions,
        "m.relates_to": { "rel_type": "m.reference", "event_id": member_event },
        "m.call.intent": intent,
    });
    // Only the current MSC4075 format: the first version's `call.notify` was sent too, but apps
    // that know neither showed each as its own notification. Both are still read when others
    // ring us.
    match room.send_raw(RTC_NOTIFICATION, notification).await {
        // Known at once: an answer ("ringing", "declined") can come before our own copy.
        Ok(r) => {
            OUR_RINGS
                .lock()
                .unwrap()
                .insert(r.response.event_id.to_string());
            true
        }
        Err(e) => {
            warn!("couldn't ring: {e}");
            false
        }
    }
}

/// Tell the caller's devices that their ring is ringing here (`CallSignal::Ringing` there).
/// Plain to-device: it says nothing the server doesn't know, and needs no encryption session.
async fn say_ringing(
    client: &matrix_sdk::Client,
    room_id: &matrix_sdk::ruma::RoomId,
    caller: &UserId,
    notification_id: &str,
) {
    use matrix_sdk::ruma::{
        TransactionId,
        api::client::to_device::send_event_to_device::v3 as send,
        events::{AnyToDeviceEventContent, ToDeviceEventType},
        to_device::DeviceIdOrAllDevices,
    };
    let Some(device) = client.device_id() else {
        return;
    };
    let content = json!({
        "room_id": room_id,
        "notification_id": notification_id,
        "device_id": device,
    });
    let Ok(content) = Raw::new(&content) else {
        return;
    };
    let messages = [(
        caller.to_owned(),
        [(
            DeviceIdOrAllDevices::AllDevices,
            content.cast_unchecked::<AnyToDeviceEventContent>(),
        )]
        .into(),
    )]
    .into();
    let request = send::Request::new_raw(
        ToDeviceEventType::from(crate::names::CALL_RINGING),
        TransactionId::new(),
        messages,
    );
    if let Err(e) = client.send(request).await {
        warn!("couldn't tell the caller it rings: {e}");
    }
}

/// `say_ringing` in the background: the ring shows meanwhile.
pub(crate) fn say_ringing_soon(
    client: &matrix_sdk::Client,
    room_id: &matrix_sdk::ruma::RoomId,
    caller: &UserId,
    notification_id: &str,
) {
    let (client, room_id, caller, id) = (
        client.clone(),
        room_id.to_owned(),
        caller.to_owned(),
        notification_id.to_owned(),
    );
    crate::rt::spawn(async move {
        say_ringing(&client, &room_id, &caller, &id).await;
    });
}

#[derive(Deserialize)]
struct RingingContent {
    room_id: String,
    notification_id: String,
    device_id: String,
}

#[derive(Deserialize)]
struct Mentions {
    #[serde(default)]
    user_ids: Vec<OwnedUserId>,
    #[serde(default)]
    room: bool,
}

#[derive(Deserialize)]
struct NotificationContent {
    #[serde(rename = "m.mentions", default)]
    mentions: Option<Mentions>,
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
    let ringing_handler = handler.clone();
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
    // "It's ringing" from the people we ring.
    client.add_event_handler(
        move |event: Raw<matrix_sdk::ruma::events::AnyToDeviceEvent>| async move {
            if event.get_field::<String>("type").ok().flatten().as_deref()
                != Some(crate::names::CALL_RINGING)
            {
                return;
            }
            let (Ok(Some(sender)), Ok(Some(content))) = (
                event.get_field::<OwnedUserId>("sender"),
                event.get_field::<RingingContent>("content"),
            ) else {
                return;
            };
            if !OUR_RINGS.lock().unwrap().contains(&content.notification_id) {
                return;
            }
            let signal = CallSignal::Ringing {
                room_id: content.room_id,
                user_id: sender.to_string(),
                device_id: content.device_id,
            };
            if let Some(handler) = ringing_handler.lock().unwrap().as_ref() {
                handler(signal);
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
            let (video, expires_at) = fresh_ring(event, room.own_user_id())?;
            let sent = event
                .get_field::<u64>("origin_server_ts")
                .ok()
                .flatten()
                .unwrap_or(expires_at);
            // Ringing with both formats: the legacy one is ignored if the other came first.
            if !RECENT_RINGS
                .lock()
                .unwrap()
                .insert(room.room_id().to_string(), sent)
            {
                return None;
            }
            // A muted chat doesn't ring.
            if room.cached_user_defined_notification_mode()
                == Some(matrix_sdk::notification_settings::RoomNotificationMode::Mute)
            {
                return None;
            }
            say_ringing_soon(&room.client(), room.room_id(), &sender, &event_id);
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
        "m.reaction" if !own => {
            let relates: RelatesTo = event
                .get_field::<serde_json::Value>("content")
                .ok()??
                .get("m.relates_to")
                .and_then(|r| serde_json::from_value(r.clone()).ok())?;
            let key = relates.key.as_deref()?.trim_end_matches('\u{fe0f}');
            if relates.rel_type.as_deref() != Some("m.annotation") || key != "🖐" {
                return None;
            }
            let (user, device) = membership_of(room, &relates.event_id).await?;
            HANDS.lock().unwrap().insert(
                event_id,
                (room.room_id().to_string(), user.to_string(), device.clone()),
            );
            Some(CallSignal::Hand {
                room_id: room.room_id().to_string(),
                user_id: user.to_string(),
                device_id: device,
                raised: true,
            })
        }
        "m.room.redaction" if !own => {
            let redacts = event
                .get_field::<String>("redacts")
                .ok()
                .flatten()
                .or_else(|| {
                    event
                        .get_field::<serde_json::Value>("content")
                        .ok()
                        .flatten()
                        .and_then(|c| {
                            c.get("redacts")
                                .and_then(|r| r.as_str())
                                .map(ToOwned::to_owned)
                        })
                })?;
            let (room_id, user_id, device_id) = HANDS.lock().unwrap().remove(&redacts)?;
            Some(CallSignal::Hand {
                room_id,
                user_id,
                device_id,
                raised: false,
            })
        }
        CALL_REACTION if !own => {
            let content: CallReactionContent = event.get_field("content").ok()??;
            let (user, device) = membership_of(room, &content.relates_to.event_id).await?;
            // One emoji (Element Call trims excess characters too).
            let emoji: String = content.emoji.chars().take(8).collect();
            Some(CallSignal::Reaction {
                room_id: room.room_id().to_string(),
                user_id: user.to_string(),
                device_id: device,
                emoji,
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
                    user_id: sender.to_string(),
                })
        }
        _ => None,
    }
}

/// When a membership still in the call joined (ms): its `created_ts` (kept across refreshes),
/// else when it was sent. None: empty, or past its `expires`.
fn joined_at(content: &serde_json::Value, sent: u64, now: u64) -> Option<u64> {
    if !in_call(content) {
        return None;
    }
    // The older list form: the earliest of its entries.
    let one = |m: &serde_json::Value| {
        let created = m.get("created_ts").and_then(|v| v.as_u64()).unwrap_or(sent);
        let alive = m
            .get("expires")
            .and_then(|v| v.as_u64())
            .is_none_or(|e| created.saturating_add(e) > now);
        alive.then_some(created)
    };
    match content.get("memberships").and_then(|l| l.as_array()) {
        Some(list) => list.iter().filter_map(one).min(),
        None => one(content),
    }
}

/// A call membership's content means "in the call": the per-device form (non-empty), or the
/// older list form with at least one entry.
fn in_call(content: &serde_json::Value) -> bool {
    match content.get("memberships") {
        Some(list) => list.as_array().is_some_and(|l| !l.is_empty()),
        None => content.as_object().is_some_and(|o| !o.is_empty()),
    }
}

/// A ring for `own` that is still ringing: (video call, expiry in ms since epoch). Old rings come
/// in with the backlog after a restart, and a push can arrive late: only fresh ones count. A ring
/// naming people (ringing someone into a voice channel) is only for them.
pub(crate) fn fresh_ring(
    event: &Raw<matrix_sdk::ruma::events::AnySyncTimelineEvent>,
    own: &UserId,
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
    if let Some(m) = &content.mentions
        && !m.room
        && !m.user_ids.is_empty()
        && !m.user_ids.iter().any(|u| u == own)
    {
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

/// Room → when the ring we last showed there was sent. A caller sends two ring formats back to
/// back (one call, two events): only the first rings. A new call rings again, even right after
/// the previous one (keyed on the ring's expiry before, a call back within 30 s didn't ring).
pub(crate) struct RecentRings(std::collections::BTreeMap<String, u64>);

/// Rings of one call are sent together; a ring this long after the last one is a new call.
const SAME_CALL_MS: u64 = 5_000;

impl RecentRings {
    pub(crate) const fn new() -> Self {
        Self(std::collections::BTreeMap::new())
    }

    /// False if this is the other format of a ring already shown (`sent`: its server time, ms).
    pub(crate) fn insert(&mut self, room: String, sent: u64) -> bool {
        self.0
            .retain(|_, at| sent.saturating_sub(*at) < 10 * 60_000 || *at > sent);
        if self
            .0
            .get(&room)
            .is_some_and(|&at| sent.abs_diff(at) < SAME_CALL_MS)
        {
            return false;
        }
        self.0.insert(room, sent);
        true
    }
}

static RECENT_RINGS: std::sync::Mutex<RecentRings> = std::sync::Mutex::new(RecentRings::new());

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

/// The server's copy of our call membership is empty (we count as gone). Unknown = not gone.
async fn membership_gone(room: &Room, state_key: &str) -> bool {
    use matrix_sdk::ruma::api::client::state::get_state_event_for_key;
    let request = get_state_event_for_key::v3::Request::new(
        room.room_id().to_owned(),
        StateEventType::from(CALL_MEMBER),
        state_key.to_owned(),
    );
    match room.client().send(request).await {
        Ok(r) => serde_json::from_str::<serde_json::Value>(r.event_or_content.get())
            .is_ok_and(|content| !in_call(&content)),
        Err(e) => {
            e.client_api_error_kind() == Some(&matrix_sdk::ruma::api::error::ErrorKind::NotFound)
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
    let mut beats = 0u32;
    loop {
        crate::rt::sleep(HEARTBEAT).await;
        let Some(id) = delay.lock().unwrap().clone() else {
            return;
        };
        // Every ~30 s, also look at the room itself: if our membership is empty there for any
        // reason (seen 2026-11-20: someone stayed connected for minutes while everyone else's
        // list showed them gone), publish it again.
        beats = beats.wrapping_add(1);
        if !absent && beats.is_multiple_of(4) && membership_gone(&room, &state_key).await {
            warn!(room = %room.room_id(), "our call membership is gone from the room: publishing it again");
            absent = true;
        }
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
    use matrix_sdk::ruma::{UserId, serde::Raw};

    #[test]
    fn call_start_is_the_oldest_live_join() {
        use serde_json::json;
        let now = 1_800_000_000_000u64;
        let h = 3_600_000u64;
        // Per-device form: created_ts wins over the (refreshed) send time.
        assert_eq!(
            super::joined_at(
                &json!({"created_ts": now - h, "expires": 4 * h}),
                now - 5,
                now
            ),
            Some(now - h)
        );
        // Expired, empty (left), or no created_ts (send time).
        assert_eq!(
            super::joined_at(
                &json!({"created_ts": now - 5 * h, "expires": 4 * h}),
                now,
                now
            ),
            None
        );
        assert_eq!(super::joined_at(&json!({}), now, now), None);
        assert_eq!(
            super::joined_at(&json!({"device_id": "D"}), now - 9, now),
            Some(now - 9)
        );
        // Older list form: earliest live entry.
        let list = json!({"memberships": [{"created_ts": now - 2 * h, "expires": 4 * h}, {"created_ts": now - 10 * h, "expires": 4 * h}]});
        assert_eq!(super::joined_at(&list, now, now), Some(now - 2 * h));
    }

    #[test]
    fn rings_once_per_call() {
        let mut rings = super::RecentRings::new();
        let t = 1_800_000_000_000;
        assert!(rings.insert("!r".into(), t));
        // The same call's other format, sent right after: no second ring.
        assert!(!rings.insert("!r".into(), t + 40));
        // Another chat rings.
        assert!(rings.insert("!other".into(), t + 100));
        // Hung up and called back 26 s later, while the first ring would still be "live": rings.
        assert!(rings.insert("!r".into(), t + 26_000));
    }

    fn fresh_ring(
        event: &Raw<matrix_sdk::ruma::events::AnySyncTimelineEvent>,
    ) -> Option<(bool, u64)> {
        super::fresh_ring(event, <&UserId>::try_from("@me:x").unwrap())
    }

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
    fn memberships() {
        use serde_json::json;
        assert!(super::in_call(
            &json!({ "application": "m.call", "device_id": "D" })
        ));
        assert!(!super::in_call(&json!({})));
        assert!(super::in_call(
            &json!({ "memberships": [{ "device_id": "D" }] })
        ));
        assert!(!super::in_call(&json!({ "memberships": [] })));
    }

    #[test]
    fn a_ring_for_someone_else() {
        let t = now() - 1_000;
        let mine = serde_json::json!({ "notification_type": "ring", "lifetime": 30_000,
            "m.mentions": { "user_ids": ["@me:x"], "room": false } });
        assert!(fresh_ring(&event("m.rtc.notification", t, mine)).is_some());
        let theirs = serde_json::json!({ "notification_type": "ring", "lifetime": 30_000,
            "m.mentions": { "user_ids": ["@other:x"], "room": false } });
        assert_eq!(fresh_ring(&event("m.rtc.notification", t, theirs)), None);
        // Everyone (a 1:1 call), or no names at all.
        let all = serde_json::json!({ "notification_type": "ring", "lifetime": 30_000,
            "m.mentions": { "user_ids": [], "room": true } });
        assert!(fresh_ring(&event("m.rtc.notification", t, all)).is_some());
        let unnamed =
            serde_json::json!({ "notify_type": "ring", "lifetime": 30_000, "m.mentions": {} });
        assert!(fresh_ring(&event("org.matrix.msc4075.call.notify", t, unnamed)).is_some());
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
