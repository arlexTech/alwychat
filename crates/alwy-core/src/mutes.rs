//! Muting for a while, and muting a whole space.
//!
//! A mute itself is the homeserver's push rule (`RoomNotificationMode::Mute`, as Element sets
//! it), so it works on every device and app. What Matrix can't say is "until when", or "every
//! chat of this space, including ones joined later": that is kept in global account data
//! `com.alwy.chat.mutes` and applied by AlwyChat itself (`apply_mutes`): when the app starts,
//! every minute while it runs, and whenever a push wakes it. A timed mute therefore ends once
//! one of your AlwyChat apps is running or receives a notification after that time.

use std::collections::HashMap;

use matrix_sdk::{
    notification_settings::RoomNotificationMode,
    ruma::{OwnedRoomId, events::GlobalAccountDataEventType, serde::Raw},
};
use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::{Error, Result, names::MUTES};

/// What's muted beyond the push rules themselves.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Mutes {
    /// Chats muted for a while: when the mute ends (ms since 1970). Chats muted until turned
    /// back on aren't listed (the push rule is enough).
    #[serde(default)]
    pub rooms: HashMap<String, u64>,
    /// Muted spaces (top-level).
    #[serde(default)]
    pub spaces: HashMap<String, SpaceMute>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct SpaceMute {
    /// When it ends; none = until turned back on.
    pub until: Option<u64>,
    /// The chats this space mute muted (unmuted with it; chats muted on their own aren't).
    #[serde(default)]
    pub rooms: Vec<String>,
}

fn now_ms() -> u64 {
    matrix_sdk::ruma::MilliSecondsSinceUnixEpoch::now()
        .get()
        .into()
}

/// Mutes that are over at `now`: (rooms, spaces).
fn expired(mutes: &Mutes, now: u64) -> (Vec<String>, Vec<String>) {
    let rooms = mutes
        .rooms
        .iter()
        .filter(|(_, until)| **until <= now)
        .map(|(id, _)| id.clone())
        .collect();
    let spaces = mutes
        .spaces
        .iter()
        .filter(|(_, m)| m.until.is_some_and(|u| u <= now))
        .map(|(id, _)| id.clone())
        .collect();
    (rooms, spaces)
}

impl crate::Core {
    /// From the server (another device may have changed it), else the store's copy.
    pub async fn mutes(&self) -> Result<Mutes> {
        let account = self.client()?.account();
        let kind = GlobalAccountDataEventType::from(MUTES);
        let raw = match account.fetch_account_data(kind.clone()).await {
            Ok(raw) => raw,
            Err(e) => {
                warn!("mutes from the server: {e}");
                account.account_data_raw(kind).await?
            }
        };
        Ok(raw
            .and_then(|r| serde_json::from_str(r.json().get()).ok())
            .unwrap_or_default())
    }

    async fn save_mutes(&self, mutes: &Mutes) -> Result<()> {
        self.client()?
            .account()
            .set_account_data_raw(
                GlobalAccountDataEventType::from(MUTES),
                Raw::new(mutes)?.cast_unchecked(),
            )
            .await?;
        Ok(())
    }

    /// Mute a chat, until `until` (ms since 1970) or until turned back on (`None`).
    pub async fn mute_room(&self, room_id: &str, until: Option<u64>) -> Result<()> {
        self.set_muted(room_id, true).await?;
        let mut mutes = self.mutes().await?;
        match until {
            Some(until) => mutes.rooms.insert(room_id.to_owned(), until),
            None => mutes.rooms.remove(room_id),
        };
        self.save_mutes(&mutes).await
    }

    /// Unmute a chat (also when its space is muted: it's then left out of that mute).
    pub async fn unmute_room(&self, room_id: &str) -> Result<()> {
        self.set_muted(room_id, false).await?;
        let mut mutes = self.mutes().await?;
        mutes.rooms.remove(room_id);
        for space in mutes.spaces.values_mut() {
            space.rooms.retain(|r| r != room_id);
        }
        self.save_mutes(&mutes).await
    }

    /// The joined chats of a top-level space, at any depth.
    async fn space_chats(&self, space_id: &str) -> Result<Vec<OwnedRoomId>> {
        let service = self.space_service().await?;
        let space: OwnedRoomId = space_id.try_into().map_err(|_| Error::UnknownRoom)?;
        let mut chats = Vec::new();
        for room in self.client()?.joined_rooms() {
            if room.is_space() {
                continue;
            }
            if service
                .top_level_ancestors_of(room.room_id())
                .await
                .contains(&space)
            {
                chats.push(room.room_id().to_owned());
            }
        }
        Ok(chats)
    }

    /// Mute every chat of a space (and those joined later), until `until` or turned back on.
    /// Chats that already have their own setting (muted, mentions only…) keep it.
    pub async fn mute_space(&self, space_id: &str, until: Option<u64>) -> Result<()> {
        let mut mutes = self.mutes().await?;
        let mut entry = mutes.spaces.remove(space_id).unwrap_or_default();
        entry.until = until;
        for chat in self.space_chats(space_id).await? {
            self.mute_for_space(&chat, &mut entry).await;
        }
        mutes.spaces.insert(space_id.to_owned(), entry);
        self.save_mutes(&mutes).await
    }

    /// Mute one chat for its space's mute, unless it has its own setting.
    async fn mute_for_space(&self, chat: &OwnedRoomId, entry: &mut SpaceMute) {
        let id = chat.to_string();
        if entry.rooms.contains(&id) {
            return;
        }
        let Ok(room) = self.room(&id) else { return };
        if room.cached_user_defined_notification_mode().is_some() {
            return;
        }
        match self.set_muted(&id, true).await {
            Ok(()) => entry.rooms.push(id),
            Err(e) => warn!(room = %id, "couldn't mute for its space: {e}"),
        }
    }

    /// Unmute a space: the chats its mute muted go back to their default.
    pub async fn unmute_space(&self, space_id: &str) -> Result<()> {
        let mut mutes = self.mutes().await?;
        if let Some(entry) = mutes.spaces.remove(space_id) {
            for chat in entry.rooms {
                // Still muted (not changed since) and not muted on its own for a while.
                let muted = self.room(&chat).is_ok_and(|r| {
                    r.cached_user_defined_notification_mode() == Some(RoomNotificationMode::Mute)
                });
                if muted
                    && !mutes.rooms.contains_key(&chat)
                    && let Err(e) = self.set_muted(&chat, false).await
                {
                    warn!(room = %chat, "couldn't unmute with its space: {e}");
                }
            }
        }
        self.save_mutes(&mutes).await
    }

    /// End the mutes whose time is up, and mute chats newly joined in muted spaces. Called at
    /// start, every minute by the app while it runs, and when a push wakes it.
    pub async fn apply_mutes(&self) -> Result<Mutes> {
        let mut mutes = self.mutes().await?;
        let (rooms, spaces) = expired(&mutes, now_ms());
        for space in &spaces {
            self.unmute_space(space).await?;
        }
        for room in &rooms {
            // Changed since (another mode chosen elsewhere): only forget the end time.
            let still_muted = self.room(room).is_ok_and(|r| {
                r.cached_user_defined_notification_mode() == Some(RoomNotificationMode::Mute)
            });
            if still_muted {
                self.unmute_room(room).await?;
            } else {
                let mut m = self.mutes().await?;
                m.rooms.remove(room);
                self.save_mutes(&m).await?;
            }
        }
        if !rooms.is_empty() || !spaces.is_empty() {
            mutes = self.mutes().await?;
        }
        // Chats joined in a muted space since.
        let mut changed = false;
        let ids: Vec<String> = mutes.spaces.keys().cloned().collect();
        for space in ids {
            let Ok(chats) = self.space_chats(&space).await else {
                continue;
            };
            let mut entry = mutes.spaces.remove(&space).unwrap_or_default();
            let before = entry.rooms.len();
            for chat in chats {
                self.mute_for_space(&chat, &mut entry).await;
            }
            changed |= entry.rooms.len() != before;
            mutes.spaces.insert(space, entry);
        }
        if changed {
            self.save_mutes(&mutes).await?;
        }
        Ok(mutes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_has_expired() {
        let mut mutes = Mutes::default();
        mutes.rooms.insert("!a:x".into(), 100);
        mutes.rooms.insert("!b:x".into(), 300);
        mutes.spaces.insert(
            "!s:x".into(),
            SpaceMute {
                until: Some(200),
                rooms: vec!["!c:x".into()],
            },
        );
        mutes
            .spaces
            .insert("!forever:x".into(), SpaceMute::default());
        let (rooms, spaces) = expired(&mutes, 250);
        assert_eq!(rooms, vec!["!a:x".to_owned()]);
        assert_eq!(spaces, vec!["!s:x".to_owned()]);
        assert_eq!(expired(&mutes, 50), (vec![], vec![]));
    }

    #[test]
    fn parses_partial_data() {
        let m: Mutes = serde_json::from_str(r#"{"spaces":{"!s:x":{"until":null}}}"#).unwrap();
        assert!(m.rooms.is_empty());
        assert_eq!(m.spaces["!s:x"], SpaceMute::default());
    }
}
