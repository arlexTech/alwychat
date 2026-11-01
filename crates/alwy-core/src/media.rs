//! Media for the UI: avatars, and images/videos/files of timeline messages (decrypted).

#[cfg(not(target_family = "wasm"))]
use std::path::{Path, PathBuf};
use std::{collections::HashMap, sync::Mutex};

use matrix_sdk::{
    media::{MediaFormat, MediaRequestParameters, MediaThumbnailSettings},
    ruma::{OwnedMxcUri, UInt, events::room::MediaSource},
};

use crate::{Error, Result};

impl crate::Core {
    /// A thumbnail of at least `size`×`size` px for an unencrypted `mxc://` URI (cached locally).
    pub async fn thumbnail(&self, mxc: &str, size: u32) -> Result<Vec<u8>> {
        let client = self.client()?;
        let uri = OwnedMxcUri::from(mxc);
        if !uri.is_valid() {
            return Err(Error::InvalidMedia(mxc.to_owned()));
        }
        let side = UInt::from(size.clamp(16, 1024));
        let request = MediaRequestParameters {
            source: MediaSource::Plain(uri),
            format: MediaFormat::Thumbnail(MediaThumbnailSettings::new(side, side)),
        };
        Ok(client.media().get_media_content(&request, true).await?)
    }
}

/// Media of timeline messages, by item ID, so the UI can request it with a short URL
/// (`media://localhost/_event/<item id>`) without ever seeing encryption keys.
#[derive(Default)]
pub(crate) struct MediaRegistry(Mutex<HashMap<String, EventMedia>>);

#[derive(Clone)]
pub(crate) struct EventMedia {
    pub source: MediaSource,
    pub thumbnail: Option<MediaSource>,
    pub mimetype: Option<String>,
    pub filename: String,
}

impl MediaRegistry {
    pub(crate) fn insert(&self, key: String, media: EventMedia) {
        self.0.lock().unwrap().insert(key, media);
    }

    fn get(&self, key: &str) -> Result<EventMedia> {
        self.0
            .lock()
            .unwrap()
            .get(key)
            .cloned()
            .ok_or_else(|| Error::InvalidMedia(key.to_owned()))
    }
}

impl crate::Core {
    /// The file name a message's media was sent with (for downloads in a browser).
    pub fn media_filename(&self, key: &str) -> Result<String> {
        Ok(self.media.get(key)?.filename)
    }

    /// Bytes and MIME type of a timeline message's media (decrypted, cached locally).
    /// `thumbnail` prefers the sender's thumbnail, else a server thumbnail for unencrypted media.
    pub async fn event_media(&self, key: &str, thumbnail: bool) -> Result<(Vec<u8>, String)> {
        let client = self.client()?;
        let media = self.media.get(key)?;
        let (request, mimetype) = match (thumbnail, &media.thumbnail, &media.source) {
            (true, Some(thumb), _) => (
                MediaRequestParameters {
                    source: thumb.clone(),
                    format: MediaFormat::File,
                },
                None,
            ),
            (true, None, MediaSource::Plain(_)) => (
                MediaRequestParameters {
                    source: media.source.clone(),
                    format: MediaFormat::Thumbnail(MediaThumbnailSettings::new(
                        UInt::from(800u32),
                        UInt::from(800u32),
                    )),
                },
                None,
            ),
            _ => (
                MediaRequestParameters {
                    source: media.source.clone(),
                    format: MediaFormat::File,
                },
                media.mimetype.clone(),
            ),
        };
        let bytes = client.media().get_media_content(&request, true).await?;
        let mimetype = mimetype.unwrap_or_else(|| sniff(&bytes).to_owned());
        Ok((bytes, mimetype))
    }

    /// Save a message's media into `dir` (created if needed) and return the file path.
    /// Never overwrites: `photo.jpg` becomes `photo (1).jpg` if taken. (A browser downloads.)
    #[cfg(not(target_family = "wasm"))]
    pub async fn save_event_media(&self, key: &str, dir: &Path) -> Result<PathBuf> {
        let media = self.media.get(key)?;
        let (bytes, _) = self.event_media(key, false).await?;
        tokio::fs::create_dir_all(dir).await?;
        let name = Path::new(&media.filename)
            .file_name()
            .and_then(|n| n.to_str())
            .filter(|n| !n.is_empty())
            .unwrap_or("media")
            .to_owned();
        let (stem, ext) = match name.rsplit_once('.') {
            Some((s, e)) if !s.is_empty() => (s.to_owned(), format!(".{e}")),
            _ => (name.clone(), String::new()),
        };
        let mut path = dir.join(&name);
        let mut n = 1;
        while tokio::fs::try_exists(&path).await? {
            path = dir.join(format!("{stem} ({n}){ext}"));
            n += 1;
        }
        tokio::fs::write(&path, bytes).await?;
        Ok(path)
    }
}

/// MIME type from magic bytes, for images served without a declared type.
pub fn sniff(bytes: &[u8]) -> &'static str {
    match bytes {
        [0x89, b'P', b'N', b'G', ..] => "image/png",
        [0xFF, 0xD8, ..] => "image/jpeg",
        [b'G', b'I', b'F', ..] => "image/gif",
        [
            b'R',
            b'I',
            b'F',
            b'F',
            _,
            _,
            _,
            _,
            b'W',
            b'E',
            b'B',
            b'P',
            ..,
        ] => "image/webp",
        [_, _, _, _, b'f', b't', b'y', b'p', ..] => "video/mp4",
        _ => "application/octet-stream",
    }
}
