//! Spaces: the joined spaces and which chats belong to each (for the "Spaces" tab, and to
//! keep space chats out of "Chats"), the children of one space (joined or not), and the
//! synced "show in Chats" choices (global account data `com.alwy.chat.chat_list`).

use std::{collections::HashMap, sync::Arc};

use futures_util::StreamExt;
use matrix_sdk::{
    Client,
    ruma::{
        OwnedRoomId, OwnedRoomOrAliasId, events::GlobalAccountDataEventType, room::RoomType,
        serde::Raw,
    },
};
use matrix_sdk_ui::spaces::{SpaceRoom, SpaceService};
use serde::{Deserialize, Serialize};
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
}

fn child(room: SpaceRoom) -> SpaceChild {
    SpaceChild {
        id: room.room_id.to_string(),
        name: room.display_name,
        avatar: room.avatar_url.map(|u| u.to_string()),
        topic: room.topic,
        members: room.num_joined_members,
        is_space: room.room_type == Some(RoomType::Space),
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
        let (_, mut updates) = service.subscribe_to_space_filters().await;
        sink(spaces_overview(&client, &service).await);
        let task = tokio::spawn(async move {
            // The filters change when spaces are joined or left, or their children change.
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
        let list = Arc::new(service.space_room_list(space_id).await);
        let (initial, mut updates) = list.subscribe_to_room_updates().await;
        sink(initial.into_iter().map(child).collect());
        let feed = list.clone();
        let task = tokio::spawn(async move {
            let paginate = async {
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
            };
            let forward = async {
                while updates.next().await.is_some() {
                    sink(feed.rooms().await.into_iter().map(child).collect());
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

    pub async fn chat_list_prefs(&self) -> Result<ChatListPrefs> {
        let raw = self
            .client()?
            .account()
            .account_data_raw(GlobalAccountDataEventType::from(CHAT_LIST_PREFS))
            .await?;
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
