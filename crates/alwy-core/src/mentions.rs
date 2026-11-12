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
    pub is_me: bool,
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

    // Where each mention sits: longest first, never overlapping, and only as a whole word
    // ("@alex" is not inside "@alexis"); one pass, so a link never gets rewritten.
    let mut spans: Vec<(usize, usize, usize)> = Vec::new(); // (start, end, index in picked)
    for (k, (m, _)) in picked.iter().enumerate() {
        for (i, _) in body.match_indices(m.text.as_str()) {
            let end = i + m.text.len();
            // Not followed by more of a name, nor by ":server" (part of a full user ID);
            // "@Alex: hi" is fine.
            let mut rest = body[end..].chars();
            let whole = match rest.next() {
                Some(c) if c.is_alphanumeric() || c == '_' => false,
                Some(':') => !rest.next().is_some_and(|c| c.is_alphanumeric()),
                _ => true,
            };
            if whole && !spans.iter().any(|&(s, e, _)| i < e && s < end) {
                spans.push((i, end, k));
            }
        }
    }
    spans.sort_unstable();
    let mut plain = String::with_capacity(body.len());
    let mut markdown = String::with_capacity(body.len() + spans.len() * 40);
    let mut at = 0;
    for &(start, end, k) in &spans {
        let (m, user_id) = &picked[k];
        plain.push_str(&body[at..start]);
        markdown.push_str(&body[at..start]);
        let name = m.text.trim_start_matches('@');
        plain.push_str(name);
        let label = name
            .replace('\\', "\\\\")
            .replace('[', "\\[")
            .replace(']', "\\]");
        markdown.push_str(&format!("[{label}](https://matrix.to/#/{user_id})"));
        at = end;
    }
    plain.push_str(&body[at..]);
    markdown.push_str(&body[at..]);
    // Only the people actually linked are notified.
    let linked: std::collections::BTreeSet<usize> = spans.iter().map(|s| s.2).collect();
    let picked: Vec<_> = picked
        .into_iter()
        .enumerate()
        .filter(|(k, _)| linked.contains(k))
        .map(|(_, p)| p)
        .collect();

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
    /// Joined members of a room (us included, flagged `is_me`), sorted by name.
    pub async fn room_members(&self, room_id: &str) -> Result<Vec<Member>> {
        let client = self.client()?;
        let room_id = OwnedRoomId::try_from(room_id).map_err(|_| Error::UnknownRoom)?;
        let room = client.get_room(&room_id).ok_or(Error::UnknownRoom)?;
        let own = client.user_id().map(ToOwned::to_owned);
        let mut members: Vec<Member> = crate::offline::members(&room, RoomMemberships::JOIN)
            .await?
            .into_iter()
            .map(|m| Member {
                is_me: Some(m.user_id()) == own.as_deref(),
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
    fn mentions_that_contain_each_other_stay_apart() {
        let m = [
            MentionInput {
                user_id: "@alexis:example.org".into(),
                text: "@alexis".into(),
            },
            MentionInput {
                user_id: "@alex:example.org".into(),
                text: "@alex".into(),
            },
        ];
        let c = compose("@alexis and @alex: hi", &m);
        let (plain, html) = text(&c);
        assert_eq!(plain, "alexis and alex: hi");
        let html = html.unwrap();
        assert!(html.contains(r#"<a href="https://matrix.to/#/@alexis:example.org">alexis</a>"#));
        assert!(html.contains(r#"<a href="https://matrix.to/#/@alex:example.org">alex</a>: hi"#));
        assert_eq!(c.mentions.unwrap().user_ids.len(), 2);
        // A full user ID typed in the text is one mention, not "@alex" plus leftovers.
        let only = [MentionInput {
            user_id: "@alex:example.org".into(),
            text: "@alex".into(),
        }];
        let c = compose("ask @alex:example.org", &only);
        assert!(c.mentions.is_none_or(|m| m.user_ids.is_empty()));
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
