//! Per-chat customisation shared by everyone in the chat (Messenger-style), kept in room state:
//! - `com.alwy.chat.room.quick_reaction`: `{ "emoji": "🔥" }`, sent by the composer's quick button and
//!   on double-tap;
//! - `com.alwy.chat.room.theme`: `{ "preset": "ocean" }`, colours of the chat (presets live in the UI);
//! - `com.alwy.chat.room.nicknames`: `{ "nicknames": { "@user:server": "Nick" } }`, names others gave
//!   someone. Your own nickname is your native per-room display name instead (`m.room.member`),
//!   so Element and FluffyChat show it too.
//!
//! Other clients ignore these events. Room state isn't end-to-end encrypted (see architecture.md).

use std::collections::BTreeMap;

use matrix_sdk::{
    Room,
    event_handler::EventHandlerDropGuard,
    ruma::{
        OwnedUserId,
        api::client::state::get_state_event_for_key,
        events::{AnySyncStateEvent, StateEventType},
        serde::Raw,
    },
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use tracing::warn;

use crate::{Error, Result};

pub const QUICK_REACTION: &str = "com.alwy.chat.room.quick_reaction";
pub const THEME: &str = "com.alwy.chat.room.theme";
pub const NICKNAMES: &str = "com.alwy.chat.room.nicknames";

/// Everything the chat view needs to look the way its members chose.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ChatStyle {
    pub quick_reaction: Option<String>,
    /// A theme preset id (see `app/src/lib/themes.ts`); unknown ids fall back to the default.
    pub theme: Option<String>,
    /// User ID → nickname given in this chat.
    pub nicknames: BTreeMap<String, String>,
    /// `mxc://` background image, shown instead of the theme's scenery.
    pub wallpaper: Option<String>,
    /// Whether our power level allows changing these.
    pub can_edit: bool,
}

#[derive(Deserialize)]
struct QuickReactionContent {
    emoji: Option<String>,
}

#[derive(Deserialize)]
struct ThemeContent {
    preset: Option<String>,
    /// `mxc://` background image chosen for the chat (over the preset's scenery).
    #[serde(default)]
    wallpaper: Option<String>,
}

#[derive(Default, Deserialize)]
struct NicknamesContent {
    #[serde(default)]
    nicknames: BTreeMap<String, String>,
}

impl crate::Core {
    /// Send `on_style` the chat's style now and after every change, until the returned guard drops.
    /// Read from the server: sliding sync doesn't fetch custom state types into the store.
    pub(crate) async fn watch_chat_style(
        &self,
        room: &Room,
        on_style: impl Fn(ChatStyle) + Send + Sync + 'static,
    ) -> EventHandlerDropGuard {
        let on_style = std::sync::Arc::new(on_style);
        let first = room.clone();
        let send = on_style.clone();
        tokio::spawn(async move { send(chat_style(&first).await) });

        let client = room.client();
        let handle = room.add_event_handler(move |event: Raw<AnySyncStateEvent>, room: Room| {
            let on_style = on_style.clone();
            async move {
                let kind = event.get_field::<String>("type").ok().flatten();
                if matches!(kind.as_deref(), Some(QUICK_REACTION | THEME | NICKNAMES))
                    || kind.as_deref() == Some("m.room.power_levels")
                {
                    on_style(chat_style(&room).await);
                }
            }
        });
        client.event_handler_drop_guard(handle)
    }

    pub async fn set_quick_reaction(&self, room_id: &str, emoji: Option<String>) -> Result<()> {
        let room = self.room(room_id)?;
        room.send_state_event_raw(QUICK_REACTION, "", json!({ "emoji": emoji }))
            .await?;
        Ok(())
    }

    /// The chat's theme for everyone: a preset, and optionally a background image (`mxc://`).
    pub async fn set_theme(
        &self,
        room_id: &str,
        preset: Option<String>,
        wallpaper: Option<String>,
    ) -> Result<()> {
        let room = self.room(room_id)?;
        let mut content = json!({ "preset": preset });
        if let Some(w) = wallpaper.filter(|w| w.starts_with("mxc://")) {
            content["wallpaper"] = json!(w);
        }
        room.send_state_event_raw(THEME, "", content).await?;
        Ok(())
    }

    /// Upload a chat background image (already resized by the UI); returns its `mxc://` URI.
    /// Like room avatars it isn't encrypted (state events can't point at encrypted media).
    pub async fn upload_wallpaper(&self, bytes: Vec<u8>, mime: &str) -> Result<String> {
        let mime = mime
            .parse()
            .map_err(|_| Error::InvalidMedia(mime.to_owned()))?;
        let response = self.client()?.media().upload(&mime, bytes, None).await?;
        Ok(response.content_uri.to_string())
    }

    /// Give `user_id` a nickname in this chat (`None` removes it).
    pub async fn set_nickname(
        &self,
        room_id: &str,
        user_id: &str,
        nickname: Option<String>,
    ) -> Result<()> {
        let room = self.room(room_id)?;
        let user = OwnedUserId::try_from(user_id).map_err(|_| Error::UnknownUser)?;
        let nickname = nickname
            .map(|n| n.trim().to_owned())
            .filter(|n| !n.is_empty());

        if room.own_user_id() == user {
            return set_own_room_name(&room, nickname).await;
        }
        // Re-read right before writing so a nickname someone else just set isn't lost.
        let mut content: NicknamesContent = fetch(&room, NICKNAMES, "").await?.unwrap_or_default();
        match nickname {
            Some(n) => content.nicknames.insert(user.to_string(), n),
            None => content.nicknames.remove(user.as_str()),
        };
        room.send_state_event_raw(NICKNAMES, "", json!({ "nicknames": content.nicknames }))
            .await?;
        Ok(())
    }
}

async fn chat_style(room: &Room) -> ChatStyle {
    let (quick, theme, nicknames) = tokio::join!(
        fetch::<QuickReactionContent>(room, QUICK_REACTION, ""),
        fetch::<ThemeContent>(room, THEME, ""),
        fetch::<NicknamesContent>(room, NICKNAMES, ""),
    );
    let log = |what: &str, e: &Error| warn!("couldn't read the chat's {what}: {e}");
    let can_edit = match room.power_levels().await {
        Ok(levels) => levels.user_can_send_state(room.own_user_id(), THEME.into()),
        Err(_) => false,
    };
    ChatStyle {
        quick_reaction: quick
            .inspect_err(|e| log("quick reaction", e))
            .ok()
            .flatten()
            .and_then(|c| c.emoji)
            .filter(|e| !e.is_empty()),
        wallpaper: theme
            .as_ref()
            .ok()
            .and_then(|t| t.as_ref())
            .and_then(|c| c.wallpaper.clone())
            .filter(|w| w.starts_with("mxc://")),
        theme: theme
            .inspect_err(|e| log("theme", e))
            .ok()
            .flatten()
            .and_then(|c| c.preset),
        nicknames: nicknames
            .inspect_err(|e| log("nicknames", e))
            .ok()
            .flatten()
            .map(|c| c.nicknames)
            .unwrap_or_default(),
        can_edit,
    }
}

/// A state event's content from the server; `None` if it was never set.
pub(crate) async fn fetch<T: DeserializeOwned>(
    room: &Room,
    event_type: &str,
    key: &str,
) -> Result<Option<T>> {
    let request = get_state_event_for_key::v3::Request::new(
        room.room_id().to_owned(),
        StateEventType::from(event_type),
        key.to_owned(),
    );
    match room.client().send(request).await {
        Ok(response) => Ok(serde_json::from_str(response.event_or_content.get()).ok()),
        Err(e) if is_not_found(&e) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

fn is_not_found(e: &matrix_sdk::HttpError) -> bool {
    matches!(
        e.client_api_error_kind(),
        Some(matrix_sdk::ruma::api::error::ErrorKind::NotFound)
    )
}

/// Our own nickname in a chat = our per-room display name (`None`: back to the global one).
async fn set_own_room_name(room: &Room, nickname: Option<String>) -> Result<()> {
    let me = room.own_user_id().to_owned();
    let mut member: serde_json::Map<String, Value> = fetch(room, "m.room.member", me.as_str())
        .await?
        .ok_or(Error::UnknownUser)?;
    let name = match nickname {
        Some(n) => Some(n),
        None => room.client().account().get_display_name().await?,
    };
    match name {
        Some(n) => member.insert("displayname".into(), n.into()),
        None => member.remove("displayname"),
    };
    room.send_state_event_raw("m.room.member", me.as_str(), Value::Object(member))
        .await?;
    Ok(())
}
