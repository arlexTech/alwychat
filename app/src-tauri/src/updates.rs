//! The "updates itself" builds: CI compiles them with `ALWY_UPDATE_FEED`, the `latest.json` of
//! their channel (stable releases or the rolling beta). Builds without it (local ones, and the
//! plain Stable / Beta downloads meant for stores) never look for updates.
//! - desktop: tauri-plugin-updater; updates must be signed with the key whose public half is
//!   `plugins.updater.pubkey` in tauri.conf.json;
//! - Android: the APK listed in the feed is downloaded and handed to Android's installer, which
//!   only accepts it if it's signed like the installed app.

/// This build's update feed, if it updates itself.
const FEED: Option<&str> = option_env!("ALWY_UPDATE_FEED");

use serde::Serialize;
use tauri::{AppHandle, ipc::Channel};

#[derive(Debug, Clone, Serialize)]
pub struct UpdateInfo {
    version: String,
    notes: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum UpdateProgress {
    Downloading {
        done: u64,
        total: Option<u64>,
    },
    /// Desktop: installing, then the app restarts. Android: the system installer is open.
    Installing,
    /// Android: the user must first allow AlwyChat to install apps (settings page opened).
    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    NeedsPermission,
}

#[cfg(desktop)]
static PENDING: std::sync::Mutex<Option<tauri_plugin_updater::Update>> =
    std::sync::Mutex::new(None);

#[cfg(target_os = "android")]
static PENDING: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

/// "0.1.42" > "0.1.9": numeric parts compared in order (pre-release suffixes ignored).
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn newer(remote: &str, local: &str) -> bool {
    let parts = |v: &str| -> Vec<u64> {
        v.split(['-', '+'])
            .next()
            .unwrap_or("")
            .split('.')
            .map(|p| p.parse().unwrap_or(0))
            .collect()
    };
    parts(remote) > parts(local)
}

/// A newer version on this build's channel, if any.
#[tauri::command]
pub async fn update_check(app: AppHandle) -> Result<Option<UpdateInfo>, String> {
    #[cfg(desktop)]
    {
        use tauri_plugin_updater::UpdaterExt;
        let Some(feed) = FEED.filter(|f| !f.is_empty()) else {
            return Ok(None);
        };
        let url = feed.parse().map_err(|e| format!("{e}"))?;
        let updater = app
            .updater_builder()
            .endpoints(vec![url])
            .and_then(|b| b.build())
            .map_err(|e| e.to_string())?;
        let update = match updater.check().await {
            Ok(update) => update,
            Err(e) => {
                tracing::info!("no update check: {e}");
                return Ok(None);
            }
        };
        let info = update.as_ref().map(|u| UpdateInfo {
            version: u.version.clone(),
            notes: u.body.clone(),
        });
        *PENDING.lock().unwrap() = update;
        Ok(info)
    }
    #[cfg(target_os = "android")]
    {
        let Some(feed) = FEED.filter(|f| !f.is_empty()) else {
            return Ok(None);
        };
        let text = alwy_core::download::fetch_text(feed)
            .await
            .map_err(|e| e.to_string())?;
        let manifest: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
        let version = manifest["version"].as_str().unwrap_or_default().to_owned();
        let url = manifest["platforms"]["android"]["url"]
            .as_str()
            .map(ToOwned::to_owned);
        let local = app.package_info().version.to_string();
        match url {
            Some(url) if newer(&version, &local) => {
                *PENDING.lock().unwrap() = Some(url);
                Ok(Some(UpdateInfo {
                    version,
                    notes: manifest["notes"].as_str().map(ToOwned::to_owned),
                }))
            }
            _ => Ok(None),
        }
    }
    #[cfg(not(any(desktop, target_os = "android")))]
    {
        let _ = app;
        Ok(None)
    }
}

/// Download and install the update found by `update_check`. Desktop restarts into it.
#[tauri::command]
pub async fn update_install(
    app: AppHandle,
    on_progress: Channel<UpdateProgress>,
) -> Result<(), String> {
    #[cfg(desktop)]
    {
        let update = PENDING
            .lock()
            .unwrap()
            .take()
            .ok_or("no update to install")?;
        let mut done = 0u64;
        let progress = on_progress.clone();
        update
            .download_and_install(
                move |chunk, total| {
                    done += chunk as u64;
                    let _ = progress.send(UpdateProgress::Downloading { done, total });
                },
                move || {
                    let _ = on_progress.send(UpdateProgress::Installing);
                },
            )
            .await
            .map_err(|e| e.to_string())?;
        app.restart();
    }
    #[cfg(target_os = "android")]
    {
        use tauri::Manager;
        let url = PENDING
            .lock()
            .unwrap()
            .clone()
            .ok_or("no update to install")?;
        let path = app
            .path()
            .app_cache_dir()
            .map_err(|e| e.to_string())?
            .join("update.apk");
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let progress = on_progress.clone();
        alwy_core::download::download_to(&url, &path, move |done, total| {
            let _ = progress.send(UpdateProgress::Downloading { done, total });
        })
        .await
        .map_err(|e| e.to_string())?;
        let answer: serde_json::Value = app
            .state::<crate::android::System>()
            .0
            .run_mobile_plugin_async("installApk", serde_json::json!({ "path": path }))
            .await
            .map_err(|e| e.to_string())?;
        let _ = on_progress.send(if answer["needsPermission"].as_bool() == Some(true) {
            UpdateProgress::NeedsPermission
        } else {
            UpdateProgress::Installing
        });
        Ok(())
    }
    #[cfg(not(any(desktop, target_os = "android")))]
    {
        let _ = (app, on_progress);
        Err("updates aren't available here".into())
    }
}

#[cfg(test)]
mod tests {
    use super::newer;

    #[test]
    fn versions() {
        assert!(newer("0.1.42", "0.1.9"));
        assert!(newer("0.2.0", "0.1.99"));
        assert!(!newer("0.1.9", "0.1.9"));
        assert!(!newer("0.1.8", "0.1.9"));
    }
}
