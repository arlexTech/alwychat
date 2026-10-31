//! The open chat: a live timeline streamed to the UI as diffs, plus sending, replies and reactions.

use std::{collections::HashMap, sync::Arc};

use eyeball_im::VectorDiff;
use futures_util::StreamExt;
use matrix_sdk::room::edit::EditedContent;
use matrix_sdk::ruma::{
    OwnedEventId, OwnedRoomId, OwnedTransactionId, OwnedUserId,
    api::client::receipt::create_receipt::v3::ReceiptType,
    events::room::message::{MessageType, RoomMessageEventContent},
};
use matrix_sdk_ui::timeline::{
    EventSendState, EventTimelineItem, MembershipChange, MsgLikeKind, RoomExt, Timeline,
    TimelineDetails, TimelineEventFocusThreadMode, TimelineEventItemId, TimelineFocus,
    TimelineItem, TimelineItemContent, TimelineItemKind, VirtualTimelineItem,
};
use serde::Serialize;
use tracing::warn;

use matrix_sdk::ruma::events::room::MediaSource;

use crate::{
    Error, Result, html,
    media::{EventMedia, MediaRegistry},
    mentions::{self, MentionInput},
    rooms::ListDiff,
    upload::GalleryRef,
};

/// Events fetched per back-pagination request.
const PAGE_EVENTS: u16 = 40;
/// Messages loaded around a search result.
const FOCUS_CONTEXT_EVENTS: u16 = 30;

/// One row of the timeline, in the same order as the SDK's items so diff indices stay valid.
#[derive(Debug, Clone, Serialize)]
pub struct TimelineEntry {
    /// Stable across updates of the same item; use it as the UI key.
    pub key: String,
    #[serde(flatten)]
    pub kind: EntryKind,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EntryKind {
    Message(Box<MessageEntry>),
    /// A small centred line: joins, leaves, name changes…
    Notice {
        text: String,
    },
    /// Milliseconds since the Unix epoch, on the day to show.
    DateDivider {
        timestamp: u64,
    },
    ReadMarker,
    TimelineStart,
    /// Something we don't render; kept so indices match.
    Hidden,
}

#[derive(Debug, Clone, Serialize)]
pub struct MessageEntry {
    /// Event ID (`$…`) once known, otherwise the local transaction ID. Pass it back for actions.
    pub item_id: String,
    pub sender: String,
    pub sender_name: String,
    pub sender_avatar: Option<String>,
    pub is_own: bool,
    pub timestamp: u64,
    pub content: MessageContent,
    pub is_edited: bool,
    /// Short messages made only of emoji, shown big and without a bubble.
    pub only_emojis: bool,
    pub can_reply: bool,
    /// Our own text message, still editable.
    pub can_edit: bool,
    /// Our own message, not already deleted.
    pub can_delete: bool,
    pub reply_to: Option<ReplyPreview>,
    pub reactions: Vec<Reaction>,
    pub send_state: Option<SendState>,
    /// People (not us) whose latest read receipt is on this message.
    pub seen_by: Vec<Reader>,
    /// Mentions us (or @room), per our push rules.
    pub mentions_me: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Reader {
    pub name: String,
    pub avatar: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MessageContent {
    /// `html` is sanitised (Matrix allowlist) and safe to insert; fall back to `body` without it.
    /// `link`: the URL to show a preview card for.
    Text {
        body: String,
        html: Option<String>,
        link: Option<String>,
        /// Message effect to play when it arrives (`crate::effects`).
        effect: Option<&'static str>,
    },
    Emote {
        body: String,
        html: Option<String>,
        effect: Option<&'static str>,
    },
    Notice {
        body: String,
        html: Option<String>,
        link: Option<String>,
    },
    /// `media` is a key for `media://localhost/_event/<key>` (add `?thumb=1` for previews).
    Image {
        media: String,
        width: Option<u32>,
        height: Option<u32>,
        blurhash: Option<String>,
        caption: Option<String>,
        /// Album this photo belongs to (`com.alwy.chat.gallery`), if sent as one.
        gallery: Option<GalleryRef>,
    },
    Video {
        media: String,
        width: Option<u32>,
        height: Option<u32>,
        blurhash: Option<String>,
        duration_ms: Option<u64>,
        has_thumbnail: bool,
        caption: Option<String>,
    },
    File {
        media: String,
        filename: String,
        size: Option<u64>,
        caption: Option<String>,
    },
    Sticker {
        media: String,
        body: String,
        width: Option<u32>,
        height: Option<u32>,
    },
    /// Voice messages (Element/FluffyChat) and other audio files.
    Audio {
        media: String,
        duration_ms: Option<u64>,
        /// Loudness bars, 0–1024 (MSC1767); empty if the sender didn't include one.
        waveform: Vec<u16>,
        is_voice: bool,
        filename: String,
        size: Option<u64>,
    },
    /// Locations: a label until they get their own rendering.
    Media {
        label: String,
    },
    /// MSC3381 poll (Element-compatible). Counts are hidden until the end for
    /// undisclosed polls, except our own choice.
    Poll {
        question: String,
        answers: Vec<PollAnswer>,
        /// Number of people who voted.
        voters: u32,
        max_selections: u32,
        /// Results visible before the end.
        disclosed: bool,
        ended: bool,
        /// We started it (only the creator can end it).
        is_mine: bool,
    },
    Redacted,
    /// Encrypted and we don't have the key (yet).
    UnableToDecrypt,
    Unsupported,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReplyPreview {
    pub event_id: String,
    /// `None` while the replied-to event is being fetched.
    pub sender_name: Option<String>,
    pub text: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Reaction {
    pub key: String,
    pub count: usize,
    /// Whether we're one of the senders (clicking it removes ours).
    pub mine: bool,
}

/// One version of an edited message.
#[derive(Debug, Clone, Serialize)]
pub struct Revision {
    pub text: String,
    pub timestamp: Option<u64>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SendState {
    Sending,
    Failed,
}

/// The chat currently on screen. One at a time; opening another replaces it.
pub(crate) struct OpenChat {
    room_id: OwnedRoomId,
    timeline: Arc<Timeline>,
    task: tokio::task::AbortHandle,
    typing_task: tokio::task::AbortHandle,
    /// Keeps the theme/quick reaction/nicknames watch alive while the chat is open.
    _style: matrix_sdk::event_handler::EventHandlerDropGuard,
    _pins: matrix_sdk::event_handler::EventHandlerDropGuard,
}

impl Drop for OpenChat {
    fn drop(&mut self) {
        self.task.abort();
        self.typing_task.abort();
    }
}

impl crate::Core {
    /// Open `room_id` and stream its timeline to `sink` until closed or replaced.
    pub async fn open_chat(
        &self,
        room_id: &str,
        sink: impl FnMut(Vec<ListDiff<TimelineEntry>>) + Send + 'static,
        on_typing: impl Fn(Vec<String>) + Send + 'static,
        on_style: impl Fn(crate::custom::ChatStyle) + Send + Sync + 'static,
        on_pins: impl Fn(crate::pins::Pins) + Send + Sync + 'static,
        focus: Option<&str>,
    ) -> Result<()> {
        let client = self.client()?;
        let room_id = OwnedRoomId::try_from(room_id).map_err(|_| Error::UnknownRoom)?;
        let room = client.get_room(&room_id).ok_or(Error::UnknownRoom)?;
        // Focused on one message (a search result): its surroundings, paginated both ways,
        // without live updates until the UI goes back to the latest messages.
        let builder = match focus {
            Some(event_id) => room.timeline_builder().with_focus(TimelineFocus::Event {
                target: OwnedEventId::try_from(event_id).map_err(|_| Error::UnknownEvent)?,
                num_context_events: FOCUS_CONTEXT_EVENTS,
                thread_mode: TimelineEventFocusThreadMode::Automatic {
                    hide_threaded_events: false,
                },
            }),
            None => room.timeline_builder(),
        };
        let timeline = Arc::new(builder.build().await.map_err(timeline_err)?);

        let task = tokio::spawn(stream_timeline(timeline.clone(), self.media.clone(), sink))
            .abort_handle();
        let typing_task = tokio::spawn(stream_typing(room.clone(), on_typing)).abort_handle();
        let style = self.watch_chat_style(&room, on_style).await;
        let pins = self.watch_pins(&room, on_pins).await;
        self.with_active(|a| {
            a.chat = Some(OpenChat {
                room_id,
                timeline,
                task,
                typing_task,
                _style: style,
                _pins: pins,
            })
        })
    }

    pub fn close_chat(&self) -> Result<()> {
        self.with_active(|a| a.chat = None)
    }

    /// Load older messages. Returns `true` once the start of the room is reached.
    pub async fn load_older(&self, room_id: &str) -> Result<bool> {
        let timeline = self.timeline(room_id)?;
        timeline
            .paginate_backwards(PAGE_EVENTS)
            .await
            .map_err(timeline_err)
    }

    /// Load newer messages (a timeline focused on a search result). Returns `true` once
    /// the newest message is reached.
    pub async fn load_newer(&self, room_id: &str) -> Result<bool> {
        let timeline = self.timeline(room_id)?;
        timeline
            .paginate_forwards(PAGE_EVENTS)
            .await
            .map_err(timeline_err)
    }

    /// Send a Markdown message with mention pills, optionally as a reply to `reply_to` (an event ID).
    pub async fn send_message(
        &self,
        room_id: &str,
        body: &str,
        mentions: &[MentionInput],
        reply_to: Option<&str>,
    ) -> Result<()> {
        let timeline = self.timeline(room_id)?;
        let content = mentions::compose(body, mentions);
        match reply_to {
            Some(id) => {
                let event_id = OwnedEventId::try_from(id).map_err(|_| Error::UnknownEvent)?;
                timeline.send_reply(content, event_id).await
            }
            None => {
                timeline
                    .send(RoomMessageEventContent::from(content).into())
                    .await
            }
        }
        .map_err(timeline_err)?;
        Ok(())
    }

    /// Add our `key` reaction to an item, or remove it if already there.
    pub async fn toggle_reaction(&self, room_id: &str, item_id: &str, key: &str) -> Result<()> {
        let timeline = self.timeline(room_id)?;
        timeline
            .toggle_reaction(&parse_item_id(item_id)?, key)
            .await
            .map_err(timeline_err)?;
        Ok(())
    }

    /// Tell the room we're typing (or stopped). No-op when typing notices are turned off.
    pub async fn set_typing(&self, room_id: &str, typing: bool) -> Result<()> {
        if !self.settings().send_typing {
            return Ok(());
        }
        let timeline = self.timeline(room_id)?;
        timeline.room().typing_notice(typing).await?;
        Ok(())
    }

    /// Replace the text of one of our messages (Markdown, with mention pills).
    pub async fn edit_message(
        &self,
        room_id: &str,
        item_id: &str,
        body: &str,
        mentions: &[MentionInput],
    ) -> Result<()> {
        let timeline = self.timeline(room_id)?;
        let content = mentions::compose(body, mentions);
        timeline
            .edit(
                &parse_item_id(item_id)?,
                EditedContent::RoomMessage(content),
            )
            .await
            .map_err(timeline_err)
    }

    /// Send copies of messages from the open chat to `target_room_id` (same content, media
    /// included without re-uploading; replies lose their quote). Returns how many were sent.
    pub async fn forward_messages(
        &self,
        room_id: &str,
        event_ids: &[String],
        target_room_id: &str,
    ) -> Result<usize> {
        let timeline = self.timeline(room_id)?;
        let target = self.room(target_room_id)?;
        let mut sent = 0;
        for id in event_ids {
            let event_id = OwnedEventId::try_from(id.as_str()).map_err(|_| Error::UnknownEvent)?;
            let Some(item) = timeline.item_by_event_id(&event_id).await else {
                continue;
            };
            let Some(message) = item.content().as_message() else {
                continue;
            };
            target
                .send(RoomMessageEventContent::new(message.msgtype().clone()))
                .await?;
            sent += 1;
        }
        Ok(sent)
    }

    /// Delete (redact) a message for everyone.
    pub async fn delete_message(&self, room_id: &str, item_id: &str) -> Result<()> {
        let timeline = self.timeline(room_id)?;
        timeline
            .redact(&parse_item_id(item_id)?, None)
            .await
            .map_err(timeline_err)
    }

    /// Every version of an edited message, oldest first.
    pub async fn edit_history(&self, room_id: &str, event_id: &str) -> Result<Vec<Revision>> {
        let timeline = self.timeline(room_id)?;
        let event_id = OwnedEventId::try_from(event_id).map_err(|_| Error::UnknownEvent)?;
        let revisions = timeline
            .edit_revisions(&event_id)
            .await
            .map_err(timeline_err)?;
        Ok(revisions
            .into_iter()
            .map(|r| Revision {
                text: plain_text(&r.content),
                timestamp: r.timestamp.map(|t| t.0.into()),
            })
            .collect())
    }

    /// Send a read receipt for the latest event of the open chat (no-op if already read).
    pub async fn mark_read(&self, room_id: &str) -> Result<()> {
        let timeline = self.timeline(room_id)?;
        // With read receipts hidden, the server still learns (privately) what we've read,
        // so unread counts stay in sync across our devices.
        let receipt = if self.settings().send_read_receipts {
            ReceiptType::Read
        } else {
            ReceiptType::ReadPrivate
        };
        timeline.mark_as_read(receipt).await.map_err(timeline_err)?;
        Ok(())
    }

    pub(crate) fn timeline(&self, room_id: &str) -> Result<Arc<Timeline>> {
        self.with_active(|a| {
            a.chat
                .as_ref()
                .filter(|c| c.room_id.as_str() == room_id)
                .map(|c| c.timeline.clone())
        })?
        .ok_or(Error::ChatNotOpen)
    }
}

async fn stream_timeline(
    timeline: Arc<Timeline>,
    media: Arc<MediaRegistry>,
    mut sink: impl FnMut(Vec<ListDiff<TimelineEntry>>) + Send,
) {
    let (initial, stream) = timeline.subscribe().await;
    let mut mapper = Mapper {
        timeline: timeline.clone(),
        media,
        readers: HashMap::new(),
    };
    mapper.load_readers(initial.iter()).await;
    sink(vec![ListDiff::Reset {
        values: initial.iter().map(|i| mapper.entry(i)).collect(),
    }]);

    let mut stream = std::pin::pin!(stream);
    while let Some(diffs) = stream.next().await {
        mapper
            .load_readers(diffs.iter().flat_map(|d| diff_values(d)))
            .await;
        sink(diffs.into_iter().map(|d| mapper.diff(d)).collect());
    }
}

/// Names of people typing in `room` (never us), until the task is aborted.
/// Notice for our own `com.alwy.chat.room.*` state events (theme, quick reaction, nicknames).
fn custom_state_verb(event_type: &str) -> Option<&'static str> {
    match event_type {
        crate::custom::THEME => Some("changed the chat theme"),
        crate::custom::QUICK_REACTION => Some("changed the quick reaction"),
        crate::custom::NICKNAMES => Some("changed a nickname"),
        _ => None,
    }
}

async fn stream_typing(room: matrix_sdk::Room, on_typing: impl Fn(Vec<String>)) {
    let (_guard, mut typing) = room.subscribe_to_typing_notifications();
    while let Ok(users) = typing.recv().await {
        let mut names = Vec::with_capacity(users.len());
        for user in users {
            names.push(match room.get_member_no_sync(&user).await {
                Ok(Some(m)) => m.display_name().unwrap_or(user.localpart()).to_owned(),
                _ => user.localpart().to_owned(),
            });
        }
        on_typing(names);
    }
}

struct Mapper {
    timeline: Arc<Timeline>,
    media: Arc<MediaRegistry>,
    /// Display names and avatars of people with read receipts, loaded before each batch.
    readers: HashMap<OwnedUserId, Reader>,
}

impl Mapper {
    fn diff(&mut self, diff: VectorDiff<Arc<TimelineItem>>) -> ListDiff<TimelineEntry> {
        match diff {
            VectorDiff::Append { values } => ListDiff::Append {
                values: values.iter().map(|i| self.entry(i)).collect(),
            },
            VectorDiff::Clear => ListDiff::Clear,
            VectorDiff::PushFront { value } => ListDiff::PushFront {
                value: self.entry(&value),
            },
            VectorDiff::PushBack { value } => ListDiff::PushBack {
                value: self.entry(&value),
            },
            VectorDiff::PopFront => ListDiff::PopFront,
            VectorDiff::PopBack => ListDiff::PopBack,
            VectorDiff::Insert { index, value } => ListDiff::Insert {
                index,
                value: self.entry(&value),
            },
            VectorDiff::Set { index, value } => ListDiff::Set {
                index,
                value: self.entry(&value),
            },
            VectorDiff::Remove { index } => ListDiff::Remove { index },
            VectorDiff::Truncate { length } => ListDiff::Truncate { length },
            VectorDiff::Reset { values } => ListDiff::Reset {
                values: values.iter().map(|i| self.entry(i)).collect(),
            },
        }
    }

    fn entry(&mut self, item: &TimelineItem) -> TimelineEntry {
        let kind = match item.kind() {
            TimelineItemKind::Virtual(VirtualTimelineItem::DateDivider(ts)) => {
                EntryKind::DateDivider {
                    timestamp: ts.0.into(),
                }
            }
            TimelineItemKind::Virtual(VirtualTimelineItem::ReadMarker) => EntryKind::ReadMarker,
            TimelineItemKind::Virtual(VirtualTimelineItem::TimelineStart) => {
                EntryKind::TimelineStart
            }
            TimelineItemKind::Event(event) => self.event(event),
        };
        TimelineEntry {
            key: item.unique_id().0.clone(),
            kind,
        }
    }

    fn event(&mut self, event: &EventTimelineItem) -> EntryKind {
        let name = sender_name(event);
        match event.content() {
            TimelineItemContent::MsgLike(msg) => {
                let content = match &msg.kind {
                    MsgLikeKind::Message(m) => self.rich_content(event, m.msgtype()),
                    MsgLikeKind::Sticker(s) => {
                        let c = s.content();
                        let key = item_id_string(&event.identifier());
                        self.media.insert(
                            key.clone(),
                            EventMedia {
                                source: MediaSource::from(c.source.clone()),
                                thumbnail: None,
                                mimetype: c.info.mimetype.clone(),
                                filename: c.body.clone(),
                            },
                        );
                        MessageContent::Sticker {
                            media: key,
                            body: c.body.clone(),
                            width: c.info.width.map(uint),
                            height: c.info.height.map(uint),
                        }
                    }
                    MsgLikeKind::Poll(p) => {
                        let own = self.timeline.room().own_user_id().to_string();
                        poll_content(p.results(), &own, event.sender().as_str() == own)
                    }
                    MsgLikeKind::Redacted => MessageContent::Redacted,
                    MsgLikeKind::UnableToDecrypt(_) => MessageContent::UnableToDecrypt,
                    MsgLikeKind::Other(_) | MsgLikeKind::LiveLocation(_) => {
                        MessageContent::Unsupported
                    }
                };
                let own_id = self.timeline.room().own_user_id().to_owned();
                let reactions = msg
                    .reactions
                    .iter()
                    .map(|(key, senders)| Reaction {
                        key: key.clone(),
                        count: senders.len(),
                        mine: senders.contains_key(&own_id),
                    })
                    .collect();
                let reply_to = msg.in_reply_to.as_ref().map(|r| {
                    let (sender_name, text) = match &r.event {
                        TimelineDetails::Ready(e) => (
                            Some(profile_name(&e.sender_profile, e.sender.localpart())),
                            Some(summary(&e.content)),
                        ),
                        TimelineDetails::Unavailable => {
                            if let Some(id) = event.event_id() {
                                self.fetch_reply(id.to_owned());
                            }
                            (None, None)
                        }
                        TimelineDetails::Pending | TimelineDetails::Error(_) => (None, None),
                    };
                    ReplyPreview {
                        event_id: r.event_id.to_string(),
                        sender_name,
                        text,
                    }
                });
                let send_state = match event.send_state() {
                    Some(EventSendState::NotSentYet { .. }) => Some(SendState::Sending),
                    Some(EventSendState::SendingFailed { error, .. }) => {
                        warn!("message not sent: {error}");
                        Some(SendState::Failed)
                    }
                    _ => None,
                };
                EntryKind::Message(Box::new(MessageEntry {
                    item_id: item_id_string(&event.identifier()),
                    sender: event.sender().to_string(),
                    sender_name: name,
                    sender_avatar: match event.sender_profile() {
                        TimelineDetails::Ready(p) => p.avatar_url.as_ref().map(|u| u.to_string()),
                        _ => None,
                    },
                    is_own: event.is_own(),
                    timestamp: event.timestamp().0.into(),
                    is_edited: matches!(&msg.kind, MsgLikeKind::Message(m) if m.is_edited()),
                    only_emojis: event.contains_only_emojis(),
                    can_reply: event.can_be_replied_to(),
                    can_edit: event.is_editable(),
                    can_delete: event.is_own() && !matches!(msg.kind, MsgLikeKind::Redacted),
                    content,
                    reply_to,
                    reactions,
                    send_state,
                    seen_by: event
                        .read_receipts()
                        .keys()
                        .filter(|u| **u != own_id)
                        .map(|u| self.reader(u))
                        .collect(),
                    mentions_me: event.is_highlighted(),
                }))
            }
            TimelineItemContent::MembershipChange(change) => match membership_text(change.change())
            {
                Some(verb) => EntryKind::Notice {
                    text: format!(
                        "{} {verb}",
                        change
                            .display_name()
                            .unwrap_or_else(|| change.user_id().localpart().to_owned())
                    ),
                },
                None => EntryKind::Hidden,
            },
            TimelineItemContent::ProfileChange(change) => match change.displayname_change() {
                Some(c) => EntryKind::Notice {
                    text: match (&c.old, &c.new) {
                        (Some(old), Some(new)) => format!("{old} is now {new}"),
                        (None, Some(new)) => format!("{} is now {new}", change.user_id()),
                        _ => format!("{name} removed their display name"),
                    },
                },
                // Avatar-only changes are noise in a chat.
                None => EntryKind::Hidden,
            },
            TimelineItemContent::OtherState(state) => {
                match custom_state_verb(&state.content().event_type().to_string()) {
                    Some(verb) => EntryKind::Notice {
                        text: format!("{name} {verb}"),
                    },
                    None => EntryKind::Hidden,
                }
            }
            TimelineItemContent::CallInvite | TimelineItemContent::RtcNotification { .. } => {
                EntryKind::Notice {
                    text: format!("📞 {name} started a call"),
                }
            }
            _ => EntryKind::Hidden,
        }
    }

    /// Fetch profiles of read-receipt holders we haven't seen yet.
    async fn load_readers<'a>(&mut self, items: impl Iterator<Item = &'a Arc<TimelineItem>>) {
        let room = self.timeline.room().clone();
        let missing: Vec<OwnedUserId> = items
            .filter_map(|i| i.as_event())
            .flat_map(|e| e.read_receipts().keys())
            .filter(|u| !self.readers.contains_key(*u))
            .cloned()
            .collect();
        for user in missing {
            let reader = match room.get_member_no_sync(&user).await {
                Ok(Some(m)) => Reader {
                    name: m.display_name().unwrap_or(user.localpart()).to_owned(),
                    avatar: m.avatar_url().map(|u| u.to_string()),
                },
                _ => Reader {
                    name: user.localpart().to_owned(),
                    avatar: None,
                },
            };
            self.readers.insert(user, reader);
        }
    }

    fn reader(&self, user: &OwnedUserId) -> Reader {
        self.readers.get(user).cloned().unwrap_or_else(|| Reader {
            name: user.localpart().to_owned(),
            avatar: None,
        })
    }

    /// Like `message_content`, but images, videos and files become displayable media.
    fn rich_content(&self, event: &EventTimelineItem, msgtype: &MessageType) -> MessageContent {
        let key = item_id_string(&event.identifier());
        let register =
            |source: &MediaSource, thumbnail: Option<&MediaSource>, mimetype, filename: &str| {
                self.media.insert(
                    key.clone(),
                    EventMedia {
                        source: source.clone(),
                        thumbnail: thumbnail.cloned(),
                        mimetype,
                        filename: filename.to_owned(),
                    },
                );
            };
        match msgtype {
            MessageType::Image(c) => {
                let info = c.info.as_deref();
                register(
                    &c.source,
                    info.and_then(|i| i.thumbnail_source.as_ref()),
                    info.and_then(|i| i.mimetype.clone()),
                    c.filename(),
                );
                MessageContent::Image {
                    media: key.clone(),
                    width: info.and_then(|i| i.width).map(uint),
                    height: info.and_then(|i| i.height).map(uint),
                    blurhash: info.and_then(|i| i.blurhash.clone()),
                    caption: c.caption().map(ToOwned::to_owned),
                    gallery: gallery_of(event),
                }
            }
            MessageType::Video(c) => {
                let info = c.info.as_deref();
                let thumbnail = info.and_then(|i| i.thumbnail_source.as_ref());
                register(
                    &c.source,
                    thumbnail,
                    info.and_then(|i| i.mimetype.clone()),
                    c.filename(),
                );
                MessageContent::Video {
                    media: key.clone(),
                    width: info.and_then(|i| i.width).map(uint),
                    height: info.and_then(|i| i.height).map(uint),
                    blurhash: info.and_then(|i| i.blurhash.clone()),
                    duration_ms: info.and_then(|i| i.duration).map(|d| d.as_millis() as u64),
                    has_thumbnail: thumbnail.is_some(),
                    caption: c.caption().map(ToOwned::to_owned),
                }
            }
            MessageType::Audio(c) => {
                let info = c.info.as_deref();
                register(
                    &c.source,
                    None,
                    info.and_then(|i| i.mimetype.clone()),
                    c.filename(),
                );
                MessageContent::Audio {
                    media: key.clone(),
                    duration_ms: c
                        .audio
                        .as_ref()
                        .map(|a| a.duration)
                        .or(info.and_then(|i| i.duration))
                        .map(|d| d.as_millis() as u64),
                    waveform: c
                        .audio
                        .as_ref()
                        .map(|a| {
                            a.waveform
                                .iter()
                                .map(|v| u16::try_from(u64::from(v.get())).unwrap_or(1024))
                                .collect()
                        })
                        .unwrap_or_default(),
                    is_voice: c.voice.is_some(),
                    filename: c.filename().to_owned(),
                    size: info.and_then(|i| i.size).map(u64::from),
                }
            }
            MessageType::File(c) => {
                let info = c.info.as_deref();
                register(
                    &c.source,
                    None,
                    info.and_then(|i| i.mimetype.clone()),
                    c.filename(),
                );
                MessageContent::File {
                    media: key.clone(),
                    filename: c.filename().to_owned(),
                    size: info.and_then(|i| i.size).map(u64::from),
                    caption: c.caption().map(ToOwned::to_owned),
                }
            }
            other => message_content(other),
        }
    }

    /// Load the message quoted by `event_id` (the reply itself, not the quoted one).
    fn fetch_reply(&self, event_id: OwnedEventId) {
        let timeline = self.timeline.clone();
        tokio::spawn(async move {
            if let Err(e) = timeline.fetch_details_for_event(&event_id).await {
                warn!(%event_id, "couldn't load replied-to event: {e}");
            }
        });
    }
}

fn message_content(msgtype: &MessageType) -> MessageContent {
    let effect = crate::effects::effect_of(msgtype.msgtype(), msgtype.body());
    match msgtype {
        MessageType::Text(t) => MessageContent::Text {
            body: t.body.clone(),
            html: html::formatted(t.formatted.as_ref()).or_else(|| html::linkify(&t.body)),
            link: html::first_link(&t.body),
            effect,
        },
        MessageType::Emote(e) => MessageContent::Emote {
            body: e.body.clone(),
            html: html::formatted(e.formatted.as_ref()).or_else(|| html::linkify(&e.body)),
            effect,
        },
        MessageType::Notice(n) => MessageContent::Notice {
            body: n.body.clone(),
            html: html::formatted(n.formatted.as_ref()).or_else(|| html::linkify(&n.body)),
            link: html::first_link(&n.body),
        },
        MessageType::Image(_) => media("📷 Photo"),
        MessageType::Video(_) => media("🎬 Video"),
        MessageType::Audio(a) if a.voice.is_some() => media("🎤 Voice message"),
        MessageType::Audio(_) => media("🎵 Audio"),
        MessageType::File(f) => media(&format!("📎 {}", f.filename())),
        MessageType::Location(_) => media("📍 Location"),
        other => MessageContent::Text {
            body: other.body().to_owned(),
            html: None,
            link: None,
            effect,
        },
    }
}

/// `com.alwy.chat.gallery` from the event's content (absent on local echoes until sent).
fn gallery_of(event: &EventTimelineItem) -> Option<GalleryRef> {
    let content: serde_json::Value = event.original_json()?.get_field("content").ok()??;
    serde_json::from_value(content.get("com.alwy.chat.gallery")?.clone()).ok()
}

/// The items a diff carries, if any.
fn diff_values<T>(diff: &VectorDiff<T>) -> Vec<&T> {
    match diff {
        VectorDiff::Append { values } | VectorDiff::Reset { values } => values.iter().collect(),
        VectorDiff::PushFront { value }
        | VectorDiff::PushBack { value }
        | VectorDiff::Insert { value, .. }
        | VectorDiff::Set { value, .. } => vec![value],
        _ => Vec::new(),
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PollAnswer {
    pub id: String,
    pub text: String,
    pub votes: u32,
    pub mine: bool,
}

fn poll_content(
    results: matrix_sdk_ui::timeline::PollResult,
    own: &str,
    is_mine: bool,
) -> MessageContent {
    let voters: std::collections::HashSet<&String> = results.votes.values().flatten().collect();
    let answers = results
        .answers
        .iter()
        .map(|a| {
            let votes = results.votes.get(&a.id);
            PollAnswer {
                id: a.id.clone(),
                text: a.text.clone(),
                votes: votes.map_or(0, |v| v.len() as u32),
                mine: votes.is_some_and(|v| v.iter().any(|u| u == own)),
            }
        })
        .collect();
    MessageContent::Poll {
        question: results.question,
        answers,
        voters: voters.len() as u32,
        max_selections: results.max_selections as u32,
        disclosed: results.kind == matrix_sdk::ruma::events::poll::start::PollKind::Disclosed,
        ended: results.end_time.is_some(),
        is_mine,
    }
}

fn uint(v: matrix_sdk::ruma::UInt) -> u32 {
    u32::try_from(u64::from(v)).unwrap_or(u32::MAX)
}

fn media(label: &str) -> MessageContent {
    MessageContent::Media {
        label: label.to_owned(),
    }
}

/// One-line text for reply previews.
fn summary(content: &TimelineItemContent) -> String {
    plain_text(content)
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("")
        .to_owned()
}

/// Full plain text of a message-like item (labels for media and special kinds).
fn plain_text(content: &TimelineItemContent) -> String {
    let Some(msg) = content.as_msglike() else {
        return String::new();
    };
    match &msg.kind {
        MsgLikeKind::Message(m) => match message_content(m.msgtype()) {
            MessageContent::Text { body, .. }
            | MessageContent::Emote { body, .. }
            | MessageContent::Notice { body, .. } => body,
            MessageContent::Media { label } => label,
            _ => String::new(),
        },
        MsgLikeKind::Sticker(_) => "Sticker".to_owned(),
        MsgLikeKind::Poll(_) => "📊 Poll".to_owned(),
        MsgLikeKind::Redacted => "Message deleted".to_owned(),
        MsgLikeKind::UnableToDecrypt(_) => "🔒 Encrypted message".to_owned(),
        _ => String::new(),
    }
}

fn membership_text(change: Option<MembershipChange>) -> Option<&'static str> {
    Some(match change? {
        MembershipChange::Joined | MembershipChange::InvitationAccepted => "joined",
        MembershipChange::Left => "left",
        MembershipChange::Invited => "was invited",
        MembershipChange::Kicked | MembershipChange::KickedAndBanned => "was removed",
        MembershipChange::Banned => "was banned",
        _ => return None,
    })
}

fn sender_name(event: &EventTimelineItem) -> String {
    profile_name(event.sender_profile(), event.sender().localpart())
}

fn profile_name(
    profile: &TimelineDetails<matrix_sdk_ui::timeline::Profile>,
    fallback: &str,
) -> String {
    match profile {
        TimelineDetails::Ready(p) => p.display_name.clone(),
        _ => None,
    }
    .unwrap_or_else(|| fallback.to_owned())
}

/// Event IDs always start with `$`; anything else is one of our transaction IDs.
fn item_id_string(id: &TimelineEventItemId) -> String {
    match id {
        TimelineEventItemId::EventId(e) => e.to_string(),
        TimelineEventItemId::TransactionId(t) => t.to_string(),
    }
}

fn parse_item_id(id: &str) -> Result<TimelineEventItemId> {
    if id.starts_with('$') {
        OwnedEventId::try_from(id)
            .map(TimelineEventItemId::EventId)
            .map_err(|_| Error::UnknownEvent)
    } else {
        Ok(TimelineEventItemId::TransactionId(
            OwnedTransactionId::from(id),
        ))
    }
}

fn timeline_err(e: impl std::fmt::Display) -> Error {
    Error::Timeline(e.to_string())
}
