//! Extended profile fields (MSC4133, stable in recent Synapse): a custom status with an emoji
//! (MSC4426's `org.matrix.msc4426.status`, as other clients will read it) and the time zone
//! (`m.tz`), so a chat can show someone's local time.

use futures_util::future::join_all;
use matrix_sdk::ruma::{
    OwnedUserId,
    api::client::profile::{ProfileFieldName, ProfileFieldValue},
};
use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::Result;

const STATUS: &str = "org.matrix.msc4426.status";
const TIME_ZONE: &str = "m.tz";
/// MSC4426 limits, in bytes.
const STATUS_TEXT_MAX: usize = 256;
const STATUS_EMOJI_MAX: usize = 32;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Status {
    pub text: String,
    pub emoji: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ProfileExtras {
    pub user_id: String,
    pub status: Option<Status>,
    /// IANA name, e.g. `Europe/Brussels`.
    pub time_zone: Option<String>,
}

/// `field` reads one profile field (the response's `get`).
fn extras<'a>(
    user_id: &OwnedUserId,
    field: impl Fn(&str) -> Option<&'a serde_json::Value>,
) -> ProfileExtras {
    ProfileExtras {
        user_id: user_id.to_string(),
        status: field(STATUS)
            .and_then(|v| serde_json::from_value::<Status>(v.clone()).ok())
            .filter(|s| !s.text.is_empty() || !s.emoji.is_empty()),
        time_zone: field(TIME_ZONE)
            .and_then(|v| v.as_str())
            .filter(|tz| !tz.is_empty())
            .map(ToOwned::to_owned),
    }
}

/// Cut to at most `max` bytes on a character boundary.
fn clip(s: &str, max: usize) -> String {
    let mut end = s.len().min(max);
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].to_owned()
}

impl crate::Core {
    /// Status and time zone of each user (ours included); unreadable profiles are left out.
    pub async fn profile_extras(&self, user_ids: &[String]) -> Result<Vec<ProfileExtras>> {
        let client = self.client()?;
        let requests = user_ids
            .iter()
            .filter_map(|id| OwnedUserId::try_from(id.as_str()).ok())
            .map(|user| {
                let client = client.clone();
                async move {
                    match client.account().fetch_user_profile_of(&user).await {
                        Ok(profile) => Some(extras(&user, |f| profile.get(f))),
                        Err(e) => {
                            debug!(%user, "profile: {e}");
                            None
                        }
                    }
                }
            });
        Ok(join_all(requests).await.into_iter().flatten().collect())
    }

    /// Set our status, or clear it with `None` (or empty text and emoji).
    pub async fn set_status(&self, status: Option<Status>) -> Result<()> {
        let account = self.client()?.account();
        match status.filter(|s| !s.text.trim().is_empty() || !s.emoji.is_empty()) {
            Some(s) => {
                let value = serde_json::json!({
                    "text": clip(s.text.trim(), STATUS_TEXT_MAX),
                    "emoji": clip(&s.emoji, STATUS_EMOJI_MAX),
                });
                account
                    .set_profile_field(ProfileFieldValue::new(STATUS, value)?)
                    .await?;
            }
            None => {
                account
                    .delete_profile_field(ProfileFieldName::from(STATUS))
                    .await?
            }
        }
        Ok(())
    }

    /// Share our time zone (IANA name), or stop sharing it.
    pub async fn set_time_zone(&self, time_zone: Option<&str>) -> Result<()> {
        let account = self.client()?.account();
        match time_zone {
            Some(tz) if !tz.is_empty() => {
                account
                    .set_profile_field(ProfileFieldValue::TimeZone(tz.to_owned()))
                    .await?
            }
            _ => {
                account
                    .delete_profile_field(ProfileFieldName::TimeZone)
                    .await?
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_status_and_time_zone() {
        let user = OwnedUserId::try_from("@a:b.c").unwrap();
        let profile = serde_json::json!({
            "displayname": "A",
            "org.matrix.msc4426.status": { "text": "Studying", "emoji": "📚" },
            "m.tz": "Europe/Brussels",
        });
        let e = extras(&user, |f| profile.get(f));
        assert_eq!(e.status.unwrap().text, "Studying");
        assert_eq!(e.time_zone.as_deref(), Some("Europe/Brussels"));

        let empty = serde_json::json!({
            "org.matrix.msc4426.status": { "text": "", "emoji": "" },
        });
        assert!(extras(&user, |f| empty.get(f)).status.is_none());
    }

    #[test]
    fn clips_on_char_boundaries() {
        assert_eq!(clip("ééé", 3), "é");
        assert_eq!(clip("abc", 10), "abc");
    }
}
