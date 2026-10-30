//! Sign-in through the homeserver's OAuth 2.0 service (MAS), session persistence and sign-out.
//!
//! Desktop flow (RFC 8252): a one-shot HTTP server on 127.0.0.1 receives the browser redirect.
//! Mobile will use a custom-scheme redirect instead.

use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

use matrix_sdk::{
    Client, SessionChange,
    authentication::oauth::{
        ClientId, OAuthSession, UserSession,
        registration::{ApplicationType, ClientMetadata, Localized, OAuthGrantType},
    },
    ruma::serde::Raw,
    utils::{
        UrlOrQuery,
        local_server::{LocalServerBuilder, LocalServerResponse},
    },
};
use matrix_sdk_ui::sync_service::SyncService;
use rand::distr::{Alphanumeric, SampleString};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};
use url::Url;

use crate::{Error, Result};

const LOGIN_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const CLIENT_URI: &str = "https://github.com/alex/alwychat";
const SESSION_FILE: &str = "session.json";
const STORE_DIR: &str = "store";

const REDIRECT_PAGE: &str = r#"<!doctype html><meta charset="utf-8"><title>AlwyChat</title>
<body style="font-family:system-ui;background:#0f1115;color:#e8eaf0;display:grid;place-items:center;height:100vh;margin:0">
<div style="text-align:center"><h1>You're signed in ✨</h1><p>You can close this tab and go back to AlwyChat.</p></div>
<script>setTimeout(() => window.close(), 800)</script>"#;

/// The signed-in user, as shown by the UI.
#[derive(Debug, Clone, Serialize)]
pub struct Account {
    pub user_id: String,
    pub device_id: String,
    pub display_name: Option<String>,
}

/// What we persist between launches.
// TODO: move tokens and store passphrase to the OS keychain before the first release.
#[derive(Serialize, Deserialize)]
struct StoredSession {
    homeserver: Url,
    client_id: String,
    user: UserSession,
    store_passphrase: String,
}

/// App-wide handle to the Matrix account. One per process.
/// Feature modules (rooms, media, …) add their own `impl Core` blocks.
pub struct Core {
    data_dir: PathBuf,
    pub(crate) active: Mutex<Option<Active>>,
}

/// Everything that only exists while signed in.
pub(crate) struct Active {
    pub client: Client,
    pub sync: Arc<SyncService>,
    /// Background tasks feeding the UI; aborted on sign-out or re-subscribe.
    pub tasks: Vec<tokio::task::AbortHandle>,
}

impl Core {
    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        Self {
            data_dir: data_dir.into(),
            active: Mutex::new(None),
        }
    }

    pub fn client(&self) -> Result<Client> {
        self.with_active(|a| a.client.clone())
    }

    pub(crate) fn with_active<T>(&self, f: impl FnOnce(&mut Active) -> T) -> Result<T> {
        self.active
            .lock()
            .unwrap()
            .as_mut()
            .map(f)
            .ok_or(Error::NotLoggedIn)
    }

    /// Restore the previous session, if any. Returns `None` when signed out.
    pub async fn restore(&self) -> Result<Option<Account>> {
        let path = self.data_dir.join(SESSION_FILE);
        let stored: StoredSession = match tokio::fs::read(&path).await {
            Ok(bytes) => serde_json::from_slice(&bytes)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };

        let client = self
            .build_client(Server::Url(&stored.homeserver), &stored.store_passphrase)
            .await?;
        let session = OAuthSession {
            client_id: ClientId::new(stored.client_id),
            user: stored.user,
        };
        client.restore_session(session).await?;
        self.activate(client.clone(), stored.store_passphrase)
            .await?;
        Ok(Some(account(&client).await))
    }

    /// Sign in with the server's web login. `open_url` must show the URL in the system browser.
    pub async fn login(
        &self,
        server: &str,
        open_url: impl FnOnce(&Url) -> std::result::Result<(), String>,
    ) -> Result<Account> {
        // A fresh login always starts from an empty store: keys from another account must not leak in.
        self.wipe_local_data().await?;
        let passphrase = Alphanumeric.sample_string(&mut rand::rng(), 48);
        let client = self.build_client(Server::Name(server), &passphrase).await?;
        let oauth = client.oauth();

        match oauth.server_metadata().await {
            Ok(_) => {}
            Err(e) if e.is_not_supported() => return Err(Error::OAuthUnsupported),
            Err(e) => return Err(Error::OAuth(e.to_string())),
        }

        let (redirect_uri, redirect) = LocalServerBuilder::new()
            .response(LocalServerResponse::Html(REDIRECT_PAGE.to_owned()))
            .spawn()
            .await?;

        let auth = oauth
            .login(
                redirect_uri.clone(),
                None,
                Some(client_metadata(redirect_uri)?.into()),
                None,
            )
            .build()
            .await
            .map_err(|e| Error::OAuth(e.to_string()))?;

        open_url(&auth.url).map_err(Error::Browser)?;

        let query = match tokio::time::timeout(LOGIN_TIMEOUT, redirect).await {
            Ok(Some(query)) => query,
            Ok(None) => {
                oauth.abort_login(&auth.state).await;
                return Err(Error::LoginCancelled);
            }
            Err(_) => {
                oauth.abort_login(&auth.state).await;
                return Err(Error::LoginTimedOut);
            }
        };
        oauth.finish_login(UrlOrQuery::Query(query.0)).await?;

        self.activate(client.clone(), passphrase).await?;
        info!(user = ?client.user_id(), "signed in");
        Ok(account(&client).await)
    }

    /// Sign out on the server (best effort) and delete all local data.
    pub async fn logout(&self) -> Result<()> {
        let active = self.active.lock().unwrap().take();
        if let Some(active) = active {
            active.tasks.iter().for_each(|t| t.abort());
            active.sync.stop().await;
            if let Err(e) = active.client.oauth().logout().await {
                warn!("server-side logout failed, wiping locally anyway: {e}");
            }
        }
        self.wipe_local_data().await
    }

    async fn build_client(&self, server: Server<'_>, passphrase: &str) -> Result<Client> {
        let builder = match server {
            // User input: resolve via .well-known.
            Server::Name(name) => Client::builder().server_name_or_homeserver_url(name),
            // Already resolved at login: skip discovery.
            Server::Url(url) => Client::builder().homeserver_url(url.as_str()),
        };
        Ok(builder
            .sqlite_store(self.data_dir.join(STORE_DIR), Some(passphrase))
            .handle_refresh_tokens()
            .build()
            .await?)
    }

    /// Start syncing, store the client and keep the session file up to date when tokens rotate.
    async fn activate(&self, client: Client, passphrase: String) -> Result<()> {
        let path = self.data_dir.join(SESSION_FILE);
        persist(&path, &client, &passphrase);

        let mut changes = client.subscribe_to_session_changes();
        let watched = client.clone();
        tokio::spawn(async move {
            while let Ok(change) = changes.recv().await {
                match change {
                    SessionChange::TokensRefreshed => persist(&path, &watched, &passphrase),
                    SessionChange::UnknownToken(_) => warn!("session revoked by the server"),
                }
            }
        });

        // The event cache feeds timelines and chat-list previews; it must run before sync starts.
        client
            .event_cache()
            .subscribe()
            .map_err(|e| Error::Sync(e.to_string()))?;
        let sync = Arc::new(
            SyncService::builder(client.clone())
                .build()
                .await
                .map_err(|e| Error::Sync(e.to_string()))?,
        );
        sync.start().await;

        *self.active.lock().unwrap() = Some(Active {
            client,
            sync,
            tasks: Vec::new(),
        });
        Ok(())
    }

    async fn wipe_local_data(&self) -> Result<()> {
        for dir in [STORE_DIR] {
            match tokio::fs::remove_dir_all(self.data_dir.join(dir)).await {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e.into()),
                _ => {}
            }
        }
        match tokio::fs::remove_file(self.data_dir.join(SESSION_FILE)).await {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.into()),
            _ => Ok(()),
        }
    }
}

enum Server<'a> {
    Name(&'a str),
    Url(&'a Url),
}

fn client_metadata(redirect_uri: Url) -> Result<Raw<ClientMetadata>> {
    let client_uri = Url::parse(CLIENT_URI).expect("valid constant URL");
    let metadata = ClientMetadata {
        client_name: Some(Localized::new("AlwyChat".to_owned(), [])),
        ..ClientMetadata::new(
            ApplicationType::Native,
            vec![OAuthGrantType::AuthorizationCode {
                redirect_uris: vec![redirect_uri],
            }],
            Localized::new(client_uri, []),
        )
    };
    Ok(Raw::new(&metadata)?)
}

async fn account(client: &Client) -> Account {
    let display_name = client.account().get_display_name().await.ok().flatten();
    Account {
        user_id: client
            .user_id()
            .map(ToString::to_string)
            .unwrap_or_default(),
        device_id: client
            .device_id()
            .map(ToString::to_string)
            .unwrap_or_default(),
        display_name,
    }
}

fn persist(path: &Path, client: &Client, passphrase: &str) {
    let Some(oauth) = client.oauth().full_session() else {
        warn!("no OAuth session to persist");
        return;
    };
    let stored = StoredSession {
        homeserver: client.homeserver(),
        client_id: oauth.client_id.as_str().to_owned(),
        user: oauth.user,
        store_passphrase: passphrase.to_owned(),
    };
    if let Err(e) = write_private(path, &serde_json::to_vec(&stored).expect("serializable")) {
        warn!("couldn't save session: {e}");
    }
}

/// Atomic write, readable by the current user only.
fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options.open(&tmp)?.write_all(bytes)?;
    std::fs::rename(tmp, path)
}
