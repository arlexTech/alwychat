//! Managing chats, spaces and the account: invites, new chats/groups/spaces, leaving, members
//! and roles, room settings (name, topic, photo, who can join, history, encryption,
//! notifications, permissions), a space's rooms, our profile, devices, blocked users and
//! notification rules.

use std::collections::BTreeMap;

use matrix_sdk::{
    Room, RoomMemberships, RoomState,
    notification_settings::RoomNotificationMode,
    ruma::{
        OwnedRoomId, OwnedRoomOrAliasId, OwnedServerName, OwnedUserId,
        api::client::{
            discovery::get_authorization_server_metadata::v1::{
                AccountManagementActionData, DeviceDeleteData,
            },
            room::create_room,
            state::get_state_event_for_key,
        },
        events::{
            StateEventType,
            ignored_user_list::IgnoredUserListEventContent,
            room::{
                history_visibility::HistoryVisibility,
                join_rules::{AllowRule, JoinRule, Restricted},
                power_levels::UserPowerLevel,
            },
        },
        push::{PredefinedOverrideRuleId, RuleKind},
        serde::Raw,
    },
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tracing::warn;

use crate::{Error, Result};

/// Room-version-12 creators have no number: shown above admins.
const CREATOR: i64 = 150;

#[derive(Debug, Clone, Serialize)]
pub struct UserResult {
    pub user_id: String,
    pub name: String,
    pub avatar: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InviteInfo {
    pub inviter_id: String,
    pub inviter_name: Option<String>,
    pub inviter_avatar: Option<String>,
    pub is_space: bool,
    pub topic: Option<String>,
    pub members: u64,
}

/// What to create (`kind` "group" or "space").
#[derive(Debug, Clone, Deserialize)]
pub struct NewRoom {
    pub kind: String,
    pub name: String,
    #[serde(default)]
    pub topic: String,
    /// "private" (invite only), "space" (members of `parent`) or "public" (anyone with the link).
    pub access: String,
    pub encrypted: bool,
    pub parent: Option<String>,
    #[serde(default)]
    pub invite: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MemberInfo {
    pub user_id: String,
    pub name: String,
    pub avatar: Option<String>,
    pub is_me: bool,
    pub power: i64,
    /// "join", "invite", "ban" or "knock".
    pub membership: String,
}

/// A chat's or space's settings, and what we may change.
#[derive(Debug, Clone, Serialize)]
pub struct RoomDetails {
    pub id: String,
    pub name: String,
    pub topic: Option<String>,
    pub avatar: Option<String>,
    pub alias: Option<String>,
    pub is_space: bool,
    pub is_dm: bool,
    pub encrypted: bool,
    /// "invite", "restricted" (members of `allow_spaces`), "public", "knock" or other.
    pub join_rule: String,
    pub allow_spaces: Vec<String>,
    /// "shared", "invited", "joined" or "world_readable".
    pub history: String,
    pub members: u64,
    /// "all", "mentions" or "mute"; `notify_custom` false = the account default.
    pub notify: String,
    pub notify_custom: bool,
    pub my_power: i64,
    /// Levels needed for each permission (see `PERMISSIONS`).
    pub levels: BTreeMap<String, i64>,
    /// A space we're in that this chat is listed in (for "Space members" access).
    pub parent_spaces: Vec<String>,
}

/// The permissions shown on the roles page: a power-levels key, or an event type.
pub const PERMISSIONS: [&str; 17] = [
    "users_default",
    "events_default",
    "invite",
    "kick",
    "ban",
    "redact",
    "m.room.name",
    "m.room.avatar",
    "m.room.topic",
    "m.room.pinned_events",
    "m.room.join_rules",
    "m.room.history_visibility",
    "m.room.encryption",
    "m.room.power_levels",
    "m.space.child",
    "org.matrix.msc3401.call.member",
    "state_default",
];

#[derive(Debug, Clone, Serialize)]
pub struct DeviceInfo {
    pub device_id: String,
    pub name: Option<String>,
    pub last_seen_ip: Option<String>,
    pub last_seen_ts: Option<u64>,
    pub is_current: bool,
    /// Cross-signed by us (`None`: no keys known for it).
    pub verified: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SecurityStatus {
    /// "enabled", "disabled", "incomplete" or "unknown".
    pub recovery: String,
    /// "enabled", "disabled", "creating", "downloading"… (key backup).
    pub backup: String,
    pub cross_signed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationRules {
    pub keywords: Vec<String>,
    pub room_mentions: bool,
    pub invites: bool,
}

fn level(power: UserPowerLevel) -> i64 {
    match power {
        UserPowerLevel::Infinite => CREATOR,
        UserPowerLevel::Int(i) => i.into(),
        _ => 0,
    }
}

fn membership(m: &matrix_sdk::room::RoomMember) -> String {
    m.membership().as_str().to_owned()
}

/// `#alias:server`, `!id:server`, `@user:server` or a matrix.to link → the target and its
/// `via` servers.
fn parse_address(input: &str) -> Option<(String, Vec<String>)> {
    let input = input.trim();
    let rest = input
        .strip_prefix("https://matrix.to/#/")
        .or_else(|| input.strip_prefix("matrix.to/#/"))
        .unwrap_or(input);
    let (target, query) = rest.split_once('?').unwrap_or((rest, ""));
    // An event link: the room is the first part.
    let target = target.split('/').next().unwrap_or(target);
    let target = urlencoding_decode(target);
    let via = query
        .split('&')
        .filter_map(|p| p.strip_prefix("via="))
        .map(urlencoding_decode)
        .collect();
    matches!(target.chars().next(), Some('#' | '!' | '@'))
        .then_some(())
        .filter(|_| target.contains(':'))
        .map(|_| (target, via))
}

fn urlencoding_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Some(Ok(b)) = s.get(i + 1..i + 3).map(|h| u8::from_str_radix(h, 16))
        {
            out.push(b);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn user_id(id: &str) -> Result<OwnedUserId> {
    OwnedUserId::try_from(id.trim()).map_err(|_| Error::UnknownUser)
}

impl crate::Core {
    // ---- Invites, new chats, joining and leaving --------------------------------------------

    pub async fn invite_info(&self, room_id: &str) -> Result<InviteInfo> {
        let room = self.room(room_id)?;
        let details = room.invite_details().await?;
        Ok(InviteInfo {
            inviter_id: details.inviter_id.to_string(),
            inviter_name: details
                .inviter
                .as_ref()
                .and_then(|m| m.display_name().map(ToOwned::to_owned)),
            inviter_avatar: details
                .inviter
                .and_then(|m| m.avatar_url().map(ToString::to_string)),
            is_space: room.is_space(),
            topic: room.topic(),
            members: room.joined_members_count(),
        })
    }

    pub async fn accept_invite(&self, room_id: &str) -> Result<()> {
        self.room(room_id)?.join().await?;
        Ok(())
    }

    /// Decline an invite, or leave a chat or space.
    pub async fn leave_room(&self, room_id: &str) -> Result<()> {
        let room = self.room(room_id)?;
        let invited = room.state() == RoomState::Invited;
        room.leave().await?;
        // A declined invite shouldn't linger in "left rooms" either.
        if invited && let Err(e) = room.forget().await {
            warn!("forget declined invite: {e}");
        }
        Ok(())
    }

    /// The user directory (people sharing a room with us, or everyone, as the server allows).
    pub async fn search_users(&self, term: &str) -> Result<Vec<UserResult>> {
        let client = self.client()?;
        let term = term.trim();
        if term.is_empty() {
            return Ok(Vec::new());
        }
        let mut out: Vec<UserResult> = client
            .search_users(term, 20)
            .await?
            .results
            .into_iter()
            .map(|u| UserResult {
                user_id: u.user_id.to_string(),
                name: u
                    .display_name
                    .unwrap_or_else(|| u.user_id.localpart().to_owned()),
                avatar: u.avatar_url.map(|a| a.to_string()),
            })
            .collect();
        // A full user ID the directory doesn't know (another server): offer it as is.
        if let Ok(id) = OwnedUserId::try_from(term)
            && !out.iter().any(|u| u.user_id == id.as_str())
        {
            let profile = client.account().fetch_user_profile_of(&id).await.ok();
            out.insert(
                0,
                UserResult {
                    name: profile
                        .as_ref()
                        .and_then(|p| p.get("displayname"))
                        .and_then(|v| v.as_str())
                        .map(ToOwned::to_owned)
                        .unwrap_or_else(|| id.localpart().to_owned()),
                    avatar: profile
                        .as_ref()
                        .and_then(|p| p.get("avatar_url"))
                        .and_then(|v| v.as_str())
                        .map(ToOwned::to_owned),
                    user_id: id.to_string(),
                },
            );
        }
        Ok(out)
    }

    /// Our 1:1 chat with someone, created (encrypted) if there's none yet.
    pub async fn open_dm(&self, user: &str) -> Result<String> {
        let client = self.client()?;
        let user = user_id(user)?;
        if let Some(room) = client.get_dm_room(&user) {
            return Ok(room.room_id().to_string());
        }
        Ok(client.create_dm(&user).await?.room_id().to_string())
    }

    pub async fn create_room(&self, new: NewRoom) -> Result<String> {
        let client = self.client()?;
        let server = client
            .user_id()
            .ok_or(Error::NotLoggedIn)?
            .server_name()
            .to_owned();
        let name = new.name.trim();
        if name.is_empty() {
            return Err(Error::Timeline("a name is needed".into()));
        }
        let is_space = new.kind == "space";
        let parent = match &new.parent {
            Some(id) => Some(self.room(id)?),
            None => None,
        };

        let mut request = create_room::v3::Request::new();
        request.name = Some(name.to_owned());
        let topic = new.topic.trim();
        if !topic.is_empty() {
            request.topic = Some(topic.to_owned());
        }
        request.invite = new
            .invite
            .iter()
            .filter_map(|id| OwnedUserId::try_from(id.as_str()).ok())
            .collect();
        request.preset = Some(if new.access == "public" {
            create_room::v3::RoomPreset::PublicChat
        } else {
            create_room::v3::RoomPreset::PrivateChat
        });
        if is_space {
            request.creation_content =
                Some(Raw::new(&json!({ "type": "m.space" }))?.cast_unchecked());
            // Only admins post in a space's own timeline (as Element does).
            request.power_level_content_override =
                Some(Raw::new(&json!({ "events_default": 100, "invite": 0 }))?.cast_unchecked());
        }
        let mut state = Vec::new();
        if let Some(parent) = &parent {
            state.push(json!({
                "type": "m.space.parent",
                "state_key": parent.room_id(),
                "content": { "via": [server], "canonical": true },
            }));
        }
        match (new.access.as_str(), &parent) {
            ("space", Some(parent)) => state.push(json!({
                "type": "m.room.join_rules",
                "state_key": "",
                "content": {
                    "join_rule": "restricted",
                    "allow": [{ "type": "m.room_membership", "room_id": parent.room_id() }],
                },
            })),
            ("public", _) => {}
            _ => state.push(json!({
                "type": "m.room.join_rules",
                "state_key": "",
                "content": { "join_rule": "invite" },
            })),
        }
        if new.encrypted && !is_space {
            state.push(json!({
                "type": "m.room.encryption",
                "state_key": "",
                "content": { "algorithm": "m.megolm.v1.aes-sha2" },
            }));
        }
        request.initial_state = state
            .iter()
            .map(|s| Raw::new(s).map(Raw::cast_unchecked))
            .collect::<std::result::Result<_, _>>()?;

        let room = client.create_room(request).await?;
        if let Some(parent) = parent {
            parent
                .send_state_event_raw(
                    "m.space.child",
                    room.room_id().as_str(),
                    json!({ "via": [server] }),
                )
                .await?;
        }
        Ok(room.room_id().to_string())
    }

    /// Join by `#alias`, room ID or matrix.to link; a user address opens our chat with them.
    pub async fn join_address(&self, address: &str) -> Result<String> {
        let (target, via) = parse_address(address).ok_or(Error::UnknownRoom)?;
        if target.starts_with('@') {
            return self.open_dm(&target).await;
        }
        let id = OwnedRoomOrAliasId::try_from(target.as_str()).map_err(|_| Error::UnknownRoom)?;
        let via: Vec<OwnedServerName> = via
            .iter()
            .filter_map(|s| OwnedServerName::try_from(s.as_str()).ok())
            .collect();
        let room = self.client()?.join_room_by_id_or_alias(&id, &via).await?;
        Ok(room.room_id().to_string())
    }

    /// Joined chats listed in a space (to offer leaving them with it).
    pub async fn space_rooms_joined(&self, space_id: &str) -> Result<Vec<String>> {
        let space = self.room(space_id)?;
        let mut out = Vec::new();
        for raw in space.get_state_events(StateEventType::SpaceChild).await? {
            let Ok(value) = (match raw {
                matrix_sdk::deserialized_responses::RawAnySyncOrStrippedState::Sync(r) => {
                    r.deserialize_as::<Value>()
                }
                matrix_sdk::deserialized_responses::RawAnySyncOrStrippedState::Stripped(r) => {
                    r.deserialize_as::<Value>()
                }
            }) else {
                continue;
            };
            let has_via = value["content"]["via"]
                .as_array()
                .is_some_and(|v| !v.is_empty());
            let Some(id) = value["state_key"].as_str().filter(|_| has_via) else {
                continue;
            };
            if let Ok(id) = OwnedRoomId::try_from(id)
                && let Some(room) = self.client()?.get_room(&id)
                && room.state() == RoomState::Joined
            {
                out.push(id.to_string());
            }
        }
        Ok(out)
    }

    // ---- Members and roles -------------------------------------------------------------------

    /// Joined, invited and banned members with their power level; admins first.
    pub async fn members(&self, room_id: &str) -> Result<Vec<MemberInfo>> {
        let room = self.room(room_id)?;
        let own = room.own_user_id().to_owned();
        let mut out: Vec<MemberInfo> = room
            .members(
                RoomMemberships::JOIN
                    | RoomMemberships::INVITE
                    | RoomMemberships::BAN
                    | RoomMemberships::KNOCK,
            )
            .await?
            .into_iter()
            .map(|m| MemberInfo {
                is_me: m.user_id() == own,
                user_id: m.user_id().to_string(),
                name: m
                    .display_name()
                    .unwrap_or(m.user_id().localpart())
                    .to_owned(),
                avatar: m.avatar_url().map(ToString::to_string),
                power: level(m.power_level()),
                membership: membership(&m),
            })
            .collect();
        out.sort_by_cached_key(|m| (-m.power, m.name.to_lowercase()));
        Ok(out)
    }

    pub async fn invite_user(&self, room_id: &str, user: &str) -> Result<()> {
        self.room(room_id)?
            .invite_user_by_id(&user_id(user)?)
            .await?;
        Ok(())
    }

    /// "kick", "ban" or "unban".
    pub async fn moderate(
        &self,
        room_id: &str,
        user: &str,
        action: &str,
        reason: Option<&str>,
    ) -> Result<()> {
        let room = self.room(room_id)?;
        let user = user_id(user)?;
        let reason = reason.map(str::trim).filter(|r| !r.is_empty());
        match action {
            "kick" => room.kick_user(&user, reason).await?,
            "ban" => room.ban_user(&user, reason).await?,
            "unban" => room.unban_user(&user, reason).await?,
            _ => return Err(Error::Timeline(format!("unknown action {action}"))),
        }
        Ok(())
    }

    pub async fn set_user_power(&self, room_id: &str, user: &str, power: i64) -> Result<()> {
        let room = self.room(room_id)?;
        let user = user_id(user)?;
        let level = matrix_sdk::ruma::Int::new(power).ok_or(Error::UnknownUser)?;
        room.update_power_levels(vec![(&user, level)]).await?;
        Ok(())
    }

    /// Change the level needed for one of `PERMISSIONS`.
    pub async fn set_permission(&self, room_id: &str, key: &str, power: i64) -> Result<()> {
        if !PERMISSIONS.contains(&key) {
            return Err(Error::Timeline(format!("unknown permission {key}")));
        }
        let room = self.room(room_id)?;
        let mut content = self
            .state_content(&room, StateEventType::RoomPowerLevels, "")
            .await?
            .unwrap_or_else(|| json!({}));
        if key.contains('.') {
            content["events"][key] = json!(power);
        } else {
            content[key] = json!(power);
        }
        room.send_state_event_raw("m.room.power_levels", "", content)
            .await?;
        Ok(())
    }

    /// A state event's content, from the server (fresh, and not limited to what sliding sync
    /// keeps in the store).
    pub(crate) async fn state_content(
        &self,
        room: &Room,
        kind: StateEventType,
        key: &str,
    ) -> Result<Option<Value>> {
        let request = get_state_event_for_key::v3::Request::new(
            room.room_id().to_owned(),
            kind,
            key.to_owned(),
        );
        match room.client().send(request).await {
            Ok(response) => Ok(serde_json::from_str(response.event_or_content.get()).ok()),
            Err(e)
                if e.client_api_error_kind()
                    == Some(&matrix_sdk::ruma::api::error::ErrorKind::NotFound) =>
            {
                Ok(None)
            }
            Err(e) => Err(e.into()),
        }
    }

    // ---- Chat and space settings -------------------------------------------------------------

    pub async fn room_details(&self, room_id: &str) -> Result<RoomDetails> {
        let room = self.room(room_id)?;
        let client = room.client();
        let pl = self
            .state_content(&room, StateEventType::RoomPowerLevels, "")
            .await
            .unwrap_or_else(|e| {
                warn!("power levels: {e}");
                None
            })
            .unwrap_or_else(|| json!({}));
        let num = |v: &Value, default: i64| v.as_i64().unwrap_or(default);
        let state_default = num(&pl["state_default"], 50);
        let mut levels = BTreeMap::new();
        for key in PERMISSIONS {
            let value = if key.contains('.') {
                num(&pl["events"][key], state_default)
            } else {
                let default = match key {
                    "users_default" | "events_default" | "invite" => 0,
                    _ => 50,
                };
                num(&pl[key], default)
            };
            levels.insert(key.to_owned(), value);
        }
        let my_power = level(
            room.get_user_power_level(room.own_user_id())
                .await
                .unwrap_or(UserPowerLevel::Int(0u8.into())),
        );
        let (join_rule, allow_spaces) = match room.join_rule() {
            Some(JoinRule::Invite) => ("invite".to_owned(), Vec::new()),
            Some(JoinRule::Public) => ("public".to_owned(), Vec::new()),
            Some(JoinRule::Knock) => ("knock".to_owned(), Vec::new()),
            Some(JoinRule::Restricted(r) | JoinRule::KnockRestricted(r)) => (
                "restricted".to_owned(),
                r.allow
                    .iter()
                    .filter_map(|a| match a {
                        AllowRule::RoomMembership(m) => Some(m.room_id.to_string()),
                        _ => None,
                    })
                    .collect(),
            ),
            Some(other) => (other.as_str().to_owned(), Vec::new()),
            None => ("invite".to_owned(), Vec::new()),
        };
        let settings = client.notification_settings().await;
        let custom = settings
            .get_user_defined_room_notification_mode(room.room_id())
            .await;
        let mode = match custom {
            Some(mode) => mode,
            None => {
                settings
                    .get_default_room_notification_mode(
                        room.encryption_state().is_encrypted().into(),
                        (room.active_members_count() <= 2).into(),
                    )
                    .await
            }
        };
        let parent_spaces = room
            .get_state_events(StateEventType::SpaceParent)
            .await
            .unwrap_or_default()
            .into_iter()
            .filter_map(|raw| match raw {
                matrix_sdk::deserialized_responses::RawAnySyncOrStrippedState::Sync(r) => {
                    r.get_field::<String>("state_key").ok().flatten()
                }
                _ => None,
            })
            .collect();
        Ok(RoomDetails {
            id: room.room_id().to_string(),
            name: room
                .cached_display_name()
                .map(|n| n.to_string())
                .unwrap_or_else(|| room.room_id().to_string()),
            topic: room.topic(),
            avatar: room.avatar_url().map(|u| u.to_string()),
            alias: room.canonical_alias().map(|a| a.to_string()),
            is_space: room.is_space(),
            is_dm: room.is_direct().await.unwrap_or(false),
            encrypted: room.encryption_state().is_encrypted(),
            join_rule,
            allow_spaces,
            history: room
                .history_visibility()
                .map(|h| h.as_str().to_owned())
                .unwrap_or_else(|| "shared".to_owned()),
            members: room.joined_members_count(),
            notify: match mode {
                RoomNotificationMode::AllMessages => "all",
                RoomNotificationMode::MentionsAndKeywordsOnly => "mentions",
                RoomNotificationMode::Mute => "mute",
            }
            .to_owned(),
            notify_custom: custom.is_some(),
            my_power,
            levels,
            parent_spaces,
        })
    }

    pub async fn set_room_name(&self, room_id: &str, name: &str) -> Result<()> {
        self.room(room_id)?.set_name(name.trim().to_owned()).await?;
        Ok(())
    }

    pub async fn set_room_topic(&self, room_id: &str, topic: &str) -> Result<()> {
        self.room(room_id)?.set_room_topic(topic.trim()).await?;
        Ok(())
    }

    /// `image` None removes the photo.
    pub async fn set_room_avatar(
        &self,
        room_id: &str,
        image: Option<(Vec<u8>, String)>,
    ) -> Result<()> {
        let room = self.room(room_id)?;
        match image {
            Some((bytes, mime)) => {
                let mime = mime.parse().map_err(|_| Error::InvalidMedia(mime))?;
                room.upload_avatar(&mime, bytes, None).await?;
            }
            None => {
                room.remove_avatar().await?;
            }
        }
        Ok(())
    }

    /// `rule`: "invite", "public" or "restricted" (members of `spaces`).
    pub async fn set_join_rule(&self, room_id: &str, rule: &str, spaces: &[String]) -> Result<()> {
        let rule = match rule {
            "invite" => JoinRule::Invite,
            "public" => JoinRule::Public,
            "knock" => JoinRule::Knock,
            "restricted" => JoinRule::Restricted(Restricted::new(
                spaces
                    .iter()
                    .filter_map(|id| OwnedRoomId::try_from(id.as_str()).ok())
                    .map(AllowRule::room_membership)
                    .collect(),
            )),
            other => return Err(Error::Timeline(format!("unknown join rule {other}"))),
        };
        self.room(room_id)?
            .privacy_settings()
            .update_join_rule(rule)
            .await?;
        Ok(())
    }

    pub async fn set_history_visibility(&self, room_id: &str, history: &str) -> Result<()> {
        self.room(room_id)?
            .privacy_settings()
            .update_room_history_visibility(HistoryVisibility::from(history))
            .await?;
        Ok(())
    }

    pub async fn enable_room_encryption(&self, room_id: &str) -> Result<()> {
        self.room(room_id)?.enable_encryption().await?;
        Ok(())
    }

    /// `mode`: "all", "mentions", "mute", or "default" (the account's default for this kind
    /// of chat).
    pub async fn set_notify_mode(&self, room_id: &str, mode: &str) -> Result<()> {
        let room = self.room(room_id)?;
        let settings = room.client().notification_settings().await;
        let new = match mode {
            "all" => Some(RoomNotificationMode::AllMessages),
            "mentions" => Some(RoomNotificationMode::MentionsAndKeywordsOnly),
            "mute" => Some(RoomNotificationMode::Mute),
            _ => None,
        };
        let result = match new {
            Some(new) => {
                settings
                    .set_room_notification_mode(room.room_id(), new)
                    .await
            }
            None => {
                settings
                    .delete_user_defined_room_rules(room.room_id())
                    .await
            }
        };
        result.map_err(|e| Error::Sync(e.to_string()))?;
        match new {
            Some(new) => room.update_cached_user_defined_notification_mode(new),
            None => room.clear_user_defined_notification_mode(),
        }
        Ok(())
    }

    pub async fn report_room(&self, room_id: &str, reason: &str) -> Result<()> {
        self.room(room_id)?
            .report_room(reason.trim().to_owned())
            .await?;
        Ok(())
    }

    // ---- A space's rooms ---------------------------------------------------------------------

    /// List `room_id` in `space_id` (and point the room at the space when we may).
    pub async fn add_to_space(&self, space_id: &str, room_id: &str) -> Result<()> {
        let space = self.room(space_id)?;
        let room = self.room(room_id)?;
        let server = space.own_user_id().server_name().to_owned();
        space
            .send_state_event_raw("m.space.child", room_id, json!({ "via": [server] }))
            .await?;
        if let Err(e) = room
            .send_state_event_raw(
                "m.space.parent",
                space_id,
                json!({ "via": [server], "canonical": true }),
            )
            .await
        {
            warn!("space parent of {room_id}: {e}");
        }
        Ok(())
    }

    pub async fn remove_from_space(&self, space_id: &str, room_id: &str) -> Result<()> {
        let space = self.room(space_id)?;
        space
            .send_state_event_raw("m.space.child", room_id, json!({}))
            .await?;
        if let Ok(room) = self.room(room_id)
            && room.state() == RoomState::Joined
            && let Err(e) = room
                .send_state_event_raw("m.space.parent", space_id, json!({}))
                .await
        {
            warn!("space parent of {room_id}: {e}");
        }
        Ok(())
    }

    pub async fn set_suggested(
        &self,
        space_id: &str,
        room_id: &str,
        suggested: bool,
    ) -> Result<()> {
        let space = self.room(space_id)?;
        let server = space.own_user_id().server_name().to_owned();
        let mut content = self
            .state_content(&space, StateEventType::SpaceChild, room_id)
            .await?
            .filter(|c| c["via"].is_array())
            .unwrap_or_else(|| json!({ "via": [server] }));
        content["suggested"] = json!(suggested);
        space
            .send_state_event_raw("m.space.child", room_id, content)
            .await?;
        Ok(())
    }

    // ---- Our account -------------------------------------------------------------------------

    pub async fn own_profile(&self) -> Result<UserResult> {
        let client = self.client()?;
        let user = client.user_id().ok_or(Error::NotLoggedIn)?.to_owned();
        let profile = client.account().fetch_user_profile().await?;
        Ok(UserResult {
            name: profile
                .get("displayname")
                .and_then(|v| v.as_str())
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| user.localpart().to_owned()),
            avatar: profile
                .get("avatar_url")
                .and_then(|v| v.as_str())
                .map(ToOwned::to_owned),
            user_id: user.to_string(),
        })
    }

    pub async fn set_display_name(&self, name: &str) -> Result<()> {
        let name = name.trim();
        self.client()?
            .account()
            .set_display_name((!name.is_empty()).then_some(name))
            .await?;
        Ok(())
    }

    /// `image` None removes our photo.
    pub async fn set_own_avatar(&self, image: Option<(Vec<u8>, String)>) -> Result<()> {
        let account = self.client()?.account();
        match image {
            Some((bytes, mime)) => {
                let mime = mime.parse().map_err(|_| Error::InvalidMedia(mime))?;
                account.upload_avatar(&mime, bytes).await?;
            }
            None => account.set_avatar_url(None).await?,
        }
        Ok(())
    }

    /// Someone's profile (name, photo), for their profile page.
    pub async fn user_profile(&self, user: &str) -> Result<UserResult> {
        let client = self.client()?;
        let user = user_id(user)?;
        let profile = client.account().fetch_user_profile_of(&user).await?;
        Ok(UserResult {
            name: profile
                .get("displayname")
                .and_then(|v| v.as_str())
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| user.localpart().to_owned()),
            avatar: profile
                .get("avatar_url")
                .and_then(|v| v.as_str())
                .map(ToOwned::to_owned),
            user_id: user.to_string(),
        })
    }

    pub async fn devices(&self) -> Result<Vec<DeviceInfo>> {
        let client = self.client()?;
        let own = client.user_id().ok_or(Error::NotLoggedIn)?.to_owned();
        let current = client.device_id().map(ToOwned::to_owned);
        let mut out = Vec::new();
        for d in client.devices().await?.devices {
            let verified = client
                .encryption()
                .get_device(&own, &d.device_id)
                .await
                .ok()
                .flatten()
                .map(|dev| dev.is_cross_signed_by_owner());
            out.push(DeviceInfo {
                is_current: Some(&d.device_id) == current.as_ref(),
                device_id: d.device_id.to_string(),
                name: d.display_name,
                last_seen_ip: d.last_seen_ip,
                last_seen_ts: d.last_seen_ts.map(|t| t.get().into()),
                verified,
            });
        }
        out.sort_by_key(|d| (!d.is_current, std::cmp::Reverse(d.last_seen_ts)));
        Ok(out)
    }

    pub async fn rename_device(&self, device_id: &str, name: &str) -> Result<()> {
        self.client()?
            .rename_device(device_id.into(), name.trim())
            .await?;
        Ok(())
    }

    /// The account page (MAS) for `action`: "profile", "devices", "sign_out:<device id>",
    /// "deactivate" or "reset_identity". Sessions end there (OAuth accounts can't delete
    /// devices through the Matrix API).
    pub async fn account_url(&self, action: &str) -> Result<String> {
        let client = self.client()?;
        let metadata = client
            .oauth()
            .server_metadata()
            .await
            .map_err(|e| Error::OAuth(e.to_string()))?;
        let device;
        let action = match action.split_once(':') {
            Some(("sign_out", id)) => {
                device = matrix_sdk::ruma::OwnedDeviceId::from(id);
                AccountManagementActionData::DeviceDelete(DeviceDeleteData::new(&device))
            }
            _ => match action {
                "devices" => AccountManagementActionData::DevicesList,
                "deactivate" => AccountManagementActionData::AccountDeactivate,
                "reset_identity" => AccountManagementActionData::CrossSigningReset,
                _ => AccountManagementActionData::Profile,
            },
        };
        metadata
            .account_management_url_with_action(action)
            .map(|u| u.to_string())
            .ok_or_else(|| Error::OAuth("the server has no account page".into()))
    }

    pub async fn security_status(&self) -> Result<SecurityStatus> {
        let client = self.client()?;
        let encryption = client.encryption();
        let cross_signed = match encryption.get_own_device().await {
            Ok(Some(device)) => device.is_cross_signed_by_owner(),
            _ => false,
        };
        Ok(SecurityStatus {
            recovery: format!("{:?}", encryption.recovery().state()).to_lowercase(),
            backup: format!("{:?}", encryption.backups().state()).to_lowercase(),
            cross_signed,
        })
    }

    /// A new recovery key (set up recovery first if it's off). Shown once to the user.
    pub async fn new_recovery_key(&self) -> Result<String> {
        let recovery = self.client()?.encryption().recovery();
        let result = match recovery.state() {
            matrix_sdk::encryption::recovery::RecoveryState::Disabled => recovery.enable().await,
            _ => recovery.reset_key().await,
        };
        result.map_err(|e| Error::Encryption(e.to_string()))
    }

    pub async fn ignored_users(&self) -> Result<Vec<UserResult>> {
        let client = self.client()?;
        let list = client
            .account()
            .account_data::<IgnoredUserListEventContent>()
            .await?
            .and_then(|raw| raw.deserialize().ok())
            .map(|c| c.ignored_users.into_keys().collect::<Vec<_>>())
            .unwrap_or_default();
        let mut out = Vec::new();
        for user in list {
            out.push(
                self.user_profile(user.as_str())
                    .await
                    .unwrap_or_else(|_| UserResult {
                        name: user.localpart().to_owned(),
                        avatar: None,
                        user_id: user.to_string(),
                    }),
            );
        }
        Ok(out)
    }

    pub async fn set_ignored(&self, user: &str, ignored: bool) -> Result<()> {
        let account = self.client()?.account();
        let user = user_id(user)?;
        if ignored {
            account.ignore_user(&user).await?;
        } else {
            account.unignore_user(&user).await?;
        }
        Ok(())
    }

    pub async fn notification_rules(&self) -> Result<NotificationRules> {
        let settings = self.client()?.notification_settings().await;
        let enabled = |id: PredefinedOverrideRuleId| {
            let settings = &settings;
            async move {
                settings
                    .is_push_rule_enabled(RuleKind::Override, id.as_str())
                    .await
                    .unwrap_or(true)
            }
        };
        Ok(NotificationRules {
            keywords: settings.enabled_keywords().await.into_iter().collect(),
            room_mentions: enabled(PredefinedOverrideRuleId::IsRoomMention).await,
            invites: enabled(PredefinedOverrideRuleId::InviteForMe).await,
        })
    }

    pub async fn set_notification_rule(&self, rule: &str, enabled: bool) -> Result<()> {
        let settings = self.client()?.notification_settings().await;
        let ids: &[PredefinedOverrideRuleId] = match rule {
            "room_mentions" => &[PredefinedOverrideRuleId::IsRoomMention],
            "invites" => &[PredefinedOverrideRuleId::InviteForMe],
            other => return Err(Error::Push(format!("unknown rule {other}"))),
        };
        for id in ids {
            if let Err(e) = settings
                .set_push_rule_enabled(RuleKind::Override, id.as_str(), enabled)
                .await
            {
                warn!("push rule {}: {e}", id.as_str());
            }
        }
        Ok(())
    }

    pub async fn set_keyword(&self, keyword: &str, add: bool) -> Result<()> {
        let client = self.client()?;
        let settings = client.notification_settings().await;
        let keyword = keyword.trim();
        if add {
            return settings
                .add_keyword(keyword.to_owned())
                .await
                .map_err(|e| Error::Push(e.to_string()));
        }
        // The SDK only knows the rules of the last sync: a keyword added since would be
        // skipped. Its push rule (a content rule named after the keyword) goes directly.
        if let Err(e) = settings.remove_keyword(keyword).await {
            tracing::debug!("cached keyword rules: {e}");
        }
        let request = matrix_sdk::ruma::api::client::push::delete_pushrule::v3::Request::new(
            RuleKind::Content,
            keyword.to_owned(),
        );
        match client.send(request).await {
            Ok(_) => Ok(()),
            Err(e)
                if e.client_api_error_kind()
                    == Some(&matrix_sdk::ruma::api::error::ErrorKind::NotFound) =>
            {
                Ok(())
            }
            Err(e) => Err(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_addresses() {
        assert_eq!(
            parse_address("https://matrix.to/#/%23test:example.org?via=example.org"),
            Some(("#test:example.org".to_owned(), vec!["example.org".to_owned()]))
        );
        assert_eq!(
            parse_address("!abc:example.org").map(|a| a.0),
            Some("!abc:example.org".to_owned())
        );
        assert_eq!(
            parse_address("https://matrix.to/#/!abc:example.org/$event?via=a.b&via=c.d").map(|a| a.1),
            Some(vec!["a.b".to_owned(), "c.d".to_owned()])
        );
        assert_eq!(
            parse_address("@alex:example.org").map(|a| a.0),
            Some("@alex:example.org".to_owned())
        );
        assert_eq!(parse_address("hello"), None);
    }
}
