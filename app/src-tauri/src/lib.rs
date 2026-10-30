//! Tauri shell. Commands here stay thin: parse input, call alwy-core, return.

use alwy_core::{
    Account, Core,
    discovery::{self, ServerInfo},
    rooms::{ListDiff, RoomSummary},
};
use tauri::{Manager, State, http, ipc::Channel};
use tauri_plugin_opener::OpenerExt;

#[tauri::command]
async fn discover_server(server: String) -> alwy_core::Result<ServerInfo> {
    discovery::discover(&server).await
}

#[tauri::command]
async fn restore_session(core: State<'_, Core>) -> alwy_core::Result<Option<Account>> {
    core.restore().await
}

#[tauri::command]
async fn login(
    app: tauri::AppHandle,
    core: State<'_, Core>,
    server: String,
) -> alwy_core::Result<Account> {
    core.login(&server, |url| {
        app.opener()
            .open_url(url.as_str(), None::<&str>)
            .map_err(|e| e.to_string())
    })
    .await
}

// Async so it runs on the Tokio runtime: the core spawns the stream task from here.
#[tauri::command]
async fn subscribe_chat_list(
    core: State<'_, Core>,
    on_diffs: Channel<Vec<ListDiff<RoomSummary>>>,
) -> alwy_core::Result<()> {
    core.subscribe_chat_list(move |diffs| {
        let _ = on_diffs.send(diffs);
    })
}

#[tauri::command]
async fn logout(core: State<'_, Core>) -> alwy_core::Result<()> {
    core.logout().await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,matrix_sdk=warn".into()),
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            app.manage(Core::new(data_dir));
            Ok(())
        })
        .register_asynchronous_uri_scheme_protocol("media", |ctx, request, responder| {
            let app = ctx.app_handle().clone();
            tauri::async_runtime::spawn(async move {
                responder.respond(serve_media(&app, request.uri()).await);
            });
        })
        .invoke_handler(tauri::generate_handler![
            discover_server,
            restore_session,
            login,
            subscribe_chat_list,
            logout
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// `media://localhost/<server>/<media-id>?size=96` → thumbnail bytes (see app/src/lib/media.ts).
async fn serve_media(app: &tauri::AppHandle, uri: &http::Uri) -> http::Response<Vec<u8>> {
    let mxc = format!("mxc:/{}", uri.path());
    let size = uri
        .query()
        .and_then(|q| q.split('&').find_map(|kv| kv.strip_prefix("size=")))
        .and_then(|v| v.parse().ok())
        .unwrap_or(96);

    match app.state::<Core>().thumbnail(&mxc, size).await {
        Ok(bytes) => http::Response::builder()
            .header(http::header::CONTENT_TYPE, sniff_image(&bytes))
            .header(http::header::CACHE_CONTROL, "max-age=31536000, immutable")
            .body(bytes),
        Err(e) => {
            tracing::debug!("media {mxc}: {e}");
            http::Response::builder()
                .status(http::StatusCode::NOT_FOUND)
                .body(Vec::new())
        }
    }
    .expect("valid response")
}

fn sniff_image(bytes: &[u8]) -> &'static str {
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
        _ => "application/octet-stream",
    }
}
