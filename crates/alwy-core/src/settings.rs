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
    /// Play message effects (confetti, snow…) when they arrive.
    pub chat_effects: bool,
    /// "system", "light", "dark" or "amoled".
    pub app_theme: String,
    /// App lock: ask again after the app was in the background this long (0 = every time).
    pub app_lock_after_s: u32,
    /// App lock: offer fingerprint / face before the PIN.
    pub app_lock_biometric: bool,
    /// Android: blank in the app switcher, no screenshots.
    pub hide_in_switcher: bool,
    /// Enter sends (Shift+Enter: new line); off: Enter adds a line, Ctrl+Enter sends.
    /// Keyboards (desktop) only: on touch screens the send button sends.
    pub enter_sends: bool,
    /// Text size, in percent of the default.
    pub text_scale: u16,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            send_read_receipts: true,
            send_typing: true,
            share_presence: true,
            notifications: true,
            hd_media: false,
            chat_effects: true,
            app_theme: "system".to_owned(),
            app_lock_after_s: 60,
            app_lock_biometric: true,
            hide_in_switcher: false,
            enter_sends: true,
            text_scale: 100,
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
        crate::files::create_dir_all(&self.data_dir).await?;
        crate::files::write(self.data_dir.join(SETTINGS_FILE), bytes).await?;
        *self.settings.lock().unwrap() = settings;
        Ok(())
    }
}
