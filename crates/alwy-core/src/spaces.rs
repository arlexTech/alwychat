//! Spaces: the joined spaces and which chats belong to each (for the "Spaces" tab, and to
//! keep space chats out of "Chats"), the children of one space (joined or not), and the
//! synced "show in Chats" choices (global account data `com.alwy.chat.chat_list`).

use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use futures_util::StreamExt;
use matrix_sdk::{
    Client,
    ruma::{
        OwnedRoomId, OwnedRoomOrAliasId,
        api::client::room::create_room,
        events::{
            GlobalAccountDataEventType, StateEventType, SyncStateEvent,
            space::child::SpaceChildEventContent,
        },
        room::RoomType,
        serde::Raw,
    },
};
use matrix_sdk_ui::spaces::{SpaceRoom, SpaceService};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::Notify;
use tracing::warn;

use crate::{Error, Result};

const CHAT_LIST_PREFS: &str = "com.alwy.chat.chat_list";

/// A joined top-level space.
#[derive(Debug, Clone, Serialize)]
pub struct SpaceInfo {
    pub id: String,
    pub name: String,
    pub avatar: Option<String>,
    /// Joined chats inside it, at any depth (not sub-spaces themselves).
    pub rooms: Vec<String>,
}

/// A child of a space, from the server's space hierarchy (joined or not).
#[derive(Debug, Clone, Serialize)]
pub struct SpaceChild {
    pub id: String,
    pub name: String,
    pub avatar: Option<String>,
    pub topic: Option<String>,
    pub members: u64,
    pub is_space: bool,
    /// A voice channel (see `rooms::is_voice_type`).
    pub is_voice: bool,
    pub joined: bool,
    /// Recommended by the space's admins.
    pub suggested: bool,
    /// Servers to join through.
    pub via: Vec<String>,
}

/// Which space chats also appear in "Chats" (synced between our devices).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ChatListPrefs {
    /// All of them, unless a space or chat says otherwise.
    pub show_space_chats: bool,
    /// Per top-level space.
    pub spaces: HashMap<String, bool>,
    /// Per chat (wins over its spaces).
    pub rooms: HashMap<String, bool>,
    /// Custom chat-list folders (Telegram style), in display order.
    pub folders: Vec<Folder>,
}

/// A chat-list folder: chats picked by type and/or by hand.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Folder {
    pub id: String,
    pub name: String,
    /// Every 1:1 chat.
    pub people: bool,
    /// Every group (chats inside spaces only when they show in "Chats").
    pub groups: bool,
    /// Always in, whatever the rules above.
    pub rooms: Vec<String>,
    /// Never in.
    pub exclude: Vec<String>,
    pub unread_only: bool,
    pub hide_muted: bool,
}

fn child(room: SpaceRoom) -> SpaceChild {
    SpaceChild {
        id: room.room_id.to_string(),
        name: room.display_name,
        avatar: room.avatar_url.map(|u| u.to_string()),
        topic: room.topic,
        members: room.num_joined_members,
        is_space: room.room_type == Some(RoomType::Space),
        is_voice: crate::rooms::is_voice_type(room.room_type.as_ref()),
        joined: room.state == Some(matrix_sdk::RoomState::Joined),
        suggested: room.suggested,
        via: room.via.iter().map(ToString::to_string).collect(),
    }
}

impl crate::Core {
    async fn space_service(&self) -> Result<Arc<SpaceService>> {
        if let Some(service) = self.with_active(|a| a.spaces.clone())? {
            return Ok(service);
        }
        let service = Arc::new(SpaceService::new(self.client()?).await);
        self.with_active(|a| a.spaces.get_or_insert(service).clone())
    }

    /// The joined top-level spaces and their chats; `sink` gets the whole list on each change.
    pub async fn subscribe_spaces(
        &self,
        sink: impl Fn(Vec<SpaceInfo>) + Send + Sync + 'static,
    ) -> Result<()> {
        let service = self.space_service().await?;
        let client = self.client()?;
        let (_, filters) = service.subscribe_to_space_filters().await;
        // Filters only exist for spaces with chats: a new, empty space shows up here.
        let (_, top_level) = service.subscribe_to_top_level_joined_spaces().await;
        sink(spaces_overview(&client, &service).await);
        let task = tokio::spawn(async move {
            // Spaces joined or left, or their children changed.
            let mut updates =
                futures_util::stream::select(filters.map(|_| ()), top_level.map(|_| ()));
            while updates.next().await.is_some() {
                sink(spaces_overview(&client, &service).await);
            }
        });
        self.with_active(|a| {
            if let Some(old) = a.spaces_task.replace(task.abort_handle()) {
                old.abort();
            }
        })
    }

    /// The children of one space (joined or not), loaded page by page; `sink` gets the whole
    /// list on each change. Replaces the previously opened space.
    pub async fn open_space(
        &self,
        space_id: &str,
        sink: impl Fn(Vec<SpaceChild>) + Send + Sync + 'static,
    ) -> Result<()> {
        let service = self.space_service().await?;
        let space_id = OwnedRoomId::try_from(space_id).map_err(|_| Error::UnknownRoom)?;
        let list = Arc::new(service.space_room_list(space_id.clone()).await);
        let (initial, mut updates) = list.subscribe_to_room_updates().await;
        sink(initial.into_iter().map(child).collect());
        // The hierarchy is fetched once: reload it when the space's children change (a chat
        // or channel added or removed, by us or anyone).
        let changed = Arc::new(Notify::new());
        let notify = changed.clone();
        let client = self.client()?;
        let handle = client.add_room_event_handler(
            &space_id,
            move |_: SyncStateEvent<SpaceChildEventContent>| {
                let notify = notify.clone();
                async move { notify.notify_one() }
            },
        );
        let guard = client.event_handler_drop_guard(handle);
        let feed = list.clone();
        let reloading = AtomicBool::new(false);
        let task = tokio::spawn(async move {
            let _guard = guard;
            let paginate = async {
                loop {
                    loop {
                        if let Err(e) = feed.paginate().await {
                            warn!("space hierarchy: {e}");
                            break;
                        }
                        if matches!(
                            feed.pagination_state(),
                            matrix_sdk_ui::spaces::room_list::SpaceRoomListPaginationState::Idle {
                                end_reached: true
                            }
                        ) {
                            break;
                        }
                    }
                    changed.notified().await;
                    // Let a burst of changes settle, then start over.
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    reloading.store(true, Ordering::Relaxed);
                    feed.reset().await;
                }
            };
            let forward = async {
                while updates.next().await.is_some() {
                    let rooms = feed.rooms().await;
                    // A reload clears the list first: keep showing the old one meanwhile.
                    if rooms.is_empty() && reloading.load(Ordering::Relaxed) {
                        continue;
                    }
                    reloading.store(false, Ordering::Relaxed);
                    sink(rooms.into_iter().map(child).collect());
                }
            };
            tokio::join!(paginate, forward);
        });
        self.with_active(|a| {
            if let Some(old) = a.space_task.replace(task.abort_handle()) {
                old.abort();
            }
        })
    }

    pub fn close_space(&self) -> Result<()> {
        self.with_active(|a| {
            if let Some(task) = a.space_task.take() {
                task.abort();
            }
        })
    }

    /// Join a chat or space listed in a space.
    pub async fn join_room(&self, room_id: &str, via: &[String]) -> Result<()> {
        let id = OwnedRoomOrAliasId::try_from(room_id).map_err(|_| Error::UnknownRoom)?;
        let via: Vec<_> = via
            .iter()
            .filter_map(|s| s.as_str().try_into().ok())
            .collect();
        self.client()?.join_room_by_id_or_alias(&id, &via).await?;
        Ok(())
    }

    /// Whether we may add chats to this space (send `m.space.child`).
    pub async fn can_add_to_space(&self, space_id: &str) -> Result<bool> {
        let space = self.room(space_id)?;
        let user = space.own_user_id().to_owned();
        Ok(space
            .power_levels()
            .await
            .map(|pl| pl.user_can_send_state(&user, StateEventType::SpaceChild))
            .unwrap_or(false))
    }

    /// Create a voice channel in a space: an Element Call "video room" that space members can
    /// join. Not end-to-end encrypted, so Element Call users can talk with us (we don't do
    /// call media encryption yet); its text chat isn't encrypted either.
    pub async fn create_voice_channel(&self, space_id: &str, name: &str) -> Result<String> {
        let name = name.trim();
        if name.is_empty() {
            return Err(Error::Call("the channel needs a name".into()));
        }
        let client = self.client()?;
        let space = self.room(space_id)?;
        let server = client.user_id().ok_or(Error::NotLoggedIn)?.server_name();
        let via = json!([server]);

        let mut request = create_room::v3::Request::new();
        request.name = Some(name.to_owned());
        request.creation_content =
            Some(Raw::new(&json!({ "type": crate::rooms::VOICE_ROOM_TYPE }))?.cast_unchecked());
        request.initial_state = vec![
            Raw::new(&json!({
                "type": "m.space.parent",
                "state_key": space.room_id(),
                "content": { "via": via, "canonical": true },
            }))?
            .cast_unchecked(),
            Raw::new(&json!({
                "type": "m.room.join_rules",
                "state_key": "",
                "content": {
                    "join_rule": "restricted",
                    "allow": [{ "type": "m.room_membership", "room_id": space.room_id() }],
                },
            }))?
            .cast_unchecked(),
            Raw::new(&json!({
                "type": "m.room.history_visibility",
                "state_key": "",
                "content": { "history_visibility": "shared" },
            }))?
            .cast_unchecked(),
        ];
        // Everyone may join the call (publish their call membership), as Element does.
        request.power_level_content_override = Some(
            Raw::new(&json!({
                "events": {
                    "org.matrix.msc3401.call.member": 0,
                    "org.matrix.msc3401.call": 0,
                },
            }))?
            .cast_unchecked(),
        );
        let room = client.create_room(request).await?;
        space
            .send_state_event_raw(
                "m.space.child",
                room.room_id().as_str(),
                json!({ "via": via }),
            )
            .await?;
        Ok(room.room_id().to_string())
    }

    /// From the server when reachable, not the store: the store only catches up with our own
    /// writes at the next sync, and changes are applied to this copy.
    pub async fn chat_list_prefs(&self) -> Result<ChatListPrefs> {
        let account = self.client()?.account();
        let kind = GlobalAccountDataEventType::from(CHAT_LIST_PREFS);
        // Offline: the last synced copy.
        let raw = match account.fetch_account_data(kind.clone()).await {
            Ok(raw) => raw,
            Err(e) => {
                warn!("chat list prefs from the server: {e}");
                account.account_data_raw(kind).await?
            }
        };
        Ok(raw
            .and_then(|r| serde_json::from_str::<ChatListPrefs>(r.json().get()).ok())
            .unwrap_or_default())
    }

    pub async fn set_chat_list_prefs(&self, prefs: ChatListPrefs) -> Result<()> {
        self.client()?
            .account()
            .set_account_data_raw(
                GlobalAccountDataEventType::from(CHAT_LIST_PREFS),
                Raw::new(&prefs)?.cast_unchecked(),
            )
            .await?;
        Ok(())
    }
}

async fn spaces_overview(client: &Client, service: &SpaceService) -> Vec<SpaceInfo> {
    let mut rooms_of: HashMap<OwnedRoomId, Vec<String>> = HashMap::new();
    for room in client.joined_rooms() {
        if room.is_space() {
            continue;
        }
        for top in service.top_level_ancestors_of(room.room_id()).await {
            rooms_of
                .entry(top)
                .or_default()
                .push(room.room_id().to_string());
        }
    }
    service
        .top_level_joined_spaces()
        .await
        .into_iter()
        .map(|space| SpaceInfo {
            rooms: rooms_of.remove(&space.room_id).unwrap_or_default(),
            id: space.room_id.to_string(),
            name: space.display_name,
            avatar: space.avatar_url.map(|u| u.to_string()),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_prefs_without_folders_still_parse() {
        let prefs: ChatListPrefs =
            serde_json::from_str(r#"{"show_space_chats":true,"spaces":{},"rooms":{}}"#).unwrap();
        assert!(prefs.show_space_chats);
        assert!(prefs.folders.is_empty());
        let prefs: ChatListPrefs =
            serde_json::from_str(r#"{"folders":[{"id":"a","name":"Work","rooms":["!x:y"]}]}"#)
                .unwrap();
        assert_eq!(prefs.folders[0].rooms, ["!x:y"]);
        assert!(!prefs.folders[0].people);
    }
}
