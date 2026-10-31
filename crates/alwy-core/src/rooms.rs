//! The chat list: a live, sorted list of rooms streamed to the UI as small diffs.

use std::collections::HashSet;

use eyeball_im::VectorDiff;
use futures_util::StreamExt;
use matrix_sdk::{
    Client, Room, RoomState,
    latest_events::LatestEventValue,
    notification_settings::RoomNotificationMode,
    ruma::{
        OwnedUserId,
        events::{AnyMessageLikeEventContent, AnySyncTimelineEvent, room::message::MessageType},
    },
};
use matrix_sdk_ui::room_list_service::{
    RoomListItem, RoomListService,
    filters::{
        new_filter_all, new_filter_deduplicate_versions, new_filter_non_left, new_filter_not,
        new_filter_space,
    },
};
use serde::Serialize;
use tracing::warn;

use crate::Result;

/// Rooms loaded at once. Pagination comes later; this covers most accounts.
const PAGE_SIZE: usize = 500;
const PREVIEW_MAX_CHARS: usize = 140;

/// One row of the chat list.
#[derive(Debug, Clone, Serialize)]
pub struct RoomSummary {
    pub id: String,
    pub name: String,
    /// `mxc://` URI of the room avatar, or of the other person in a DM.
    pub avatar: Option<String>,
    pub is_dm: bool,
    /// The other person of a 1:1 chat (for the nickname shown as its title).
    pub dm_user: Option<String>,
    pub is_invite: bool,
    pub is_favourite: bool,
    /// Tagged low priority: shown under "Archived" (Element: "Low priority").
    pub is_archived: bool,
    pub is_muted: bool,
    pub is_encrypted: bool,
    pub unread_messages: u64,
    pub unread_mentions: u64,
    pub marked_unread: bool,
    pub latest: Option<LatestMessage>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LatestMessage {
    /// `None` for our own messages and in DMs, where the sender is obvious.
    pub sender_name: Option<String>,
    pub is_own: bool,
    pub text: String,
    /// Milliseconds since the Unix epoch.
    pub timestamp: Option<u64>,
    pub is_sending: bool,
}

/// Same shape as `eyeball_im::VectorDiff`, but serializable for the UI.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum ListDiff<T> {
    Append { values: Vec<T> },
    Clear,
    PushFront { value: T },
    PushBack { value: T },
    PopFront,
    PopBack,
    Insert { index: usize, value: T },
    Set { index: usize, value: T },
    Remove { index: usize },
    Truncate { length: usize },
    Reset { values: Vec<T> },
}

/// Stream the chat list (rooms and DMs, without spaces) until the returned future is dropped.
pub(crate) async fn stream_chat_list(
    service: &RoomListService,
    mut sink: impl FnMut(Vec<ListDiff<RoomSummary>>) + Send,
) -> Result<()> {
    let client = service.client().clone();
    let all_rooms = service
        .all_rooms()
        .await
        .map_err(|e| crate::Error::Sync(e.to_string()))?;
    let (stream, controller) = all_rooms.entries_with_dynamic_adapters(PAGE_SIZE);
    controller.set_filter(Box::new(new_filter_all(vec![
        Box::new(new_filter_non_left()),
        Box::new(new_filter_not(Box::new(new_filter_space()))),
        Box::new(new_filter_deduplicate_versions()),
    ])));

    let mut mapper = Mapper {
        client,
        listened: HashSet::new(),
    };
    let mut stream = std::pin::pin!(stream);
    while let Some(diffs) = stream.next().await {
        let mut out = Vec::with_capacity(diffs.len());
        for diff in diffs {
            out.push(mapper.diff(diff).await);
        }
        if let Some(ListDiff::Reset { values }) = out.first() {
            tracing::debug!(rooms = values.len(), "chat list reset");
        }
        sink(out);
    }
    Ok(())
}

struct Mapper {
    client: Client,
    /// Rooms whose latest event we asked the SDK to compute (it's lazy).
    listened: HashSet<matrix_sdk::ruma::OwnedRoomId>,
}

impl Mapper {
    async fn diff(&mut self, diff: VectorDiff<RoomListItem>) -> ListDiff<RoomSummary> {
        match diff {
            VectorDiff::Append { values } => ListDiff::Append {
                values: self.all(values).await,
            },
            VectorDiff::Clear => ListDiff::Clear,
            VectorDiff::PushFront { value } => ListDiff::PushFront {
                value: self.one(&value).await,
            },
            VectorDiff::PushBack { value } => ListDiff::PushBack {
                value: self.one(&value).await,
            },
            VectorDiff::PopFront => ListDiff::PopFront,
            VectorDiff::PopBack => ListDiff::PopBack,
            VectorDiff::Insert { index, value } => ListDiff::Insert {
                index,
                value: self.one(&value).await,
            },
            VectorDiff::Set { index, value } => ListDiff::Set {
                index,
                value: self.one(&value).await,
            },
            VectorDiff::Remove { index } => ListDiff::Remove { index },
            VectorDiff::Truncate { length } => ListDiff::Truncate { length },
            VectorDiff::Reset { values } => ListDiff::Reset {
                values: self.all(values).await,
            },
        }
    }

    async fn all(&mut self, rooms: impl IntoIterator<Item = RoomListItem>) -> Vec<RoomSummary> {
        let mut out = Vec::new();
        for room in rooms {
            out.push(self.one(&room).await);
        }
        out
    }

    async fn one(&mut self, room: &Room) -> RoomSummary {
        if self.listened.insert(room.room_id().to_owned())
            && let Err(e) = self
                .client
                .latest_events()
                .await
                .listen_to_room(room.room_id())
                .await
        {
            warn!(room = %room.room_id(), "can't compute latest event: {e}");
        }
        summarize(room).await
    }
}

async fn summarize(room: &Room) -> RoomSummary {
    let is_dm = room.is_direct().await.unwrap_or(false);
    let mut avatar = room.avatar_url();
    if avatar.is_none() && is_dm {
        avatar = room.heroes().await.into_iter().find_map(|h| h.avatar_url);
    }
    let avatar = avatar.map(|uri| uri.to_string());
    let name = room
        .cached_display_name()
        .map(|n| n.to_string())
        .unwrap_or_else(|| room.room_id().to_string());

    RoomSummary {
        id: room.room_id().to_string(),
        name,
        avatar,
        is_dm,
        dm_user: is_dm
            .then(|| {
                room.direct_targets()
                    .iter()
                    .find_map(|t| t.as_user_id().map(ToString::to_string))
            })
            .flatten(),
        is_invite: room.state() == RoomState::Invited,
        is_favourite: room.is_favourite(),
        is_archived: room.is_low_priority(),
        is_muted: room.cached_user_defined_notification_mode() == Some(RoomNotificationMode::Mute),
        is_encrypted: room.encryption_state().is_encrypted(),
        unread_messages: room.num_unread_messages(),
        unread_mentions: room.num_unread_mentions(),
        marked_unread: room.is_marked_unread(),
        latest: latest_message(room, is_dm).await,
    }
}

async fn latest_message(room: &Room, is_dm: bool) -> Option<LatestMessage> {
    let own_id = room.own_user_id().to_owned();
    let value = room.latest_event();
    let timestamp = value.timestamp().map(|t| u64::from(t.0));

    let (sender, text, is_sending): (Option<OwnedUserId>, String, bool) = match &value {
        LatestEventValue::None => return None,
        LatestEventValue::RemoteInvite { inviter, .. } => {
            (inviter.clone(), "Invited you".to_owned(), false)
        }
        LatestEventValue::Remote(event) => {
            let text = match event.raw().deserialize() {
                Ok(AnySyncTimelineEvent::MessageLike(ev)) => match ev.original_content() {
                    Some(content) => preview(&content)?,
                    None => "Message deleted".to_owned(),
                },
                _ => return None,
            };
            (event.sender(), text, false)
        }
        LatestEventValue::LocalIsSending(local)
        | LatestEventValue::LocalCannotBeSent(local)
        | LatestEventValue::LocalHasBeenSent { value: local, .. } => {
            let text = preview(&local.content.deserialize().ok()?)?;
            (Some(own_id.clone()), text, value.is_unsent())
        }
    };

    let is_own = sender.as_deref() == Some(&*own_id);
    let sender_name = match sender {
        Some(user) if !is_own && !is_dm => Some(member_name(room, &user).await),
        _ => None,
    };

    Some(LatestMessage {
        sender_name,
        is_own,
        text: truncate(&text),
        timestamp,
        is_sending,
    })
}

async fn member_name(room: &Room, user: &matrix_sdk::ruma::UserId) -> String {
    match room.get_member_no_sync(user).await {
        Ok(Some(member)) => member.display_name().unwrap_or(user.localpart()).to_owned(),
        _ => user.localpart().to_owned(),
    }
}

/// One-line, human preview of a message, or `None` if it shouldn't appear in the list.
pub(crate) fn preview(content: &AnyMessageLikeEventContent) -> Option<String> {
    Some(match content {
        AnyMessageLikeEventContent::RoomMessage(msg) => match &msg.msgtype {
            MessageType::Image(_) => "📷 Photo".to_owned(),
            MessageType::Video(_) => "🎬 Video".to_owned(),
            MessageType::Audio(a) if a.voice.is_some() => "🎤 Voice message".to_owned(),
            MessageType::Audio(_) => "🎵 Audio".to_owned(),
            MessageType::File(f) => format!("📎 {}", f.filename()),
            MessageType::Location(_) => "📍 Location".to_owned(),
            MessageType::Emote(e) => format!("* {}", e.body),
            other => other.body().to_owned(),
        },
        AnyMessageLikeEventContent::Sticker(_) => "Sticker".to_owned(),
        AnyMessageLikeEventContent::UnstablePollStart(_) => "📊 Poll".to_owned(),
        _ => return None,
    })
}

fn truncate(text: &str) -> String {
    let line = text
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("")
        .trim();
    match line.char_indices().nth(PREVIEW_MAX_CHARS) {
        Some((cut, _)) => format!("{}…", &line[..cut]),
        None => line.to_owned(),
    }
}

impl crate::Core {
    pub(crate) fn room(&self, room_id: &str) -> Result<Room> {
        let room_id = matrix_sdk::ruma::OwnedRoomId::try_from(room_id)
            .map_err(|_| crate::Error::UnknownRoom)?;
        self.client()?
            .get_room(&room_id)
            .ok_or(crate::Error::UnknownRoom)
    }

    /// Pin a chat to the top (the spec's favourite tag, shared with Element).
    pub async fn set_pinned(&self, room_id: &str, pinned: bool) -> Result<()> {
        self.room(room_id)?.set_is_favourite(pinned, None).await?;
        Ok(())
    }

    /// Archive a chat (low-priority tag); a new message doesn't unarchive it.
    pub async fn set_archived(&self, room_id: &str, archived: bool) -> Result<()> {
        self.room(room_id)?
            .set_is_low_priority(archived, None)
            .await?;
        Ok(())
    }

    /// Mute (no notifications or badges) or restore the default notification mode.
    pub async fn set_muted(&self, room_id: &str, muted: bool) -> Result<()> {
        let room = self.room(room_id)?;
        let settings = room.client().notification_settings().await;
        let result = if muted {
            settings
                .set_room_notification_mode(room.room_id(), RoomNotificationMode::Mute)
                .await
        } else {
            settings
                .delete_user_defined_room_rules(room.room_id())
                .await
        };
        result.map_err(|e| crate::Error::Sync(e.to_string()))?;
        // The push-rule change only reaches the room's cached mode on a later sync, and that
        // alone doesn't refresh the chat list: update it now so the row changes right away.
        if muted {
            room.update_cached_user_defined_notification_mode(RoomNotificationMode::Mute);
        } else {
            room.clear_user_defined_notification_mode();
        }
        Ok(())
    }

    /// Flag a chat as unread (or clear the flag) without touching read receipts.
    pub async fn set_marked_unread(&self, room_id: &str, unread: bool) -> Result<()> {
        self.room(room_id)?.set_unread_flag(unread).await?;
        Ok(())
    }

    /// Start streaming the chat list to `sink`. Replaces any previous subscription.
    pub fn subscribe_chat_list(
        &self,
        sink: impl FnMut(Vec<ListDiff<RoomSummary>>) + Send + 'static,
    ) -> Result<()> {
        let service = self.with_active(|a| a.sync.room_list_service())?;
        let task = tokio::spawn(async move {
            if let Err(e) = stream_chat_list(&service, sink).await {
                warn!("chat list stream ended: {e}");
            }
        });
        self.with_active(|a| {
            a.tasks.retain(|t| !t.is_finished());
            // One chat list at a time: the newest subscriber wins.
            a.tasks.drain(..).for_each(|t| t.abort());
            a.tasks.push(task.abort_handle());
        })
    }
}
