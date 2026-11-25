//! iPhone: saving photos and videos straight into the Photos library, like Messenger (the app's
//! sandbox can't write there itself). Asks iOS's "Add to Photos" permission the first time
//! (`NSPhotoLibraryAddUsageDescription` in the app's Info.ios.plist). Other systems: no-op.

use tauri::{
    Runtime,
    plugin::{Builder, TauriPlugin},
};

#[cfg(target_os = "ios")]
tauri::ios_plugin_binding!(init_plugin_photos);

#[cfg(target_os = "ios")]
struct Handle<R: Runtime>(tauri::plugin::PluginHandle<R>);

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("photos")
        .setup(|_app, _api| {
            #[cfg(target_os = "ios")]
            {
                use tauri::Manager;
                let handle = _api.register_ios_plugin(init_plugin_photos)?;
                _app.manage(Handle(handle));
            }
            Ok(())
        })
        .build()
}

/// The system's photo picker (`kind` = "media") or the Files app (anything else); the chosen
/// files are copied into `dir`. Answers `{files: [{path, name, mime, size}]}`.
#[cfg(target_os = "ios")]
pub async fn pick<R: Runtime, T: serde::de::DeserializeOwned>(
    app: &tauri::AppHandle<R>,
    kind: &str,
    dir: &std::path::Path,
) -> Result<T, String> {
    use tauri::Manager;
    #[derive(serde::Serialize)]
    struct Args {
        kind: String,
        dir: String,
    }
    app.state::<Handle<R>>()
        .0
        .run_mobile_plugin_async::<T>(
            "pick",
            Args {
                kind: kind.to_owned(),
                dir: dir.display().to_string(),
            },
        )
        .await
        .map_err(|e| e.to_string())
}

/// The system camera: `kind` = "photo" or "video"; the shot is written into `dir`. Answers
/// `{files: [...]}` like [`pick`] (empty when cancelled).
#[cfg(target_os = "ios")]
pub async fn capture<R: Runtime, T: serde::de::DeserializeOwned>(
    app: &tauri::AppHandle<R>,
    kind: &str,
    dir: &std::path::Path,
) -> Result<T, String> {
    use tauri::Manager;
    #[derive(serde::Serialize)]
    struct Args {
        kind: String,
        dir: String,
    }
    app.state::<Handle<R>>()
        .0
        .run_mobile_plugin_async::<T>(
            "capture",
            Args {
                kind: kind.to_owned(),
                dir: dir.display().to_string(),
            },
        )
        .await
        .map_err(|e| e.to_string())
}

/// Moves the file at `path` into Photos (as a video when `video`).
#[cfg(target_os = "ios")]
pub async fn save<R: Runtime>(
    app: &tauri::AppHandle<R>,
    path: &std::path::Path,
    video: bool,
) -> Result<(), String> {
    use tauri::Manager;
    #[derive(serde::Serialize)]
    struct Args {
        path: String,
        video: bool,
    }
    app.state::<Handle<R>>()
        .0
        .run_mobile_plugin_async::<()>(
            "save",
            Args {
                path: path.display().to_string(),
                video,
            },
        )
        .await
        .map_err(|e| e.to_string())
}
