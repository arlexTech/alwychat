//! Link preview cards, fetched by the homeserver (`/media/preview_url`, OpenGraph data).

use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex},
};

use matrix_sdk::ruma::api::client::authenticated_media::get_media_preview;
use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::Result;

#[derive(Debug, Clone, Serialize)]
pub struct LinkPreview {
    pub url: String,
    pub site_name: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    /// `mxc://` image, stored by the homeserver.
    pub image: Option<String>,
    pub image_width: Option<u32>,
    pub image_height: Option<u32>,
}

#[derive(Deserialize)]
struct OpenGraph {
    #[serde(rename = "og:site_name")]
    site_name: Option<String>,
    #[serde(rename = "og:title")]
    title: Option<String>,
    #[serde(rename = "og:description")]
    description: Option<String>,
    #[serde(rename = "og:image")]
    image: Option<String>,
    #[serde(rename = "og:image:width", default, deserialize_with = "number")]
    image_width: Option<u32>,
    #[serde(rename = "og:image:height", default, deserialize_with = "number")]
    image_height: Option<u32>,
}

/// Sites send sizes as numbers or strings.
fn number<'de, D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Option<u32>, D::Error> {
    Ok(match serde_json::Value::deserialize(d)? {
        serde_json::Value::Number(n) => n.as_u64().map(|n| n as u32),
        serde_json::Value::String(s) => s.parse().ok(),
        _ => None,
    })
}

/// Previews already asked for this run (also remembers "nothing to show").
static CACHE: LazyLock<Mutex<HashMap<String, Option<LinkPreview>>>> = LazyLock::new(Mutex::default);

impl crate::Core {
    /// The preview card for `url`, or `None` when the page has nothing to show.
    pub async fn link_preview(&self, url: &str) -> Result<Option<LinkPreview>> {
        if let Some(hit) = CACHE.lock().unwrap().get(url) {
            return Ok(hit.clone());
        }
        let client = self.client()?;
        let preview = match client
            .send(get_media_preview::v1::Request::new(url.to_owned()))
            .await
        {
            Ok(response) => response
                .data
                .and_then(|raw| serde_json::from_str::<OpenGraph>(raw.get()).ok())
                .map(|og| LinkPreview {
                    url: url.to_owned(),
                    site_name: og.site_name.filter(|s| !s.trim().is_empty()),
                    title: og.title.filter(|s| !s.trim().is_empty()),
                    description: og.description.filter(|s| !s.trim().is_empty()),
                    image: og.image.filter(|i| i.starts_with("mxc://")),
                    image_width: og.image_width,
                    image_height: og.image_height,
                })
                .filter(|p| p.title.is_some() || p.description.is_some() || p.image.is_some()),
            // The server answered "no" (unreachable page, blocked address…): no card, and
            // don't ask again this run.
            Err(e) if e.client_api_error_kind().is_some() => {
                debug!(url, "no link preview: {e}");
                None
            }
            // We couldn't reach the server: no card for now, asked again next time.
            Err(e) => {
                debug!(url, "link preview not loaded: {e}");
                return Ok(None);
            }
        };
        CACHE
            .lock()
            .unwrap()
            .insert(url.to_owned(), preview.clone());
        Ok(preview)
    }
}
