//! Background tasks and timers that work both natively (Tokio) and in a browser
//! (WebAssembly, single-threaded): matrix-sdk's executor is Tokio's outside the browser.

pub use matrix_sdk::{
    executor::{AbortHandle, spawn},
    sleep::sleep,
};

/// CPU-heavy or blocking work: on Tokio's blocking pool natively, inline in a browser.
#[cfg(not(target_family = "wasm"))]
pub async fn blocking<T, F>(work: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|e| e.to_string())
}

/// CPU-heavy or blocking work: on Tokio's blocking pool natively, inline in a browser.
#[cfg(target_family = "wasm")]
pub async fn blocking<T, F>(work: F) -> Result<T, String>
where
    F: FnOnce() -> T,
{
    Ok(work())
}
