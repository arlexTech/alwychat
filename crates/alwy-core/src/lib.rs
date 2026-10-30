//! AlwyChat core: all Matrix state and logic lives here. UI shells (Tauri) stay thin.

pub mod discovery;
pub mod error;
pub mod media;
pub mod rooms;
pub mod session;

pub use error::{Error, Result};
pub use session::{Account, Core};
