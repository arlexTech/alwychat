//! Small app files (session, settings, app lock): real files natively, localStorage in a
//! browser, where the path is only a key. Same `io::Result` contract as `tokio::fs`.

#[cfg(not(target_family = "wasm"))]
pub use tokio::fs::{create_dir_all, read, remove_file, rename, write};

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
        storage()?
            .get_item(&key(path.as_ref()))
            .map_err(js)?
            .map(String::into_bytes)
            .ok_or_else(|| io::ErrorKind::NotFound.into())
    }

    pub async fn write(path: impl AsRef<Path>, bytes: impl AsRef<[u8]>) -> io::Result<()> {
        let text = std::str::from_utf8(bytes.as_ref()).map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidData, "only text files in a browser")
        })?;
        storage()?.set_item(&key(path.as_ref()), text).map_err(js)
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
