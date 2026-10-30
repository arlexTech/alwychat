//! Tauri shell. Commands here stay thin: parse input, call alwy-core, return.
#![recursion_limit = "256"]

use alwy_core::{
    Account, Core,
    discovery::{self, ServerInfo},
    encryption::{EncryptionStatus, VerificationUpdate},
    rooms::{ListDiff, RoomSummary},
    settings::Settings,
    timeline::{Revision, TimelineEntry},
};
use tauri::{
    Emitter, Manager, State, WindowEvent, http,
    ipc::Channel,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};
use tauri_plugin_notification::NotificationExt;
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
async fn open_chat(
    core: State<'_, Core>,
    room_id: String,
    on_diffs: Channel<Vec<ListDiff<TimelineEntry>>>,
    on_typing: Channel<Vec<String>>,
) -> alwy_core::Result<()> {
    core.open_chat(
        &room_id,
        move |diffs| {
            let _ = on_diffs.send(diffs);
        },
        move |names| {
            let _ = on_typing.send(names);
        },
    )
    .await
}

#[tauri::command]
async fn set_typing(core: State<'_, Core>, room_id: String, typing: bool) -> alwy_core::Result<()> {
    core.set_typing(&room_id, typing).await
}

#[tauri::command]
async fn edit_message(
    core: State<'_, Core>,
    room_id: String,
    item_id: String,
    body: String,
) -> alwy_core::Result<()> {
    core.edit_message(&room_id, &item_id, &body).await
}

#[tauri::command]
async fn delete_message(
    core: State<'_, Core>,
    room_id: String,
    item_id: String,
) -> alwy_core::Result<()> {
    core.delete_message(&room_id, &item_id).await
}

#[tauri::command]
async fn edit_history(
    core: State<'_, Core>,
    room_id: String,
    event_id: String,
) -> alwy_core::Result<Vec<Revision>> {
    core.edit_history(&room_id, &event_id).await
}

/// Save a message's media: photos and videos to ~/Pictures/AlwyChat, other files to Downloads.
#[tauri::command]
async fn save_media(
    app: tauri::AppHandle,
    core: State<'_, Core>,
    media: String,
    to_gallery: bool,
) -> Result<String, String> {
    let paths = app.path();
    let dir = if to_gallery {
        paths.picture_dir().map(|d| d.join("AlwyChat"))
    } else {
        paths.download_dir()
    }
    .map_err(|e| e.to_string())?;
    let path = core
        .save_event_media(&media, &dir)
        .await
        .map_err(|e| e.to_string())?;
    Ok(path.display().to_string())
}

#[tauri::command]
async fn get_settings(core: State<'_, Core>) -> alwy_core::Result<Settings> {
    Ok(core.settings())
}

#[tauri::command]
async fn set_settings(core: State<'_, Core>, settings: Settings) -> alwy_core::Result<()> {
    core.set_settings(settings).await
}

#[tauri::command]
async fn set_pinned(core: State<'_, Core>, room_id: String, pinned: bool) -> alwy_core::Result<()> {
    core.set_pinned(&room_id, pinned).await
}

#[tauri::command]
async fn set_archived(
    core: State<'_, Core>,
    room_id: String,
    archived: bool,
) -> alwy_core::Result<()> {
    core.set_archived(&room_id, archived).await
}

#[tauri::command]
async fn set_muted(core: State<'_, Core>, room_id: String, muted: bool) -> alwy_core::Result<()> {
    core.set_muted(&room_id, muted).await
}

#[tauri::command]
async fn set_marked_unread(
    core: State<'_, Core>,
    room_id: String,
    unread: bool,
) -> alwy_core::Result<()> {
    core.set_marked_unread(&room_id, unread).await
}

#[tauri::command]
async fn close_chat(core: State<'_, Core>) -> alwy_core::Result<()> {
    core.close_chat()
}

#[tauri::command]
async fn load_older(core: State<'_, Core>, room_id: String) -> alwy_core::Result<bool> {
    core.load_older(&room_id).await
}

#[tauri::command]
async fn send_message(
    core: State<'_, Core>,
    room_id: String,
    body: String,
    reply_to: Option<String>,
) -> alwy_core::Result<()> {
    core.send_message(&room_id, &body, reply_to.as_deref())
        .await
}

#[tauri::command]
async fn toggle_reaction(
    core: State<'_, Core>,
    room_id: String,
    item_id: String,
    key: String,
) -> alwy_core::Result<()> {
    core.toggle_reaction(&room_id, &item_id, &key).await
}

#[tauri::command]
async fn mark_read(core: State<'_, Core>, room_id: String) -> alwy_core::Result<()> {
    core.mark_read(&room_id).await
}

#[tauri::command]
async fn subscribe_encryption(
    core: State<'_, Core>,
    on_status: Channel<EncryptionStatus>,
    on_verification: Channel<VerificationUpdate>,
) -> alwy_core::Result<()> {
    core.subscribe_encryption(
        move |status| {
            let _ = on_status.send(status);
        },
        move |update| {
            let _ = on_verification.send(update);
        },
    )
}

#[tauri::command]
async fn recover(core: State<'_, Core>, key: String) -> alwy_core::Result<()> {
    core.recover(&key).await
}

#[tauri::command]
async fn verify_this_device(core: State<'_, Core>) -> alwy_core::Result<()> {
    core.verify_this_device().await
}

#[tauri::command]
async fn accept_verification(core: State<'_, Core>) -> alwy_core::Result<()> {
    core.accept_verification().await
}

#[tauri::command]
async fn confirm_emojis(core: State<'_, Core>, matching: bool) -> alwy_core::Result<()> {
    core.confirm_emojis(matching).await
}

#[tauri::command]
async fn cancel_verification(core: State<'_, Core>) -> alwy_core::Result<()> {
    core.cancel_verification().await
}

/// Interface errors, copied into the terminal log so they're visible without devtools.
#[tauri::command]
fn log_ui_error(message: String) {
    tracing::error!(target: "ui", "{message}");
}

/// The server ended our session: stop using it locally, keep the store (see Core::end_session).
#[tauri::command]
async fn end_session(core: State<'_, Core>) -> alwy_core::Result<()> {
    core.end_session().await;
    Ok(())
}

#[tauri::command]
async fn logout(core: State<'_, Core>) -> alwy_core::Result<()> {
    core.logout().await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let filter = || {
        tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "info,matrix_sdk=warn".into())
    };
    // ALWYCHAT_LOG_FILE: log to a file (e.g. when the app is launched by a test driver).
    match std::env::var_os("ALWYCHAT_LOG_FILE").map(std::fs::File::create) {
        Some(Ok(file)) => tracing_subscriber::fmt()
            .with_env_filter(filter())
            .with_ansi(false)
            .with_writer(std::sync::Mutex::new(file))
            .init(),
        _ => tracing_subscriber::fmt().with_env_filter(filter()).init(),
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let core = Core::new(data_dir);
            let handle = app.handle().clone();
            core.on_session_ended(move || {
                let _ = handle.emit("session-ended", ());
            });
            let handle = app.handle().clone();
            core.on_notification(move |n| {
                // Only when the window isn't in front: otherwise the chat list already shows it.
                let focused = handle
                    .get_webview_window("main")
                    .and_then(|w| w.is_focused().ok())
                    .unwrap_or(false);
                if focused {
                    return;
                }
                let title = if n.is_dm {
                    n.sender_name
                } else {
                    format!("{} · {}", n.sender_name, n.room_name)
                };
                if let Err(e) = handle
                    .notification()
                    .builder()
                    .title(title)
                    .body(n.body)
                    .show()
                {
                    tracing::warn!("couldn't show notification: {e}");
                }
            });
            app.manage(core);
            app.manage(TrayHost(has_tray_host()));
            setup_tray(app)?;
            Ok(())
        })
        // Closing the window keeps AlwyChat running (and notifying): hidden in the tray when
        // the desktop has one, otherwise minimised so it can't get lost. Quit from the tray.
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                if window.state::<TrayHost>().0 {
                    let _ = window.hide();
                } else {
                    let _ = window.minimize();
                }
            }
        })
        .register_asynchronous_uri_scheme_protocol("media", |ctx, request, responder| {
            let app = ctx.app_handle().clone();
            tauri::async_runtime::spawn(async move {
                responder.respond(serve_media(&app, &request).await);
            });
        })
        .invoke_handler(tauri::generate_handler![
            discover_server,
            restore_session,
            login,
            subscribe_chat_list,
            open_chat,
            set_typing,
            edit_message,
            delete_message,
            edit_history,
            save_media,
            get_settings,
            set_settings,
            set_pinned,
            set_archived,
            set_muted,
            set_marked_unread,
            close_chat,
            load_older,
            send_message,
            toggle_reaction,
            mark_read,
            subscribe_encryption,
            recover,
            verify_this_device,
            accept_verification,
            confirm_emojis,
            cancel_verification,
            log_ui_error,
            end_session,
            logout
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Open AlwyChat", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &quit])?;
    TrayIconBuilder::with_id("main")
        .icon(app.default_window_icon().cloned().expect("bundled icon"))
        .tooltip("AlwyChat")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_window(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_window(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

/// Whether the desktop can show our tray icon.
struct TrayHost(bool);

/// Linux tray icons need a StatusNotifier host (KDE, most desktops; GNOME only with the
/// AppIndicator extension). Other platforms always have a tray.
fn has_tray_host() -> bool {
    #[cfg(target_os = "linux")]
    {
        tauri::async_runtime::block_on(async {
            let Ok(conn) = zbus::Connection::session().await else {
                return false;
            };
            let Ok(dbus) = zbus::fdo::DBusProxy::new(&conn).await else {
                return false;
            };
            let name = zbus::names::BusName::try_from("org.kde.StatusNotifierWatcher")
                .expect("valid bus name");
            dbus.name_has_owner(name).await.unwrap_or(false)
        })
    }
    #[cfg(not(target_os = "linux"))]
    true
}

fn show_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// The `media://` scheme (URL helpers in app/src/lib/media.ts):
/// - `media://localhost/<server>/<media-id>?size=96`: avatar thumbnail of an `mxc://` URI;
/// - `media://localhost/_event/<item id>[?thumb=1]`: a message's (decrypted) media.
async fn serve_media(
    app: &tauri::AppHandle,
    request: &http::Request<Vec<u8>>,
) -> http::Response<Vec<u8>> {
    let uri = request.uri();
    let core = app.state::<Core>();
    let query = uri.query().unwrap_or("");
    let param = |name: &str| {
        query
            .split('&')
            .find_map(|kv| kv.strip_prefix(name)?.strip_prefix('='))
    };

    let result = match uri.path().strip_prefix("/_event/") {
        Some(key) => {
            let key = percent_encoding::percent_decode_str(key).decode_utf8_lossy();
            core.event_media(&key, param("thumb") == Some("1")).await
        }
        None => {
            let mxc = format!("mxc:/{}", uri.path());
            let size = param("size").and_then(|v| v.parse().ok()).unwrap_or(96);
            core.thumbnail(&mxc, size).await.map(|bytes| {
                let mime = alwy_core::media::sniff(&bytes).to_owned();
                (bytes, mime)
            })
        }
    };

    match result {
        Ok((bytes, mime)) => respond_with_range(request, bytes, &mime),
        Err(e) => {
            tracing::debug!("media {uri}: {e}");
            http::Response::builder()
                .status(http::StatusCode::NOT_FOUND)
                .body(Vec::new())
                .expect("valid response")
        }
    }
}

/// Full body, or the requested `Range` (videos need it to play and seek in WebKit).
fn respond_with_range(
    request: &http::Request<Vec<u8>>,
    bytes: Vec<u8>,
    mime: &str,
) -> http::Response<Vec<u8>> {
    let builder = http::Response::builder()
        .header(http::header::CONTENT_TYPE, mime)
        .header(http::header::ACCEPT_RANGES, "bytes")
        .header(http::header::CACHE_CONTROL, "max-age=31536000, immutable");
    let total = bytes.len();
    let range = request
        .headers()
        .get(http::header::RANGE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("bytes="))
        .and_then(|v| v.split_once('-'))
        .and_then(|(start, end)| {
            let start: usize = start.parse().ok()?;
            let end: usize = if end.is_empty() {
                total.checked_sub(1)?
            } else {
                end.parse().ok()?
            };
            (start <= end && end < total).then_some((start, end))
        });
    match range {
        Some((start, end)) => builder
            .status(http::StatusCode::PARTIAL_CONTENT)
            .header(
                http::header::CONTENT_RANGE,
                format!("bytes {start}-{end}/{total}"),
            )
            .body(bytes[start..=end].to_vec()),
        None => builder.body(bytes),
    }
    .expect("valid response")
}
