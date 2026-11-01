//! AlwyChat core: all Matrix state and logic lives here. UI shells (Tauri) stay thin.
#![recursion_limit = "256"]
// In a browser everything runs on one thread, so shared state needn't be Send/Sync there.
#![cfg_attr(target_family = "wasm", allow(clippy::arc_with_non_send_sync))]

pub mod applock;
pub mod custom;
pub mod discovery;
pub mod drafts;
mod effects;
pub mod encryption;
pub mod error;
mod files;
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
mod rt;
pub mod rtc;
pub mod saved;
pub mod search;
pub mod session;
pub mod settings;
pub mod spaces;
pub mod timeline;
pub mod upload;
pub mod voice;

pub use error::{Error, Result};
pub use session::{Account, Core, LoginRedirect};
