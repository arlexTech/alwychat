//! Pinned messages (`m.room.pinned_events`, shared with Element and FluffyChat).
//!
//! Read from the server when a chat opens and re-read when the state changes (the open chat
//! isn't a sliding-sync room subscription, so the store may not have it).

use std::sync::Arc;

use matrix_sdk::{
    Room,
    event_handler::EventHandlerDropGuard,
    ruma::{
        OwnedEventId,
        events::{AnySyncMessageLikeEvent, AnySyncStateEvent, AnySyncTimelineEvent},
        serde::Raw,
    },
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::warn;

use crate::{Error, Result};

const PINNED_EVENTS: &str = "m.room.pinned_events";

#[derive(Debug, Clone, Serialize)]
pub struct PinnedMessage {
    pub event_id: String,
    pub sender_name: String,
    /// One-line preview ("📷 Photo", the text…).
    pub preview: String,
}

/// The chat's pins, most recently pinned first.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Pins {
    pub messages: Vec<PinnedMessage>,
    /// Whether our power level allows pinning/unpinning.
    pub can_pin: bool,
}

#[derive(Default, Deserialize)]
struct PinnedContent {
    #[serde(default)]
    pinned: Vec<OwnedEventId>,
}

impl crate::Core {
    /// Send `on_pins` the chat's pins now and after every change, until the guard drops.
    pub(crate) async fn watch_pins(
        &self,
        room: &Room,
        on_pins: impl Fn(Pins) + Send + Sync + 'static,
    ) -> EventHandlerDropGuard {
        let on_pins = Arc::new(on_pins);
        let first = room.clone();
        let send = on_pins.clone();
        tokio::spawn(async move { send(pins(&first).await) });

        let handle = room.add_event_handler(move |event: Raw<AnySyncStateEvent>, room: Room| {
            let on_pins = on_pins.clone();
            async move {
                let kind = event.get_field::<String>("type").ok().flatten();
                if matches!(kind.as_deref(), Some(PINNED_EVENTS | "m.room.power_levels")) {
                    on_pins(pins(&room).await);
                }
            }
        });
        room.client().event_handler_drop_guard(handle)
    }

    /// Pin (`pin = true`) or unpin a message.
    pub async fn pin_message(&self, room_id: &str, event_id: &str, pin: bool) -> Result<()> {
        let room = self.room(room_id)?;
        let event_id = OwnedEventId::try_from(event_id).map_err(|_| Error::UnknownEvent)?;
        // Re-read right before writing so a pin someone else just added isn't lost.
        let mut list = pinned_ids(&room).await?;
        list.retain(|id| *id != event_id);
        if pin {
            list.push(event_id);
        }
        room.send_state_event_raw(PINNED_EVENTS, "", json!({ "pinned": list }))
            .await?;
        Ok(())
    }
}

async fn pinned_ids(room: &Room) -> Result<Vec<OwnedEventId>> {
    Ok(
        crate::custom::fetch::<PinnedContent>(room, PINNED_EVENTS, "")
            .await?
            .unwrap_or_default()
            .pinned,
    )
}

async fn pins(room: &Room) -> Pins {
    let ids = pinned_ids(room).await.unwrap_or_else(|e| {
        warn!("couldn't read pinned messages: {e}");
        Vec::new()
    });
    let mut messages = Vec::new();
    // The list is oldest pin first; show the newest first.
    for id in ids.iter().rev() {
        if let Some(message) = pinned_message(room, id).await {
            messages.push(message);
        }
    }
    let can_pin = match room.power_levels().await {
        Ok(levels) => levels.user_can_send_state(room.own_user_id(), PINNED_EVENTS.into()),
        Err(_) => false,
    };
    Pins { messages, can_pin }
}

/// Deleted or unreachable pins are skipped (Element does the same).
async fn pinned_message(room: &Room, id: &OwnedEventId) -> Option<PinnedMessage> {
    let event = room.load_or_fetch_event(id, None).await.ok()?;
    let event: AnySyncTimelineEvent = event.raw().deserialize().ok()?;
    let AnySyncTimelineEvent::MessageLike(message) = &event else {
        return None;
    };
    let preview = match message {
        AnySyncMessageLikeEvent::RoomEncrypted(_) => "🔒 Encrypted message".to_owned(),
        other => crate::rooms::preview(&other.original_content()?)?,
    };
    let sender = message.sender();
    let sender_name = match room.get_member_no_sync(sender).await {
        Ok(Some(m)) => m.display_name().unwrap_or(sender.localpart()).to_owned(),
        _ => sender.localpart().to_owned(),
    };
    Some(PinnedMessage {
        event_id: id.to_string(),
        sender_name,
        preview,
    })
}
