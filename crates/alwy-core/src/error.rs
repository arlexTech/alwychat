use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("could not reach homeserver: {0}")]
    Client(#[from] matrix_sdk::ClientBuildError),
    #[error(transparent)]
    Http(#[from] matrix_sdk::HttpError),
    #[error(transparent)]
    Matrix(#[from] matrix_sdk::Error),
    #[error("OAuth: {0}")]
    OAuth(String),
    #[error("this server doesn't support browser sign-in")]
    OAuthUnsupported,
    #[error("couldn't open the browser: {0}")]
    Browser(String),
    #[error("sign-in was not completed in time")]
    LoginTimedOut,
    #[error("sign-in was cancelled")]
    LoginCancelled,
    #[error("sync: {0}")]
    Sync(String),
    #[error("invalid media URI: {0}")]
    InvalidMedia(String),
    #[error("this chat isn't available")]
    UnknownRoom,
    #[error("this message isn't available")]
    UnknownEvent,
    #[error("this person isn't in the chat")]
    UnknownUser,
    #[error("call: {0}")]
    Call(String),
    #[error("this chat isn't open")]
    ChatNotOpen,
    #[error("timeline: {0}")]
    Timeline(String),
    #[error("encryption: {0}")]
    Encryption(String),
    #[error("no verification in progress")]
    NoVerification,
    #[error("couldn't send the file: {0}")]
    Upload(String),
    #[error("not logged in")]
    NotLoggedIn,
    #[error("local storage: {0}")]
    Io(#[from] std::io::Error),
    #[error("corrupted session file: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

// Errors cross the IPC bridge as plain messages; the UI never branches on Rust internals.
impl Serialize for Error {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}
