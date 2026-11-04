//! Small app files (session, settings, app lock): real files natively, localStorage in a
//! browser, where the path is only a key. Same `io::Result` contract as `tokio::fs`.

#[cfg(not(target_family = "wasm"))]
pub use tokio::fs::{create_dir_all, read, remove_file, rename, write};

/// Read a small file now (outside async code).
#[cfg(not(target_family = "wasm"))]
pub fn read_now(path: &std::path::Path) -> std::io::Result<Vec<u8>> {
    std::fs::read(path)
}

/// Atomic write, readable by the current user only (sessions hold tokens).
#[cfg(not(target_family = "wasm"))]
pub fn write_private_now(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
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

/// The session file (tokens, the store passphrase). Natively a private file (the secrets are in
/// the OS keychain when there is one); in a browser it's sealed, see `web::write_sealed`.
#[cfg(not(target_family = "wasm"))]
pub async fn read_sealed(path: &std::path::Path) -> std::io::Result<Vec<u8>> {
    read(path).await
}

#[cfg(not(target_family = "wasm"))]
pub fn write_sealed(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    write_private_now(path, bytes)
}

#[cfg(not(target_family = "wasm"))]
pub async fn remove_sealed(path: &std::path::Path) -> std::io::Result<()> {
    remove_file(path).await
}

#[cfg(target_family = "wasm")]
mod web {
    use std::{io, path::Path};

    fn storage() -> io::Result<web_sys::Storage> {
        web_sys::window()
            .and_then(|w| w.local_storage().ok().flatten())
            .ok_or_else(|| io::Error::other("no localStorage"))
    }

    fn key(path: &Path) -> String {
        format!("alwychat:{}", path.to_string_lossy())
    }

    fn js(e: wasm_bindgen::JsValue) -> io::Error {
        io::Error::other(format!("{e:?}"))
    }

    pub async fn read(path: impl AsRef<Path>) -> io::Result<Vec<u8>> {
        read_now(path.as_ref())
    }

    pub fn read_now(path: &Path) -> io::Result<Vec<u8>> {
        storage()?
            .get_item(&key(path))
            .map_err(js)?
            .map(String::into_bytes)
            .ok_or_else(|| io::ErrorKind::NotFound.into())
    }

    pub async fn write(path: impl AsRef<Path>, bytes: impl AsRef<[u8]>) -> io::Result<()> {
        write_private_now(path.as_ref(), bytes.as_ref())
    }

    /// localStorage belongs to this site only; writes are atomic.
    pub fn write_private_now(path: &Path, bytes: &[u8]) -> io::Result<()> {
        let text = std::str::from_utf8(bytes).map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidData, "only text files in a browser")
        })?;
        storage()?.set_item(&key(path), text).map_err(js)
    }

    #[wasm_bindgen::prelude::wasm_bindgen(module = "/src/web/sealed.js")]
    extern "C" {
        #[wasm_bindgen(catch)]
        async fn seal(text: &str) -> Result<wasm_bindgen::JsValue, wasm_bindgen::JsValue>;
        #[wasm_bindgen(catch)]
        async fn unseal(text: &str) -> Result<wasm_bindgen::JsValue, wasm_bindgen::JsValue>;
        #[wasm_bindgen(js_name = isSealed)]
        fn is_sealed(text: &str) -> bool;
    }

    /// Sealed writes finish out of order (WebCrypto is asynchronous): each gets a number and
    /// only a newer one may replace what's stored, so a token refresh is never overwritten by
    /// the one before it. A removal skips every write still on its way.
    static QUEUED: AtomicU64 = AtomicU64::new(0);
    static WRITTEN: AtomicU64 = AtomicU64::new(0);
    use std::sync::atomic::{AtomicU64, Ordering::SeqCst};

    /// The session file, unsealed. One saved before sealing existed is sealed now. One that
    /// can't be opened (the browser's key was cleared) counts as missing: signed out.
    pub async fn read_sealed(path: &Path) -> io::Result<Vec<u8>> {
        let text = String::from_utf8(read_now(path)?)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "not text"))?;
        if !is_sealed(&text) {
            write_sealed(path, text.as_bytes())?;
            return Ok(text.into_bytes());
        }
        match unseal(&text).await {
            Ok(plain) => plain
                .as_string()
                .map(String::into_bytes)
                .ok_or_else(|| io::ErrorKind::NotFound.into()),
            Err(e) => {
                tracing::warn!("couldn't unseal the saved session: {e:?}");
                Err(io::ErrorKind::NotFound.into())
            }
        }
    }

    /// Seal with a non-extractable browser key (src/web/sealed.js), then store. Returns at
    /// once; the write lands a moment later.
    pub fn write_sealed(path: &Path, bytes: &[u8]) -> io::Result<()> {
        let text = std::str::from_utf8(bytes)
            .map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidData, "only text files in a browser")
            })?
            .to_owned();
        let n = QUEUED.fetch_add(1, SeqCst) + 1;
        let key = key(path);
        wasm_bindgen_futures::spawn_local(async move {
            let sealed = match seal(&text).await {
                Ok(v) => v.as_string(),
                Err(e) => {
                    // No WebCrypto (a page not served over https): kept as before, unsealed.
                    tracing::warn!("couldn't seal the session: {e:?}");
                    None
                }
            };
            if WRITTEN.load(SeqCst) >= n {
                return;
            }
            if let Ok(storage) = storage()
                && storage.set_item(&key, &sealed.unwrap_or(text)).is_ok()
            {
                WRITTEN.store(n, SeqCst);
            }
        });
        Ok(())
    }

    /// Remove the session file, and drop the sealed writes still on their way (sign-out).
    pub async fn remove_sealed(path: &Path) -> io::Result<()> {
        WRITTEN.fetch_max(QUEUED.load(SeqCst), SeqCst);
        remove_file(path).await
    }

    pub async fn remove_file(path: impl AsRef<Path>) -> io::Result<()> {
        let storage = storage()?;
        let key = key(path.as_ref());
        if storage.get_item(&key).map_err(js)?.is_none() {
            return Err(io::ErrorKind::NotFound.into());
        }
        storage.remove_item(&key).map_err(js)
    }

    pub async fn rename(from: impl AsRef<Path>, to: impl AsRef<Path>) -> io::Result<()> {
        let bytes = read(&from).await?;
        write(&to, bytes).await?;
        remove_file(from).await
    }

    /// Folders don't exist in localStorage.
    pub async fn create_dir_all(_path: impl AsRef<Path>) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(target_family = "wasm")]
pub use web::*;
