//! Plain HTTP downloads outside Matrix (app updates), with the same client stack as the SDK
//! (it already has working TLS on every platform, Android included).

#[cfg(not(target_family = "wasm"))]
use std::path::Path;

use matrix_sdk::reqwest;

use crate::{Error, Result};

fn client() -> Result<reqwest::Client> {
    let builder =
        reqwest::Client::builder().user_agent(concat!("AlwyChat/", env!("CARGO_PKG_VERSION")));
    // A stalled server must not hang an update check or download for ever (browsers have
    // their own limits).
    #[cfg(not(target_family = "wasm"))]
    let builder = builder
        .connect_timeout(std::time::Duration::from_secs(20))
        .read_timeout(std::time::Duration::from_secs(60));
    builder.build().map_err(|e| Error::Download(e.to_string()))
}

/// GET `url` as text (follows redirects: GitHub release files live on another host).
pub async fn fetch_text(url: &str) -> Result<String> {
    client()?
        .get(url)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|e| Error::Download(e.to_string()))?
        .text()
        .await
        .map_err(|e| Error::Download(e.to_string()))
}

/// Like `fetch_text`, but a missing file (404) is `None`: only real failures (offline,
/// server error) are errors.
pub async fn fetch_text_if_any(url: &str) -> Result<Option<String>> {
    let response = client()?
        .get(url)
        .send()
        .await
        .map_err(|e| Error::Download(e.to_string()))?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    let response = response
        .error_for_status()
        .map_err(|e| Error::Download(e.to_string()))?;
    response
        .text()
        .await
        .map(Some)
        .map_err(|e| Error::Download(e.to_string()))
}

/// Download `url` into `path`, reporting (bytes so far, total if known). Not in a browser.
#[cfg(not(target_family = "wasm"))]
pub async fn download_to(
    url: &str,
    path: &Path,
    progress: impl Fn(u64, Option<u64>),
) -> Result<()> {
    use tokio::io::AsyncWriteExt;
    let mut response = client()?
        .get(url)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|e| Error::Download(e.to_string()))?;
    let total = response.content_length();
    let mut file = tokio::fs::File::create(path).await?;
    let mut done = 0u64;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| Error::Download(e.to_string()))?
    {
        file.write_all(&chunk).await?;
        done += chunk.len() as u64;
        progress(done, total);
    }
    file.flush().await?;
    Ok(())
}
