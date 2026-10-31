//! "Online" / "last seen 5 min ago" (Matrix presence).
//!
//! Simplified sliding sync doesn't carry presence, so the UI polls the people it shows (open
//! 1:1 chat, 1:1 chats in the list) and reports our own activity while the app is in front.
//! "Show when I'm active" off: we tell the server we're offline and stop reporting.

use futures_util::future::join_all;
use matrix_sdk::ruma::{
    OwnedUserId,
    api::client::presence::{get_presence, set_presence},
    presence::PresenceState,
};
use serde::Serialize;
use tracing::debug;

use crate::{Error, Result};

#[derive(Debug, Clone, Serialize)]
pub struct Presence {
    pub user_id: String,
    /// Active right now (or within the server's idle window).
    pub online: bool,
    /// Milliseconds since their last activity, if the server knows.
    pub last_active_ago_ms: Option<u64>,
}

impl crate::Core {
    /// Presence of each user; people whose presence can't be read are left out.
    pub async fn presence(&self, user_ids: &[String]) -> Result<Vec<Presence>> {
        let client = self.client()?;
        let requests = user_ids
            .iter()
            .filter_map(|id| OwnedUserId::try_from(id.as_str()).ok())
            .map(|user| {
                let client = client.clone();
                async move {
                    match client
                        .send(get_presence::v3::Request::new(user.clone()))
                        .await
                    {
                        Ok(r) => Some(Presence {
                            user_id: user.to_string(),
                            online: r.presence == PresenceState::Online
                                || r.currently_active == Some(true),
                            last_active_ago_ms: r.last_active_ago.map(|d| d.as_millis() as u64),
                        }),
                        Err(e) => {
                            debug!(%user, "no presence: {e}");
                            None
                        }
                    }
                }
            });
        Ok(join_all(requests).await.into_iter().flatten().collect())
    }

    /// Report whether we're using the app (`active`), unless the user hid their activity.
    pub async fn report_activity(&self, active: bool) -> Result<()> {
        let client = self.client()?;
        let me = client.user_id().ok_or(Error::NotLoggedIn)?.to_owned();
        let state = match (self.settings().share_presence, active) {
            (false, _) => PresenceState::Offline,
            (true, true) => PresenceState::Online,
            (true, false) => PresenceState::Unavailable,
        };
        client
            .send(set_presence::v3::Request::new(me, state))
            .await?;
        Ok(())
    }
}
