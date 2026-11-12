//! Auto-save: new photos and videos others send in chosen chats are saved to the gallery by
//! themselves, while the app runs (also when a push wakes it). The choice is per chat and per
//! device (`Settings::auto_save_rooms`): a phone saving to its gallery shouldn't fill a
//! computer's Pictures folder.
//!
//! The core downloads (and decrypts) the file into the folder the app gives in
//! [`Core::on_auto_save`], then tells the app, which on Android hands it to MediaStore.
//! Only messages newer than the last one looked at are saved (kept in `autosave.json`), so the
//! backlog loaded at start or when scrolling never saves old photos again.

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{Arc, LazyLock, Mutex},
};

use matrix_sdk::{
    Client, Room,
    media::{MediaFormat, MediaRequestParameters},
    ruma::{
        MilliSecondsSinceUnixEpoch, OwnedEventId,
        events::room::message::{MessageType, OriginalSyncRoomMessageEvent, Relation},
    },
};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::settings::Settings;

/// A file just saved by auto-save.
#[derive(Debug, Clone, Serialize)]
pub struct AutoSaved {
    pub path: String,
    pub mime: String,
    pub room_id: String,
}

pub(crate) type AutoSaveHandler = Box<dyn Fn(AutoSaved) + Send + Sync>;

/// Where files go and who to tell.
type Target = (PathBuf, Arc<AutoSaveHandler>);
static TARGET: LazyLock<Mutex<Option<Target>>> = LazyLock::new(Default::default);
/// Saved during this run (an event can be handed over twice: sync and backfill).
static SEEN: LazyLock<Mutex<HashSet<OwnedEventId>>> = LazyLock::new(Default::default);

/// Per chat: messages sent up to this (ms) were already looked at.
#[derive(Default, Clone, Serialize, Deserialize)]
struct Marks {
    rooms: HashMap<String, u64>,
}

/// A chat seen for the first time (auto-save just turned on): only what arrives from now on,
/// with a minute's slack for the message that is arriving.
const FIRST_LOOK_SLACK_MS: u64 = 60_000;

fn now_ms() -> u64 {
    MilliSecondsSinceUnixEpoch::now().get().into()
}

/// Watch new messages (once per signed-in client). `mark`: where the "since" mark is kept.
pub(crate) fn install(client: &Client, settings: Arc<Mutex<Settings>>, mark: PathBuf) {
    let marks = crate::files::read_now(&mark)
        .ok()
        .and_then(|b| serde_json::from_slice::<Marks>(&b).ok())
        .unwrap_or_default();
    let marks = Arc::new(Mutex::new(marks));
    client.add_event_handler(
        move |event: OriginalSyncRoomMessageEvent,
              raw: matrix_sdk::event_handler::RawEvent,
              room: Room| {
            let settings = settings.clone();
            let marks = marks.clone();
            let mark = mark.clone();
            async move {
                // A view-once photo is meant to be seen once, not kept.
                let view_once = serde_json::from_str::<serde_json::Value>(raw.get())
                    .ok()
                    .and_then(|e| e.get("content")?.get(crate::names::VIEW_ONCE)?.as_bool())
                    == Some(true);
                if view_once {
                    return;
                }
                let wanted = settings
                    .lock()
                    .unwrap()
                    .auto_save_rooms
                    .iter()
                    .any(|r| r == room.room_id().as_str());
                // No target yet (a push woke the app before its UI): leave the message for later.
                if !wanted || event.sender == room.own_user_id() || TARGET.lock().unwrap().is_none()
                {
                    return;
                }
                let sent: u64 = event.origin_server_ts.get().into();
                let snapshot = {
                    let mut marks = marks.lock().unwrap();
                    let since = marks
                        .rooms
                        .entry(room.room_id().to_string())
                        .or_insert_with(|| now_ms().saturating_sub(FIRST_LOOK_SLACK_MS));
                    if sent <= *since {
                        return;
                    }
                    *since = sent;
                    marks.clone()
                };
                if let Ok(bytes) = serde_json::to_vec(&snapshot)
                    && let Err(e) = crate::files::write(&mark, bytes).await
                {
                    warn!("auto-save mark: {e}");
                }
                save(&room, event).await;
            }
        },
    );
}

/// Bigger files aren't saved by themselves: the whole file is held in memory on its way to
/// the disk, and a phone's storage isn't for the taking.
const MAX_AUTO_SAVE_BYTES: u64 = 200 * 1024 * 1024;

async fn save(room: &Room, event: OriginalSyncRoomMessageEvent) {
    // An edit isn't a new photo.
    if matches!(event.content.relates_to, Some(Relation::Replacement(_))) {
        return;
    }
    let (source, name, mime, size) = match &event.content.msgtype {
        MessageType::Image(c) => (
            c.source.clone(),
            c.filename().to_owned(),
            c.info.as_ref().and_then(|i| i.mimetype.clone()),
            c.info.as_ref().and_then(|i| i.size),
        ),
        MessageType::Video(c) => (
            c.source.clone(),
            c.filename().to_owned(),
            c.info.as_ref().and_then(|i| i.mimetype.clone()),
            c.info.as_ref().and_then(|i| i.size),
        ),
        _ => return,
    };
    if size.is_some_and(|s| u64::from(s) > MAX_AUTO_SAVE_BYTES) {
        info!(event = %event.event_id, "too big to save by itself");
        return;
    }
    if !SEEN.lock().unwrap().insert(event.event_id.clone()) {
        return;
    }
    let Some((dir, handler)) = TARGET.lock().unwrap().clone() else {
        return;
    };
    let request = MediaRequestParameters {
        source,
        format: MediaFormat::File,
    };
    let bytes = match room
        .client()
        .media()
        .get_media_content(&request, true)
        .await
    {
        Ok(bytes) => bytes,
        Err(e) => {
            warn!(event = %event.event_id, "auto-save download: {e}");
            return;
        }
    };
    let mime = mime.unwrap_or_else(|| crate::media::sniff(&bytes).to_owned());
    match crate::media::write_unique(&dir, &with_extension(&name, &mime), &bytes).await {
        Ok(path) => {
            info!(room = %room.room_id(), "auto-saved a {mime}");
            handler(AutoSaved {
                path: path.display().to_string(),
                mime,
                room_id: room.room_id().to_string(),
            });
        }
        Err(e) => warn!("auto-save: {e}"),
    }
}

/// `name` with an extension matching `mime` if it has none (Android picks the gallery from it).
fn with_extension(name: &str, mime: &str) -> String {
    if name
        .rsplit_once('.')
        .is_some_and(|(stem, ext)| !stem.is_empty() && ext.len() <= 5)
    {
        return name.to_owned();
    }
    let ext = match mime {
        "image/jpeg" => "jpg",
        "image/png" => "png",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/heic" | "image/heif" => "heic",
        "image/avif" => "avif",
        "video/mp4" => "mp4",
        "video/quicktime" => "mov",
        "video/webm" => "webm",
        m if m.starts_with("image/") => "jpg",
        m if m.starts_with("video/") => "mp4",
        _ => return name.to_owned(),
    };
    let stem = if name.trim().is_empty() {
        "media"
    } else {
        name
    };
    format!("{stem}.{ext}")
}

impl crate::Core {
    /// Auto-saved files are written to `dir`, then `handler` is told (to move them to the
    /// gallery where that's needed).
    pub fn on_auto_save(&self, dir: &Path, handler: impl Fn(AutoSaved) + Send + Sync + 'static) {
        *TARGET.lock().unwrap() = Some((dir.to_owned(), Arc::new(Box::new(handler))));
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn extensions() {
        assert_eq!(
            super::with_extension("IMG_1.jpg", "image/jpeg"),
            "IMG_1.jpg"
        );
        assert_eq!(super::with_extension("IMG_1", "image/heic"), "IMG_1.heic");
        assert_eq!(super::with_extension("", "video/mp4"), "media.mp4");
        assert_eq!(
            super::with_extension("Photo de Margot", "image/png"),
            "Photo de Margot.png"
        );
        assert_eq!(
            super::with_extension("v1.2 final", "video/quicktime"),
            "v1.2 final.mov"
        );
    }
}
