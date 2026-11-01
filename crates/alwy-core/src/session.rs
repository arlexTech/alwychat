//! Sign-in through the homeserver's OAuth 2.0 service (MAS), session persistence and sign-out.
//!
//! Desktop flow (RFC 8252): a one-shot HTTP server on 127.0.0.1 receives the browser redirect.
//! Mobile flow: the browser redirects to `com.alwy.chat:/callback`, the OS reopens the app
//! with that URL and the shell hands it to [`Core::handle_login_redirect`]. (A loopback server
//! can't work there: the app is paused while the browser is in front.)

use std::{
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

#[cfg(not(target_family = "wasm"))]
use matrix_sdk::utils::local_server::{LocalServerBuilder, LocalServerResponse};
use matrix_sdk::{
    Client, SessionChange,
    authentication::oauth::{
        ClientId, OAuthSession, UserSession,
        registration::{ApplicationType, ClientMetadata, Localized, OAuthGrantType},
    },
    encryption::{BackupDownloadStrategy, EncryptionSettings},
    ruma::serde::Raw,
    utils::UrlOrQuery,
};
use matrix_sdk_ui::sync_service::SyncService;
use rand::distr::{Alphanumeric, SampleString};
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;
use tracing::{info, warn};
use url::Url;

use crate::{Error, Result};

const LOGIN_TIMEOUT: Duration = Duration::from_secs(10 * 60);
/// MAS only accepts a custom-scheme redirect whose reversed scheme is under this host.
const CLIENT_URI: &str = "https://arlextech.github.io/alwychat";
/// Mobile redirect target, declared to the OS by the deep-link plugin (tauri.conf.json).
pub const REDIRECT_SCHEME: &str = "com.alwy.chat";
const SESSION_FILE: &str = "session.json";
/// The store of sessions saved before stores got their own folder per sign-in.
const STORE_DIR: &str = "store";
/// Each sign-in gets a fresh `store-<random>` folder (recorded in `session.json`): a client
/// left over from a failed sign-in can still be running, and its database connections would
/// otherwise open the new session's files at the same path and corrupt them (seen: the crypto
/// store lost its account).
const STORE_PREFIX: &str = "store-";
/// Created by matrix-sdk-sqlite inside the store folder; holds this device's keys.
#[cfg(not(target_family = "wasm"))]
const CRYPTO_DB: &str = "matrix-sdk-crypto.sqlite3";

#[cfg(not(target_family = "wasm"))]
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

/// What we persist between launches. On desktop the secrets (tokens, store passphrase) go to
/// the OS keychain and are blanked in the file (`in_keychain`); if the keychain isn't
/// available they stay in the file (0600), as on Android (app-private storage).
#[derive(Serialize, Deserialize)]
struct StoredSession {
    homeserver: Url,
    client_id: String,
    user: UserSession,
    store_passphrase: String,
    /// Folder name inside the data dir.
    #[serde(default = "legacy_store_dir")]
    store_dir: String,
    #[serde(default)]
    in_keychain: bool,
}

/// What the keychain holds for a session.
#[derive(Serialize, Deserialize)]
struct Secrets {
    tokens: matrix_sdk::SessionTokens,
    store_passphrase: String,
}

#[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
mod keychain {
    use super::Secrets;

    const SERVICE: &str = "com.alwy.chat";

    fn entry(account: &str) -> keyring::Result<keyring::Entry> {
        keyring::Entry::new(SERVICE, account)
    }

    pub(super) fn save(account: &str, secrets: &Secrets) -> bool {
        let json = serde_json::to_string(secrets).expect("serializable");
        match entry(account).and_then(|e| e.set_password(&json)) {
            Ok(()) => true,
            Err(e) => {
                tracing::warn!("keychain unavailable, keeping the session in its file: {e}");
                false
            }
        }
    }

    pub(super) fn load(account: &str) -> Result<Secrets, String> {
        let json = entry(account)
            .and_then(|e| e.get_password())
            .map_err(|e| e.to_string())?;
        serde_json::from_str(&json).map_err(|e| e.to_string())
    }

    pub(super) fn delete(account: &str) {
        if let Ok(e) = entry(account) {
            let _ = e.delete_credential();
        }
    }
}

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
mod keychain {
    use super::Secrets;
    pub(super) fn save(_: &str, _: &Secrets) -> bool {
        false
    }
    pub(super) fn load(_: &str) -> Result<Secrets, String> {
        Err("no keychain on this platform".into())
    }
    pub(super) fn delete(_: &str) {}
}

/// The keychain entry of a session: one per account and device.
fn keychain_account(user: &UserSession) -> String {
    format!("{}/{}", user.meta.user_id, user.meta.device_id)
}

fn legacy_store_dir() -> String {
    STORE_DIR.to_owned()
}

/// App-wide handle to the Matrix account. One per process.
/// Feature modules (rooms, media, …) add their own `impl Core` blocks.
pub struct Core {
    pub(crate) data_dir: PathBuf,
    pub(crate) active: Mutex<Option<Active>>,
    /// Called when the server revokes our session (signed out elsewhere, token reuse…).
    on_session_ended: Arc<Mutex<Option<SessionEndedHandler>>>,
    pub(crate) media: Arc<crate::media::MediaRegistry>,
    pub(crate) settings: Arc<Mutex<crate::settings::Settings>>,
    pub(crate) on_notification: Arc<Mutex<Option<crate::notifications::NotificationHandler>>>,
    pub(crate) on_call_signal: Arc<Mutex<Option<crate::rtc::CallSignalHandler>>>,
    pub(crate) on_draft: Arc<Mutex<Option<crate::drafts::DraftHandler>>>,
    /// The call we're in, if any.
    pub(crate) call: tokio::sync::Mutex<Option<crate::rtc::ActiveCall>>,
    /// Serialises `restore` (a push and the UI can both start the session on Android).
    restoring: tokio::sync::Mutex<()>,
    /// Ends the sign-in in progress: the redirect URL (mobile), or `None` to cancel.
    pending_login: Mutex<Option<oneshot::Sender<Option<Url>>>>,
}

/// Where the browser sends the user back after signing in.
#[derive(Debug, Clone)]
pub enum LoginRedirect {
    /// One-shot HTTP server on 127.0.0.1 (desktop).
    Loopback,
    /// `com.alwy.chat:/callback`, delivered through [`Core::handle_login_redirect`] (mobile).
    AppScheme,
    /// A page of the web app (opened in a popup), whose URL comes back through
    /// [`Core::handle_login_redirect`] (browser).
    Web(Url),
}

type SessionEndedHandler = Box<dyn Fn() + Send + Sync>;

/// Everything that only exists while signed in.
pub(crate) struct Active {
    pub client: Client,
    pub sync: Arc<SyncService>,
    /// Background tasks feeding the UI; aborted on sign-out or re-subscribe.
    pub tasks: Vec<crate::rt::AbortHandle>,
    pub chat: Option<crate::timeline::OpenChat>,
    pub verifications: Arc<crate::encryption::Verifications>,
    pub encryption_task: Option<crate::rt::AbortHandle>,
    /// Cleared when this session is torn down, so late token refreshes can't rewrite
    /// `session.json` after it was deleted (that would restore onto an empty store).
    pub may_persist: Arc<AtomicBool>,
    /// Local message index (None if it couldn't be opened).
    pub search: Option<Arc<crate::search::SearchIndex>>,
    pub search_task: Option<crate::rt::AbortHandle>,
    /// Created on first use of the Spaces tab.
    pub spaces: Option<Arc<matrix_sdk_ui::spaces::SpaceService>>,
    pub spaces_task: Option<crate::rt::AbortHandle>,
    /// The open space's children feed.
    pub space_task: Option<crate::rt::AbortHandle>,
}

impl Core {
    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        let data_dir = data_dir.into();
        Self {
            settings: Arc::new(Mutex::new(crate::settings::Settings::load(&data_dir))),
            on_notification: Arc::default(),
            on_call_signal: Arc::default(),
            on_draft: Arc::default(),
            data_dir,
            active: Mutex::new(None),
            on_session_ended: Arc::default(),
            media: Arc::default(),
            pending_login: Mutex::default(),
            call: tokio::sync::Mutex::default(),
            restoring: tokio::sync::Mutex::default(),
        }
    }

    /// `handler` runs when the server ends the session; the UI should sign out locally.
    pub fn on_session_ended(&self, handler: impl Fn() + Send + Sync + 'static) {
        *self.on_session_ended.lock().unwrap() = Some(Box::new(handler));
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
        let _restoring = self.restoring.lock().await;
        // Already running: a push woke the process before the UI started (Android).
        if let Ok(client) = self.client() {
            return Ok(Some(account(&client).await));
        }
        let path = self.data_dir.join(SESSION_FILE);
        let mut stored: StoredSession = match crate::files::read(&path).await {
            Ok(bytes) => serde_json::from_slice(&bytes)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        if stored.in_keychain {
            let account = keychain_account(&stored.user);
            // Never treat this as signed out: the store and session are fine, only locked.
            let secrets = crate::rt::blocking(move || keychain::load(&account))
                .await
                .map_err(|e| Error::Keychain(e.to_string()))?
                .map_err(|e| {
                    Error::Keychain(format!(
                        "couldn't read the session from the keychain ({e}); unlock your keyring and restart"
                    ))
                })?;
            stored.user.tokens = secrets.tokens;
            stored.store_passphrase = secrets.store_passphrase;
        }

        // Restoring a session onto a missing store would recreate this device with new keys
        // under its old device ID, breaking encryption for it. Treat it as signed out instead.
        let store_dir = stored.store_dir.clone();
        if store_dir.contains(['/', '\\']) || store_dir.contains("..") {
            warn!("unexpected store folder in session file, discarding it");
            crate::files::remove_file(&path).await?;
            return Ok(None);
        }
        // (In a browser the store is IndexedDB, which can't be looked at synchronously.)
        #[cfg(not(target_family = "wasm"))]
        if !self.data_dir.join(&store_dir).join(CRYPTO_DB).exists() {
            warn!("session file without encryption store, discarding it");
            crate::files::remove_file(&path).await?;
            return Ok(None);
        }

        let started = matrix_sdk::ruma::time::Instant::now();
        let client = self
            .build_client(
                Server::Url(&stored.homeserver),
                &stored.store_passphrase,
                &store_dir,
            )
            .await?;
        let built = started.elapsed();
        let session = OAuthSession {
            client_id: ClientId::new(stored.client_id),
            user: stored.user,
        };
        client.restore_session(session).await?;
        let restored = started.elapsed();
        self.activate(client.clone(), stored.store_passphrase, store_dir)
            .await?;
        let activated = started.elapsed();
        let account = account(&client).await;
        info!(?built, ?restored, ?activated, total = ?started.elapsed(), "session restored");
        Ok(Some(account))
    }

    /// Sign in with the server's web login. `open_url` must show the URL in the system browser.
    pub async fn login(
        &self,
        server: &str,
        redirect: LoginRedirect,
        open_url: impl FnOnce(&Url) -> std::result::Result<(), String>,
    ) -> Result<Account> {
        info!(server, "signing in");
        // Never wipe the store under a running session (it would keep syncing into deleted files).
        self.end_session().await;
        // A fresh login always starts from an empty store: keys from another account must not leak in.
        self.wipe_local_data().await?;
        let passphrase = Alphanumeric.sample_string(&mut rand::rng(), 48);
        let store_dir = format!(
            "{STORE_PREFIX}{}",
            Alphanumeric
                .sample_string(&mut rand::rng(), 10)
                .to_lowercase()
        );
        let client = self
            .build_client(Server::Name(server), &passphrase, &store_dir)
            .await?;
        info!("client built");
        let oauth = client.oauth();

        match oauth.server_metadata().await {
            Ok(_) => {}
            Err(e) if e.is_not_supported() => return Err(Error::OAuthUnsupported),
            Err(e) => return Err(Error::OAuth(e.to_string())),
        }

        info!("server metadata ok");
        let (finish, finished) = oneshot::channel();
        // A newer sign-in replaces (and so cancels) an older one.
        *self.pending_login.lock().unwrap() = Some(finish);

        let web = matches!(redirect, LoginRedirect::Web(_));
        let kind = format!("{redirect:?}");
        let (redirect_uri, loopback) = match redirect {
            #[cfg(target_family = "wasm")]
            LoginRedirect::Loopback => {
                return Err(Error::OAuth("no loopback sign-in in a browser".into()));
            }
            #[cfg(not(target_family = "wasm"))]
            LoginRedirect::Loopback => {
                let (uri, server) = LocalServerBuilder::new()
                    .response(LocalServerResponse::Html(REDIRECT_PAGE.to_owned()))
                    .spawn()
                    .await?;
                (uri, Some(server))
            }
            LoginRedirect::AppScheme => (
                Url::parse(&format!("{REDIRECT_SCHEME}:/callback")).expect("valid constant URL"),
                None,
            ),
            LoginRedirect::Web(url) => (url, None::<()>.map(|_| unreachable!())),
        };
        info!(%redirect_uri, "registering");
        let callback = async move {
            match loopback {
                #[cfg(not(target_family = "wasm"))]
                Some(server) => tokio::select! {
                    query = server => query.map(|q| UrlOrQuery::Query(q.0)),
                    _ = finished => None,
                },
                #[cfg(target_family = "wasm")]
                Some(()) => None,
                None => finished.await.ok().flatten().map(UrlOrQuery::Url),
            }
        };

        let auth = oauth
            .login(
                redirect_uri.clone(),
                None,
                Some(client_metadata(redirect_uri, web)?.into()),
                None,
            )
            .build()
            .await
            .map_err(|e| Error::OAuth(e.to_string()))?;

        info!(redirect = kind, "opening the sign-in page");
        open_url(&auth.url).map_err(Error::Browser)?;
        info!("waiting for the sign-in redirect");

        // No need to clear `pending_login` afterwards: a stale sender just fails to send.
        let callback = match matrix_sdk::timeout::timeout(callback, LOGIN_TIMEOUT).await {
            Ok(Some(callback)) => callback,
            Ok(None) => {
                oauth.abort_login(&auth.state).await;
                return Err(Error::LoginCancelled);
            }
            Err(_) => {
                oauth.abort_login(&auth.state).await;
                return Err(Error::LoginTimedOut);
            }
        };
        oauth.finish_login(callback).await?;

        self.activate(client.clone(), passphrase, store_dir).await?;
        info!(user = ?client.user_id(), "signed in");
        Ok(account(&client).await)
    }

    /// The OS opened the app with our redirect URL (mobile sign-in). Returns whether a
    /// sign-in was waiting for it.
    pub fn handle_login_redirect(&self, url: Url) -> bool {
        info!("sign-in redirect received");
        match self.pending_login.lock().unwrap().take() {
            Some(finish) => finish.send(Some(url)).is_ok(),
            None => false,
        }
    }

    /// Give up on the sign-in in progress (the user came back without finishing it).
    pub fn cancel_login(&self) {
        if let Some(finish) = self.pending_login.lock().unwrap().take() {
            let _ = finish.send(None);
        }
    }

    /// Stop using a session the server ended, *without* deleting anything: the store keeps
    /// this device's keys in case the report was spurious, and the next sign-in wipes it.
    pub async fn end_session(&self) {
        let active = self.active.lock().unwrap().take();
        if let Some(active) = active {
            active.may_persist.store(false, Ordering::SeqCst);
            active.tasks.iter().for_each(|t| t.abort());
            active.encryption_task.iter().for_each(|t| t.abort());
            active.search_task.iter().for_each(|t| t.abort());
            active
                .spaces_task
                .iter()
                .chain(&active.space_task)
                .for_each(|t| t.abort());
            active.sync.stop().await;
        }
    }

    /// Sign out on the server (best effort) and delete all local data.
    pub async fn logout(&self) -> Result<()> {
        if let Err(e) = self.leave_call().await {
            warn!("couldn't leave the call before signing out: {e}");
        }
        let active = self.active.lock().unwrap().take();
        if let Some(active) = active {
            active.may_persist.store(false, Ordering::SeqCst);
            active.tasks.iter().for_each(|t| t.abort());
            active.encryption_task.iter().for_each(|t| t.abort());
            active.search_task.iter().for_each(|t| t.abort());
            active
                .spaces_task
                .iter()
                .chain(&active.space_task)
                .for_each(|t| t.abort());
            active.sync.stop().await;
            if let Err(e) = active.client.oauth().logout().await {
                warn!("server-side logout failed, wiping locally anyway: {e}");
            }
        }
        self.wipe_local_data().await
    }

    async fn build_client(
        &self,
        server: Server<'_>,
        passphrase: &str,
        store_dir: &str,
    ) -> Result<Client> {
        let builder = match server {
            // User input: resolve via .well-known.
            Server::Name(name) => Client::builder().server_name_or_homeserver_url(name),
            // Already resolved at login: skip discovery.
            Server::Url(url) => Client::builder().homeserver_url(url.as_str()),
        };
        #[cfg(not(target_family = "wasm"))]
        let builder = builder.sqlite_store(self.data_dir.join(store_dir), Some(passphrase));
        // In a browser: IndexedDB databases named after the store "folder".
        #[cfg(target_family = "wasm")]
        let builder = builder.indexeddb_store(store_dir, Some(passphrase));
        Ok(builder
            .handle_refresh_tokens()
            // Same as Element X: fetch missing room keys from the key backup when a message
            // can't be decrypted, and keep backing up our own keys.
            .with_encryption_settings(EncryptionSettings {
                auto_enable_cross_signing: true,
                backup_download_strategy: BackupDownloadStrategy::AfterDecryptionFailure,
                auto_enable_backups: true,
            })
            .build()
            .await?)
    }

    /// Start syncing, store the client and keep the session file up to date when tokens rotate.
    async fn activate(&self, client: Client, passphrase: String, store_dir: String) -> Result<()> {
        let path = self.data_dir.join(SESSION_FILE);
        persist(&path, &client, &passphrase, &store_dir)?;
        let saved_dir = store_dir.clone();
        let may_persist = Arc::new(AtomicBool::new(true));
        let allowed = may_persist.clone();

        // Refresh tokens rotate and the old one dies: save the new pair *inside* the refresh,
        // before it's used, so a crash or restart can never leave a stale token on disk
        // (MAS revokes the whole session when a used refresh token comes back).
        client.set_session_callbacks(
            Box::new(|client| {
                client
                    .session_tokens()
                    .ok_or_else(|| "no session tokens".into())
            }),
            Box::new(move |client| {
                if !allowed.load(Ordering::SeqCst) {
                    warn!("tokens refreshed after sign-out, not saving");
                    return Ok(());
                }
                info!("tokens refreshed, saving session");
                Ok(persist(&path, &client, &passphrase, &saved_dir)?)
            }),
        )?;

        let mut changes = client.subscribe_to_session_changes();
        let ended = self.on_session_ended.clone();
        let checked = client.clone();
        crate::rt::spawn(async move {
            while let Ok(change) = changes.recv().await {
                let SessionChange::UnknownToken(info) = change else {
                    continue;
                };
                // The SDK also sends this for refresh failures that aren't a sign-out ("this
                // isn't necessarily correct", matrix-sdk client/futures.rs). Ask the server.
                warn!(
                    soft_logout = info.soft_logout,
                    "server may have ended the session, checking"
                );
                crate::rt::sleep(Duration::from_secs(2)).await;
                match checked.whoami().await {
                    Ok(_) => warn!("session is fine, ignoring the signal"),
                    Err(e) if is_unknown_token(&e) => {
                        warn!("session ended by the server");
                        if let Some(handler) = ended.lock().unwrap().as_ref() {
                            handler();
                        }
                        return;
                    }
                    Err(e) => warn!("couldn't check the session ({e}), keeping it"),
                }
            }
        });

        let verifications = crate::encryption::Verifications::install(&client);
        crate::notifications::install(&client, self.on_notification.clone(), self.settings.clone())
            .await;
        crate::rtc::install(&client, self.on_call_signal.clone());
        crate::drafts::install(&client, self.on_draft.clone());
        let search = crate::search::install(&client, &self.data_dir.join(&store_dir));

        // The event cache feeds timelines and chat-list previews; it must run before sync starts.
        client
            .event_cache()
            .subscribe()
            .map_err(|e| Error::Sync(e.to_string()))?;
        let sync = Arc::new(
            SyncService::builder(client.clone())
                // Network loss: wait for the server to answer again instead of stopping.
                .with_offline_mode()
                .build()
                .await
                .map_err(|e| Error::Sync(e.to_string()))?,
        );
        sync.start().await;

        *self.active.lock().unwrap() = Some(Active {
            client,
            sync,
            tasks: Vec::new(),
            chat: None,
            verifications,
            encryption_task: None,
            may_persist,
            search,
            search_task: None,
            spaces: None,
            spaces_task: None,
            space_task: None,
        });
        Ok(())
    }

    /// The connection as the UI shows it, on each change: "connecting" (first sync of this
    /// start, or catching up), "online", "offline" (no network: retried by the SDK) or "error".
    /// A sync stopped by an error is restarted after a pause.
    pub fn subscribe_connection(
        &self,
        sink: impl Fn(&'static str) + Send + Sync + 'static,
    ) -> Result<()> {
        use matrix_sdk_ui::{
            room_list_service::State as ListState, sync_service::State as SyncState,
        };
        let sync = self.with_active(|a| a.sync.clone())?;
        let client = self.client()?;
        let mut sync_state = sync.state();
        let mut list_state = sync.room_list_service().state();
        let task = crate::rt::spawn(async move {
            let mut last = "";
            loop {
                let status = match (sync_state.get(), list_state.get()) {
                    (SyncState::Offline, _) => "offline",
                    (SyncState::Error(_), _) => "error",
                    (SyncState::Running, ListState::Running) => "online",
                    _ => "connecting",
                };
                if status != last {
                    // Back online: a send that failed on the network paused its chat's queue;
                    // resume them all (failed messages go out by themselves).
                    if status == "online" && !last.is_empty() {
                        client.send_queue().set_enabled(true).await;
                    }
                    sink(status);
                    last = status;
                }
                if status == "error" {
                    crate::rt::sleep(Duration::from_secs(3)).await;
                    sync.start().await;
                }
                tokio::select! {
                    _ = sync_state.next() => {}
                    _ = list_state.next() => {}
                }
            }
        });
        self.with_active(|a| a.tasks.push(task.abort_handle()))
    }

    /// The app is back in front: make sure the sync runs (no-op when it does).
    pub async fn resume_sync(&self) -> Result<()> {
        let sync = self.with_active(|a| a.sync.clone())?;
        sync.start().await;
        self.client()?.send_queue().set_enabled(true).await;
        Ok(())
    }

    #[cfg(not(target_family = "wasm"))]
    async fn remove_store_dirs(&self) -> Result<()> {
        let mut dirs = vec![self.data_dir.join(STORE_DIR)];
        if let Ok(mut entries) = tokio::fs::read_dir(&self.data_dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                if entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(STORE_PREFIX)
                {
                    dirs.push(entry.path());
                }
            }
        }
        for dir in dirs {
            match tokio::fs::remove_dir_all(&dir).await {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e.into()),
                _ => {}
            }
        }
        Ok(())
    }

    async fn wipe_local_data(&self) -> Result<()> {
        // The old single folder and every per-sign-in one. (In a browser, the web app deletes
        // old IndexedDB stores: Rust can't list them.)
        #[cfg(not(target_family = "wasm"))]
        self.remove_store_dirs().await?;
        // The previous session's keychain entry.
        if let Ok(bytes) = crate::files::read(self.data_dir.join(SESSION_FILE)).await
            && let Ok(old) = serde_json::from_slice::<StoredSession>(&bytes)
        {
            let account = keychain_account(&old.user);
            let _ = crate::rt::blocking(move || keychain::delete(&account)).await;
        }
        // A new sign-in starts without the previous person's app lock.
        self.set_app_lock(None).await?;
        match crate::files::remove_file(self.data_dir.join(SESSION_FILE)).await {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.into()),
            _ => Ok(()),
        }
    }
}

fn is_unknown_token(e: &matrix_sdk::HttpError) -> bool {
    matches!(
        e.client_api_error_kind(),
        Some(matrix_sdk::ruma::api::error::ErrorKind::UnknownToken { .. })
    ) || matches!(e, matrix_sdk::HttpError::RefreshToken(_))
}

enum Server<'a> {
    Name(&'a str),
    Url(&'a Url),
}

/// `web`: the web app, which the server knows by its own address (its redirect must be there).
fn client_metadata(redirect_uri: Url, web: bool) -> Result<Raw<ClientMetadata>> {
    let client_uri = if web {
        let mut origin = redirect_uri.clone();
        origin.set_path("/");
        origin.set_query(None);
        origin.set_fragment(None);
        origin
    } else {
        Url::parse(CLIENT_URI).expect("valid constant URL")
    };
    let metadata = ClientMetadata {
        client_name: Some(Localized::new("AlwyChat".to_owned(), [])),
        ..ClientMetadata::new(
            if web {
                ApplicationType::Web
            } else {
                ApplicationType::Native
            },
            vec![OAuthGrantType::AuthorizationCode {
                redirect_uris: vec![redirect_uri],
            }],
            Localized::new(client_uri, []),
        )
    };
    Ok(Raw::new(&metadata)?)
}

/// From what we already know: no network here, it would hold up the start (the UI fetches the
/// profile in the background).
async fn account(client: &Client) -> Account {
    Account {
        user_id: client
            .user_id()
            .map(ToString::to_string)
            .unwrap_or_default(),
        device_id: client
            .device_id()
            .map(ToString::to_string)
            .unwrap_or_default(),
        display_name: None,
    }
}

fn persist(path: &Path, client: &Client, passphrase: &str, store_dir: &str) -> std::io::Result<()> {
    let Some(oauth) = client.oauth().full_session() else {
        warn!("no OAuth session to persist");
        return Ok(());
    };
    let mut stored = StoredSession {
        homeserver: client.homeserver(),
        client_id: oauth.client_id.as_str().to_owned(),
        user: oauth.user,
        store_passphrase: passphrase.to_owned(),
        store_dir: store_dir.to_owned(),
        in_keychain: false,
    };
    let secrets = Secrets {
        tokens: stored.user.tokens.clone(),
        store_passphrase: passphrase.to_owned(),
    };
    if keychain::save(&keychain_account(&stored.user), &secrets) {
        stored.in_keychain = true;
        stored.store_passphrase = String::new();
        stored.user.tokens = matrix_sdk::SessionTokens {
            access_token: String::new(),
            refresh_token: None,
        };
    }
    write_private(path, &serde_json::to_vec(&stored).expect("serializable")).inspect_err(|e| {
        warn!("couldn't save session: {e}");
    })
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
