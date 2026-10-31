//! App preferences kept on this device (`settings.json` next to the session).

use std::path::Path;

use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::Result;

const SETTINGS_FILE: &str = "settings.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Others see when we've read their messages (otherwise receipts stay private).
    pub send_read_receipts: bool,
    /// Others see when we're typing.
    pub send_typing: bool,
    /// Others see when we're online / last active ("Show when I'm active").
    pub share_presence: bool,
    /// Desktop notifications for new messages.
    pub notifications: bool,
    /// Send photos in original quality by default (chats can override). Off: compressed.
    pub hd_media: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            send_read_receipts: true,
            send_typing: true,
            share_presence: true,
            notifications: true,
            hd_media: false,
        }
    }
}

impl Settings {
    pub(crate) fn load(data_dir: &Path) -> Self {
        match std::fs::read(data_dir.join(SETTINGS_FILE)) {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|e| {
                warn!("ignoring unreadable settings: {e}");
                Self::default()
            }),
            Err(_) => Self::default(),
        }
    }
}

impl crate::Core {
    pub fn settings(&self) -> Settings {
        self.settings.lock().unwrap().clone()
    }

    pub async fn set_settings(&self, settings: Settings) -> Result<()> {
        let bytes = serde_json::to_vec_pretty(&settings)?;
        tokio::fs::create_dir_all(&self.data_dir).await?;
        tokio::fs::write(self.data_dir.join(SETTINGS_FILE), bytes).await?;
        *self.settings.lock().unwrap() = settings;
        Ok(())
    }
}
