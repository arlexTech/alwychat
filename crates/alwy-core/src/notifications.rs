//! New-message notifications while the app runs (desktop). The SDK applies the account's
//! push rules, so muted chats and "mentions only" settings are respected.

use std::sync::{Arc, Mutex};

use matrix_sdk::{
    Client, Room,
    deserialized_responses::RawAnySyncOrStrippedTimelineEvent,
    ruma::events::{AnySyncMessageLikeEvent, AnySyncTimelineEvent},
    sync::Notification,
};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct MessageNotification {
    pub room_id: String,
    pub room_name: String,
    pub sender_name: String,
    pub body: String,
    pub is_dm: bool,
}

pub(crate) type NotificationHandler = Box<dyn Fn(MessageNotification) + Send + Sync>;

impl crate::Core {
    /// `handler` gets every notifying message from someone else, when notifications are on.
    pub fn on_notification(&self, handler: impl Fn(MessageNotification) + Send + Sync + 'static) {
        *self.on_notification.lock().unwrap() = Some(Box::new(handler));
    }
}

/// Hook the client's notifications to `handler` (called once per signed-in client).
pub(crate) async fn install(
    client: &Client,
    handler: Arc<Mutex<Option<NotificationHandler>>>,
    settings: Arc<Mutex<crate::settings::Settings>>,
) {
    client
        .register_notification_handler(
            move |notification: Notification, room: Room, client: Client| {
                let handler = handler.clone();
                let enabled = settings.lock().unwrap().notifications;
                async move {
                    if !enabled || !notification.actions.iter().any(|a| a.should_notify()) {
                        return;
                    }
                    let RawAnySyncOrStrippedTimelineEvent::Sync(raw) = &notification.event else {
                        return;
                    };
                    let Ok(AnySyncTimelineEvent::MessageLike(event)) = raw.deserialize() else {
                        return;
                    };
                    if Some(event.sender()) == client.user_id() {
                        return;
                    }
                    let body = match &event {
                        AnySyncMessageLikeEvent::RoomEncrypted(_) => "🔒 New message".to_owned(),
                        other => match other.original_content() {
                            Some(content) => match crate::rooms::preview(&content) {
                                Some(text) => text,
                                None => return,
                            },
                            None => return,
                        },
                    };
                    let sender = event.sender().to_owned();
                    let sender_name = match room.get_member_no_sync(&sender).await {
                        Ok(Some(m)) => m.display_name().unwrap_or(sender.localpart()).to_owned(),
                        _ => sender.localpart().to_owned(),
                    };
                    let message = MessageNotification {
                        room_id: room.room_id().to_string(),
                        room_name: room
                            .cached_display_name()
                            .map(|n| n.to_string())
                            .unwrap_or_else(|| sender_name.clone()),
                        sender_name,
                        body,
                        is_dm: room.is_direct().await.unwrap_or(false),
                    };
                    if let Some(handler) = handler.lock().unwrap().as_ref() {
                        handler(message);
                    }
                }
            },
        )
        .await;
}
