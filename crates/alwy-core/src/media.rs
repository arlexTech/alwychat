//! Media download for the UI (avatars and thumbnails for now).

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
