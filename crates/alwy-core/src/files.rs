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
