//! Custom emoji and sticker packs (MSC2545, the format FluffyChat, Cinny and Nheko share):
//! - our own pack: global account data `im.ponies.user_emotes`;
//! - a chat's packs: room state `im.ponies.room_emotes` (one per state key);
//! - packs of other chats we turned on everywhere: account data `im.ponies.emote_rooms`.
//!
//! An emoji is sent as `<img data-mx-emoticon src="mxc://…" alt=":code:">` in the message's
//! HTML (`:code:` stays in the plain body); a sticker as an `m.sticker` event.

use std::collections::BTreeMap;

use matrix_sdk::{
    Room,
    ruma::{
        OwnedRoomId,
        api::client::state::get_state_events,
        events::{
            AnyMessageLikeEventContent, GlobalAccountDataEventType,
            room::message::{FormattedBody, MessageType, RoomMessageEventContentWithoutRelation},
        },
        serde::Raw,
    },
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tracing::warn;

use crate::{Error, Result};

const USER_EMOTES: &str = "im.ponies.user_emotes";
const ROOM_EMOTES: &str = "im.ponies.room_emotes";
const EMOTE_ROOMS: &str = "im.ponies.emote_rooms";
/// Height of a custom emoji in the HTML we send (other clients size it from this).
const EMOTE_HEIGHT: u32 = 32;

/// One pack, ready for the pickers.
#[derive(Debug, Clone, Serialize)]
pub struct EmotePack {
    /// "user", or "room|<room id>|<state key>".
    pub id: String,
    pub name: String,
    pub avatar: Option<String>,
    pub emotes: Vec<Emote>,
    /// Our own pack (we can add and remove).
    pub editable: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Emote {
    pub shortcode: String,
    /// `mxc://` URI.
    pub url: String,
    pub body: Option<String>,
    /// Usable inline in text.
    pub emoticon: bool,
    /// Usable as a sticker.
    pub sticker: bool,
    /// The image's `info` (w, h, mimetype, size), passed on with stickers.
    pub info: Option<Value>,
}

/// An emoji used in a message being sent (from the UI's picker).
#[derive(Debug, Clone, Deserialize)]
pub struct EmoteInput {
    pub shortcode: String,
    pub url: String,
}

#[derive(Deserialize, Default)]
struct PackContent {
    #[serde(default)]
    images: BTreeMap<String, ImageContent>,
    #[serde(default)]
    pack: PackMeta,
}

#[derive(Deserialize, Default)]
struct PackMeta {
    display_name: Option<String>,
    avatar_url: Option<String>,
    usage: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct ImageContent {
    url: String,
    body: Option<String>,
    info: Option<Value>,
    usage: Option<Vec<String>>,
}

fn parse_pack(
    id: String,
    fallback_name: &str,
    content: Value,
    editable: bool,
) -> Option<EmotePack> {
    let pack: PackContent = serde_json::from_value(content).ok()?;
    let emotes: Vec<Emote> = pack
        .images
        .into_iter()
        .filter(|(code, image)| !code.is_empty() && image.url.starts_with("mxc://"))
        .map(|(shortcode, image)| {
            // The image's usage wins over the pack's; neither means both.
            let usage = image
                .usage
                .or_else(|| pack.pack.usage.clone())
                .unwrap_or_default();
            let any = usage.is_empty();
            Emote {
                emoticon: any || usage.iter().any(|u| u == "emoticon"),
                sticker: any || usage.iter().any(|u| u == "sticker"),
                shortcode,
                url: image.url,
                body: image.body,
                info: image.info,
            }
        })
        .collect();
    (!emotes.is_empty() || editable).then(|| EmotePack {
        id,
        name: pack
            .pack
            .display_name
            .filter(|n| !n.trim().is_empty())
            .unwrap_or_else(|| fallback_name.to_owned()),
        avatar: pack.pack.avatar_url.filter(|a| a.starts_with("mxc://")),
        emotes,
        editable,
    })
}

impl crate::Core {
    /// Our pack first, then the open chat's packs, then the packs turned on everywhere.
    pub async fn emote_packs(&self, room_id: Option<&str>) -> Result<Vec<EmotePack>> {
        let client = self.client()?;
        let _account = client.account();
        let mut packs = Vec::new();

        let own = crate::offline::read_global(&client, USER_EMOTES)
            .await
            .ok()
            .flatten()
            .unwrap_or_else(|| json!({}));
        packs.extend(parse_pack("user".into(), "My emoji", own, true));

        let mut seen = std::collections::HashSet::new();
        if let Some(room) = room_id.and_then(|id| self.room(id).ok()) {
            for (key, content) in room_packs(&room).await {
                seen.insert((room.room_id().to_owned(), key.clone()));
                let name = room
                    .cached_display_name()
                    .map(|n| n.to_string())
                    .unwrap_or_default();
                packs.extend(parse_pack(
                    format!("room|{}|{key}", room.room_id()),
                    &name,
                    content,
                    false,
                ));
            }
        }

        // Packs from other chats, turned on everywhere ({"rooms": {room: {state key: {}}}}).
        let enabled = crate::offline::read_global(&client, EMOTE_ROOMS)
            .await
            .ok()
            .flatten();
        if let Some(rooms) = enabled.as_ref().and_then(|v| v["rooms"].as_object()) {
            for (room_id, keys) in rooms {
                let Ok(room_id) = OwnedRoomId::try_from(room_id.as_str()) else {
                    continue;
                };
                let Some(_room) = client.get_room(&room_id) else {
                    continue;
                };
                for key in keys
                    .as_object()
                    .map(|k| k.keys().cloned().collect::<Vec<_>>())
                    .unwrap_or_default()
                {
                    if !seen.insert((room_id.clone(), key.clone())) {
                        continue;
                    }
                    let Some(room) = client.get_room(&room_id) else {
                        continue;
                    };
                    let Ok(Some(content)) =
                        crate::offline::read_state(&room, ROOM_EMOTES, &key).await
                    else {
                        continue;
                    };
                    let name = room
                        .cached_display_name()
                        .map(|n| n.to_string())
                        .unwrap_or_default();
                    packs.extend(parse_pack(
                        format!("room|{room_id}|{key}"),
                        &name,
                        content,
                        false,
                    ));
                }
            }
        }
        Ok(packs)
    }

    /// Send a sticker from a pack.
    pub async fn send_sticker(
        &self,
        room_id: &str,
        url: &str,
        body: &str,
        info: Option<Value>,
    ) -> Result<()> {
        if !url.starts_with("mxc://") {
            return Err(Error::InvalidMedia(url.to_owned()));
        }
        let timeline = self.timeline(room_id)?;
        let content =
            json!({ "body": body, "url": url, "info": info.unwrap_or_else(|| json!({})) });
        let content = Raw::new(&content)?.cast_unchecked();
        let content = AnyMessageLikeEventContent::Sticker(
            content
                .deserialize()
                .map_err(|e| Error::InvalidMedia(e.to_string()))?,
        );
        timeline
            .send(content)
            .await
            .map_err(|e| Error::Timeline(e.to_string()))?;
        Ok(())
    }

    /// Add an emoji/sticker to our own pack (the image already uploaded: `mxc://`).
    pub async fn add_own_emote(
        &self,
        shortcode: &str,
        url: &str,
        info: Option<Value>,
        emoticon: bool,
        sticker: bool,
    ) -> Result<()> {
        let code =
            clean_shortcode(shortcode).ok_or_else(|| Error::InvalidMedia("shortcode".into()))?;
        let mut usage = Vec::new();
        if emoticon {
            usage.push("emoticon");
        }
        if sticker {
            usage.push("sticker");
        }
        self.change_own_pack(|pack| {
            pack["images"][&code] = json!({ "url": url, "body": code, "info": info.clone().unwrap_or_else(|| json!({})), "usage": usage });
        })
        .await
    }

    pub async fn remove_own_emote(&self, shortcode: &str) -> Result<()> {
        let code = shortcode.to_owned();
        self.change_own_pack(|pack| {
            if let Some(images) = pack["images"].as_object_mut() {
                images.remove(&code);
            }
        })
        .await
    }

    /// Re-read our pack from the server, change it, write it back (another device's change
    /// since the picker opened isn't lost).
    async fn change_own_pack(&self, change: impl FnOnce(&mut Value)) -> Result<()> {
        let _account = self.client()?.account();
        let _kind = GlobalAccountDataEventType::from(USER_EMOTES);
        let mut pack = crate::offline::read_global(&self.client()?, USER_EMOTES)
            .await?
            .filter(Value::is_object)
            .unwrap_or_else(|| json!({}));
        if !pack["images"].is_object() {
            pack["images"] = json!({});
        }
        change(&mut pack);
        crate::offline::write_global(&self.client()?, USER_EMOTES, pack).await
    }
}

/// The chat's own packs (`im.ponies.room_emotes`, any state key), from the server: sliding sync
/// doesn't keep custom state types in the store.
async fn room_packs(room: &Room) -> Vec<(String, Value)> {
    let request = get_state_events::v3::Request::new(room.room_id().to_owned());
    let remembered = format!("emote-packs|{}", room.room_id());
    match room.client().send(request).await {
        Ok(response) => {
            let packs: Vec<(String, Value)> = response
                .room_state
                .iter()
                .filter(|raw| {
                    raw.get_field::<String>("type").ok().flatten().as_deref() == Some(ROOM_EMOTES)
                })
                .filter_map(|raw| {
                    let key = raw.get_field::<String>("state_key").ok().flatten()?;
                    let content = raw.get_field::<Value>("content").ok().flatten()?;
                    Some((key, content))
                })
                .collect();
            crate::offline::remember_value(&remembered, json!(packs));
            packs
        }
        Err(e) => {
            // Offline: the packs we saw last time.
            warn!("emote packs of {}: {e}", room.room_id());
            crate::offline::last_seen(&remembered)
                .and_then(|v| serde_json::from_value(v).ok())
                .unwrap_or_default()
        }
    }
}

/// Letters, digits, `_` and `-` only (what every client parses between colons).
fn clean_shortcode(code: &str) -> Option<String> {
    let code = code.trim().trim_matches(':');
    (!code.is_empty()
        && code.len() <= 64
        && code
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-'))
    .then(|| code.to_owned())
}

/// Turn `:code:` into custom emoji images in the message's HTML (the plain body keeps the
/// text). Only the emoji the UI says were picked, and never inside code.
pub(crate) fn with_emotes(
    content: RoomMessageEventContentWithoutRelation,
    emotes: &[EmoteInput],
) -> RoomMessageEventContentWithoutRelation {
    let MessageType::Text(mut text) = content.msgtype.clone() else {
        return content;
    };
    let used: Vec<&EmoteInput> = emotes
        .iter()
        .filter(|e| e.url.starts_with("mxc://") && clean_shortcode(&e.shortcode).is_some())
        .filter(|e| text.body.contains(&format!(":{}:", e.shortcode)))
        .collect();
    if used.is_empty() {
        return content;
    }
    let mut html = match &text.formatted {
        Some(f) => f.body.clone(),
        None => crate::html::escape_text(&text.body),
    };
    for e in used {
        let code = format!(":{}:", e.shortcode);
        let img = format!(
            r#"<img data-mx-emoticon src="{}" alt="{code}" title="{code}" height="{EMOTE_HEIGHT}">"#,
            e.url.replace('"', "")
        );
        html = replace_outside_code(&html, &code, &img);
    }
    text.formatted = Some(FormattedBody::html(html));
    let mut out = content;
    out.msgtype = MessageType::Text(text);
    out
}

/// Replace `what` in HTML, except inside <code>/<pre> blocks.
fn replace_outside_code(html: &str, what: &str, with: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    let mut depth = 0usize;
    while !rest.is_empty() {
        let next_tag = rest.find('<').unwrap_or(rest.len());
        let (text, after) = rest.split_at(next_tag);
        out.push_str(&if depth == 0 {
            text.replace(what, with)
        } else {
            text.to_owned()
        });
        if after.is_empty() {
            break;
        }
        let end = after.find('>').map(|i| i + 1).unwrap_or(after.len());
        let tag = &after[..end];
        let name = tag
            .trim_start_matches("</")
            .trim_start_matches('<')
            .split(|c: char| !c.is_alphanumeric())
            .next()
            .unwrap_or("");
        if name.eq_ignore_ascii_case("code") || name.eq_ignore_ascii_case("pre") {
            if tag.starts_with("</") {
                depth = depth.saturating_sub(1);
            } else {
                depth += 1;
            }
        }
        out.push_str(tag);
        rest = &after[end..];
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(code: &str) -> EmoteInput {
        EmoteInput {
            shortcode: code.into(),
            url: format!("mxc://x.org/{code}"),
        }
    }

    #[test]
    fn plain_text_gets_html_with_images() {
        let content = RoomMessageEventContentWithoutRelation::text_plain("hi :blob: & :blob:");
        let out = with_emotes(content, &[input("blob"), input("unused")]);
        let MessageType::Text(text) = out.msgtype else {
            panic!()
        };
        assert_eq!(text.body, "hi :blob: & :blob:");
        let html = text.formatted.unwrap().body;
        assert_eq!(html.matches("data-mx-emoticon").count(), 2, "{html}");
        assert!(html.contains("&amp;"), "{html}");
        assert!(!html.contains("unused"), "{html}");
    }

    #[test]
    fn not_inside_code() {
        let content = RoomMessageEventContentWithoutRelation::text_markdown("`:blob:` and :blob:");
        let out = with_emotes(content, &[input("blob")]);
        let MessageType::Text(text) = out.msgtype else {
            panic!()
        };
        let html = text.formatted.unwrap().body;
        assert_eq!(html.matches("data-mx-emoticon").count(), 1, "{html}");
        assert!(html.contains("<code>:blob:</code>"), "{html}");
    }

    #[test]
    fn packs_and_usage() {
        let content = json!({
            "pack": { "display_name": "Cats", "usage": ["emoticon"] },
            "images": {
                "cat": { "url": "mxc://x/cat" },
                "big_cat": { "url": "mxc://x/big", "usage": ["sticker"] },
                "bad": { "url": "https://x/bad" }
            }
        });
        let pack = parse_pack("room|!r|".into(), "Room", content, false).unwrap();
        assert_eq!(pack.name, "Cats");
        assert_eq!(pack.emotes.len(), 2);
        let cat = pack.emotes.iter().find(|e| e.shortcode == "cat").unwrap();
        assert!(cat.emoticon && !cat.sticker);
        let big = pack
            .emotes
            .iter()
            .find(|e| e.shortcode == "big_cat")
            .unwrap();
        assert!(big.sticker && !big.emoticon);
    }

    #[test]
    fn shortcodes() {
        assert_eq!(
            clean_shortcode(":party_parrot:").as_deref(),
            Some("party_parrot")
        );
        assert_eq!(clean_shortcode("a b"), None);
        assert_eq!(clean_shortcode("<img>"), None);
    }
}
