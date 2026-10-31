//! AlwyChat core: all Matrix state and logic lives here. UI shells (Tauri) stay thin.
#![recursion_limit = "256"]

pub mod custom;
pub mod discovery;
pub mod encryption;
pub mod error;
mod html;
pub mod media;
pub mod mentions;
pub mod notifications;
pub mod presence;
pub mod rooms;
pub mod rtc;
pub mod session;
pub mod settings;
pub mod timeline;
pub mod upload;
pub mod voice;

pub use error::{Error, Result};
pub use session::{Account, Core, LoginRedirect};
