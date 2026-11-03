//! Push notifications: FCM on Android, through a push gateway (Sygnal holding the app's
//! Firebase credentials). No gateway is built into the app: the user's homeserver advertises one
//! in its `/.well-known/matrix/client` as `"com.alwy.chat.push": { "gateway": "<url>" }` (the URL is
//! given to the homeserver as the pusher's, so it can be an address only that homeserver
//! reaches). Servers without it get no push: the app notifies while it runs.
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
};
use serde::Serialize;
use tracing::info;

use crate::Result;

/// Key in the homeserver's `/.well-known/matrix/client` naming its AlwyChat push gateway.
const WELL_KNOWN_KEY: &str = crate::names::PUSH_WELL_KNOWN;
/// Must match the app id in the gateway's config.
pub const PUSH_APP_ID: &str = crate::names::APP_ID;
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
    /// Sender (1:1 chats) or "Sender · Room".
    pub title: String,
    pub body: String,
    /// Sound/vibration, per the push rules (false for "silent" rules).
    pub noisy: bool,
}

impl crate::Core {
    /// The push gateway our homeserver advertises, if any (asked each time: it's one small
    /// request at startup, and a server can add or remove it).
    pub async fn push_gateway(&self) -> Result<Option<String>> {
        let client = self.client()?;
        let server = client
            .user_id()
            .ok_or(crate::Error::NotLoggedIn)?
            .server_name()
            .to_owned();
        let url = format!("https://{server}/.well-known/matrix/client");
        let Ok(text) = crate::download::fetch_text(&url).await else {
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
        use matrix_sdk::ruma::api::client::push::get_pushers;
        let client = self.client()?;
        let gateway = self.gateway().await?;
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
        let client = self.client()?;
        let mut data = HttpPusherData::new(self.gateway().await?);
        data.format = Some(PushFormat::EventIdOnly);
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
        let notifications = NotificationClient::new(
            client,
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
            _ => return Ok(None),
        };

        let sender = item
            .sender_display_name
            .clone()
            .unwrap_or_else(|| "Someone".into());
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
        let title = if item.is_direct_message_room {
            sender
        } else {
            format!("{sender} · {}", item.room_computed_display_name)
        };
        Ok(Some(PushNotification {
            room_id: room_id.to_string(),
            title,
            body,
            noisy: item.is_noisy.unwrap_or(true),
        }))
    }
}

/// The gateway couldn't be reached at all (an address only the homeserver can reach).
fn unreachable(e: &reqwest::Error) -> bool {
    #[cfg(not(target_family = "wasm"))]
    return e.is_connect() || e.is_timeout();
    #[cfg(target_family = "wasm")]
    return e.is_timeout() || e.is_request();
}
