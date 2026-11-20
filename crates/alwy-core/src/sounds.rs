//! Soundboard: a chat's or voice channel's own short sounds (room state
//! `com.alwy.chat.room.sounds`, `{items: [...], hidden: [built-in ids]}`), shared by everyone
//! in it: its own sounds, and the built-in ones it took off its soundboard. The files are
//! uploaded unencrypted (like room photos) as short WAVs. Built-in sounds live in the UI.
//! Playing goes through the call's media (an extra audio track), not through Matrix.

use matrix_sdk::ruma::events::StateEventType;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{Error, Result, names::SOUNDS};

/// At most this many sounds per chat.
const MAX_SOUNDS: usize = 48;
/// The longest sound, in milliseconds.
pub const MAX_SOUND_MS: u32 = 6_000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Sound {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub emoji: Option<String>,
    /// `mxc://` of the file.
    pub url: String,
    pub mime: String,
    pub duration_ms: u32,
    /// Who added it.
    #[serde(default)]
    pub by: Option<String>,
}

#[derive(Default, Deserialize)]
struct SoundsContent {
    #[serde(default)]
    items: Vec<Sound>,
    /// Built-in sounds (UI ids) this chat took off its soundboard.
    #[serde(default)]
    hidden: Vec<String>,
}

/// Keep what's well-formed (names trimmed, limits applied).
fn clean(items: Vec<Sound>) -> Vec<Sound> {
    items
        .into_iter()
        .filter(|s| {
            s.url.starts_with("mxc://")
                && s.mime.starts_with("audio/")
                && s.duration_ms > 0
                && s.duration_ms <= MAX_SOUND_MS
                && !s.id.is_empty()
        })
        .map(|mut s| {
            s.name = s.name.trim().chars().take(32).collect();
            s.emoji = s
                .emoji
                .filter(|e| !e.trim().is_empty() && e.chars().count() <= 8);
            s
        })
        .filter(|s| !s.name.is_empty())
        .take(MAX_SOUNDS)
        .collect()
}

impl crate::Core {
    async fn sounds_content(&self, room: &matrix_sdk::Room) -> Result<SoundsContent> {
        Ok(crate::custom::fetch(room, SOUNDS, "")
            .await?
            .unwrap_or_default())
    }

    async fn write_sounds(
        &self,
        room: &matrix_sdk::Room,
        items: Vec<Sound>,
        hidden: Vec<String>,
    ) -> Result<()> {
        let mut hidden: Vec<String> = hidden
            .into_iter()
            .filter(|id| !id.is_empty() && id.len() <= 32)
            .take(64)
            .collect();
        hidden.sort();
        hidden.dedup();
        let content = json!({ "items": clean(items), "hidden": hidden });
        crate::offline::write_state(room, SOUNDS, "", content).await?;
        Ok(())
    }

    /// The chat's own sounds.
    pub async fn room_sounds(&self, room_id: &str) -> Result<Vec<Sound>> {
        let room = self.room(room_id)?;
        Ok(clean(self.sounds_content(&room).await?.items))
    }

    /// Replace the chat's sounds (adding or removing one: read, change, write).
    pub async fn set_room_sounds(&self, room_id: &str, items: Vec<Sound>) -> Result<()> {
        let room = self.room(room_id)?;
        let hidden = self.sounds_content(&room).await?.hidden;
        self.write_sounds(&room, items, hidden).await
    }

    /// The built-in sounds this chat took off its soundboard.
    pub async fn hidden_builtin_sounds(&self, room_id: &str) -> Result<Vec<String>> {
        let room = self.room(room_id)?;
        Ok(self.sounds_content(&room).await?.hidden)
    }

    /// Choose which built-in sounds this chat's soundboard leaves out.
    pub async fn set_hidden_builtin_sounds(
        &self,
        room_id: &str,
        hidden: Vec<String>,
    ) -> Result<()> {
        let room = self.room(room_id)?;
        let items = self.sounds_content(&room).await?.items;
        self.write_sounds(&room, items, hidden).await
    }

    /// Whether we may change the chat's sounds.
    pub async fn can_edit_sounds(&self, room_id: &str) -> Result<bool> {
        let room = self.room(room_id)?;
        let user = room.own_user_id().to_owned();
        Ok(room
            .power_levels()
            .await
            .map(|pl| pl.user_can_send_state(&user, StateEventType::from(SOUNDS)))
            .unwrap_or(false))
    }

    /// Upload a sound file (unencrypted, like room photos); returns its `mxc://`.
    pub async fn upload_sound(&self, bytes: Vec<u8>, mime: &str) -> Result<String> {
        if !mime.starts_with("audio/") || bytes.len() > 2 * 1024 * 1024 {
            return Err(Error::InvalidMedia(mime.to_owned()));
        }
        self.upload_wallpaper(bytes, mime).await
    }
}

#[cfg(test)]
mod tests {
    use super::Sound;

    fn sound(name: &str, url: &str, ms: u32) -> Sound {
        Sound {
            id: "a".into(),
            name: name.into(),
            emoji: Some("🎺".into()),
            url: url.into(),
            mime: "audio/wav".into(),
            duration_ms: ms,
            by: None,
        }
    }

    #[test]
    fn cleaning() {
        let items = vec![
            sound("  Trompette  ", "mxc://x/1", 2000),
            sound("too long", "mxc://x/2", 60_000),
            sound("bad url", "https://x/3", 1000),
            sound("   ", "mxc://x/4", 1000),
        ];
        let clean = super::clean(items);
        assert_eq!(clean.len(), 1);
        assert_eq!(clean[0].name, "Trompette");
    }
}
