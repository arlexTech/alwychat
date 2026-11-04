//! View-once photos and videos (WhatsApp style, AlwyChat only; other apps show them normally,
//! so it's a courtesy, not a protection): the message carries `com.alwy.chat.view_once`.
//!
//! - The recipient's AlwyChat shows a closed envelope instead of the picture; opening it shows
//!   the media once, remembered in global account data `com.alwy.chat.viewed_once` (so our
//!   other devices don't offer it again), and sends a `com.alwy.chat.view_once.viewed` event
//!   referencing the message.
//! - The sender's AlwyChat deletes (redacts) the message once everyone else in the chat has
//!   opened it. Only the sender may delete it, so this runs on the sender's devices.

use std::collections::HashSet;

use matrix_sdk::{
    Client, Room,
    room::{IncludeRelations, RelationsOptions},
    ruma::{
        OwnedEventId, OwnedUserId, UInt,
        events::{AnySyncTimelineEvent, GlobalAccountDataEventType, relation::RelationType},
        serde::Raw,
    },
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{info, warn};

use crate::{
    Error, Result,
    names::{VIEW_ONCE_VIEWED, VIEWED_ONCE},
};

/// How many opened messages are remembered.
const KEEP: usize = 500;
const REASON: &str = "Viewed once";

#[derive(Default, Serialize, Deserialize)]
struct Viewed {
    #[serde(default)]
    events: Vec<String>,
}

/// Watch for "viewed" events about our own view-once messages (once per signed-in client).
pub(crate) fn install(client: &Client) {
    client.add_event_handler(|event: Raw<AnySyncTimelineEvent>, room: Room| async move {
        if event.get_field::<String>("type").ok().flatten().as_deref() != Some(VIEW_ONCE_VIEWED) {
            return;
        }
        let Ok(Some(sender)) = event.get_field::<OwnedUserId>("sender") else {
            return;
        };
        if sender == room.own_user_id() {
            return;
        }
        let Some(target) = event
            .get_field::<serde_json::Value>("content")
            .ok()
            .flatten()
            .and_then(|c| {
                c.pointer("/m.relates_to/event_id")?
                    .as_str()
                    .map(str::to_owned)
            })
            .and_then(|id| OwnedEventId::try_from(id).ok())
        else {
            return;
        };
        if let Err(e) = maybe_delete(&room, target).await {
            warn!("view-once: {e}");
        }
    });
}

/// Delete our view-once message once every other member has opened it.
async fn maybe_delete(room: &Room, target: OwnedEventId) -> Result<()> {
    let original = room.load_or_fetch_event(&target, None).await?;
    let raw = original.raw();
    let ours = raw
        .get_field::<OwnedUserId>("sender")
        .ok()
        .flatten()
        .is_some_and(|s| s == room.own_user_id());
    let view_once = raw
        .get_field::<serde_json::Value>("content")
        .ok()
        .flatten()
        .is_some_and(|c| c.get(crate::names::VIEW_ONCE).and_then(|v| v.as_bool()) == Some(true));
    if !ours || !view_once {
        return Ok(());
    }
    // Who opened it: senders of references to it (only "viewed" events point at a photo).
    let mut options = RelationsOptions {
        include_relations: IncludeRelations::RelationsOfType(RelationType::Reference),
        limit: Some(UInt::from(100u32)),
        ..Default::default()
    };
    let mut viewers = HashSet::new();
    loop {
        let page = room.relations(target.clone(), options.clone()).await?;
        for event in &page.chunk {
            if let Ok(Some(sender)) = event.raw().get_field::<OwnedUserId>("sender")
                && sender != room.own_user_id()
            {
                viewers.insert(sender);
            }
        }
        match page.next_batch_token {
            Some(token) => options.from = Some(token),
            None => break,
        }
    }
    let others = room
        .members(matrix_sdk::RoomMemberships::JOIN)
        .await?
        .into_iter()
        .filter(|m| m.user_id() != room.own_user_id())
        .count();
    if viewers.len() >= others.max(1) {
        room.redact(&target, Some(REASON), None).await?;
        info!(room = %room.room_id(), "view-once message opened by everyone: deleted");
    }
    Ok(())
}

impl crate::Core {
    /// View-once messages we already opened (on any of our devices).
    pub async fn viewed_once(&self) -> Result<Vec<String>> {
        let account = self.client()?.account();
        let kind = GlobalAccountDataEventType::from(VIEWED_ONCE);
        let raw = match account.fetch_account_data(kind.clone()).await {
            Ok(raw) => raw,
            Err(_) => account.account_data_raw(kind).await?,
        };
        Ok(raw
            .and_then(|r| serde_json::from_str::<Viewed>(r.json().get()).ok())
            .unwrap_or_default()
            .events)
    }

    /// We opened a view-once message: never offered again here, and its sender is told.
    pub async fn open_view_once(&self, room_id: &str, event_id: &str) -> Result<()> {
        let room = self.room(room_id)?;
        let event = OwnedEventId::try_from(event_id).map_err(|_| Error::UnknownEvent)?;
        let mut viewed = self.viewed_once().await?;
        if !viewed.iter().any(|e| e == event_id) {
            viewed.push(event_id.to_owned());
            let skip = viewed.len().saturating_sub(KEEP);
            viewed.drain(..skip);
            self.client()?
                .account()
                .set_account_data_raw(
                    GlobalAccountDataEventType::from(VIEWED_ONCE),
                    Raw::new(&json!({ "events": viewed }))?.cast_unchecked(),
                )
                .await?;
        }
        room.send_raw(
            VIEW_ONCE_VIEWED,
            json!({ "m.relates_to": { "rel_type": "m.reference", "event_id": event } }),
        )
        .await?;
        Ok(())
    }
}
