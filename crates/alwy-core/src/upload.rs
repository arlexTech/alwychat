//! Sending photos, videos and files: HD/compressed policy, thumbnails, albums.

use std::io::Cursor;

use image::{DynamicImage, ImageDecoder, ImageReader, codecs::jpeg::JpegEncoder};
use matrix_sdk::{
    attachment::{AttachmentInfo, BaseFileInfo, BaseImageInfo, BaseVideoInfo, Thumbnail},
    ruma::{UInt, events::RoomAccountDataEventType, serde::Raw},
};
use matrix_sdk_ui::timeline::{AttachmentConfig, AttachmentSource};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{Error, Result};

/// Longest side of a compressed photo, and of a thumbnail.
const COMPRESSED_MAX_SIDE: u32 = 2048;
const THUMBNAIL_MAX_SIDE: u32 = 800;
const COMPRESSED_QUALITY: u8 = 82;
const THUMBNAIL_QUALITY: u8 = 75;
/// Per-chat media preferences (room account data, private, synced between our devices).
const MEDIA_PREFS: &str = "com.alwy.chat.room.media_prefs";

/// One file to send, as described by the UI (the bytes travel separately).
#[derive(Debug, Deserialize)]
pub struct OutgoingFile {
    pub filename: String,
    pub mime: Option<String>,
    pub caption: Option<String>,
    /// Set on every photo of an album, so we can show them as one grid.
    pub gallery: Option<GalleryRef>,
}

/// `com.alwy.chat.gallery`: which album a photo belongs to (other clients just show the photos).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GalleryRef {
    pub id: String,
    pub index: u32,
    pub count: u32,
}

impl crate::Core {
    /// Send one photo/video/file to the open chat. Photos follow the chat's HD setting
    /// (else the app default): compressed photos are resized and re-encoded as JPEG.
    pub async fn send_file(&self, room_id: &str, file: OutgoingFile, bytes: Vec<u8>) -> Result<()> {
        let timeline = self.timeline(room_id)?;
        let hd = self
            .chat_media_hd(room_id)
            .await?
            .unwrap_or(self.settings().hd_media);

        let mime = file
            .mime
            .filter(|m| !m.is_empty() && m != "application/octet-stream")
            .unwrap_or_else(|| guess_mime(&file.filename, &bytes).to_owned());
        let mut config = AttachmentConfig {
            caption: file
                .caption
                .filter(|c| !c.trim().is_empty())
                .map(matrix_sdk::ruma::events::room::message::TextMessageEventContent::markdown),
            extra_content: file.gallery.map(|g| {
                let mut map = serde_json::Map::new();
                map.insert("com.alwy.chat.gallery".to_owned(), json!(g));
                map
            }),
            ..Default::default()
        };

        let (bytes, mime, filename) = if is_processable_image(&mime) {
            let filename = file.filename.clone();
            let processed = tokio::task::spawn_blocking(move || prepare_image(bytes, &mime, hd))
                .await
                .map_err(|e| Error::Upload(e.to_string()))??;
            config.info = Some(AttachmentInfo::Image(BaseImageInfo {
                width: Some(UInt::from(processed.width)),
                height: Some(UInt::from(processed.height)),
                size: UInt::new(processed.bytes.len() as u64),
                ..Default::default()
            }));
            config.thumbnail = processed.thumbnail;
            let filename = if processed.reencoded {
                with_extension(&filename, "jpg")
            } else {
                filename
            };
            (processed.bytes, processed.mime, filename)
        } else {
            let size = UInt::new(bytes.len() as u64);
            config.info = Some(if mime.starts_with("video/") {
                AttachmentInfo::Video(BaseVideoInfo {
                    size,
                    ..Default::default()
                })
            } else {
                AttachmentInfo::File(BaseFileInfo { size })
            });
            (bytes, mime, file.filename)
        };

        let mime: mime::Mime = mime.parse().unwrap_or(mime::APPLICATION_OCTET_STREAM);
        timeline
            .send_attachment(AttachmentSource::Data { bytes, filename }, mime, config)
            .use_send_queue()
            .await
            .map_err(|e| Error::Upload(e.to_string()))
    }

    /// The chat's own HD choice, or `None` to follow the app default.
    pub async fn chat_media_hd(&self, room_id: &str) -> Result<Option<bool>> {
        let room = self.room(room_id)?;
        let Some(raw) = room
            .account_data(RoomAccountDataEventType::from(MEDIA_PREFS))
            .await?
        else {
            return Ok(None);
        };
        let content: serde_json::Value = raw.get_field("content")?.unwrap_or_default();
        Ok(content.get("hd").and_then(|v| v.as_bool()))
    }

    /// Set (or clear, with `None`) the chat's HD choice.
    pub async fn set_chat_media_hd(&self, room_id: &str, hd: Option<bool>) -> Result<()> {
        let room = self.room(room_id)?;
        let content = match hd {
            Some(hd) => json!({ "hd": hd }),
            None => json!({}),
        };
        room.set_account_data_raw(
            RoomAccountDataEventType::from(MEDIA_PREFS),
            Raw::new(&content)?.cast_unchecked(),
        )
        .await?;
        Ok(())
    }
}

struct PreparedImage {
    bytes: Vec<u8>,
    mime: String,
    width: u32,
    height: u32,
    thumbnail: Option<Thumbnail>,
    reencoded: bool,
}

/// Decode once (applying the camera's EXIF rotation), maybe compress, and make a thumbnail.
fn prepare_image(bytes: Vec<u8>, mime: &str, hd: bool) -> Result<PreparedImage> {
    let image = decode_oriented(&bytes)?;
    let (width, height) = (image.width(), image.height());

    let thumbnail = (width.max(height) > THUMBNAIL_MAX_SIDE)
        .then(|| {
            let small = image.thumbnail(THUMBNAIL_MAX_SIDE, THUMBNAIL_MAX_SIDE);
            let data = encode_jpeg(&small, THUMBNAIL_QUALITY).ok()?;
            Some(Thumbnail {
                height: UInt::from(small.height()),
                width: UInt::from(small.width()),
                size: UInt::new(data.len() as u64)?,
                data,
                content_type: mime::IMAGE_JPEG,
            })
        })
        .flatten();

    if hd {
        // Original bytes untouched (EXIF kept; viewers rotate), but report rotated size.
        return Ok(PreparedImage {
            bytes,
            mime: mime.to_owned(),
            width,
            height,
            thumbnail,
            reencoded: false,
        });
    }

    let resized = if width.max(height) > COMPRESSED_MAX_SIDE {
        image.resize(
            COMPRESSED_MAX_SIDE,
            COMPRESSED_MAX_SIDE,
            image::imageops::FilterType::Lanczos3,
        )
    } else {
        image
    };
    let jpeg = encode_jpeg(&resized, COMPRESSED_QUALITY)?;
    // Small originals can be smaller than our re-encode (e.g. PNG screenshots): keep them.
    if jpeg.len() >= bytes.len() && resized.width() == width {
        return Ok(PreparedImage {
            bytes,
            mime: mime.to_owned(),
            width,
            height,
            thumbnail,
            reencoded: false,
        });
    }
    Ok(PreparedImage {
        width: resized.width(),
        height: resized.height(),
        bytes: jpeg,
        mime: "image/jpeg".to_owned(),
        thumbnail,
        reencoded: true,
    })
}

fn decode_oriented(bytes: &[u8]) -> Result<DynamicImage> {
    let bad = |e: image::ImageError| Error::Upload(format!("unreadable image: {e}"));
    let mut decoder = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| Error::Upload(e.to_string()))?
        .into_decoder()
        .map_err(bad)?;
    let orientation = decoder.orientation().map_err(bad)?;
    let mut image = DynamicImage::from_decoder(decoder).map_err(bad)?;
    image.apply_orientation(orientation);
    Ok(image)
}

fn encode_jpeg(image: &DynamicImage, quality: u8) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    // JPEG has no alpha: flatten to RGB first.
    image
        .to_rgb8()
        .write_with_encoder(JpegEncoder::new_with_quality(&mut out, quality))
        .map_err(|e| Error::Upload(e.to_string()))?;
    Ok(out)
}

/// Photos we can decode and resize. GIFs stay untouched (animation).
fn is_processable_image(mime: &str) -> bool {
    matches!(mime, "image/jpeg" | "image/png" | "image/webp")
}

fn guess_mime(filename: &str, bytes: &[u8]) -> &'static str {
    let ext = filename
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "mp4" | "m4v" => "video/mp4",
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        "mp3" => "audio/mpeg",
        "ogg" | "oga" => "audio/ogg",
        "pdf" => "application/pdf",
        "txt" => "text/plain",
        _ => crate::media::sniff(bytes),
    }
}

fn with_extension(filename: &str, ext: &str) -> String {
    match filename.rsplit_once('.') {
        Some((stem, _)) if !stem.is_empty() => format!("{stem}.{ext}"),
        _ => format!("{filename}.{ext}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(width: u32, height: u32) -> Vec<u8> {
        let img = DynamicImage::new_rgb8(width, height);
        let mut out = Vec::new();
        img.write_to(&mut Cursor::new(&mut out), image::ImageFormat::Png)
            .unwrap();
        out
    }

    #[test]
    fn compressed_photos_are_resized_with_a_thumbnail() {
        let p = prepare_image(png(4000, 3000), "image/png", false).unwrap();
        assert_eq!((p.width, p.height), (2048, 1536));
        assert_eq!(p.mime, "image/jpeg");
        assert!(p.reencoded);
        let t = p.thumbnail.unwrap();
        assert_eq!((u64::from(t.width), u64::from(t.height)), (800, 600));
    }

    #[test]
    fn hd_keeps_the_original_bytes() {
        let original = png(3000, 1000);
        let p = prepare_image(original.clone(), "image/png", true).unwrap();
        assert_eq!(p.bytes, original);
        assert_eq!((p.width, p.height), (3000, 1000));
        assert!(p.thumbnail.is_some());
    }

    #[test]
    fn small_images_get_no_thumbnail() {
        let p = prepare_image(png(400, 300), "image/png", false).unwrap();
        assert!(p.thumbnail.is_none());
    }

    #[test]
    fn guesses_types() {
        assert_eq!(guess_mime("IMG_1.JPG", &[]), "image/jpeg");
        assert_eq!(guess_mime("clip.mov", &[]), "video/quicktime");
        assert_eq!(with_extension("photo.png", "jpg"), "photo.jpg");
    }
}
