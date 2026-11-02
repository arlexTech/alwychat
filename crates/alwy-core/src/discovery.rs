use matrix_sdk::Client;
use serde::Serialize;

use crate::{Error, Result};

/// What we learn about a homeserver before logging in.
#[derive(Debug, Clone, Serialize)]
pub struct ServerInfo {
    pub homeserver: String,
    pub versions: Vec<String>,
    /// MAS / OAuth 2.0 issuer, if the server uses next-gen auth.
    pub oauth_issuer: Option<String>,
    pub sliding_sync: bool,
}

/// Resolve `server` (e.g. `matrix.org`) via .well-known and probe its capabilities.
pub async fn discover(server: &str) -> Result<ServerInfo> {
    let client = Client::builder()
        .server_name_or_homeserver_url(server)
        .build()
        .await?;

    let versions = client.server_versions().await?;
    let oauth_issuer = match client.oauth().server_metadata().await {
        Ok(meta) => Some(meta.issuer.to_string()),
        Err(e) if e.is_not_supported() => None,
        Err(e) => return Err(Error::OAuth(e.to_string())),
    };
    let sliding_sync = !client.available_sliding_sync_versions().await.is_empty();

    Ok(ServerInfo {
        homeserver: client.homeserver().to_string(),
        versions: versions
            .iter()
            .filter_map(|v| v.as_str())
            .map(str::to_owned)
            .collect(),
        oauth_issuer,
        sliding_sync,
    })
}
