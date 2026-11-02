//! Plain HTTP downloads outside Matrix (app updates), with the same client stack as the SDK
//! (it already has working TLS on every platform, Android included).

use std::path::Path;

use matrix_sdk::reqwest;

use crate::{Error, Result};

fn client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(concat!("AlwyChat/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| Error::Download(e.to_string()))
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

/// Download `url` into `path`, reporting (bytes so far, total if known).
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
