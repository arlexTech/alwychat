//! Unsent composer text, synced between our devices through room account data
//! (`com.alwy.chat.draft`). Account data isn't encrypted: the homeserver can read drafts.
//! An empty content (`{}`) means no draft, since account data can't be deleted.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use matrix_sdk::{
    Client, Room,
    ruma::{
        events::{AnyRoomAccountDataEvent, RoomAccountDataEventType},
        serde::Raw,
    },
};
use serde::{Deserialize, Serialize};

use crate::Result;

pub const DRAFT: &str = "com.alwy.chat.draft";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Draft {
    pub text: String,
    /// Event ID of the message being replied to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply_to: Option<String>,
    /// Milliseconds since the Unix epoch: the newest version wins between devices.
    pub updated: u64,
}

/// A draft changed on another device (or ours, echoed by sync): room ID and the draft.
#[derive(Debug, Clone, Serialize)]
pub struct DraftChange {
    pub room_id: String,
    pub draft: Option<Draft>,
}

pub(crate) type DraftHandler = Box<dyn Fn(DraftChange) + Send + Sync>;

fn parse(content: &serde_json::Value) -> Option<Draft> {
    serde_json::from_value::<Draft>(content.clone())
        .ok()
        .filter(|d| !d.text.trim().is_empty())
}

async fn read(room: &Room) -> Option<Draft> {
    let raw = room
        .account_data(RoomAccountDataEventType::from(DRAFT))
        .await
        .ok()??;
    parse(&raw.get_field::<serde_json::Value>("content").ok()??)
}

impl crate::Core {
    /// Drafts of all joined chats, by room ID.
    pub async fn drafts(&self) -> Result<HashMap<String, Draft>> {
        let client = self.client()?;
        let mut out = HashMap::new();
        for room in client.joined_rooms() {
            if let Some(draft) = read(&room).await {
                out.insert(room.room_id().to_string(), draft);
            }
        }
        Ok(out)
    }

    /// Save (or clear, with `None`) a chat's draft.
    pub async fn set_draft(&self, room_id: &str, draft: Option<Draft>) -> Result<()> {
        let room = self.room(room_id)?;
        let content = match draft.filter(|d| !d.text.trim().is_empty()) {
            Some(d) => serde_json::to_value(d)?,
            None => serde_json::json!({}),
        };
        room.set_account_data_raw(
            RoomAccountDataEventType::from(DRAFT),
            Raw::new(&content)?.cast_unchecked(),
        )
        .await?;
        Ok(())
    }

    /// `handler` gets drafts changed through sync while signed in.
    pub fn on_draft(&self, handler: impl Fn(DraftChange) + Send + Sync + 'static) {
        *self.on_draft.lock().unwrap() = Some(Box::new(handler));
    }
}

/// Forward `com.alwy.chat.draft` changes to the handler (called once per signed-in client).
pub(crate) fn install(client: &Client, handler: Arc<Mutex<Option<DraftHandler>>>) {
    client.add_event_handler(move |event: Raw<AnyRoomAccountDataEvent>, room: Room| {
        let handler = handler.clone();
        async move {
            if event.get_field::<String>("type").ok().flatten().as_deref() != Some(DRAFT) {
                return;
            }
            let content: serde_json::Value = event
                .get_field("content")
                .ok()
                .flatten()
                .unwrap_or_default();
            let change = DraftChange {
                room_id: room.room_id().to_string(),
                draft: parse(&content),
            };
            if let Some(handler) = handler.lock().unwrap().as_ref() {
                handler(change);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_content_is_no_draft() {
        assert_eq!(parse(&serde_json::json!({})), None);
        assert_eq!(
            parse(&serde_json::json!({ "text": "  ", "updated": 1 })),
            None
        );
    }

    #[test]
    fn reads_a_draft() {
        let d = parse(&serde_json::json!({ "text": "hi", "reply_to": "$e", "updated": 5 }));
        assert_eq!(
            d,
            Some(Draft {
                text: "hi".into(),
                reply_to: Some("$e".into()),
                updated: 5
            })
        );
    }
}
