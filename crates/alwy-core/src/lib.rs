//! AlwyChat core: all Matrix state and logic lives here. UI shells (Tauri) stay thin.
#![recursion_limit = "256"]

pub mod applock;
pub mod custom;
pub mod discovery;
pub mod drafts;
mod effects;
pub mod encryption;
pub mod error;
mod html;
pub mod links;
pub mod manage;
pub mod media;
pub mod mentions;
pub mod notifications;
pub mod pins;
pub mod polls;
pub mod presence;
pub mod profile;
pub mod push;
pub mod rooms;
pub mod rtc;
pub mod search;
pub mod session;
pub mod settings;
pub mod spaces;
pub mod timeline;
pub mod upload;
pub mod voice;

pub use error::{Error, Result};
pub use session::{Account, Core, LoginRedirect};
