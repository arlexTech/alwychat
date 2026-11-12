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
/// Closed chats whose timeline is kept for a quick reopen.
const RECENT_CHATS: usize = 10;
/// A new chat's timeline is filled to this many items (at most this many pages) before it's shown.
const PREFILL_ITEMS: usize = 30;
const PREFILL_PAGES: usize = 3;
/** Fewer items than this right after opening: fill before showing. */
const PREFILL_MIN: usize = 12;
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
        /// A link after the text: "theme" (opens the theme picker).
        #[serde(skip_serializing_if = "Option::is_none")]
        action: Option<&'static str>,
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
    /// Not sent because these people, verified before, reset their encryption since: sending
    /// anyway accepts that (`send_despite_identity_changes`).
    pub identity_changed: Vec<String>,
    /// People (not us) whose latest read receipt is on this message.
    pub seen_by: Vec<Reader>,
    /// Mentions us (or @room), per our push rules.
    pub mentions_me: bool,
    /// Replies in the thread started on this message (Element-compatible threads).
    pub thread: Option<ThreadInfo>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ThreadInfo {
    pub replies: u32,
    /// The latest reply, once loaded.
    pub latest_sender: Option<String>,
    pub latest_avatar: Option<String>,
    pub latest_text: Option<String>,
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
        /// Shown once (view_once.rs).
        view_once: bool,
    },
    Video {
        media: String,
        width: Option<u32>,
        height: Option<u32>,
        blurhash: Option<String>,
        duration_ms: Option<u64>,
        has_thumbnail: bool,
        caption: Option<String>,
        /// Shown once (view_once.rs).
        view_once: bool,
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
    /// Something shown as a label only.
    Media {
        label: String,
    },
    /// A shared location (`m.location`): `geo:lat,lon` and the sender's description.
    Location {
        geo_uri: String,
        description: String,
    },
    /// A live location share (MSC3489 beacons, as Element sends them): the latest point.
    LiveLocation {
        /// None until the first position arrives.
        geo_uri: Option<String>,
        description: Option<String>,
        is_live: bool,
        /// When the latest position was sent, and when sharing ends (ms since epoch).
        updated: Option<u64>,
        ends_at: u64,
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
    /// Who reacted with it (user IDs), first first.
    pub senders: Vec<String>,
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
    /// The live timeline (not one focused on a search result): kept for a quick reopen.
    live: bool,
    task: crate::rt::AbortHandle,
    typing_task: crate::rt::AbortHandle,
    /// Keeps the theme/quick reaction/nicknames watch alive while the chat is open.
    _style: matrix_sdk::event_handler::EventHandlerDropGuard,
    _pins: matrix_sdk::event_handler::EventHandlerDropGuard,
    /// A thread of this chat, open beside it.
    thread: Option<OpenThread>,
}

pub(crate) struct OpenThread {
    root: OwnedEventId,
    timeline: Arc<Timeline>,
    task: crate::rt::AbortHandle,
}

impl Drop for OpenThread {
    fn drop(&mut self) {
        self.task.abort();
    }
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
        // Subscribe the open chat in sliding sync (like Element X): every sync then carries its
        // events and receipts. Without it, the echo of our own read receipt could wait for the
        // next message there, leaving the chat marked unread in the list after reading it. Kept
        // until another chat opens, so the echo also arrives after going back to the list.
        let (list, ticket) = self.with_active(|a| {
            a.opening += 1;
            (a.sync.room_list_service(), a.opening)
        })?;
        let subscribed = room_id.clone();
        crate::rt::spawn(async move { list.set_room_subscriptions(&[&subscribed]).await });
        // Focused on one message (a search result): its surroundings, paginated both ways,
        // without live updates until the UI goes back to the latest messages.
        let builder = match focus {
            Some(event_id) => room.timeline_builder().with_focus(TimelineFocus::Event {
                target: OwnedEventId::try_from(event_id).map_err(|_| Error::UnknownEvent)?,
                num_context_events: FOCUS_CONTEXT_EVENTS,
                thread_mode: TimelineEventFocusThreadMode::Automatic {
                    hide_threaded_events: true,
                },
            }),
            // Thread replies live in their thread; the root shows a summary instead.
            None => room.timeline_builder().with_focus(TimelineFocus::Live {
                hide_threaded_events: true,
            }),
        };
        // A chat opened recently reopens instantly: its timeline was kept, already filled
        // and following new messages. A new one is filled before the first batch goes out,
        // so the chat appears whole instead of a few items followed by a loading spinner.
        let kept = match focus {
            Some(_) => None,
            None => self.with_active(|a| {
                let at = a.recent_chats.iter().position(|(id, _)| *id == room_id)?;
                Some(a.recent_chats.remove(at).1)
            })?,
        };
        let timeline = match kept {
            Some(timeline) => timeline,
            None => {
                let timeline = Arc::new(builder.build().await.map_err(timeline_err)?);
                // Only a nearly empty chat is filled first (there'd be nothing to show): one
                // with a screenful already goes out at once, the UI loads more behind it.
                if focus.is_none() && timeline.items().await.len() < PREFILL_MIN {
                    for _ in 0..PREFILL_PAGES {
                        if timeline.items().await.len() >= PREFILL_ITEMS
                            || timeline
                                .paginate_backwards(PAGE_EVENTS)
                                .await
                                .unwrap_or(true)
                        {
                            break;
                        }
                    }
                }
                timeline
            }
        };

        let task = crate::rt::spawn(stream_timeline(timeline.clone(), self.media.clone(), sink))
            .abort_handle();
        let typing_task = crate::rt::spawn(stream_typing(room.clone(), on_typing)).abort_handle();
        let style = self.watch_chat_style(&room, on_style).await;
        let pins = self.watch_pins(&room, on_pins).await;
        self.with_active(|a| {
            let chat = OpenChat {
                room_id,
                timeline,
                live: focus.is_none(),
                task,
                typing_task,
                _style: style,
                _pins: pins,
                thread: None,
            };
            // Another chat was asked for while this one loaded (the UI already ignores this
            // one): it must not take the place of the newer one. Its timeline is kept for a
            // quick reopen.
            if a.opening != ticket {
                let newer = a.chat.take();
                a.chat = Some(chat);
                keep_recent(a);
                a.chat = newer;
                return;
            }
            keep_recent(a);
            a.chat = Some(chat);
        })
    }

    pub fn close_chat(&self) -> Result<()> {
        self.with_active(keep_recent)
    }

    /// Open the thread started on `root` (it may have no replies yet) in the open chat and
    /// stream it to `sink`. Every command that takes a `room_id` acts on the thread when
    /// given the root's event ID instead (sending there adds the thread relation).
    pub async fn open_thread(
        &self,
        room_id: &str,
        root: &str,
        sink: impl FnMut(Vec<ListDiff<TimelineEntry>>) + Send + 'static,
    ) -> Result<()> {
        let room = self.timeline(room_id)?.room().clone();
        let root = OwnedEventId::try_from(root).map_err(|_| Error::UnknownEvent)?;
        let timeline = Arc::new(
            room.timeline_builder()
                .with_focus(TimelineFocus::Thread {
                    root_event_id: root.clone(),
                })
                .build()
                .await
                .map_err(timeline_err)?,
        );
        let task = crate::rt::spawn(stream_timeline(timeline.clone(), self.media.clone(), sink))
            .abort_handle();
        self.with_active(|a| match a.chat.as_mut() {
            Some(chat) if chat.room_id == room_id => {
                chat.thread = Some(OpenThread {
                    root,
                    timeline,
                    task,
                });
                Ok(())
            }
            _ => {
                task.abort();
                Err(Error::ChatNotOpen)
            }
        })?
    }

    pub fn close_thread(&self) -> Result<()> {
        self.with_active(|a| {
            if let Some(chat) = a.chat.as_mut() {
                chat.thread = None;
            }
        })
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
        emotes: &[crate::emotes::EmoteInput],
    ) -> Result<()> {
        let timeline = self.timeline(room_id)?;
        let content = crate::emotes::with_emotes(mentions::compose(body, mentions), emotes);
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

    /// Start sharing our live location in a chat for `duration_ms`, with a first position.
    /// Returns the share's event ID, which every later position refers to (sliding sync
    /// doesn't keep `beacon_info` state in the store, so the SDK's own helpers can't find it).
    pub async fn start_live_location(
        &self,
        room_id: &str,
        duration_ms: u64,
        geo_uri: &str,
    ) -> Result<String> {
        let room = self.room(room_id)?;
        let share = room.start_live_location_share(duration_ms, None).await?;
        self.update_live_location(room_id, share.event_id.as_str(), geo_uri)
            .await?;
        Ok(share.event_id.to_string())
    }

    /// A new position for our live share `share_id`.
    pub async fn update_live_location(
        &self,
        room_id: &str,
        share_id: &str,
        geo_uri: &str,
    ) -> Result<()> {
        use matrix_sdk::ruma::events::beacon::BeaconEventContent;
        let share = OwnedEventId::try_from(share_id).map_err(|_| Error::UnknownEvent)?;
        self.room(room_id)?
            .send(BeaconEventContent::new(share, geo_uri.to_owned(), None))
            .await?;
        Ok(())
    }

    /// End our live share: the server's copy of it, marked not live.
    pub async fn stop_live_location(&self, room_id: &str) -> Result<()> {
        use matrix_sdk::ruma::events::StateEventType;
        let room = self.room(room_id)?;
        let me = room.own_user_id().to_owned();
        let kind = StateEventType::from("org.matrix.msc3672.beacon_info");
        let Some(mut content) = self.state_content(&room, kind, me.as_str()).await? else {
            return Ok(());
        };
        content["live"] = serde_json::json!(false);
        room.send_state_event_raw("org.matrix.msc3672.beacon_info", me.as_str(), content)
            .await?;
        Ok(())
    }

    /// Share a location (`geo:lat,lon;u=accuracy`), as Element does (`m.location`).
    pub async fn send_location(
        &self,
        room_id: &str,
        geo_uri: &str,
        description: &str,
    ) -> Result<()> {
        use matrix_sdk::ruma::events::room::message::LocationMessageEventContent;
        if !geo_uri.starts_with("geo:") {
            return Err(Error::Timeline("not a geo: URI".into()));
        }
        let body = if description.trim().is_empty() {
            format!("Location: {geo_uri}")
        } else {
            description.trim().to_owned()
        };
        let content = RoomMessageEventContent::new(MessageType::Location(
            LocationMessageEventContent::new(body, geo_uri.to_owned()),
        ));
        self.timeline(room_id)?
            .send(content.into())
            .await
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
        let id = parse_item_id(item_id)?;
        let content = mentions::compose(body, mentions);
        // A photo, video, file or voice message: only its caption changes (empty = removed).
        let items = timeline.items().await;
        let item = items
            .iter()
            .filter_map(|item| item.as_event())
            .find(|e| e.identifier() == id);
        let is_media = item.is_some_and(|item| {
            item.content().as_message().is_some_and(|m| {
                matches!(
                    m.msgtype(),
                    MessageType::Image(_)
                        | MessageType::Video(_)
                        | MessageType::File(_)
                        | MessageType::Audio(_)
                )
            })
        });
        let edited = if is_media {
            let (caption, formatted_caption) = match content.msgtype {
                MessageType::Text(text) if !body.trim().is_empty() => {
                    (Some(text.body), text.formatted)
                }
                _ => (None, None),
            };
            EditedContent::MediaCaption {
                caption,
                formatted_caption,
                mentions: content.mentions,
            }
        } else {
            EditedContent::RoomMessage(content)
        };
        timeline.edit(&id, edited).await.map_err(timeline_err)
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
    /// Send a message that failed again (and resume the chat's queue, paused by the failure).
    /// A message stopped because people we had verified reset their encryption: accept their
    /// new identity (withdraw our verification, as Element does) and send it.
    pub async fn send_despite_identity_changes(
        &self,
        room_id: &str,
        item_id: &str,
        users: &[String],
    ) -> Result<()> {
        let encryption = self.client()?.encryption();
        for user in users {
            let user = matrix_sdk::ruma::OwnedUserId::try_from(user.as_str())
                .map_err(|_| Error::Encryption(format!("not a user ID: {user}")))?;
            if let Some(identity) = encryption
                .get_user_identity(&user)
                .await
                .map_err(|e| Error::Encryption(e.to_string()))?
            {
                identity
                    .withdraw_verification()
                    .await
                    .map_err(|e| Error::Encryption(e.to_string()))?;
            }
        }
        self.retry_send(room_id, item_id).await
    }

    pub async fn retry_send(&self, room_id: &str, item_id: &str) -> Result<()> {
        let timeline = self.timeline(room_id)?;
        let TimelineEventItemId::TransactionId(txn) = parse_item_id(item_id)? else {
            return Ok(()); // Already sent.
        };
        let handle = timeline
            .items()
            .await
            .iter()
            .filter_map(|item| item.as_event())
            .find(|e| e.transaction_id() == Some(&txn))
            .and_then(|e| e.local_echo_send_handle())
            .ok_or(Error::UnknownEvent)?;
        self.room(room_id)?.send_queue().set_enabled(true);
        handle.unwedge().await.map_err(timeline_err)?;
        Ok(())
    }

    pub async fn delete_message(&self, room_id: &str, item_id: &str) -> Result<()> {
        let timeline = self.timeline(room_id)?;
        let id = parse_item_id(item_id)?;
        match timeline.redact(&id, None).await {
            Ok(()) => Ok(()),
            // Offline: deleted when the connection is back.
            Err(_)
                if !crate::offline::online()
                    && let TimelineEventItemId::EventId(event) = &id =>
            {
                crate::offline::later_redact(timeline.room(), event.as_str());
                Ok(())
            }
            Err(e) => Err(timeline_err(e)),
        }
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
        if let Err(e) = timeline.mark_as_read(receipt.clone()).await {
            if crate::offline::online() {
                return Err(timeline_err(e));
            }
            // Offline: read up to the newest message once the connection is back.
            crate::offline::later_read(timeline.room(), receipt == ReceiptType::ReadPrivate);
            return Ok(());
        }
        // The live chat hides thread replies, so the SDK's receipt only covers the main
        // conversation ("main" thread) and replies in threads stay unread on the server for
        // ever (the badge never clears; we show no per-thread unread). Reading the chat also
        // reads its newest event, threads included, with an unthreaded receipt.
        let live_main = self.with_active(|a| {
            a.chat
                .as_ref()
                .is_some_and(|c| c.live && c.room_id.as_str() == room_id)
        })?;
        if live_main && let Err(e) = read_everything(timeline.room(), receipt).await {
            warn!("unthreaded read receipt: {e}");
        }
        Ok(())
    }

    /// The open chat's timeline, or its open thread's when `room_id` is the thread root.
    pub(crate) fn timeline(&self, room_id: &str) -> Result<Arc<Timeline>> {
        self.with_active(|a| {
            let chat = a.chat.as_ref()?;
            if chat.room_id.as_str() == room_id {
                return Some(chat.timeline.clone());
            }
            chat.thread
                .as_ref()
                .filter(|t| t.root.as_str() == room_id)
                .map(|t| t.timeline.clone())
        })?
        .ok_or(Error::ChatNotOpen)
    }
}

/// Unthreaded receipt on the room's newest loaded event (thread replies too), if not sent yet.
async fn read_everything(room: &matrix_sdk::Room, receipt: ReceiptType) -> Result<()> {
    let (cache, _handles) = room
        .event_cache()
        .await
        .map_err(|e| Error::Timeline(e.to_string()))?;
    let Some(latest) = cache
        .rfind_map_event_in_memory_by(|e| e.event_id().map(|id| id.to_owned()))
        .await
        .map_err(|e| Error::Timeline(e.to_string()))?
    else {
        return Ok(());
    };
    let own = room.own_user_id();
    let thread = matrix_sdk::ruma::events::receipt::ReceiptThread::Unthreaded;
    let sent = room
        .load_user_receipt(receipt_kind(&receipt), &thread, own)
        .await?;
    if sent.is_some_and(|(id, _)| id == latest) {
        return Ok(());
    }
    room.send_single_receipt(receipt, thread, latest).await?;
    Ok(())
}

fn receipt_kind(receipt: &ReceiptType) -> matrix_sdk::ruma::events::receipt::ReceiptType {
    use matrix_sdk::ruma::events::receipt::ReceiptType as Kind;
    match receipt {
        ReceiptType::ReadPrivate => Kind::ReadPrivate,
        _ => Kind::Read,
    }
}

/// Close the open chat, keeping its live timeline (a few chats) for a quick reopen.
fn keep_recent(active: &mut crate::session::Active) {
    let Some(chat) = active.chat.take().filter(|c| c.live) else {
        return;
    };
    if chat.timeline.room().is_space() {
        return;
    }
    let timeline = chat.timeline.clone();
    let room_id = chat.room_id.clone();
    drop(chat);
    active.recent_chats.retain(|(id, _)| *id != room_id);
    active.recent_chats.push((room_id, timeline));
    if active.recent_chats.len() > RECENT_CHATS {
        active.recent_chats.remove(0);
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
        t if t == crate::custom::THEME => Some("changed the chat theme"),
        t if t == crate::custom::QUICK_REACTION => Some("changed the quick reaction"),
        t if t == crate::custom::NICKNAMES => Some("changed a nickname"),
        t if t == crate::names::DISAPPEARING => Some("changed disappearing messages"),
        t if t == crate::names::COUNTDOWNS => Some("changed the chat's dates"),
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
                    MsgLikeKind::LiveLocation(live) => {
                        let latest = live.latest_location();
                        let start: u64 = live.ts().get().into();
                        MessageContent::LiveLocation {
                            geo_uri: latest.map(|l| l.geo_uri().to_owned()),
                            description: live.description().map(ToOwned::to_owned),
                            is_live: live.is_live(),
                            updated: latest.map(|l| l.ts().get().into()),
                            ends_at: start.saturating_add(live.timeout().as_millis() as u64),
                        }
                    }
                    MsgLikeKind::Other(_) => MessageContent::Unsupported,
                };
                let own_id = self.timeline.room().own_user_id().to_owned();
                let reactions = msg
                    .reactions
                    .iter()
                    .map(|(key, senders)| {
                        let mut by_time: Vec<_> = senders.iter().collect();
                        by_time.sort_by_key(|(_, info)| info.timestamp);
                        Reaction {
                            key: key.clone(),
                            count: senders.len(),
                            mine: senders.contains_key(&own_id),
                            senders: by_time
                                .into_iter()
                                .map(|(user, _)| user.to_string())
                                .collect(),
                        }
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
                let mut identity_changed = Vec::new();
                let send_state = match event.send_state() {
                    Some(EventSendState::NotSentYet { .. }) => Some(SendState::Sending),
                    Some(EventSendState::SendingFailed { error, .. }) => {
                        warn!("message not sent: {error}");
                        if let matrix_sdk::QueueWedgeError::IdentityViolations { users } =
                            matrix_sdk::QueueWedgeError::from(&**error)
                        {
                            identity_changed = users.iter().map(ToString::to_string).collect();
                        }
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
                    identity_changed,
                    seen_by: event
                        .read_receipts()
                        .keys()
                        .filter(|u| **u != own_id)
                        .map(|u| self.reader(u))
                        .collect(),
                    mentions_me: event.is_highlighted() && !reply_only_mention(msg, &own_id),
                    thread: msg.thread_summary.as_ref().map(|t| {
                        let latest = match &t.latest_event {
                            TimelineDetails::Ready(e) => Some(e),
                            _ => None,
                        };
                        ThreadInfo {
                            replies: t.num_replies,
                            latest_sender: latest
                                .map(|e| profile_name(&e.sender_profile, e.sender.localpart())),
                            latest_avatar: latest.and_then(|e| match &e.sender_profile {
                                TimelineDetails::Ready(p) => {
                                    p.avatar_url.as_ref().map(|u| u.to_string())
                                }
                                _ => None,
                            }),
                            latest_text: latest.map(|e| summary(&e.content)),
                        }
                    }),
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
                    action: None,
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
                    action: None,
                },
                // Avatar-only changes are noise in a chat.
                None => EntryKind::Hidden,
            },
            TimelineItemContent::OtherState(state) => {
                let kind = state.content().event_type().to_string();
                if kind == crate::custom::THEME {
                    // "… changed the theme to Ocean" (the preset id, capitalised: the UI's names).
                    let preset = event
                        .original_json()
                        .and_then(|j| j.get_field::<serde_json::Value>("content").ok().flatten())
                        .and_then(|c| {
                            c.get("preset")
                                .and_then(|p| p.as_str())
                                .map(ToOwned::to_owned)
                        });
                    let text = match preset {
                        Some(p) if !p.is_empty() => {
                            let mut chars = p.chars();
                            let first = chars
                                .next()
                                .map(|c| c.to_uppercase().to_string())
                                .unwrap_or_default();
                            format!("{name} changed the theme to {first}{}", chars.as_str())
                        }
                        _ => format!("{name} changed the chat theme"),
                    };
                    return EntryKind::Notice {
                        text,
                        action: Some("theme"),
                    };
                }
                match custom_state_verb(&kind) {
                    Some(verb) => EntryKind::Notice {
                        text: format!("{name} {verb}"),
                        action: None,
                    },
                    None => EntryKind::Hidden,
                }
            }
            TimelineItemContent::CallInvite | TimelineItemContent::RtcNotification { .. } => {
                EntryKind::Notice {
                    text: format!("📞 {name} started a call"),
                    action: None,
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
                    view_once: view_once_of(event),
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
                    view_once: view_once_of(event),
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
        crate::rt::spawn(async move {
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
        MessageType::Location(l) => MessageContent::Location {
            geo_uri: l.geo_uri.clone(),
            description: l.body.clone(),
        },
        other => MessageContent::Text {
            body: other.body().to_owned(),
            html: None,
            link: None,
            effect,
        },
    }
}

/// The album (`com.alwy.chat.gallery`) from the event's content (absent on local echoes until sent).
/// A view-once photo or video (`com.alwy.chat.view_once`).
fn view_once_of(event: &EventTimelineItem) -> bool {
    event
        .original_json()
        .and_then(|raw| raw.get_field::<serde_json::Value>("content").ok().flatten())
        .is_some_and(|c| c.get(crate::names::VIEW_ONCE).and_then(|v| v.as_bool()) == Some(true))
}

fn gallery_of(event: &EventTimelineItem) -> Option<GalleryRef> {
    let content: serde_json::Value = event.original_json()?.get_field("content").ok()??;
    let value = content.get(crate::names::GALLERY)?;
    serde_json::from_value(value.clone()).ok()
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
            MessageContent::Location { .. } => "📍 Location".to_owned(),
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

/// A reply to us lists us in its mentions (that's how Matrix notifies the replied-to person), so
/// the server highlights it like an @mention. Without our name or a link to us in the text
/// itself, it isn't one: no mention ring (the reply quote already shows it's for us).
fn reply_only_mention(
    msg: &matrix_sdk_ui::timeline::MsgLikeContent,
    own: &matrix_sdk::ruma::UserId,
) -> bool {
    use matrix_sdk::ruma::events::room::message::MessageType;
    if msg.in_reply_to.is_none() {
        return false;
    }
    let MsgLikeKind::Message(m) = &msg.kind else {
        return false;
    };
    let Some(mentions) = m.mentions() else {
        return false;
    };
    if mentions.room || !mentions.user_ids.contains(own) {
        return false;
    }
    let id = own.as_str();
    let encoded = id.replace('@', "%40");
    let html = match m.msgtype() {
        MessageType::Text(t) => t.formatted.as_ref().map(|f| f.body.as_str()),
        MessageType::Notice(t) => t.formatted.as_ref().map(|f| f.body.as_str()),
        MessageType::Emote(t) => t.formatted.as_ref().map(|f| f.body.as_str()),
        _ => None,
    };
    // The quote of the replied message (reply fallback) names us too: only the new text counts.
    let body = without_plain_quote(m.body());
    let html = html.map(without_html_quote);
    let named = body.contains(id) || html.is_some_and(|h| h.contains(id) || h.contains(&encoded));
    !named
}

/// A reply's plain body without its fallback quote ("> <@user> text" lines, then a blank line).
fn without_plain_quote(body: &str) -> &str {
    if !body.starts_with("> ") {
        return body;
    }
    body.split_once("\n\n").map_or("", |(_, rest)| rest)
}

/// A reply's HTML without its fallback quote (`<mx-reply>…</mx-reply>`).
fn without_html_quote(html: &str) -> &str {
    match html.find("</mx-reply>") {
        Some(end) => &html[end + "</mx-reply>".len()..],
        None => html,
    }
}

#[cfg(test)]
mod reply_quote_tests {
    use super::{without_html_quote, without_plain_quote};

    #[test]
    fn quotes_are_left_out() {
        assert_eq!(
            without_plain_quote("> <@a:x.org> hi\n> there\n\nbanger"),
            "banger"
        );
        assert_eq!(without_plain_quote("hello @a:x.org"), "hello @a:x.org");
        assert_eq!(
            without_html_quote(
                "<mx-reply><blockquote><a href=\"https://matrix.to/#/@a:x.org\">a</a></blockquote></mx-reply>banger"
            ),
            "banger"
        );
        assert_eq!(without_html_quote("<p>hi</p>"), "<p>hi</p>");
    }
}
