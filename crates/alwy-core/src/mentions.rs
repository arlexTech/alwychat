//! Mentions: composing messages with `@Name` pills, and the member list for autocomplete.

use matrix_sdk::{
    RoomMemberships,
    ruma::{
        OwnedRoomId, OwnedUserId,
        events::{
            Mentions,
            room::message::{FormattedBody, RoomMessageEventContentWithoutRelation},
        },
    },
};
use serde::{Deserialize, Serialize};

use crate::{Error, Result};

/// A mention picked in the composer. `text` is what the draft shows, e.g. `@Alex`.
#[derive(Debug, Clone, Deserialize)]
pub struct MentionInput {
    pub user_id: String,
    pub text: String,
}

/// A room member, for `@` autocomplete.
#[derive(Debug, Clone, Serialize)]
pub struct Member {
    pub user_id: String,
    pub name: String,
    pub avatar: Option<String>,
}

const ROOM_MENTION: &str = "@room";

/// Build a Markdown message where each picked mention becomes a matrix.to pill, with intentional
/// mentions (`m.mentions`) so other clients notify the right people.
pub(crate) fn compose(
    body: &str,
    mentions: &[MentionInput],
) -> RoomMessageEventContentWithoutRelation {
    // Longest first, so "@Al" never eats into "@Alex".
    let mut picked: Vec<(&MentionInput, OwnedUserId)> = mentions
        .iter()
        .filter(|m| body.contains(&m.text))
        .filter_map(|m| Some((m, OwnedUserId::try_from(m.user_id.as_str()).ok()?)))
        .collect();
    picked.sort_by_key(|(m, _)| std::cmp::Reverse(m.text.len()));
    picked.dedup_by(|a, b| a.1 == b.1);

    let room_mention = has_word(body, ROOM_MENTION);
    if picked.is_empty() && !room_mention {
        return RoomMessageEventContentWithoutRelation::text_markdown(body);
    }

    let mut plain = body.to_owned();
    let mut markdown = body.to_owned();
    for (m, user_id) in &picked {
        let name = m.text.trim_start_matches('@');
        plain = plain.replace(&m.text, name);
        let label = name
            .replace('\\', "\\\\")
            .replace('[', "\\[")
            .replace(']', "\\]");
        markdown = markdown.replace(
            &m.text,
            &format!("[{label}](https://matrix.to/#/{user_id})"),
        );
    }

    let content = match FormattedBody::markdown(&markdown) {
        Some(html) => RoomMessageEventContentWithoutRelation::text_html(plain, html.body),
        None => RoomMessageEventContentWithoutRelation::text_plain(plain),
    };
    let mut mentions = Mentions::with_user_ids(picked.into_iter().map(|(_, id)| id));
    mentions.room = room_mention;
    content.add_mentions(mentions)
}

fn has_word(text: &str, word: &str) -> bool {
    text.match_indices(word).any(|(i, _)| {
        let after = text[i + word.len()..].chars().next();
        !after.is_some_and(|c| c.is_alphanumeric() || c == '_')
    })
}

impl crate::Core {
    /// Joined members of a room, sorted by name, for `@` autocomplete.
    pub async fn room_members(&self, room_id: &str) -> Result<Vec<Member>> {
        let client = self.client()?;
        let room_id = OwnedRoomId::try_from(room_id).map_err(|_| Error::UnknownRoom)?;
        let room = client.get_room(&room_id).ok_or(Error::UnknownRoom)?;
        let own = client.user_id().map(ToOwned::to_owned);
        let mut members: Vec<Member> = room
            .members(RoomMemberships::JOIN)
            .await?
            .into_iter()
            .filter(|m| Some(m.user_id()) != own.as_deref())
            .map(|m| Member {
                user_id: m.user_id().to_string(),
                name: m
                    .display_name()
                    .unwrap_or(m.user_id().localpart())
                    .to_owned(),
                avatar: m.avatar_url().map(ToString::to_string),
            })
            .collect();
        members.sort_by_cached_key(|m| m.name.to_lowercase());
        Ok(members)
    }
}

#[cfg(test)]
mod tests {
    use matrix_sdk::ruma::events::room::message::MessageType;

    use super::*;

    fn text(c: &RoomMessageEventContentWithoutRelation) -> (&str, Option<&str>) {
        match &c.msgtype {
            MessageType::Text(t) => (&t.body, t.formatted.as_ref().map(|f| f.body.as_str())),
            _ => panic!("not text"),
        }
    }

    #[test]
    fn mention_becomes_pill_and_intentional_mention() {
        let m = [MentionInput {
            user_id: "@lea:example.org".into(),
            text: "@Léa".into(),
        }];
        let c = compose("hey @Léa, look", &m);
        let (plain, html) = text(&c);
        assert_eq!(plain, "hey Léa, look");
        assert!(
            html.unwrap()
                .contains(r#"<a href="https://matrix.to/#/@lea:example.org">Léa</a>"#)
        );
        assert_eq!(c.mentions.unwrap().user_ids.len(), 1);
    }

    #[test]
    fn removed_mentions_are_ignored() {
        let m = [MentionInput {
            user_id: "@lea:example.org".into(),
            text: "@Léa".into(),
        }];
        let c = compose("never mind", &m);
        assert!(c.mentions.is_none());
    }

    #[test]
    fn room_mention() {
        let c = compose("@room dinner is ready", &[]);
        assert!(c.mentions.unwrap().room);
        assert!(compose("@roomy", &[]).mentions.is_none());
    }
}
