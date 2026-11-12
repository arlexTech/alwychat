//! Push notifications: FCM on Android, through a push gateway (Sygnal holding the app's
//! Firebase credentials). No gateway is built into the app: the user's homeserver advertises one
//! in its `/.well-known/matrix/client` as `"com.alwy.chat.push": { "gateway": "<url>" }` (the URL is
//! given to the homeserver as the pusher's, so it can be an address only that homeserver
//! reaches). Servers without it get no push: the app notifies while it runs.
//!
//! UnifiedPush (any server, no Google): the phone's distributor app (ntfy…) gives an endpoint
//! URL; the pusher's gateway is the endpoint server's Matrix gateway when it has one (ntfy
//! does), else UnifiedPush's public one. The homeserver needn't offer anything.
//!
//! The homeserver only sends "event X in room Y" (`event_id_only`, nothing readable leaves
//! it); the phone wakes the app, which fetches and decrypts the event here and shows it.

use matrix_sdk::ruma::{
    EventId, RoomId,
    api::client::push::{Pusher, PusherIds, PusherInit, PusherKind},
    events::{AnySyncMessageLikeEvent, AnySyncTimelineEvent},
    push::{HttpPusherData, PushFormat},
};
use matrix_sdk_ui::notification_client::{
    NotificationClient, NotificationEvent, NotificationProcessSetup, NotificationStatus,
    RawNotificationEvent,
};
use serde::Serialize;
use tracing::info;

use crate::Result;

/// Key in the homeserver's `/.well-known/matrix/client` naming its AlwyChat push gateway.
const WELL_KNOWN_KEY: &str = crate::names::PUSH_WELL_KNOWN;
/// Must match the app id in the gateway's config. Builds made for another Android id
/// (`ALWY_PUSH_APP_ID` at build time, see scripts/release-local.sh) register under it.
pub const PUSH_APP_ID: &str = match option_env!("ALWY_PUSH_APP_ID") {
    Some(id) => id,
    None => crate::names::APP_ID,
};
/// Gateway built into this build (`ALWY_PUSH_GATEWAY` at build time), used when the
/// homeserver doesn't advertise one. Official builds have none.
const BUILT_IN_GATEWAY: Option<&str> = option_env!("ALWY_PUSH_GATEWAY");
/// UnifiedPush's public Matrix gateway, for endpoints whose server has none.
const UP_PUBLIC_GATEWAY: &str = "https://matrix.gateway.unifiedpush.org/_matrix/push/v1/notify";

/// The Matrix push gateway for a UnifiedPush endpoint (UnifiedPush's Matrix spec): the
/// endpoint server's own `/_matrix/push/v1/notify` if it answers
/// `{"unifiedpush":{"gateway":"matrix"}}`, else the public one.
pub async fn unified_push_gateway(endpoint: &str) -> String {
    let Ok(url) = reqwest::Url::parse(endpoint) else {
        return UP_PUBLIC_GATEWAY.to_owned();
    };
    let own = format!(
        "{}/_matrix/push/v1/notify",
        url.origin().ascii_serialization()
    );
    let answer = reqwest::Client::new()
        .get(&own)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await;
    let is_gateway = match answer {
        Ok(r) if r.status().is_success() => r
            .json::<serde_json::Value>()
            .await
            .is_ok_and(|j| j["unifiedpush"]["gateway"] == "matrix"),
        _ => false,
    };
    if is_gateway {
        own
    } else {
        UP_PUBLIC_GATEWAY.to_owned()
    }
}

/// Event ID of the test notification (see `test_push`).
pub const TEST_EVENT: &str = "$alwychat-push-test";

#[derive(Debug, Clone, serde::Serialize)]
pub struct PushTest {
    /// The homeserver has this phone's pusher, pointing at our gateway.
    pub registered: bool,
    /// The gateway took the test notification.
    pub gateway_ok: bool,
    /// Google said this phone's token is unknown (app reinstalled, data cleared…).
    pub rejected: bool,
    pub error: Option<String>,
}

/// What the phone shows for one pushed event.
#[derive(Debug, Clone, Serialize)]
pub struct PushNotification {
    pub room_id: String,
    /// The pushed event (Android's Reply / Mark as read act up to it).
    pub event_id: String,
    /// Sender (1:1 chats) or "Sender · Room".
    pub title: String,
    /// Who sent it, and the chat's name for groups (Android shows a conversation).
    pub sender: String,
    pub room_name: Option<String>,
    /// When it was sent (ms since 1970).
    pub sent_at: u64,
    pub body: String,
    /// Sound/vibration, per the push rules (false for "silent" rules).
    pub noisy: bool,
    /// Someone is calling: shown as an incoming call (rings until answered) instead.
    pub call: Option<PushCall>,
}

/// An incoming call delivered by push (the app may not be running).
#[derive(Debug, Clone, Serialize)]
pub struct PushCall {
    /// The ring event: pass back to `decline_call`.
    pub notification_id: String,
    pub caller_name: String,
    pub video: bool,
    /// When it stops ringing (ms since epoch).
    pub expires_at: u64,
}

/// Rooms whose call was already shown by push (callers send two ring formats at once).
static PUSHED_RINGS: std::sync::LazyLock<std::sync::Mutex<std::collections::HashMap<String, u64>>> =
    std::sync::LazyLock::new(Default::default);

impl crate::Core {
    /// The push gateway our homeserver advertises (asked each time: it's one small request at
    /// startup, and a server can add or remove it), else the one built into this build.
    pub async fn push_gateway(&self) -> Result<Option<String>> {
        Ok(self.advertised_gateway().await?.or_else(|| {
            BUILT_IN_GATEWAY
                .filter(|g| !g.is_empty())
                .map(ToOwned::to_owned)
        }))
    }

    async fn advertised_gateway(&self) -> Result<Option<String>> {
        let client = self.client()?;
        let server = client
            .user_id()
            .ok_or(crate::Error::NotLoggedIn)?
            .server_name()
            .to_owned();
        let url = format!("https://{server}/.well-known/matrix/client");
        // Unreachable is an error, not "no gateway": deciding "no push" on a bad moment left
        // the running app notifying too while the homeserver kept pushing (each message twice).
        let Some(text) = crate::download::fetch_text_if_any(&url).await? else {
            return Ok(None);
        };
        let json: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
        Ok(json[WELL_KNOWN_KEY]["gateway"]
            .as_str()
            .filter(|g| g.starts_with("https://") || g.starts_with("http://"))
            .map(ToOwned::to_owned))
    }

    async fn gateway(&self) -> Result<String> {
        self.push_gateway().await?.ok_or_else(|| {
            crate::Error::Push("your server doesn't offer push notifications for AlwyChat".into())
        })
    }

    /// Ask the homeserver to push this device's events to `token` (an FCM registration token).
    /// Check push end to end for `token`: is the pusher registered on the homeserver with our
    /// gateway, and does the gateway accept a (test) notification for it? The phone then shows
    /// it (PushService.kt recognises `TEST_EVENT`).
    pub async fn test_push(&self, token: &str) -> Result<PushTest> {
        let gateway = self.gateway().await?;
        self.test_push_with(token, &gateway).await
    }

    /// `test_push` for a UnifiedPush endpoint (and its gateway).
    pub async fn test_unified_push(&self, endpoint: &str) -> Result<PushTest> {
        let gateway = unified_push_gateway(endpoint).await;
        self.test_push_with(endpoint, &gateway).await
    }

    async fn test_push_with(&self, token: &str, gateway: &str) -> Result<PushTest> {
        use matrix_sdk::ruma::api::client::push::get_pushers;
        let client = self.client()?;
        let pushers = client.send(get_pushers::v3::Request::new()).await?.pushers;
        let registered = pushers.iter().any(|p| {
            p.ids.pushkey == token
                && p.ids.app_id == PUSH_APP_ID
                && matches!(&p.kind, matrix_sdk::ruma::api::client::push::PusherKind::Http(h) if h.url == gateway)
        });
        let body = serde_json::json!({
            "notification": {
                "event_id": TEST_EVENT,
                "room_id": "!alwychat-push-test:example.org",
                "counts": { "unread": 1 },
                "prio": "high",
                "devices": [{
                    "app_id": PUSH_APP_ID,
                    "pushkey": token,
                    "pushkey_ts": 0,
                    "data": { "format": "event_id_only" },
                }],
            },
        });
        let (gateway_ok, rejected, error) = match reqwest::Client::new()
            .post(gateway)
            .json(&body)
            .timeout(std::time::Duration::from_secs(20))
            .send()
            .await
        {
            Ok(r) if r.status().is_success() => {
                let answer: serde_json::Value = r.json().await.unwrap_or_default();
                let rejected = answer["rejected"]
                    .as_array()
                    .is_some_and(|a| a.iter().any(|k| k == token));
                (true, rejected, None)
            }
            Ok(r) => (
                false,
                false,
                Some(format!("the push gateway answered {}", r.status())),
            ),
            // An internal gateway (only the homeserver reaches it) is fine: this direct test
            // just can't run from the phone.
            Err(e) if unreachable(&e) => (
                false,
                false,
                Some("your server's push gateway can only be reached by the server itself, so this test can't run from here; send yourself a message from another device instead".into()),
            ),
            Err(e) => (false, false, Some(format!("push gateway unreachable: {e}"))),
        };
        Ok(PushTest {
            registered,
            gateway_ok,
            rejected,
            error,
        })
    }

    pub async fn register_pusher(&self, token: &str, device_name: &str) -> Result<()> {
        let gateway = self.gateway().await?;
        self.set_pusher(token, gateway, device_name).await
    }

    /// Push this device's events to a UnifiedPush `endpoint` (through its Matrix gateway).
    pub async fn register_unified_pusher(&self, endpoint: &str, device_name: &str) -> Result<()> {
        let gateway = unified_push_gateway(endpoint).await;
        info!(%gateway, "UnifiedPush gateway");
        self.set_pusher(endpoint, gateway, device_name).await
    }

    async fn set_pusher(&self, token: &str, gateway: String, device_name: &str) -> Result<()> {
        let client = self.client()?;
        let mut data = HttpPusherData::new(gateway);
        data.format = Some(PushFormat::EventIdOnly);
        // Copied into every push by the gateway; real messages replace it with their room.
        // Without it, the push saying "the unread count is now 0" (read on another device) has
        // no room, event or count, and Sygnal drops it, so the phone would never remove the
        // notification. Only a placeholder, nothing about the user. (FCM refuses some keys
        // outright, "from" for one, which then blocks every notification.)
        data.data.insert(
            "default_payload".to_owned(),
            serde_json::json!({ "room_id": "none" }),
        );
        let pusher: Pusher = PusherInit {
            ids: PusherIds::new(token.to_owned(), PUSH_APP_ID.to_owned()),
            kind: PusherKind::Http(data),
            app_display_name: "AlwyChat".to_owned(),
            device_display_name: device_name.to_owned(),
            profile_tag: None,
            lang: "en".to_owned(),
        }
        .into();
        // append = false: replaces any pusher with this token for another account.
        client.pusher().set(pusher, false).await?;
        info!("push registered");
        Ok(())
    }

    /// Stop pushes to `token` (notifications turned off).
    pub async fn remove_pusher(&self, token: &str) -> Result<()> {
        let client = self.client()?;
        client
            .pusher()
            .delete(PusherIds::new(token.to_owned(), PUSH_APP_ID.to_owned()))
            .await?;
        info!("push removed");
        Ok(())
    }

    /// The notification for a pushed event, or `None` if it shouldn't show (push rules,
    /// our own message, not found).
    pub async fn push_notification(
        &self,
        room_id: &str,
        event_id: &str,
    ) -> Result<Option<PushNotification>> {
        let room_id = <&RoomId>::try_from(room_id).map_err(|_| crate::Error::UnknownRoom)?;
        let event_id = <&EventId>::try_from(event_id).map_err(|_| crate::Error::UnknownEvent)?;
        let (client, sync_service) = self.with_active(|a| (a.client.clone(), a.sync.clone()))?;
        let own = client.user_id().map(ToOwned::to_owned);
        let notifications = NotificationClient::new(
            client.clone(),
            NotificationProcessSetup::SingleProcess { sync_service },
        )
        .await
        .map_err(|e| crate::Error::Push(e.to_string()))?;
        let item = match notifications
            .get_notification(room_id, event_id)
            .await
            .map_err(|e| crate::Error::Push(e.to_string()))?
        {
            NotificationStatus::Event(item) => item,
            // No push rule matches a call ring once decrypted (servers only have one for the old
            // `m.call.invite`), so the SDK filters it out: look at the event ourselves.
            NotificationStatus::EventFilteredOut => {
                return Ok(self.pushed_ring(&client, room_id, event_id).await);
            }
            _ => return Ok(None),
        };

        let sender = item
            .sender_display_name
            .clone()
            .unwrap_or_else(|| "Someone".into());
        // A call: rings like one, if it's still ringing and not our own.
        if let (NotificationEvent::Timeline(event), RawNotificationEvent::Timeline(raw)) =
            (&item.event, &item.raw_event)
            && Some(event.sender()) != own.as_deref()
            && let Some((video, expires_at)) = crate::rtc::fresh_ring(raw)
        {
            if !first_ring(room_id, expires_at) {
                return Ok(None);
            }
            return Ok(Some(PushNotification {
                room_id: room_id.to_string(),
                event_id: event_id.to_string(),
                title: sender.clone(),
                sender: sender.clone(),
                room_name: None,
                sent_at: event.origin_server_ts().get().into(),
                body: String::new(),
                noisy: true,
                call: Some(PushCall {
                    notification_id: event_id.to_string(),
                    caller_name: sender,
                    video,
                    expires_at,
                }),
            }));
        }
        let body = match &item.event {
            NotificationEvent::Invite(_) => "Invited you".to_owned(),
            NotificationEvent::Timeline(event) => match event.as_ref() {
                AnySyncTimelineEvent::MessageLike(AnySyncMessageLikeEvent::RoomEncrypted(_)) => {
                    "🔒 New message".to_owned()
                }
                AnySyncTimelineEvent::MessageLike(message) => {
                    match message
                        .original_content()
                        .and_then(|c| crate::rooms::preview(&c))
                    {
                        Some(text) => text,
                        None => return Ok(None),
                    }
                }
                AnySyncTimelineEvent::State(_) => return Ok(None),
            },
        };
        let room_name =
            (!item.is_direct_message_room).then(|| item.room_computed_display_name.clone());
        let title = match &room_name {
            None => sender.clone(),
            Some(name) => format!("{sender} · {name}"),
        };
        let sent_at = match &item.event {
            NotificationEvent::Timeline(event) => event.origin_server_ts().get().into(),
            NotificationEvent::Invite(_) => matrix_sdk::ruma::MilliSecondsSinceUnixEpoch::now()
                .get()
                .into(),
        };
        Ok(Some(PushNotification {
            room_id: room_id.to_string(),
            event_id: event_id.to_string(),
            title,
            sender,
            room_name,
            sent_at,
            body,
            noisy: item.is_noisy.unwrap_or(true),
            call: None,
        }))
    }
}

impl crate::Core {
    /// A reply typed in a message notification (the app may not be running): sent as
    /// Markdown, then the chat counts as read up to the message it answered.
    pub async fn reply_from_notification(
        &self,
        room_id: &str,
        event_id: &str,
        body: &str,
    ) -> Result<()> {
        let room = self.room(room_id)?;
        let content = matrix_sdk::ruma::events::room::message::RoomMessageEventContent::from(
            crate::mentions::compose(body, &[]),
        );
        room.send(content).await?;
        self.read_from_notification(room_id, event_id).await
    }

    /// "Mark as read" in a message notification: read receipt (private if receipts are off,
    /// like opening the chat) and read marker up to the pushed event.
    /// Of these chats, the ones with nothing left to notify about: read on another device.
    /// Asked of the server (a sync limited to them), as the app may have just been woken up.
    pub async fn read_chats(&self, room_ids: &[String]) -> Result<Vec<String>> {
        use matrix_sdk::ruma::api::client::{
            filter::{FilterDefinition, RoomFilter},
            sync::sync_events::v3,
        };
        let client = self.with_active(|a| a.client.clone())?;
        let mut rooms = RoomFilter::default();
        rooms.rooms = Some(
            room_ids
                .iter()
                .filter_map(|id| matrix_sdk::ruma::OwnedRoomId::try_from(id.as_str()).ok())
                .collect(),
        );
        rooms.timeline.limit = Some(1u32.into());
        rooms.state.types = Some(Vec::new());
        rooms.ephemeral.types = Some(Vec::new());
        rooms.account_data.types = Some(Vec::new());
        let mut filter = FilterDefinition::default();
        filter.room = rooms;
        filter.presence.types = Some(Vec::new());
        filter.account_data.types = Some(Vec::new());
        let mut request = v3::Request::new();
        request.filter = Some(v3::Filter::FilterDefinition(filter));
        let response = client.send(request).await?;
        Ok(response
            .rooms
            .join
            .into_iter()
            .filter(|(_, room)| {
                room.unread_notifications
                    .notification_count
                    .is_none_or(|count| count == 0u32.into())
            })
            .map(|(id, _)| id.to_string())
            .collect())
    }

    pub async fn read_from_notification(&self, room_id: &str, event_id: &str) -> Result<()> {
        let room = self.room(room_id)?;
        let event = matrix_sdk::ruma::OwnedEventId::try_from(event_id)
            .map_err(|_| crate::Error::UnknownEvent)?;
        let receipts = matrix_sdk::room::Receipts::new().fully_read_marker(event.clone());
        let receipts = if self.settings().send_read_receipts {
            receipts.public_read_receipt(event)
        } else {
            receipts.private_read_receipt(event)
        };
        room.send_multiple_receipts(receipts).await?;
        Ok(())
    }

    /// A pushed call ring, fetched and decrypted here, as an incoming-call notification.
    async fn pushed_ring(
        &self,
        client: &matrix_sdk::Client,
        room_id: &RoomId,
        event_id: &EventId,
    ) -> Option<PushNotification> {
        let room = client.get_room(room_id)?;
        let event = room.load_or_fetch_event(event_id, None).await.ok()?;
        let raw = event.raw();
        let sender = raw
            .get_field::<matrix_sdk::ruma::OwnedUserId>("sender")
            .ok()??;
        if Some(sender.as_ref()) == client.user_id() {
            return None;
        }
        let (video, expires_at) = crate::rtc::fresh_ring(raw)?;
        if !first_ring(room_id, expires_at) {
            return None;
        }
        let sent = raw
            .get_field::<u64>("origin_server_ts")
            .ok()
            .flatten()
            .unwrap_or(expires_at);
        let name = room
            .get_member_no_sync(&sender)
            .await
            .ok()
            .flatten()
            .and_then(|m| m.display_name().map(ToOwned::to_owned))
            .unwrap_or_else(|| sender.localpart().to_owned());
        Some(PushNotification {
            room_id: room_id.to_string(),
            event_id: event_id.to_string(),
            title: name.clone(),
            sender: name.clone(),
            room_name: None,
            sent_at: sent,
            body: String::new(),
            noisy: true,
            call: Some(PushCall {
                notification_id: event_id.to_string(),
                caller_name: name,
                video,
                expires_at,
            }),
        })
    }
}

/// False if this room's call was already shown (callers send two ring formats at once).
fn first_ring(room_id: &RoomId, expires_at: u64) -> bool {
    let mut pushed = PUSHED_RINGS.lock().unwrap();
    let now: u64 = matrix_sdk::ruma::MilliSecondsSinceUnixEpoch::now()
        .get()
        .into();
    pushed.retain(|_, until| *until > now);
    pushed.insert(room_id.to_string(), expires_at).is_none()
}

/// The gateway couldn't be reached at all (an address only the homeserver can reach).
fn unreachable(e: &reqwest::Error) -> bool {
    #[cfg(not(target_family = "wasm"))]
    return e.is_connect() || e.is_timeout();
    #[cfg(target_family = "wasm")]
    return e.is_timeout() || e.is_request();
}
