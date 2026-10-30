//! AlwyChat core: all Matrix state and logic lives here. UI shells (Tauri) stay thin.
#![recursion_limit = "256"]

pub mod discovery;
pub mod encryption;
pub mod error;
mod html;
pub mod media;
pub mod notifications;
pub mod rooms;
pub mod session;
pub mod settings;
pub mod timeline;
pub mod upload;

pub use error::{Error, Result};
pub use session::{Account, Core};
