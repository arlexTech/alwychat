//! Push notifications: FCM on Android, through AlwyChat's push gateway (Sygnal on example.org).
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

/// The gateway the homeserver forwards pushes to (Sygnal holds the FCM credentials).
pub const PUSH_GATEWAY: &str = "https://matrix.example.org/_matrix/push/v1/notify";
/// Must match the app id in the gateway's config.
pub const PUSH_APP_ID: &str = "com.alwy.chat";

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
    /// Ask the homeserver to push this device's events to `token` (an FCM registration token).
    pub async fn register_pusher(&self, token: &str, device_name: &str) -> Result<()> {
        let client = self.client()?;
        let mut data = HttpPusherData::new(PUSH_GATEWAY.to_owned());
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
