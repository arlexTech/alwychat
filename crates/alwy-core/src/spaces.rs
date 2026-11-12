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
        OwnedRoomId, OwnedRoomOrAliasId, api::client::room::create_room, events::StateEventType,
        room::RoomType, serde::Raw,
    },
};
use matrix_sdk_ui::spaces::{SpaceRoom, SpaceService};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::Notify;
use tracing::warn;

use crate::{Error, Result};

use crate::names::CHAT_LIST as CHAT_LIST_PREFS;

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
    pub(crate) async fn space_service(&self) -> Result<Arc<SpaceService>> {
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
        let task = crate::rt::spawn(async move {
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
        // Raw events: a removal is an *empty* m.space.child, which the typed event (it requires
        // `via`) fails to read, so removals never refreshed the list.
        let handle = client.add_room_event_handler(
            &space_id,
            move |event: Raw<matrix_sdk::ruma::events::AnySyncStateEvent>| {
                let notify = notify.clone();
                async move {
                    if event.get_field::<String>("type").ok().flatten().as_deref()
                        == Some("m.space.child")
                    {
                        notify.notify_one();
                    }
                }
            },
        );
        let guard = client.event_handler_drop_guard(handle);
        let feed = list.clone();
        let reloading = AtomicBool::new(false);
        let task = crate::rt::spawn(async move {
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
                    crate::rt::sleep(std::time::Duration::from_millis(500)).await;
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

    /// Put a space's children in this order (`order` in their `m.space.child`, which Element and
    /// the SDK's space list sort by). Only the ones whose order changes are rewritten; the rest
    /// of each event (via, suggested) is kept.
    pub async fn reorder_space(&self, space_id: &str, ids: Vec<String>) -> Result<()> {
        let room = self.room(space_id)?;
        for (i, id) in ids.iter().enumerate() {
            let order = format!("{:04}", i * 10);
            let Some(mut content) = crate::custom::fetch::<
                serde_json::Map<String, serde_json::Value>,
            >(&room, "m.space.child", id)
            .await?
            else {
                continue;
            };
            // An empty event is a removed child: leave it removed.
            let linked = content
                .get("via")
                .and_then(|v| v.as_array())
                .is_some_and(|v| !v.is_empty());
            if !linked || content.get("order").and_then(|o| o.as_str()) == Some(order.as_str()) {
                continue;
            }
            content.insert("order".into(), json!(order));
            crate::offline::write_state(
                &room,
                "m.space.child",
                id,
                serde_json::Value::Object(content),
            )
            .await?;
        }
        Ok(())
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
        Ok(
            crate::offline::read_global(&self.client()?, CHAT_LIST_PREFS)
                .await?
                .and_then(|v| serde_json::from_value::<ChatListPrefs>(v).ok())
                .unwrap_or_default(),
        )
    }

    pub async fn set_chat_list_prefs(&self, prefs: ChatListPrefs) -> Result<()> {
        crate::offline::write_global(
            &self.client()?,
            CHAT_LIST_PREFS,
            serde_json::to_value(&prefs)?,
        )
        .await
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
    #[test]
    fn children_follow_the_space_order() {
        let mut list = [
            ("!c", None, 1),
            ("!b", Some("0010"), 9),
            ("!a", Some("0000"), 5),
            ("!d", None, 0),
        ];
        list.sort_by(|a, b| super::child_order(*a, *b));
        let ids: Vec<_> = list.iter().map(|c| c.0).collect();
        assert_eq!(ids, ["!a", "!b", "!d", "!c"]);
        assert!(!super::valid_order("caf\u{e9}"));
    }

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

/// A category of a space: one of its sub-spaces, with that sub-space's own chats (Discord-style
/// sections; Element and Cinny show sub-spaces the same way).
#[derive(Debug, Clone, Serialize)]
pub struct SpaceCategory {
    pub id: String,
    pub name: String,
    /// We're in it (needed to see its chats' messages; joining a chat inside works either way).
    pub joined: bool,
    pub children: Vec<SpaceChild>,
}

/// A usable `order` (spec: printable ASCII, at most 50 characters).
fn valid_order(order: &str) -> bool {
    order.len() <= 50 && order.bytes().all(|b| (0x20..=0x7E).contains(&b))
}

/// The spec's ordering of a space's children: by `order` (those with one first), then by
/// when they were added, then by room ID.
fn child_order(a: (&str, Option<&str>, u64), b: (&str, Option<&str>, u64)) -> std::cmp::Ordering {
    use std::cmp::Ordering::*;
    let by_order = match (a.1, b.1) {
        (Some(x), Some(y)) => x.cmp(y),
        (Some(_), None) => Less,
        (None, Some(_)) => Greater,
        (None, None) => Equal,
    };
    by_order.then(a.2.cmp(&b.2)).then(a.0.cmp(b.0))
}

fn hierarchy_child(client: &Client, room: &serde_json::Value) -> Option<SpaceChild> {
    let id = room["room_id"].as_str()?.to_owned();
    let room_type = room["room_type"].as_str().map(RoomType::from);
    let joined = OwnedRoomId::try_from(id.as_str())
        .ok()
        .and_then(|rid| client.get_room(&rid))
        .is_some_and(|r| r.state() == matrix_sdk::RoomState::Joined);
    Some(SpaceChild {
        name: room["name"]
            .as_str()
            .or(room["canonical_alias"].as_str())
            .unwrap_or(&id)
            .to_owned(),
        avatar: room["avatar_url"].as_str().map(ToOwned::to_owned),
        topic: room["topic"].as_str().map(ToOwned::to_owned),
        members: room["num_joined_members"].as_u64().unwrap_or(0),
        is_space: room_type == Some(RoomType::Space),
        is_voice: crate::rooms::is_voice_type(room_type.as_ref()),
        joined,
        suggested: false,
        via: Vec::new(),
        id,
    })
}

impl crate::Core {
    /// The space's categories (sub-spaces) and what's in each, from the server's hierarchy two
    /// levels deep (the space list only has direct children).
    pub async fn space_categories(&self, space_id: &str) -> Result<Vec<SpaceCategory>> {
        use matrix_sdk::ruma::{api::client::space::get_hierarchy, uint};
        let client = self.client()?;
        let space = OwnedRoomId::try_from(space_id).map_err(|_| Error::UnknownRoom)?;
        let mut rooms: Vec<serde_json::Value> = Vec::new();
        let mut from = None;
        // A few pages at most (big public spaces).
        for _ in 0..5 {
            let mut request = get_hierarchy::v1::Request::new(space.clone());
            request.max_depth = Some(uint!(2));
            request.from = from.take();
            let response = client.send(request).await?;
            rooms.extend(
                response
                    .rooms
                    .iter()
                    .filter_map(|r| serde_json::to_value(r).ok()),
            );
            match response.next_batch {
                Some(next) => from = Some(next),
                None => break,
            }
        }
        let by_id: HashMap<&str, &serde_json::Value> = rooms
            .iter()
            .filter_map(|r| Some((r["room_id"].as_str()?, r)))
            .collect();
        // A space's children (ID and servers to join through), in its own order.
        let children_of = |id: &str| -> Vec<(String, Vec<String>)> {
            let events = by_id
                .get(id)
                .and_then(|r| r["children_state"].as_array())
                .map(Vec::as_slice)
                .unwrap_or_default();
            let mut children: Vec<_> = events
                .iter()
                .filter_map(|e| {
                    let via: Vec<String> = e["content"]["via"]
                        .as_array()?
                        .iter()
                        .filter_map(|v| v.as_str().map(ToOwned::to_owned))
                        .collect();
                    let id = e["state_key"].as_str()?.to_owned();
                    let order = e["content"]["order"]
                        .as_str()
                        .filter(|o| valid_order(o))
                        .map(ToOwned::to_owned);
                    let ts = e["origin_server_ts"].as_u64().unwrap_or(0);
                    (!via.is_empty()).then_some((id, via, order, ts))
                })
                .collect();
            children.sort_by(|a, b| {
                child_order((&a.0, a.2.as_deref(), a.3), (&b.0, b.2.as_deref(), b.3))
            });
            children
                .into_iter()
                .map(|(id, via, ..)| (id, via))
                .collect()
        };
        let mut categories = Vec::new();
        for (child_id, _) in children_of(space_id) {
            let Some(room) = by_id.get(child_id.as_str()) else {
                continue;
            };
            if room["room_type"].as_str() != Some("m.space") {
                continue;
            }
            let Some(category) = hierarchy_child(&client, room) else {
                continue;
            };
            let children = children_of(&child_id)
                .into_iter()
                .filter_map(|(id, via)| {
                    let mut child = hierarchy_child(&client, by_id.get(id.as_str())?)?;
                    child.via = via;
                    Some(child)
                })
                .collect();
            categories.push(SpaceCategory {
                id: category.id,
                name: category.name,
                joined: category.joined,
                children,
            });
        }
        Ok(categories)
    }
}

#[derive(Default, serde::Deserialize)]
struct BannerContent {
    #[serde(default)]
    url: Option<String>,
}

impl crate::Core {
    /// A space's banner (`com.alwy.chat.room.banner`, AlwyChat's own: Matrix has none), an
    /// `mxc://` URL.
    pub async fn space_banner(&self, room_id: &str) -> Result<Option<String>> {
        let room = self.room(room_id)?;
        let content: Option<BannerContent> =
            crate::custom::fetch(&room, crate::names::BANNER, "").await?;
        Ok(content
            .and_then(|c| c.url)
            .filter(|u| u.starts_with("mxc://")))
    }

    /// Set (a JPEG) or remove (`None`) a space's banner.
    pub async fn set_space_banner(&self, room_id: &str, jpeg: Option<Vec<u8>>) -> Result<()> {
        let room = self.room(room_id)?;
        let content = match jpeg {
            Some(bytes) => {
                let url = self.upload_wallpaper(bytes, "image/jpeg").await?;
                serde_json::json!({ "url": url })
            }
            None => serde_json::json!({}),
        };
        crate::offline::write_state(&room, crate::names::BANNER, "", content).await?;
        Ok(())
    }
}
