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

    /// The whole file behind an unencrypted `mxc://` (soundboard sounds).
    pub async fn plain_file(&self, mxc: &str) -> Result<Vec<u8>> {
        let client = self.client()?;
        let uri = OwnedMxcUri::from(mxc);
        if !uri.is_valid() {
            return Err(Error::InvalidMedia(mxc.to_owned()));
        }
        let request = MediaRequestParameters {
            source: MediaSource::Plain(uri),
            format: MediaFormat::File,
        };
        Ok(client.media().get_media_content(&request, true).await?)
    }
}

/// Media of timeline messages, by item ID, so the UI can request it with a short URL
/// (`media://localhost/_event/<item id>`) without ever seeing encryption keys.
/// How long a message's preview may take before it fails (and the bubble retries).
const PREVIEW_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

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
        // An encrypted video sent without a thumbnail has no preview: the whole video would
        // be downloaded just to fail as a picture.
        if thumbnail
            && media.thumbnail.is_none()
            && matches!(media.source, MediaSource::Encrypted(_))
            && media
                .mimetype
                .as_deref()
                .is_some_and(|m| !m.starts_with("image/"))
        {
            return Err(Error::InvalidMedia(format!("{key} has no preview")));
        }
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
        let media_api = client.media();
        let download = media_api.get_media_content(&request, true);
        // A preview that hangs (network gone quiet) fails after a while, so the bubble retries
        // instead of waiting forever. Whole files (videos, the viewer) may take longer.
        let bytes = if thumbnail {
            let limit = crate::rt::sleep(PREVIEW_TIMEOUT);
            futures_util::pin_mut!(download, limit);
            match futures_util::future::select(download, limit).await {
                futures_util::future::Either::Left((result, _)) => result?,
                futures_util::future::Either::Right(_) => {
                    return Err(crate::Error::Download("the preview took too long".into()));
                }
            }
        } else {
            download.await?
        };
        let mimetype = mimetype.unwrap_or_else(|| sniff(&bytes).to_owned());
        Ok((bytes, mimetype))
    }

    /// Save a message's media into `dir` (created if needed) and return the file path.
    /// Never overwrites: `photo.jpg` becomes `photo (1).jpg` if taken. (A browser downloads.)
    #[cfg(not(target_family = "wasm"))]
    pub async fn save_event_media(&self, key: &str, dir: &Path) -> Result<PathBuf> {
        let media = self.media.get(key)?;
        let (bytes, _) = self.event_media(key, false).await?;
        write_unique(dir, &media.filename, &bytes).await
    }
}

/// Write `bytes` as `name` (made safe) in `dir`, created if needed, never overwriting:
/// `photo.jpg` becomes `photo (1).jpg` if taken.
#[cfg(not(target_family = "wasm"))]
pub(crate) async fn write_unique(dir: &Path, name: &str, bytes: &[u8]) -> Result<PathBuf> {
    tokio::fs::create_dir_all(dir).await?;
    let name = safe_file_name(name);
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

/// A file name from someone else's message, safe to create anywhere: no folders, no characters
/// Windows reserves (`a.txt:stream`), no control characters, no leading dots (hidden files),
/// no reserved device names (`CON`, `NUL`…).
pub fn safe_file_name(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or("");
    let cleaned: String = base
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '|' | '?' | '*' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    let cleaned = cleaned
        .trim()
        .trim_start_matches('.')
        .trim_end_matches(['.', ' ']);
    let stem = cleaned.split('.').next().unwrap_or("").to_ascii_uppercase();
    let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.as_bytes()[3].is_ascii_digit());
    if cleaned.is_empty() || reserved {
        format!(
            "media{}",
            if cleaned.is_empty() {
                String::new()
            } else {
                format!("-{cleaned}")
            }
        )
    } else {
        cleaned.chars().take(200).collect()
    }
}

/// Whether opening this file could run something (a program, script, shortcut or installer):
/// those can be saved but not opened from a message.
pub fn runs_when_opened(name: &str) -> bool {
    const RISKY: &[&str] = &[
        "exe",
        "msi",
        "msix",
        "appx",
        "bat",
        "cmd",
        "com",
        "scr",
        "pif",
        "cpl",
        "ps1",
        "psm1",
        "vbs",
        "vbe",
        "js",
        "jse",
        "wsf",
        "wsh",
        "hta",
        "lnk",
        "url",
        "msc",
        "msp",
        "mst",
        "gadget",
        "application",
        "appref-ms",
        "scf",
        "chm",
        "hlp",
        "diagcab",
        "vb",
        "ws",
        "wsc",
        "sct",
        "ps2",
        "psd1",
        "reg",
        "inf",
        "jar",
        "jnlp",
        "desktop",
        "sh",
        "bash",
        "zsh",
        "run",
        "bin",
        "appimage",
        "deb",
        "rpm",
        "flatpakref",
        "app",
        "command",
        "pkg",
        "dmg",
        "workflow",
        "action",
        "scpt",
        "py",
        "pyw",
        "pl",
        "rb",
        "apk",
        "xapk",
        "iso",
        "img",
        "vhd",
        "vhdx",
        "library-ms",
        "settingcontent-ms",
    ];
    // Windows ignores trailing dots and spaces: "x.exe. " runs as "x.exe".
    let lower = name.to_ascii_lowercase();
    lower
        .trim_end_matches([' ', '.'])
        .rsplit_once('.')
        .is_some_and(|(_, ext)| RISKY.contains(&ext))
}

/// MIME type from magic bytes, for images served without a declared type.
/// A HEIC/HEIF photo (an ISO-BMFF "ftyp" box with a HEIF brand), which web views can't show.
pub fn is_heic(bytes: &[u8]) -> bool {
    bytes.len() >= 12
        && &bytes[4..8] == b"ftyp"
        && matches!(
            &bytes[8..12],
            b"heic" | b"heix" | b"heim" | b"heis" | b"hevc" | b"hevx" | b"mif1" | b"msf1" | b"heif"
        )
}

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
        // ISO media: the brand tells photos (AVIF, HEIC) from videos.
        [_, _, _, _, b'f', b't', b'y', b'p', brand @ ..] => match brand.get(..4) {
            Some(b"avif" | b"avis") => "image/avif",
            Some(b"heic" | b"heix" | b"hevc" | b"hevx" | b"mif1" | b"msf1") => "image/heic",
            Some(b"qt  ") => "video/quicktime",
            Some(b"M4A " | b"M4B ") => "audio/mp4",
            _ => "video/mp4",
        },
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod file_name_tests {
    use super::{runs_when_opened, safe_file_name, sniff};

    #[test]
    fn iso_media_by_brand() {
        assert_eq!(sniff(b"\0\0\0\x1cftypavif\0\0\0\0"), "image/avif");
        assert_eq!(sniff(b"\0\0\0\x18ftypheic\0\0\0\0"), "image/heic");
        assert_eq!(sniff(b"\0\0\0\x14ftypqt  \0\0\0\0"), "video/quicktime");
        assert_eq!(sniff(b"\0\0\0\x18ftypisom\0\0\0\0"), "video/mp4");
        assert_eq!(sniff(b"\0\0\0\x18ftyp"), "video/mp4");
        assert!(runs_when_opened("setup.MSC") && runs_when_opened("a.appref-ms"));
    }

    #[test]
    fn names_from_messages() {
        assert_eq!(safe_file_name("photo.jpg"), "photo.jpg");
        assert_eq!(safe_file_name("../../.bashrc"), "bashrc");
        assert_eq!(safe_file_name("..\\..\\evil.dll"), "evil.dll");
        assert_eq!(safe_file_name("a.txt:hidden"), "a.txt_hidden");
        assert_eq!(safe_file_name("CON.txt"), "media-CON.txt");
        assert_eq!(safe_file_name("com1"), "media-com1");
        assert_eq!(safe_file_name("..."), "media");
        assert_eq!(safe_file_name(""), "media");
        assert_eq!(safe_file_name("report\u{0}.pdf"), "report_.pdf");
    }

    #[test]
    fn risky_files() {
        assert!(runs_when_opened("invoice.pdf.exe"));
        assert!(runs_when_opened("Setup.MSI"));
        assert!(runs_when_opened("game.AppImage"));
        assert!(runs_when_opened("x.desktop"));
        assert!(runs_when_opened("trick.exe. "));
        assert!(!runs_when_opened("photo.jpg"));
        assert!(!runs_when_opened("notes.pdf"));
        assert!(!runs_when_opened("README"));
    }
}

#[cfg(test)]
mod heic_tests {
    #[test]
    fn detects_heic() {
        let mut heic = vec![0, 0, 0, 24];
        heic.extend(b"ftypheic");
        heic.extend([0; 16]);
        assert!(super::is_heic(&heic));
        let mut mp4 = vec![0, 0, 0, 24];
        mp4.extend(b"ftypisom");
        assert!(!super::is_heic(&mp4));
        assert!(!super::is_heic(&[
            0xFF, 0xD8, 0xFF, 0xE0, 0, 0, 0, 0, 0, 0, 0, 0
        ]));
    }
}
