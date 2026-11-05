//! Tauri shell. Commands here stay thin: parse input, call alwy-core, return.
#![recursion_limit = "256"]

use alwy_core::{
    Account, Core, LoginRedirect,
    discovery::{self, ServerInfo},
    encryption::{EncryptionStatus, VerificationUpdate},
    mentions::{Member, MentionInput},
    rooms::{ListDiff, RoomSummary},
    settings::Settings,
    timeline::{Revision, TimelineEntry},
};
use tauri::{Emitter, Manager, State, http, ipc::Channel};
#[cfg(desktop)]
use tauri::{
    WindowEvent,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_opener::OpenerExt;

mod native_call;
mod updates;

#[tauri::command]
async fn discover_server(server: String) -> alwy_core::Result<ServerInfo> {
    discovery::discover(&server).await
}

#[tauri::command]
async fn restore_session(core: State<'_, AppCore>) -> alwy_core::Result<Option<Account>> {
    core.restore().await
}

#[tauri::command]
async fn login(
    app: tauri::AppHandle,
    core: State<'_, AppCore>,
    server: String,
) -> alwy_core::Result<Account> {
    // Mobile apps are paused while the browser is in front: no loopback server there.
    let redirect = if cfg!(mobile) {
        LoginRedirect::AppScheme
    } else {
        LoginRedirect::Loopback
    };
    core.login(&server, redirect, |url| {
        app.opener()
            .open_url(url.as_str(), None::<&str>)
            .map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
fn cancel_login(core: State<'_, AppCore>) {
    core.cancel_login();
}

// Async so it runs on the Tokio runtime: the core spawns the stream task from here.
#[tauri::command]
async fn subscribe_chat_list(
    core: State<'_, AppCore>,
    on_diffs: Channel<Vec<ListDiff<RoomSummary>>>,
) -> alwy_core::Result<()> {
    core.subscribe_chat_list(move |diffs| {
        let _ = on_diffs.send(diffs);
    })
}

#[tauri::command]
async fn open_chat(
    core: State<'_, AppCore>,
    room_id: String,
    on_diffs: Channel<Vec<ListDiff<TimelineEntry>>>,
    on_typing: Channel<Vec<String>>,
    on_style: Channel<alwy_core::custom::ChatStyle>,
    on_pins: Channel<alwy_core::pins::Pins>,
    focus: Option<String>,
) -> alwy_core::Result<()> {
    core.open_chat(
        &room_id,
        move |diffs| {
            let _ = on_diffs.send(diffs);
        },
        move |names| {
            let _ = on_typing.send(names);
        },
        move |style| {
            let _ = on_style.send(style);
        },
        move |pins| {
            let _ = on_pins.send(pins);
        },
        focus.as_deref(),
    )
    .await
}

/// Newer messages of a chat opened on a search result; true once the newest is loaded.
#[tauri::command]
async fn load_newer(core: State<'_, AppCore>, room_id: String) -> alwy_core::Result<bool> {
    core.load_newer(&room_id).await
}

/// Messages matching `query`, newest first, in all chats or one; `before` pages on.
#[tauri::command]
async fn search_messages(
    core: State<'_, AppCore>,
    query: String,
    room_id: Option<String>,
    sender: Option<String>,
    kind: Option<alwy_core::search::KindFilter>,
    before: Option<i64>,
) -> alwy_core::Result<Vec<alwy_core::search::SearchHit>> {
    core.search_messages(&query, room_id.as_deref(), sender.as_deref(), kind, before)
        .await
}

/// A chat's photos and videos, files or links, newest first.
#[tauri::command]
async fn shared_media(
    core: State<'_, AppCore>,
    room_id: String,
    kind: alwy_core::search::KindFilter,
    before: Option<i64>,
) -> alwy_core::Result<Vec<alwy_core::search::SharedItem>> {
    core.shared_media(&room_id, kind, before).await
}

#[tauri::command]
fn search_status(
    core: State<'_, AppCore>,
    room_id: Option<String>,
) -> alwy_core::Result<alwy_core::search::SearchStatus> {
    core.search_status(room_id.as_deref())
}

/// Joined spaces and their chats, streamed whole on each change.
#[tauri::command]
async fn subscribe_spaces(
    core: State<'_, AppCore>,
    on_spaces: Channel<Vec<alwy_core::spaces::SpaceInfo>>,
) -> alwy_core::Result<()> {
    core.subscribe_spaces(move |spaces| {
        let _ = on_spaces.send(spaces);
    })
    .await
}

/// Connection status for the chat list's banner ("connecting", "online", "offline", "error").
/// Async: the core spawns its watcher on the Tokio runtime.
#[tauri::command]
async fn subscribe_connection(
    core: State<'_, AppCore>,
    on_status: Channel<String>,
) -> alwy_core::Result<()> {
    core.subscribe_connection(move |status| {
        let _ = on_status.send(status.to_owned());
    })
}

#[tauri::command]
async fn resume_sync(core: State<'_, AppCore>) -> alwy_core::Result<()> {
    core.resume_sync().await
}

/// A space's children (joined or not), streamed whole on each change.
#[tauri::command]
async fn open_space(
    core: State<'_, AppCore>,
    space_id: String,
    on_children: Channel<Vec<alwy_core::spaces::SpaceChild>>,
) -> alwy_core::Result<()> {
    core.open_space(&space_id, move |children| {
        let _ = on_children.send(children);
    })
    .await
}

#[tauri::command]
fn close_space(core: State<'_, AppCore>) -> alwy_core::Result<()> {
    core.close_space()
}

#[tauri::command]
async fn join_room(
    core: State<'_, AppCore>,
    room_id: String,
    via: Vec<String>,
) -> alwy_core::Result<()> {
    core.join_room(&room_id, &via).await
}

// ---- Managing chats, spaces and the account (alwy_core::manage) ----

#[tauri::command]
async fn invite_info(
    core: State<'_, AppCore>,
    room_id: String,
) -> alwy_core::Result<alwy_core::manage::InviteInfo> {
    core.invite_info(&room_id).await
}

#[tauri::command]
async fn accept_invite(core: State<'_, AppCore>, room_id: String) -> alwy_core::Result<()> {
    core.accept_invite(&room_id).await
}

#[tauri::command]
async fn leave_room(core: State<'_, AppCore>, room_id: String) -> alwy_core::Result<()> {
    core.leave_room(&room_id).await
}

#[tauri::command]
async fn search_users(
    core: State<'_, AppCore>,
    term: String,
) -> alwy_core::Result<Vec<alwy_core::manage::UserResult>> {
    core.search_users(&term).await
}

#[tauri::command]
async fn open_dm(core: State<'_, AppCore>, user_id: String) -> alwy_core::Result<String> {
    core.open_dm(&user_id).await
}

#[tauri::command]
async fn create_room(
    core: State<'_, AppCore>,
    room: alwy_core::manage::NewRoom,
) -> alwy_core::Result<String> {
    core.create_room(room).await
}

#[tauri::command]
async fn join_address(core: State<'_, AppCore>, address: String) -> alwy_core::Result<String> {
    core.join_address(&address).await
}

#[tauri::command]
async fn space_rooms_joined(
    core: State<'_, AppCore>,
    space_id: String,
) -> alwy_core::Result<Vec<String>> {
    core.space_rooms_joined(&space_id).await
}

#[tauri::command]
async fn members(
    core: State<'_, AppCore>,
    room_id: String,
) -> alwy_core::Result<Vec<alwy_core::manage::MemberInfo>> {
    core.members(&room_id).await
}

#[tauri::command]
async fn invite_user(
    core: State<'_, AppCore>,
    room_id: String,
    user_id: String,
) -> alwy_core::Result<()> {
    core.invite_user(&room_id, &user_id).await
}

#[tauri::command]
async fn moderate(
    core: State<'_, AppCore>,
    room_id: String,
    user_id: String,
    action: String,
    reason: Option<String>,
) -> alwy_core::Result<()> {
    core.moderate(&room_id, &user_id, &action, reason.as_deref())
        .await
}

#[tauri::command]
async fn set_user_power(
    core: State<'_, AppCore>,
    room_id: String,
    user_id: String,
    power: i64,
) -> alwy_core::Result<()> {
    core.set_user_power(&room_id, &user_id, power).await
}

#[tauri::command]
async fn set_permission(
    core: State<'_, AppCore>,
    room_id: String,
    key: String,
    power: i64,
) -> alwy_core::Result<()> {
    core.set_permission(&room_id, &key, power).await
}

#[tauri::command]
async fn room_details(
    core: State<'_, AppCore>,
    room_id: String,
) -> alwy_core::Result<alwy_core::manage::RoomDetails> {
    core.room_details(&room_id).await
}

#[tauri::command]
async fn set_room_name(
    core: State<'_, AppCore>,
    room_id: String,
    name: String,
) -> alwy_core::Result<()> {
    core.set_room_name(&room_id, &name).await
}

#[tauri::command]
async fn set_room_topic(
    core: State<'_, AppCore>,
    room_id: String,
    topic: String,
) -> alwy_core::Result<()> {
    core.set_room_topic(&room_id, &topic).await
}

#[tauri::command]
async fn set_join_rule(
    core: State<'_, AppCore>,
    room_id: String,
    rule: String,
    spaces: Vec<String>,
) -> alwy_core::Result<()> {
    core.set_join_rule(&room_id, &rule, &spaces).await
}

#[tauri::command]
async fn set_history_visibility(
    core: State<'_, AppCore>,
    room_id: String,
    history: String,
) -> alwy_core::Result<()> {
    core.set_history_visibility(&room_id, &history).await
}

#[tauri::command]
async fn enable_room_encryption(
    core: State<'_, AppCore>,
    room_id: String,
) -> alwy_core::Result<()> {
    core.enable_room_encryption(&room_id).await
}

#[tauri::command]
async fn set_notify_mode(
    core: State<'_, AppCore>,
    room_id: String,
    mode: String,
) -> alwy_core::Result<()> {
    core.set_notify_mode(&room_id, &mode).await
}

#[tauri::command]
async fn report_room(
    core: State<'_, AppCore>,
    room_id: String,
    reason: String,
) -> alwy_core::Result<()> {
    core.report_room(&room_id, &reason).await
}

#[tauri::command]
async fn add_to_space(
    core: State<'_, AppCore>,
    space_id: String,
    room_id: String,
) -> alwy_core::Result<()> {
    core.add_to_space(&space_id, &room_id).await
}

#[tauri::command]
async fn remove_from_space(
    core: State<'_, AppCore>,
    space_id: String,
    room_id: String,
) -> alwy_core::Result<()> {
    core.remove_from_space(&space_id, &room_id).await
}

#[tauri::command]
async fn set_suggested(
    core: State<'_, AppCore>,
    space_id: String,
    room_id: String,
    suggested: bool,
) -> alwy_core::Result<()> {
    core.set_suggested(&space_id, &room_id, suggested).await
}

#[tauri::command]
async fn own_profile(core: State<'_, AppCore>) -> alwy_core::Result<alwy_core::manage::UserResult> {
    core.own_profile().await
}

#[tauri::command]
async fn set_display_name(core: State<'_, AppCore>, name: String) -> alwy_core::Result<()> {
    core.set_display_name(&name).await
}

#[tauri::command]
async fn user_profile(
    core: State<'_, AppCore>,
    user_id: String,
) -> alwy_core::Result<alwy_core::manage::UserResult> {
    core.user_profile(&user_id).await
}

#[tauri::command]
async fn devices(
    core: State<'_, AppCore>,
) -> alwy_core::Result<Vec<alwy_core::manage::DeviceInfo>> {
    core.devices().await
}

#[tauri::command]
async fn rename_device(
    core: State<'_, AppCore>,
    device_id: String,
    name: String,
) -> alwy_core::Result<()> {
    core.rename_device(&device_id, &name).await
}

#[tauri::command]
async fn account_url(core: State<'_, AppCore>, action: String) -> alwy_core::Result<String> {
    core.account_url(&action).await
}

#[tauri::command]
async fn security_status(
    core: State<'_, AppCore>,
) -> alwy_core::Result<alwy_core::manage::SecurityStatus> {
    core.security_status().await
}

#[tauri::command]
async fn new_recovery_key(core: State<'_, AppCore>) -> alwy_core::Result<String> {
    core.new_recovery_key().await
}

#[tauri::command]
async fn ignored_users(
    core: State<'_, AppCore>,
) -> alwy_core::Result<Vec<alwy_core::manage::UserResult>> {
    core.ignored_users().await
}

#[tauri::command]
async fn set_ignored(
    core: State<'_, AppCore>,
    user_id: String,
    ignored: bool,
) -> alwy_core::Result<()> {
    core.set_ignored(&user_id, ignored).await
}

#[tauri::command]
async fn notification_rules(
    core: State<'_, AppCore>,
) -> alwy_core::Result<alwy_core::manage::NotificationRules> {
    core.notification_rules().await
}

#[tauri::command]
async fn set_notification_rule(
    core: State<'_, AppCore>,
    rule: String,
    enabled: bool,
) -> alwy_core::Result<()> {
    core.set_notification_rule(&rule, enabled).await
}

#[tauri::command]
async fn set_keyword(
    core: State<'_, AppCore>,
    keyword: String,
    add: bool,
) -> alwy_core::Result<()> {
    core.set_keyword(&keyword, add).await
}

fn decode_image(image: Option<String>, mime: String) -> Result<Option<(Vec<u8>, String)>, String> {
    use base64::Engine;
    image
        .map(|data| {
            base64::engine::general_purpose::STANDARD
                .decode(data)
                .map(|bytes| (bytes, mime))
                .map_err(|e| e.to_string())
        })
        .transpose()
}

/// A chat's or space's photo (base64, small: resized in the webview); `None` removes it.
#[tauri::command]
async fn set_room_avatar(
    core: State<'_, AppCore>,
    room_id: String,
    image: Option<String>,
    mime: String,
) -> Result<(), String> {
    core.set_room_avatar(&room_id, decode_image(image, mime)?)
        .await
        .map_err(|e| e.to_string())
}

/// Our photo (base64); `None` removes it.
#[tauri::command]
async fn set_own_avatar(
    core: State<'_, AppCore>,
    image: Option<String>,
    mime: String,
) -> Result<(), String> {
    core.set_own_avatar(decode_image(image, mime)?)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn send_location(
    core: State<'_, AppCore>,
    room_id: String,
    geo_uri: String,
    description: String,
) -> alwy_core::Result<()> {
    core.send_location(&room_id, &geo_uri, &description).await
}

#[tauri::command]
async fn start_live_location(
    core: State<'_, AppCore>,
    room_id: String,
    duration_ms: u64,
    geo_uri: String,
) -> alwy_core::Result<String> {
    core.start_live_location(&room_id, duration_ms, &geo_uri)
        .await
}

#[tauri::command]
async fn update_live_location(
    core: State<'_, AppCore>,
    room_id: String,
    share_id: String,
    geo_uri: String,
) -> alwy_core::Result<()> {
    core.update_live_location(&room_id, &share_id, &geo_uri)
        .await
}

#[tauri::command]
async fn stop_live_location(core: State<'_, AppCore>, room_id: String) -> alwy_core::Result<()> {
    core.stop_live_location(&room_id).await
}

#[tauri::command]
async fn retry_send(
    core: State<'_, AppCore>,
    room_id: String,
    item_id: String,
) -> alwy_core::Result<()> {
    core.retry_send(&room_id, &item_id).await
}

#[tauri::command]
async fn message_activity(
    core: State<'_, AppCore>,
    room_id: String,
) -> alwy_core::Result<alwy_core::search::Activity> {
    core.message_activity(&room_id).await
}

#[tauri::command]
async fn message_at(
    core: State<'_, AppCore>,
    room_id: String,
    ts: i64,
) -> alwy_core::Result<Option<String>> {
    core.message_at(&room_id, ts).await
}

#[tauri::command]
async fn can_add_to_space(core: State<'_, AppCore>, space_id: String) -> alwy_core::Result<bool> {
    core.can_add_to_space(&space_id).await
}

#[tauri::command]
async fn create_voice_channel(
    core: State<'_, AppCore>,
    space_id: String,
    name: String,
) -> alwy_core::Result<String> {
    core.create_voice_channel(&space_id, &name).await
}

#[tauri::command]
async fn chat_list_prefs(
    core: State<'_, AppCore>,
) -> alwy_core::Result<alwy_core::spaces::ChatListPrefs> {
    core.chat_list_prefs().await
}

#[tauri::command]
async fn set_chat_list_prefs(
    core: State<'_, AppCore>,
    prefs: alwy_core::spaces::ChatListPrefs,
) -> alwy_core::Result<()> {
    core.set_chat_list_prefs(prefs).await
}

#[tauri::command]
async fn saved_messages_id(core: State<'_, AppCore>) -> alwy_core::Result<Option<String>> {
    core.saved_messages_id().await
}

#[tauri::command]
async fn open_saved_messages(core: State<'_, AppCore>) -> alwy_core::Result<String> {
    core.open_saved_messages().await
}

/// Start indexing older messages for search (once per session).
#[tauri::command]
async fn start_search_backfill(core: State<'_, AppCore>) -> alwy_core::Result<()> {
    core.start_search_backfill()
}

#[tauri::command]
async fn set_quick_reaction(
    core: State<'_, AppCore>,
    room_id: String,
    emoji: Option<String>,
) -> alwy_core::Result<()> {
    core.set_quick_reaction(&room_id, emoji).await
}

#[tauri::command]
async fn set_theme(
    core: State<'_, AppCore>,
    room_id: String,
    preset: Option<String>,
    wallpaper: Option<String>,
) -> alwy_core::Result<()> {
    core.set_theme(&room_id, preset, wallpaper).await
}

/// A chat background image (base64 JPEG, resized in the webview) → its `mxc://` URI.
#[tauri::command]
async fn upload_wallpaper(core: State<'_, AppCore>, image: String) -> Result<String, String> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(image)
        .map_err(|e| e.to_string())?;
    core.upload_wallpaper(bytes, "image/jpeg")
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn set_nickname(
    core: State<'_, AppCore>,
    room_id: String,
    user_id: String,
    nickname: Option<String>,
) -> alwy_core::Result<()> {
    core.set_nickname(&room_id, &user_id, nickname).await
}

#[tauri::command]
async fn presence(
    core: State<'_, AppCore>,
    user_ids: Vec<String>,
) -> alwy_core::Result<Vec<alwy_core::presence::Presence>> {
    core.presence(&user_ids).await
}

/// Whether the app is on screen (the webview reports it on every visibility change).
/// Window focus isn't reliable on mobile, so notifications look at this there.
struct Visible(std::sync::atomic::AtomicBool);

#[tauri::command]
async fn forward_messages(
    core: State<'_, AppCore>,
    room_id: String,
    event_ids: Vec<String>,
    target_room_id: String,
) -> alwy_core::Result<usize> {
    core.forward_messages(&room_id, &event_ids, &target_room_id)
        .await
}

#[tauri::command]
async fn decline_call(
    core: State<'_, AppCore>,
    room_id: String,
    notification_id: String,
) -> alwy_core::Result<()> {
    core.decline_call(&room_id, &notification_id).await
}

#[tauri::command]
async fn create_poll(
    core: State<'_, AppCore>,
    room_id: String,
    question: String,
    answers: Vec<String>,
    disclosed: bool,
    multiple: bool,
) -> alwy_core::Result<()> {
    core.create_poll(&room_id, &question, &answers, disclosed, multiple)
        .await
}

#[tauri::command]
async fn vote_poll(
    core: State<'_, AppCore>,
    room_id: String,
    poll_id: String,
    answers: Vec<String>,
) -> alwy_core::Result<()> {
    core.vote_poll(&room_id, &poll_id, answers).await
}

#[tauri::command]
async fn end_poll(
    core: State<'_, AppCore>,
    room_id: String,
    poll_id: String,
) -> alwy_core::Result<()> {
    core.end_poll(&room_id, &poll_id).await
}

#[tauri::command]
async fn pin_message(
    core: State<'_, AppCore>,
    room_id: String,
    event_id: String,
    pin: bool,
) -> alwy_core::Result<()> {
    core.pin_message(&room_id, &event_id, pin).await
}

#[tauri::command]
async fn link_preview(
    core: State<'_, AppCore>,
    url: String,
) -> alwy_core::Result<Option<alwy_core::links::LinkPreview>> {
    core.link_preview(&url).await
}

#[tauri::command]
async fn report_activity(
    core: State<'_, AppCore>,
    visible: State<'_, Visible>,
    active: bool,
) -> alwy_core::Result<()> {
    visible
        .0
        .store(active, std::sync::atomic::Ordering::Relaxed);
    core.report_activity(active).await
}

/// What can keep notifications from reaching the user (Android: permission, battery
/// optimisation, background data). Desktop has nothing to check.
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct SystemStatus {
    notifications: bool,
    battery_unrestricted: bool,
    background_data: bool,
}

/// `fix`: None just reads the status; "notifications" / "battery" / "backgroundData" asks the
/// system or opens its settings page (the UI re-reads the status when the app comes back).
#[tauri::command]
async fn system_status(app: tauri::AppHandle, fix: Option<String>) -> Result<SystemStatus, String> {
    #[cfg(target_os = "android")]
    {
        let command = match fix.as_deref() {
            None => "status",
            Some("notifications") => "fixNotifications",
            Some("battery") => "fixBattery",
            Some("backgroundData") => "fixBackgroundData",
            Some("appSettings") => "openAppSettings",
            Some(other) => return Err(format!("unknown setting {other}")),
        };
        app.state::<android::System>()
            .0
            .run_mobile_plugin_async(command, serde_json::json!({}))
            .await
            .map_err(|e| e.to_string())
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = (app, fix);
        Ok(SystemStatus {
            notifications: true,
            battery_unrestricted: true,
            background_data: true,
        })
    }
}

/// The system's clock: 24-hour or not when it says so (Android's setting), and the locale of
/// its time format (Linux `LC_TIME`; webviews only see the language). The UI decides with these.
#[derive(serde::Serialize, serde::Deserialize, Default)]
struct SystemClock {
    h24: Option<bool>,
    locale: Option<String>,
}

/// The call notification's texts, translated by the UI.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct CallTexts {
    title: String,
    text: String,
    hang_up: String,
    channel: String,
    mute: String,
    unmute: String,
}

/// Android: the ongoing call notification (and its foreground service, which keeps the call
/// going in the background); `muted` None = no mute button. Elsewhere: nothing.
#[tauri::command]
async fn call_notification(
    app: tauri::AppHandle,
    active: bool,
    texts: Option<CallTexts>,
    room_id: Option<String>,
    muted: Option<bool>,
    camera: Option<bool>,
) -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        let mut args = serde_json::to_value(&texts).map_err(|e| e.to_string())?;
        if !args.is_object() {
            args = serde_json::json!({});
        }
        args["active"] = active.into();
        args["roomId"] = serde_json::json!(room_id);
        args["muted"] = serde_json::json!(muted);
        args["camera"] = camera.unwrap_or(false).into();
        app.state::<android::System>()
            .0
            .run_mobile_plugin_async::<serde_json::Value>("callNotification", args)
            .await
            .map_err(|e| e.to_string())?;
    }
    #[cfg(not(target_os = "android"))]
    let _ = (app, active, texts, room_id, muted, camera);
    Ok(())
}

/// Android: the call's picture-in-picture window wide (a screen share) or tall (cameras).
#[tauri::command]
async fn pip_shape(app: tauri::AppHandle, wide: bool) -> Result<(), String> {
    #[cfg(target_os = "android")]
    app.state::<android::System>()
        .0
        .run_mobile_plugin_async::<serde_json::Value>(
            "pipShape",
            serde_json::json!({ "wide": wide }),
        )
        .await
        .map_err(|e| e.to_string())?;
    #[cfg(not(target_os = "android"))]
    let _ = (app, wide);
    Ok(())
}

/// The argument the computer starts us with ("Open at startup"): start out of sight.
#[cfg(desktop)]
const STARTUP_ARG: &str = "--startup";

/// Whether AlwyChat opens when the computer starts (desktop; elsewhere always false).
#[tauri::command]
fn autostart_get(app: tauri::AppHandle) -> bool {
    #[cfg(desktop)]
    {
        use tauri_plugin_autostart::ManagerExt;
        app.autolaunch().is_enabled().unwrap_or(false)
    }
    #[cfg(not(desktop))]
    {
        let _ = app;
        false
    }
}

#[tauri::command]
fn autostart_set(app: tauri::AppHandle, on: bool) -> Result<(), String> {
    #[cfg(desktop)]
    {
        use tauri_plugin_autostart::ManagerExt;
        let launch = app.autolaunch();
        if on {
            launch.enable()
        } else {
            launch.disable()
        }
        .map_err(|e| e.to_string())
    }
    #[cfg(not(desktop))]
    {
        let _ = (app, on);
        Err("not available here".into())
    }
}

/// Android: call audio on the loudspeaker (`Some(true)`), the earpiece / headphones
/// (`Some(false)`), or back to normal after a call (`None`). Elsewhere: nothing.
#[tauri::command]
async fn call_audio(app: tauri::AppHandle, speaker: Option<bool>) -> Result<(), String> {
    #[cfg(target_os = "android")]
    app.state::<android::System>()
        .0
        .run_mobile_plugin_async::<serde_json::Value>(
            "callAudio",
            serde_json::json!({ "speaker": speaker }),
        )
        .await
        .map_err(|e| e.to_string())?;
    #[cfg(not(target_os = "android"))]
    let _ = (app, speaker);
    Ok(())
}

#[tauri::command]
async fn system_clock(app: tauri::AppHandle) -> Result<SystemClock, String> {
    #[cfg(target_os = "android")]
    {
        return app
            .state::<android::System>()
            .0
            .run_mobile_plugin_async("clock", serde_json::json!({}))
            .await
            .map_err(|e| e.to_string());
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        // LC_ALL wins over LC_TIME, which wins over LANG ("fr_BE.UTF-8" → "fr-BE").
        let locale = ["LC_ALL", "LC_TIME", "LANG"]
            .iter()
            .filter_map(|k| std::env::var(k).ok())
            .find(|v| !v.is_empty() && v != "C" && v != "POSIX")
            .map(|v| v.split(['.', '@']).next().unwrap_or("").replace('_', "-"))
            .filter(|v| !v.is_empty());
        Ok(SystemClock { h24: None, locale })
    }
}

/// Android: system bar icons (dark on a light theme) and the colour behind them.
#[tauri::command]
async fn set_system_bars(
    app: tauri::AppHandle,
    light: bool,
    background: String,
) -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        app.state::<android::System>()
            .0
            .run_mobile_plugin_async::<()>(
                "setBars",
                serde_json::json!({ "light": light, "background": background }),
            )
            .await
            .map_err(|e| e.to_string())
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = (app, light, background);
        Ok(())
    }
}

#[derive(serde::Serialize)]
struct AppLockState {
    enabled: bool,
    /// "fingerprint" or "face" when the device can unlock with it.
    biometry: Option<&'static str>,
}

#[tauri::command]
async fn app_lock_state(
    app: tauri::AppHandle,
    core: State<'_, AppCore>,
) -> alwy_core::Result<AppLockState> {
    use tauri_plugin_biometric::{BiometricExt, BiometryType};
    let biometry = match app.biometric().status() {
        Ok(s) if s.is_available => match s.biometry_type {
            BiometryType::FaceID => Some("face"),
            BiometryType::TouchID => Some("fingerprint"),
            BiometryType::None => None,
        },
        _ => None,
    };
    Ok(AppLockState {
        enabled: core.app_lock_enabled(),
        biometry,
    })
}

#[tauri::command]
async fn set_app_lock(core: State<'_, AppCore>, pin: Option<String>) -> alwy_core::Result<()> {
    core.set_app_lock(pin.as_deref()).await
}

#[tauri::command]
async fn unlock_app(
    core: State<'_, AppCore>,
    pin: String,
) -> alwy_core::Result<alwy_core::applock::Unlock> {
    core.unlock_app(&pin).await
}

/// The system fingerprint/face prompt; true once recognised.
#[tauri::command]
async fn unlock_biometric(
    app: tauri::AppHandle,
    core: State<'_, AppCore>,
) -> alwy_core::Result<bool> {
    use tauri_plugin_biometric::{AuthOptions, BiometricExt};
    let prompt = app.clone();
    let ok = tauri::async_runtime::spawn_blocking(move || {
        prompt
            .biometric()
            .authenticate(
                "Unlock AlwyChat".into(),
                AuthOptions {
                    title: Some("Unlock AlwyChat".into()),
                    cancel_title: Some("Use PIN".into()),
                    ..Default::default()
                },
            )
            .is_ok()
    })
    .await
    .unwrap_or(false);
    if ok {
        core.biometric_unlocked();
    }
    Ok(ok)
}

/// Android: hide the app's content in the app switcher and block screenshots (FLAG_SECURE).
#[tauri::command]
async fn set_secure_screen(app: tauri::AppHandle, secure: bool) -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        app.state::<android::System>()
            .0
            .run_mobile_plugin_async::<()>("setSecure", serde_json::json!({ "secure": secure }))
            .await
            .map_err(|e| e.to_string())
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = (app, secure);
        Ok(())
    }
}

#[tauri::command]
async fn room_sounds(
    core: State<'_, AppCore>,
    room_id: String,
) -> alwy_core::Result<Vec<alwy_core::sounds::Sound>> {
    core.room_sounds(&room_id).await
}

#[tauri::command]
async fn set_room_sounds(
    core: State<'_, AppCore>,
    room_id: String,
    items: Vec<alwy_core::sounds::Sound>,
) -> alwy_core::Result<()> {
    core.set_room_sounds(&room_id, items).await
}

#[tauri::command]
async fn can_edit_sounds(core: State<'_, AppCore>, room_id: String) -> alwy_core::Result<bool> {
    core.can_edit_sounds(&room_id).await
}

/// A soundboard sound (base64; small WAVs).
#[tauri::command]
async fn upload_sound(
    core: State<'_, AppCore>,
    data: String,
    mime: String,
) -> Result<String, String> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data)
        .map_err(|e| e.to_string())?;
    core.upload_sound(bytes, &mime)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn space_banner(
    core: State<'_, AppCore>,
    room_id: String,
) -> alwy_core::Result<Option<String>> {
    core.space_banner(&room_id).await
}

/// `image`: base64 JPEG, or none to remove the banner.
#[tauri::command]
async fn set_space_banner(
    core: State<'_, AppCore>,
    room_id: String,
    image: Option<String>,
) -> Result<(), String> {
    use base64::Engine;
    let bytes = image
        .map(|i| base64::engine::general_purpose::STANDARD.decode(i))
        .transpose()
        .map_err(|e| e.to_string())?;
    core.set_space_banner(&room_id, bytes)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn viewed_once(core: State<'_, AppCore>) -> alwy_core::Result<Vec<String>> {
    core.viewed_once().await
}

#[tauri::command]
async fn open_view_once(
    core: State<'_, AppCore>,
    room_id: String,
    event_id: String,
) -> alwy_core::Result<()> {
    core.open_view_once(&room_id, &event_id).await
}

#[tauri::command]
fn call_history(core: State<'_, AppCore>) -> alwy_core::call_history::CallHistory {
    core.call_history()
}

#[tauri::command]
fn mark_calls_seen(core: State<'_, AppCore>) {
    core.mark_calls_seen();
}

#[tauri::command]
fn clear_call_history(core: State<'_, AppCore>, id: Option<String>) {
    core.clear_call_history(id.as_deref());
}

/// Media keys known for the call in `room_id` (see `call_keys.rs`).
#[tauri::command]
fn call_keys(core: State<'_, AppCore>, room_id: String) -> Vec<alwy_core::call_keys::CallKey> {
    core.call_keys(&room_id)
}

#[tauri::command]
async fn join_call(
    core: State<'_, AppCore>,
    room_id: String,
    video: bool,
) -> alwy_core::Result<alwy_core::rtc::CallConnection> {
    core.join_call(&room_id, video).await
}

#[tauri::command]
async fn leave_call(core: State<'_, AppCore>) -> alwy_core::Result<()> {
    core.leave_call().await
}

#[tauri::command]
async fn call_participants(
    core: State<'_, AppCore>,
    room_id: String,
) -> alwy_core::Result<Vec<Member>> {
    core.call_participants(&room_id).await
}

#[tauri::command]
async fn set_typing(
    core: State<'_, AppCore>,
    room_id: String,
    typing: bool,
) -> alwy_core::Result<()> {
    core.set_typing(&room_id, typing).await
}

#[tauri::command]
async fn edit_message(
    core: State<'_, AppCore>,
    room_id: String,
    item_id: String,
    body: String,
    mentions: Vec<MentionInput>,
) -> alwy_core::Result<()> {
    core.edit_message(&room_id, &item_id, &body, &mentions)
        .await
}

#[tauri::command]
async fn delete_message(
    core: State<'_, AppCore>,
    room_id: String,
    item_id: String,
) -> alwy_core::Result<()> {
    core.delete_message(&room_id, &item_id).await
}

#[tauri::command]
async fn edit_history(
    core: State<'_, AppCore>,
    room_id: String,
    event_id: String,
) -> alwy_core::Result<Vec<Revision>> {
    core.edit_history(&room_id, &event_id).await
}

/// Save a message's media: photos and videos to ~/Pictures/AlwyChat, other files to Downloads.
/// On Android they go through MediaStore so the Gallery sees them (see GalleryPlugin.kt).
#[tauri::command]
async fn save_media(
    app: tauri::AppHandle,
    core: State<'_, AppCore>,
    media: String,
    to_gallery: bool,
) -> Result<String, String> {
    #[cfg(target_os = "android")]
    {
        let dir = app
            .path()
            .app_cache_dir()
            .map_err(|e| e.to_string())?
            .join("saving");
        let path = core
            .save_event_media(&media, &dir)
            .await
            .map_err(|e| e.to_string())?;
        let saved: android::Saved = app
            .state::<android::Gallery>()
            .0
            .run_mobile_plugin_async(
                "save",
                android::SaveArgs {
                    path: path.display().to_string(),
                    to_gallery,
                },
            )
            .await
            .map_err(|e| e.to_string())?;
        return Ok(saved.path);
    }
    #[cfg(not(target_os = "android"))]
    save_to_folder(&app, &core, &media, to_gallery).await
}

/// Unsent composer text of every chat (synced between our devices).
#[tauri::command]
async fn drafts(
    core: State<'_, AppCore>,
) -> Result<std::collections::HashMap<String, alwy_core::drafts::Draft>, String> {
    core.drafts().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn set_draft(
    core: State<'_, AppCore>,
    room_id: String,
    draft: Option<alwy_core::drafts::Draft>,
) -> Result<(), String> {
    core.set_draft(&room_id, draft)
        .await
        .map_err(|e| e.to_string())
}

/// A file message: "open" it in another app, "save_as" (Android picker: folder and name), or
/// "downloads". Returns what to tell the user, or None when nothing happened (picker cancelled).
#[tauri::command]
async fn file_action(
    app: tauri::AppHandle,
    core: State<'_, AppCore>,
    media: String,
    action: String,
) -> Result<Option<String>, String> {
    if action == "downloads" {
        return save_media(app, core, media, false)
            .await
            .map(|p| Some(format!("Saved to {p}")));
    }
    // Opened files stay in the cache (the other app reads them later); cleared at startup.
    let dir = opened_dir(&app)?;
    let path = core
        .save_event_media(&media, &dir)
        .await
        .map_err(|e| e.to_string())?;
    #[cfg(target_os = "android")]
    {
        let args = serde_json::json!({ "path": path.display().to_string() });
        let gallery = &app.state::<android::Gallery>().0;
        match action.as_str() {
            "open" => {
                gallery
                    .run_mobile_plugin_async::<()>("open", args)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(None)
            }
            "save_as" => {
                #[derive(serde::Deserialize)]
                struct SavedAs {
                    saved: bool,
                }
                let r: SavedAs = gallery
                    .run_mobile_plugin_async("saveAs", args)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(r.saved.then(|| "Saved".to_owned()))
            }
            other => Err(format!("unknown file action {other}")),
        }
    }
    #[cfg(not(target_os = "android"))]
    {
        use tauri_plugin_opener::OpenerExt;
        match action.as_str() {
            // A program from a message would run: it can be saved, not opened from here.
            "open" if alwy_core::media::runs_when_opened(&path.display().to_string()) => {
                let _ = std::fs::remove_file(&path);
                Err("This kind of file could run a program, so AlwyChat doesn't open it. Save it, and open it yourself only if you trust it.".into())
            }
            "open" => {
                app.opener()
                    .open_path(path.display().to_string(), None::<&str>)
                    .map_err(|e| e.to_string())?;
                Ok(None)
            }
            other => Err(format!("unknown file action {other}")),
        }
    }
}

fn opened_dir(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    Ok(app
        .path()
        .app_cache_dir()
        .map_err(|e| e.to_string())?
        .join("opened"))
}

#[cfg(not(target_os = "android"))]
async fn save_to_folder(
    app: &tauri::AppHandle,
    core: &Core,
    media: &str,
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
        .save_event_media(media, &dir)
        .await
        .map_err(|e| e.to_string())?;
    Ok(path.display().to_string())
}

/// Send a photo/video/file to the open chat. Body: raw bytes. Headers: `x-room` and
/// `x-file` (URI-encoded JSON `OutgoingFile`), since headers must be ASCII.
#[tauri::command]
async fn send_file(
    core: State<'_, AppCore>,
    request: tauri::ipc::Request<'_>,
) -> Result<(), String> {
    let tauri::ipc::InvokeBody::Raw(bytes) = request.body() else {
        return Err("expected raw file bytes".into());
    };
    let header = |name: &str| -> Result<String, String> {
        let value = request
            .headers()
            .get(name)
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| format!("missing {name}"))?;
        Ok(percent_encoding::percent_decode_str(value)
            .decode_utf8_lossy()
            .into_owned())
    };
    let room_id = header("x-room")?;
    let file: alwy_core::upload::OutgoingFile =
        serde_json::from_str(&header("x-file")?).map_err(|e| e.to_string())?;
    core.send_file(&room_id, file, bytes.clone())
        .await
        .map_err(|e| e.to_string())
}

/// Files chosen in the system picker (Android) are copied here, sent from here, then deleted.
fn picked_dir(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    Ok(app
        .path()
        .app_cache_dir()
        .map_err(|e| e.to_string())?
        .join("picked"))
}

/// A path the UI got from `pick_files`, refused if it points anywhere else.
fn picked_file(app: &tauri::AppHandle, path: &str) -> Result<std::path::PathBuf, String> {
    let path = std::path::PathBuf::from(path);
    if path.parent() != Some(picked_dir(app)?.as_path()) {
        return Err("not a picked file".into());
    }
    Ok(path)
}

/// One file from the system picker, copied into `picked_dir`.
#[derive(serde::Serialize, serde::Deserialize)]
struct Picked {
    path: String,
    name: String,
    mime: String,
    size: u64,
}

/// What another app shared with us ("Share" → AlwyChat), taken once; files are copied like
/// picked ones. Android only (empty elsewhere).
#[derive(Default, serde::Serialize, serde::Deserialize)]
struct Share {
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    files: Vec<Picked>,
}

#[tauri::command]
async fn take_share(app: tauri::AppHandle) -> Result<Share, String> {
    #[cfg(target_os = "android")]
    {
        let dir = picked_dir(&app)?.display().to_string();
        app.state::<android::Gallery>()
            .0
            .run_mobile_plugin_async("takeShare", serde_json::json!({ "path": dir }))
            .await
            .map_err(|e| e.to_string())
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        Ok(Share::default())
    }
}

/// Images, GIFs and stickers the keyboard inserted (Android), copied like picked files.
#[tauri::command]
async fn take_keyboard_content(app: tauri::AppHandle) -> Result<Share, String> {
    #[cfg(target_os = "android")]
    {
        app.state::<android::Gallery>()
            .0
            .run_mobile_plugin_async("takeKeyboard", serde_json::json!({}))
            .await
            .map_err(|e| e.to_string())
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        Ok(Share::default())
    }
}

/// Take a photo (`kind` = "photo") or record a video ("video") with the phone's camera app.
/// Empty when cancelled. Android only.
#[tauri::command]
async fn capture(app: tauri::AppHandle, kind: String) -> Result<Vec<Picked>, String> {
    #[cfg(target_os = "android")]
    {
        #[derive(serde::Deserialize)]
        struct CaptureResult {
            files: Vec<Picked>,
        }
        let dir = picked_dir(&app)?.display().to_string();
        let result: CaptureResult = app
            .state::<android::Gallery>()
            .0
            .run_mobile_plugin_async("capture", serde_json::json!({ "kind": kind, "dir": dir }))
            .await
            .map_err(|e| e.to_string())?;
        Ok(result.files)
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = (app, kind);
        Err("the camera app is only used on Android".into())
    }
}

/// Open the system photo picker (`kind` = "media") or document picker ("files").
#[tauri::command]
async fn pick_files(app: tauri::AppHandle, kind: String) -> Result<Vec<Picked>, String> {
    #[cfg(target_os = "android")]
    {
        #[derive(serde::Deserialize)]
        struct PickResult {
            files: Vec<Picked>,
        }
        let dir = picked_dir(&app)?.display().to_string();
        let result: PickResult = app
            .state::<android::Gallery>()
            .0
            .run_mobile_plugin_async("pick", serde_json::json!({ "kind": kind, "dir": dir }))
            .await
            .map_err(|e| e.to_string())?;
        Ok(result.files)
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = (app, kind);
        Err("the system picker is only used on Android".into())
    }
}

/// Send a picked file (Android: the webview can't pass raw bytes to Rust), then delete it.
#[tauri::command]
async fn send_picked(
    app: tauri::AppHandle,
    core: State<'_, AppCore>,
    room_id: String,
    path: String,
    file: alwy_core::upload::OutgoingFile,
) -> Result<(), String> {
    let path = picked_file(&app, &path)?;
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    let result = core.send_file(&room_id, file, bytes).await;
    let _ = std::fs::remove_file(&path);
    result.map_err(|e| e.to_string())
}

/// The user removed a picked file before sending it.
#[tauri::command]
async fn discard_picked(app: tauri::AppHandle, path: String) -> Result<(), String> {
    let path = picked_file(&app, &path)?;
    std::fs::remove_file(path).map_err(|e| e.to_string())
}

/// A file made in the app (edited photo, in-app camera shot) saved as a waiting "picked" file,
/// base64 in pieces (the Android webview can't hand raw bytes over, and a video is too big for
/// one message): the first call creates it, the next ones pass `append_to` (its path).
#[tauri::command]
async fn save_blob(
    app: tauri::AppHandle,
    name: String,
    mime: String,
    data: String,
    append_to: Option<String>,
) -> Result<Picked, String> {
    use base64::Engine;
    use std::io::Write;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data)
        .map_err(|e| e.to_string())?;
    let dir = picked_dir(&app)?;
    let path = match append_to {
        // Only files we made there (no path from elsewhere).
        Some(path) => {
            let path = std::path::PathBuf::from(path);
            if path.parent() != Some(dir.as_path()) {
                return Err("not a file of ours".into());
            }
            path
        }
        None => {
            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            let ext = alwy_core::media::safe_file_name(&name)
                .rsplit_once('.')
                .map(|(_, e)| e.to_owned())
                .unwrap_or_else(|| "bin".into());
            dir.join(format!("{stamp}-made.{ext}"))
        }
    };
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| e.to_string())?;
    file.write_all(&bytes).map_err(|e| e.to_string())?;
    let size = file.metadata().map(|m| m.len()).unwrap_or(0);
    Ok(Picked {
        path: path.display().to_string(),
        name,
        mime,
        size,
    })
}

/// Send a recorded voice message. The recording travels base64-encoded: the Android webview
/// can't pass raw bytes, and voice clips are small.
#[tauri::command]
async fn send_voice(
    core: State<'_, AppCore>,
    room_id: String,
    audio: String,
    mime: String,
    duration_ms: u64,
    waveform: Vec<f32>,
) -> Result<(), String> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(audio)
        .map_err(|e| e.to_string())?;
    core.send_voice(&room_id, bytes, &mime, duration_ms, waveform)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn chat_media_hd(
    core: State<'_, AppCore>,
    room_id: String,
) -> alwy_core::Result<Option<bool>> {
    core.chat_media_hd(&room_id).await
}

#[tauri::command]
async fn set_chat_media_hd(
    core: State<'_, AppCore>,
    room_id: String,
    hd: Option<bool>,
) -> alwy_core::Result<()> {
    core.set_chat_media_hd(&room_id, hd).await
}

#[tauri::command]
async fn get_settings(core: State<'_, AppCore>) -> alwy_core::Result<Settings> {
    Ok(core.settings())
}

#[tauri::command]
async fn set_settings(core: State<'_, AppCore>, settings: Settings) -> alwy_core::Result<()> {
    core.set_settings(settings).await
}

#[tauri::command]
async fn set_pinned(
    core: State<'_, AppCore>,
    room_id: String,
    pinned: bool,
) -> alwy_core::Result<()> {
    core.set_pinned(&room_id, pinned).await
}

#[tauri::command]
async fn set_archived(
    core: State<'_, AppCore>,
    room_id: String,
    archived: bool,
) -> alwy_core::Result<()> {
    core.set_archived(&room_id, archived).await
}

#[tauri::command]
async fn set_muted(
    core: State<'_, AppCore>,
    room_id: String,
    muted: bool,
) -> alwy_core::Result<()> {
    core.set_muted(&room_id, muted).await
}

/// Timed mutes and muted spaces (see alwy_core::mutes).
#[tauri::command]
async fn mutes(core: State<'_, AppCore>) -> alwy_core::Result<alwy_core::mutes::Mutes> {
    core.mutes().await
}

#[tauri::command]
async fn apply_mutes(core: State<'_, AppCore>) -> alwy_core::Result<alwy_core::mutes::Mutes> {
    core.apply_mutes().await
}

#[tauri::command]
async fn mute_room(
    core: State<'_, AppCore>,
    room_id: String,
    until: Option<u64>,
) -> alwy_core::Result<()> {
    core.mute_room(&room_id, until).await
}

#[tauri::command]
async fn unmute_room(core: State<'_, AppCore>, room_id: String) -> alwy_core::Result<()> {
    core.unmute_room(&room_id).await
}

#[tauri::command]
async fn mute_space(
    core: State<'_, AppCore>,
    space_id: String,
    until: Option<u64>,
) -> alwy_core::Result<()> {
    core.mute_space(&space_id, until).await
}

#[tauri::command]
async fn unmute_space(core: State<'_, AppCore>, space_id: String) -> alwy_core::Result<()> {
    core.unmute_space(&space_id).await
}

#[tauri::command]
async fn set_marked_unread(
    core: State<'_, AppCore>,
    room_id: String,
    unread: bool,
) -> alwy_core::Result<()> {
    core.set_marked_unread(&room_id, unread).await
}

#[tauri::command]
async fn profile_extras(
    core: State<'_, AppCore>,
    user_ids: Vec<String>,
) -> alwy_core::Result<Vec<alwy_core::profile::ProfileExtras>> {
    core.profile_extras(&user_ids).await
}

#[tauri::command]
async fn set_status(
    core: State<'_, AppCore>,
    status: Option<alwy_core::profile::Status>,
) -> alwy_core::Result<()> {
    core.set_status(status).await
}

#[tauri::command]
async fn set_time_zone(
    core: State<'_, AppCore>,
    time_zone: Option<String>,
) -> alwy_core::Result<()> {
    core.set_time_zone(time_zone.as_deref()).await
}

#[tauri::command]
async fn close_chat(core: State<'_, AppCore>) -> alwy_core::Result<()> {
    core.close_chat()
}

#[tauri::command]
async fn open_thread(
    core: State<'_, AppCore>,
    room_id: String,
    root: String,
    on_diffs: Channel<Vec<ListDiff<TimelineEntry>>>,
) -> alwy_core::Result<()> {
    core.open_thread(&room_id, &root, move |diffs| {
        let _ = on_diffs.send(diffs);
    })
    .await
}

#[tauri::command]
async fn close_thread(core: State<'_, AppCore>) -> alwy_core::Result<()> {
    core.close_thread()
}

#[tauri::command]
async fn load_older(core: State<'_, AppCore>, room_id: String) -> alwy_core::Result<bool> {
    core.load_older(&room_id).await
}

#[tauri::command]
async fn send_message(
    core: State<'_, AppCore>,
    room_id: String,
    body: String,
    mentions: Vec<MentionInput>,
    reply_to: Option<String>,
    emotes: Option<Vec<alwy_core::emotes::EmoteInput>>,
) -> alwy_core::Result<()> {
    core.send_message(
        &room_id,
        &body,
        &mentions,
        reply_to.as_deref(),
        &emotes.unwrap_or_default(),
    )
    .await
}

/// A space's categories (sub-spaces) with their chats.
#[tauri::command]
async fn space_categories(
    core: State<'_, AppCore>,
    space_id: String,
) -> alwy_core::Result<Vec<alwy_core::spaces::SpaceCategory>> {
    core.space_categories(&space_id).await
}

/// The chat's shared dates (countdowns, "days together"), replaced as a whole.
#[tauri::command]
async fn set_countdowns(
    core: State<'_, AppCore>,
    room_id: String,
    items: Vec<alwy_core::custom::Countdown>,
) -> alwy_core::Result<()> {
    core.set_countdowns(&room_id, items).await
}

/// Disappearing messages: the chat's timer (ms), or `None` for off.
#[tauri::command]
async fn set_disappearing(
    core: State<'_, AppCore>,
    room_id: String,
    timer_ms: Option<u64>,
) -> alwy_core::Result<()> {
    core.set_disappearing(&room_id, timer_ms).await
}

/// Whether the server deletes this chat's disappearing messages (else the sender's app).
#[tauri::command]
fn disappearing_by_server(core: State<'_, AppCore>, room_id: String) -> alwy_core::Result<bool> {
    core.disappearing_by_server(&room_id)
}

/// Custom emoji and sticker packs: ours, the open chat's, and those turned on everywhere.
#[tauri::command]
async fn emote_packs(
    core: State<'_, AppCore>,
    room_id: Option<String>,
) -> alwy_core::Result<Vec<alwy_core::emotes::EmotePack>> {
    core.emote_packs(room_id.as_deref()).await
}

#[tauri::command]
async fn send_sticker(
    core: State<'_, AppCore>,
    room_id: String,
    url: String,
    body: String,
    info: Option<serde_json::Value>,
) -> alwy_core::Result<()> {
    core.send_sticker(&room_id, &url, &body, info).await
}

#[tauri::command]
async fn add_own_emote(
    core: State<'_, AppCore>,
    shortcode: String,
    url: String,
    info: Option<serde_json::Value>,
    emoticon: bool,
    sticker: bool,
) -> alwy_core::Result<()> {
    core.add_own_emote(&shortcode, &url, info, emoticon, sticker)
        .await
}

#[tauri::command]
async fn remove_own_emote(core: State<'_, AppCore>, shortcode: String) -> alwy_core::Result<()> {
    core.remove_own_emote(&shortcode).await
}

/// An emoji / sticker image (base64; PNG, GIF, WebP or JPEG, made small by the UI) → `mxc://`.
#[tauri::command]
async fn upload_emote(
    core: State<'_, AppCore>,
    image: String,
    mime: String,
) -> Result<String, String> {
    use base64::Engine;
    if !matches!(
        mime.as_str(),
        "image/png" | "image/gif" | "image/webp" | "image/jpeg"
    ) {
        return Err("not an image".into());
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(image)
        .map_err(|e| e.to_string())?;
    core.upload_wallpaper(bytes, &mime)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn room_members(core: State<'_, AppCore>, room_id: String) -> alwy_core::Result<Vec<Member>> {
    core.room_members(&room_id).await
}

#[tauri::command]
async fn toggle_reaction(
    core: State<'_, AppCore>,
    room_id: String,
    item_id: String,
    key: String,
) -> alwy_core::Result<()> {
    core.toggle_reaction(&room_id, &item_id, &key).await
}

#[tauri::command]
async fn mark_read(core: State<'_, AppCore>, room_id: String) -> alwy_core::Result<()> {
    core.mark_read(&room_id).await
}

#[tauri::command]
async fn subscribe_encryption(
    core: State<'_, AppCore>,
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
async fn recover(core: State<'_, AppCore>, key: String) -> alwy_core::Result<()> {
    core.recover(&key).await
}

#[tauri::command]
async fn verify_this_device(core: State<'_, AppCore>) -> alwy_core::Result<()> {
    core.verify_this_device().await
}

#[tauri::command]
async fn accept_verification(core: State<'_, AppCore>) -> alwy_core::Result<()> {
    core.accept_verification().await
}

#[tauri::command]
async fn confirm_emojis(core: State<'_, AppCore>, matching: bool) -> alwy_core::Result<()> {
    core.confirm_emojis(matching).await
}

#[tauri::command]
fn set_can_scan_qr(core: State<'_, AppCore>, can: bool) -> alwy_core::Result<()> {
    core.set_can_scan_qr(can)
}

#[tauri::command]
async fn start_emoji_verification(core: State<'_, AppCore>) -> alwy_core::Result<()> {
    core.start_emoji_verification().await
}

/// A camera frame while scanning a verification code: greyscale, one byte per pixel, base64.
#[tauri::command]
async fn scan_verification_frame(
    core: State<'_, AppCore>,
    width: usize,
    height: usize,
    luma: String,
) -> Result<bool, String> {
    use base64::Engine;
    let luma = base64::engine::general_purpose::STANDARD
        .decode(luma)
        .map_err(|e| e.to_string())?;
    core.scan_verification_frame(width, height, &luma)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn confirm_qr_scanned(core: State<'_, AppCore>, ok: bool) -> alwy_core::Result<()> {
    core.confirm_qr_scanned(ok).await
}

#[tauri::command]
async fn cancel_verification(core: State<'_, AppCore>) -> alwy_core::Result<()> {
    core.cancel_verification().await
}

/// Interface errors, copied into the terminal log so they're visible without devtools.
#[tauri::command]
fn log_ui_error(message: String) {
    tracing::error!(target: "ui", "{message}");
}

/// The server ended our session: stop using it locally, keep the store (see Core::end_session).
#[tauri::command]
async fn end_session(core: State<'_, AppCore>) -> alwy_core::Result<()> {
    core.end_session().await;
    Ok(())
}

#[tauri::command]
async fn logout(core: State<'_, AppCore>) -> alwy_core::Result<()> {
    core.logout().await
}

/// The core is a process-wide singleton: on Android a push can start the process (and the
/// session) before the UI exists, and the UI must then reuse it (one client per store).
type AppCore = &'static Core;
static CORE: std::sync::OnceLock<Core> = std::sync::OnceLock::new();

fn shared_core(data_dir: std::path::PathBuf) -> AppCore {
    CORE.get_or_init(|| Core::new(data_dir))
}

/// Whether pushes deliver this device's notifications (then the sync-driven ones stay quiet,
/// or the phone would show each message twice).
static PUSH_ACTIVE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Register (or remove) this device's pusher. Returns whether push is in use: false on desktop
/// and on builds without Firebase, which notify from the running app instead.
#[tauri::command]
async fn setup_push(
    app: tauri::AppHandle,
    core: State<'_, AppCore>,
    enabled: bool,
) -> Result<bool, String> {
    #[cfg(target_os = "android")]
    {
        // UnifiedPush chosen (Settings → Notifications): it needs nothing from the homeserver.
        let up = unified_push_status(&app).await;
        if let Some(status) = up.as_ref().filter(|s| s.current.is_some()) {
            // Not both: Firebase's pusher goes (if this build has one).
            if let Ok(token) = fcm_token(&app).await {
                let _ = core.remove_pusher(&token).await;
            }
            let Some(endpoint) = status.endpoint.as_deref() else {
                // Registration still going: the endpoint comes to UnifiedPushService, which
                // registers it. Until then the running app notifies.
                PUSH_ACTIVE.store(false, std::sync::atomic::Ordering::Relaxed);
                return Ok(false);
            };
            let result = if enabled {
                core.register_unified_pusher(endpoint, "AlwyChat (Android)")
                    .await
            } else {
                core.remove_pusher(endpoint).await
            };
            result.map_err(|e| e.to_string())?;
            PUSH_ACTIVE.store(enabled, std::sync::atomic::Ordering::Relaxed);
            return Ok(enabled);
        }
        // A UnifiedPush endpoint left from before (switched back to Google): its pusher goes.
        if let Some(endpoint) = up.and_then(|s| s.endpoint) {
            let _ = core.remove_pusher(&endpoint).await;
        }
        // The homeserver offers no push gateway: notifications come from the running app.
        if enabled && core.push_gateway().await.ok().flatten().is_none() {
            return Ok(false);
        }
        let token = match fcm_token(&app).await {
            Ok(token) => token,
            Err(e) => {
                tracing::warn!("no push token, notifying from the running app: {e}");
                PUSH_ACTIVE.store(false, std::sync::atomic::Ordering::Relaxed);
                return Ok(false);
            }
        };
        let result = if enabled {
            core.register_pusher(&token, "AlwyChat (Android)").await
        } else {
            core.remove_pusher(&token).await
        };
        result.map_err(|e| e.to_string())?;
        PUSH_ACTIVE.store(enabled, std::sync::atomic::Ordering::Relaxed);
        Ok(enabled)
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = (app, core, enabled);
        Ok(false)
    }
}

/// UnifiedPush on this phone: distributor apps, the one in use, our endpoint.
#[derive(serde::Deserialize, serde::Serialize)]
struct UnifiedPushStatus {
    distributors: Vec<UnifiedPushDistributor>,
    current: Option<String>,
    endpoint: Option<String>,
}

#[derive(serde::Deserialize, serde::Serialize)]
struct UnifiedPushDistributor {
    id: String,
    name: String,
}

#[cfg(target_os = "android")]
async fn unified_push_status(app: &tauri::AppHandle) -> Option<UnifiedPushStatus> {
    app.state::<android::System>()
        .0
        .run_mobile_plugin_async::<UnifiedPushStatus>("unifiedPushStatus", serde_json::json!({}))
        .await
        .map_err(|e| tracing::warn!("UnifiedPush status: {e}"))
        .ok()
}

/// This phone's Firebase token (fails on builds without Firebase).
#[cfg(target_os = "android")]
async fn fcm_token(app: &tauri::AppHandle) -> Result<String, String> {
    #[derive(serde::Deserialize)]
    struct Token {
        token: String,
    }
    app.state::<android::System>()
        .0
        .run_mobile_plugin_async::<Token>("pushToken", serde_json::json!({}))
        .await
        .map(|t| t.token)
        .map_err(|e| e.to_string())
}

/// Settings → Notifications: the push services to choose from (Android).
#[tauri::command]
async fn unified_push(app: tauri::AppHandle) -> Result<Option<UnifiedPushStatus>, String> {
    #[cfg(target_os = "android")]
    {
        Ok(unified_push_status(&app).await)
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        Ok(None)
    }
}

/// Use a UnifiedPush distributor (its package), or Google again (`None`).
#[tauri::command]
async fn use_unified_push(
    app: tauri::AppHandle,
    distributor: Option<String>,
) -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        app.state::<android::System>()
            .0
            .run_mobile_plugin_async::<serde_json::Value>(
                "useUnifiedPush",
                serde_json::json!({ "distributor": distributor }),
            )
            .await
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = (app, distributor);
        Err("UnifiedPush is for the Android app".into())
    }
}

/// Settings → Notifications → "Send a test notification" (Android).
#[derive(serde::Serialize)]
struct PushTestResult {
    /// Allowed to show notifications.
    permission: bool,
    /// This build has push (Firebase) and the phone got a token.
    token: bool,
    #[serde(flatten)]
    test: Option<alwy_core::push::PushTest>,
}

#[tauri::command]
async fn test_push(
    app: tauri::AppHandle,
    core: State<'_, AppCore>,
) -> Result<PushTestResult, String> {
    #[cfg(target_os = "android")]
    {
        #[derive(serde::Deserialize)]
        struct Token {
            token: String,
        }
        #[derive(serde::Deserialize)]
        struct Status {
            notifications: bool,
        }
        let plugin = &app.state::<android::System>().0;
        let permission = plugin
            .run_mobile_plugin_async::<Status>("status", serde_json::json!({}))
            .await
            .map(|s| s.notifications)
            .unwrap_or(false);
        if let Some(status) = unified_push_status(&app)
            .await
            .filter(|s| s.current.is_some())
        {
            let Some(endpoint) = status.endpoint else {
                return Ok(PushTestResult {
                    permission,
                    token: false,
                    test: None,
                });
            };
            core.register_unified_pusher(&endpoint, "AlwyChat (Android)")
                .await
                .map_err(|e| e.to_string())?;
            let test = core
                .test_unified_push(&endpoint)
                .await
                .map_err(|e| e.to_string())?;
            return Ok(PushTestResult {
                permission,
                token: true,
                test: Some(test),
            });
        }
        let token = match plugin
            .run_mobile_plugin_async::<Token>("pushToken", serde_json::json!({}))
            .await
        {
            Ok(t) => t.token,
            Err(_) => {
                return Ok(PushTestResult {
                    permission,
                    token: false,
                    test: None,
                });
            }
        };
        // Make sure the pusher is there (as every start does), then test it.
        core.register_pusher(&token, "AlwyChat (Android)")
            .await
            .map_err(|e| e.to_string())?;
        let test = core.test_push(&token).await.map_err(|e| e.to_string())?;
        Ok(PushTestResult {
            permission,
            token: true,
            test: Some(test),
        })
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = (app, core);
        Err("push notifications are for the Android app".into())
    }
}

/// `io.github.arlextech.alwychat:/room?id=!room:server` (a tapped notification) → the room ID.
#[cfg_attr(not(mobile), allow(dead_code))]
fn room_link(url: &tauri::Url) -> Option<String> {
    (url.scheme() == alwy_core::session::REDIRECT_SCHEME && url.path() == "/room")
        .then(|| {
            url.query_pairs()
                .find(|(k, _)| k == "id")
                .map(|(_, v)| v.into_owned())
        })
        .flatten()
}

/// `io.github.arlextech.alwychat:/call?id=…&event=…&video=1&expires=…&name=…[&answer=1]`: an
/// incoming call notification (or its Answer button) was tapped.
#[derive(Debug, Clone, serde::Serialize)]
struct CallLink {
    room_id: String,
    notification_id: String,
    video: bool,
    expires_at: u64,
    caller_name: String,
    answer: bool,
}

#[cfg_attr(not(mobile), allow(dead_code))]
fn call_link(url: &tauri::Url) -> Option<CallLink> {
    if url.scheme() != alwy_core::session::REDIRECT_SCHEME || url.path() != "/call" {
        return None;
    }
    let get = |key: &str| {
        url.query_pairs()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.into_owned())
    };
    Some(CallLink {
        room_id: get("id")?,
        notification_id: get("event")?,
        video: get("video").as_deref() == Some("1"),
        expires_at: get("expires")?.parse().ok()?,
        caller_name: get("name").unwrap_or_default(),
        answer: get("answer").as_deref() == Some("1"),
    })
}

/// The incoming call whose notification launched the app, if any.
#[tauri::command]
fn launch_call(app: tauri::AppHandle) -> Option<CallLink> {
    #[cfg(mobile)]
    {
        use tauri_plugin_deep_link::DeepLinkExt;
        app.deep_link()
            .get_current()
            .ok()
            .flatten()?
            .iter()
            .find_map(call_link)
    }
    #[cfg(not(mobile))]
    {
        let _ = app;
        None
    }
}

/// The chat whose notification launched the app, if any.
#[tauri::command]
fn launch_room(app: tauri::AppHandle) -> Option<String> {
    #[cfg(mobile)]
    {
        use tauri_plugin_deep_link::DeepLinkExt;
        app.deep_link()
            .get_current()
            .ok()
            .flatten()?
            .iter()
            .find_map(room_link)
    }
    #[cfg(not(mobile))]
    {
        let _ = app;
        None
    }
}

/// Remove this chat's notifications from the phone (it was just opened).
#[tauri::command]
async fn clear_notifications(app: tauri::AppHandle, room_id: String) -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        app.state::<android::System>()
            .0
            .run_mobile_plugin_async::<serde_json::Value>(
                "clearNotifications",
                serde_json::json!({ "roomId": room_id }),
            )
            .await
            .map_err(|e| e.to_string())?;
    }
    #[cfg(not(target_os = "android"))]
    let _ = (app, room_id);
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg_attr(target_os = "android", allow(unused_variables))]
    let filter = || {
        tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "info,matrix_sdk=warn".into())
    };
    #[cfg(target_os = "android")]
    android::init_logging();
    // ALWYCHAT_LOG_FILE: log to a file (e.g. when the app is launched by a test driver).
    #[cfg(not(target_os = "android"))]
    match std::env::var_os("ALWYCHAT_LOG_FILE").map(std::fs::File::create) {
        Some(Ok(file)) => tracing_subscriber::fmt()
            .with_env_filter(filter())
            .with_ansi(false)
            .with_writer(std::sync::Mutex::new(file))
            .init(),
        _ => tracing_subscriber::fmt().with_env_filter(filter()).init(),
    }

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_biometric::init());
    #[cfg(target_os = "android")]
    let builder = builder
        .plugin(android::gallery_plugin())
        .plugin(android::system_plugin());
    #[cfg(desktop)]
    let builder = builder
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec![STARTUP_ARG]),
        ));
    let builder = builder.setup(|app| {
        #[cfg(target_os = "android")]
        init_android_tls()?;
        #[cfg(target_os = "android")]
        let _ = android::APP.set(app.handle().clone());
        #[cfg(target_os = "linux")]
        enable_media_capture(app)?;
        let data_dir = app.path().app_data_dir()?;
        let core = shared_core(data_dir);
        let handle = app.handle().clone();
        core.on_session_ended(move || {
            let _ = handle.emit("session-ended", ());
        });
        let handle = app.handle().clone();
        core.on_call_signal(move |signal| {
            let _ = handle.emit("call-signal", signal);
        });
        // Auto-save (chats chosen in their settings): Android writes to the cache, then
        // MediaStore takes the file into the gallery; computers save to ~/Pictures/AlwyChat.
        #[cfg(target_os = "android")]
        if let Ok(cache) = app.path().app_cache_dir() {
            let handle = app.handle().clone();
            core.on_auto_save(&cache.join("saving"), move |saved| {
                let handle = handle.clone();
                tauri::async_runtime::spawn(async move {
                    let result: Result<android::Saved, _> = handle
                        .state::<android::Gallery>()
                        .0
                        .run_mobile_plugin_async(
                            "save",
                            android::SaveArgs {
                                path: saved.path,
                                to_gallery: true,
                            },
                        )
                        .await;
                    if let Err(e) = result {
                        tracing::warn!("auto-save to the gallery: {e}");
                    }
                });
            });
        }
        #[cfg(not(target_os = "android"))]
        if let Ok(pictures) = app.path().picture_dir() {
            core.on_auto_save(&pictures.join("AlwyChat"), |_| {});
        }
        let handle = app.handle().clone();
        core.on_call_history(move || {
            let _ = handle.emit("call-history", ());
        });
        let handle = app.handle().clone();
        core.on_call_key(move |key| {
            native_call::on_key(&key);
            let _ = handle.emit("call-key", key);
        });
        let handle = app.handle().clone();
        core.on_draft(move |change| {
            let _ = handle.emit("draft", change);
        });
        let handle = app.handle().clone();
        core.on_notification(move |n| {
            // Only when the window isn't in front: otherwise the chat list already shows it.
            let focused = if cfg!(mobile) {
                handle
                    .state::<Visible>()
                    .0
                    .load(std::sync::atomic::Ordering::Relaxed)
            } else {
                handle
                    .get_webview_window("main")
                    .and_then(|w| w.is_focused().ok())
                    .unwrap_or(false)
            };
            if focused || PUSH_ACTIVE.load(std::sync::atomic::Ordering::Relaxed) {
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
        app.manage(Visible(std::sync::atomic::AtomicBool::new(true)));
        // Picked files left over from a previous run were never sent.
        if let Ok(dir) = picked_dir(app.handle()) {
            let _ = std::fs::remove_dir_all(dir);
        }
        if let Ok(dir) = opened_dir(app.handle()) {
            let _ = std::fs::remove_dir_all(dir);
        }
        // Mobile sign-in comes back as io.github.arlextech.alwychat:/callback?code=…
        #[cfg(mobile)]
        {
            use tauri_plugin_deep_link::DeepLinkExt;
            let handle = app.handle().clone();
            app.deep_link().on_open_url(move |event| {
                for url in event.urls() {
                    if let Some(room) = room_link(&url) {
                        let _ = handle.emit("open-room", room);
                    } else if let Some(call) = call_link(&url) {
                        let _ = handle.emit("call-link", call);
                    } else if url.scheme() == alwy_core::session::REDIRECT_SCHEME
                        && !handle.state::<AppCore>().handle_login_redirect(url)
                    {
                        tracing::warn!("sign-in redirect with no sign-in in progress");
                    }
                }
            });
        }
        // No tray library → no tray (building one would abort the app), just minimise on close.
        #[cfg(desktop)]
        {
            let tray = has_tray_host() && has_tray_library();
            if tray {
                setup_tray(app)?;
            }
            app.manage(TrayHost(tray));
            // Opened with the computer: straight to the tray (or the taskbar without one).
            if std::env::args().any(|a| a == STARTUP_ARG)
                && let Some(window) = app.get_webview_window("main")
            {
                let _ = if tray {
                    window.hide()
                } else {
                    window.minimize()
                };
            }
        }
        Ok(())
    });
    #[cfg(desktop)]
    let builder = builder
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
        });
    builder
        .register_asynchronous_uri_scheme_protocol("media", |ctx, request, responder| {
            let app = ctx.app_handle().clone();
            tauri::async_runtime::spawn(async move {
                responder.respond(harden(serve_media(&app, &request).await));
            });
        })
        .invoke_handler(tauri::generate_handler![
            discover_server,
            restore_session,
            login,
            cancel_login,
            subscribe_chat_list,
            open_chat,
            open_thread,
            profile_extras,
            set_status,
            set_time_zone,
            close_thread,
            set_typing,
            join_call,
            leave_call,
            call_keys,
            call_history,
            viewed_once,
            open_view_once,
            space_banner,
            set_space_banner,
            mark_calls_seen,
            clear_call_history,
            native_call::native_calls,
            native_call::native_call_join,
            native_call::native_call_leave,
            native_call::native_call_set_mic,
            native_call::native_call_set_deafened,
            native_call::native_call_set_muted_for_me,
            native_call::native_call_set_soundboard_off,
            native_call::native_call_set_setup,
            native_call::native_devices,
            native_call::native_camera_preview,
            native_call::native_call_set_camera,
            native_call::native_call_set_screen,
            native_call::native_call_play_sound,
            room_sounds,
            set_room_sounds,
            can_edit_sounds,
            upload_sound,
            call_participants,
            presence,
            report_activity,
            system_status,
            system_clock,
            call_notification,
            pip_shape,
            autostart_get,
            autostart_set,
            emote_packs,
            set_disappearing,
            set_countdowns,
            space_categories,
            disappearing_by_server,
            send_sticker,
            add_own_emote,
            remove_own_emote,
            upload_emote,
            save_blob,
            launch_call,
            call_audio,
            updates::update_check,
            updates::update_install,
            link_preview,
            pin_message,
            create_poll,
            forward_messages,
            file_action,
            decline_call,
            drafts,
            subscribe_spaces,
            open_space,
            close_space,
            join_room,
            can_add_to_space,
            message_activity,
            message_at,
            retry_send,
            subscribe_connection,
            resume_sync,
            send_location,
            start_live_location,
            update_live_location,
            stop_live_location,
            invite_info,
            accept_invite,
            leave_room,
            search_users,
            open_dm,
            create_room,
            join_address,
            space_rooms_joined,
            members,
            invite_user,
            moderate,
            set_user_power,
            set_permission,
            room_details,
            set_room_name,
            set_room_topic,
            set_join_rule,
            set_history_visibility,
            enable_room_encryption,
            set_notify_mode,
            report_room,
            add_to_space,
            remove_from_space,
            set_suggested,
            own_profile,
            set_display_name,
            user_profile,
            devices,
            rename_device,
            account_url,
            security_status,
            new_recovery_key,
            ignored_users,
            set_ignored,
            notification_rules,
            set_notification_rule,
            set_keyword,
            set_room_avatar,
            set_own_avatar,
            create_voice_channel,
            chat_list_prefs,
            set_chat_list_prefs,
            saved_messages_id,
            open_saved_messages,
            load_newer,
            search_messages,
            shared_media,
            search_status,
            start_search_backfill,
            set_system_bars,
            app_lock_state,
            set_app_lock,
            unlock_app,
            unlock_biometric,
            set_secure_screen,
            set_draft,
            vote_poll,
            end_poll,
            setup_push,
            test_push,
            unified_push,
            use_unified_push,
            launch_room,
            clear_notifications,
            set_quick_reaction,
            set_theme,
            upload_wallpaper,
            set_nickname,
            edit_message,
            delete_message,
            edit_history,
            save_media,
            send_file,
            pick_files,
            take_share,
            take_keyboard_content,
            capture,
            send_picked,
            send_voice,
            discard_picked,
            chat_media_hd,
            set_chat_media_hd,
            get_settings,
            set_settings,
            set_pinned,
            set_archived,
            set_muted,
            mutes,
            apply_mutes,
            mute_room,
            unmute_room,
            mute_space,
            unmute_space,
            set_marked_unread,
            close_chat,
            load_older,
            send_message,
            room_members,
            toggle_reaction,
            mark_read,
            subscribe_encryption,
            recover,
            verify_this_device,
            accept_verification,
            confirm_emojis,
            cancel_verification,
            set_can_scan_qr,
            start_emoji_verification,
            scan_verification_frame,
            confirm_qr_scanned,
            log_ui_error,
            end_session,
            logout
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// matrix-sdk's HTTP client checks certificates with Android's own verifier, reached over JNI:
/// hand it the JVM and app context before any request (else the first request panics).
#[cfg(target_os = "android")]
#[allow(unsafe_code)]
fn init_android_tls() -> Result<(), Box<dyn std::error::Error>> {
    let ctx = ndk_context::android_context();
    // SAFETY: tao fills ndk-context with the process' JavaVM and the activity before setup runs;
    // both stay valid for the life of the process (the verifier keeps a global ref).
    let vm = unsafe { jni::JavaVM::from_raw(ctx.vm().cast()) };
    vm.attach_current_thread(|env| -> Result<(), jni::errors::Error> {
        let context = unsafe { jni::objects::JObject::from_raw(env, ctx.context().cast()) };
        rustls_platform_verifier::android::init_with_env(env, context)
    })?;
    Ok(())
}

#[cfg(desktop)]
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

#[cfg(desktop)]
/// Whether the desktop can show our tray icon.
struct TrayHost(bool);

#[cfg(desktop)]
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

#[cfg(desktop)]
/// Linux tray icons are drawn through libayatana-appindicator (or the older libappindicator),
/// loaded at runtime: tauri panics if neither is installed, so look for the file first.
fn has_tray_library() -> bool {
    #[cfg(target_os = "linux")]
    {
        const LIBS: [&str; 2] = ["libayatana-appindicator3.so.1", "libappindicator3.so.1"];
        let env_dirs = std::env::var("LD_LIBRARY_PATH").unwrap_or_default();
        let dirs = env_dirs.split(':').filter(|d| !d.is_empty()).chain([
            "/usr/lib",
            "/usr/lib64",
            "/usr/local/lib",
            "/usr/lib/x86_64-linux-gnu",
            "/usr/lib/aarch64-linux-gnu",
            "/app/lib", // Flatpak
        ]);
        let found = dirs
            .flat_map(|dir| LIBS.map(|lib| std::path::Path::new(dir).join(lib)))
            .any(|path| path.exists());
        if !found {
            tracing::info!("no appindicator library: running without a tray icon");
        }
        found
    }
    #[cfg(not(target_os = "linux"))]
    true
}

#[cfg(desktop)]
fn show_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// The `media://` scheme (URL helpers in app/src/lib/media.ts):
/// - `media://localhost/<server>/<media-id>?size=96`: avatar thumbnail of an `mxc://` URI;
/// - `media://localhost/<server>/<media-id>?file=1`: the whole file (soundboard sounds);
/// - `media://localhost/_event/<item id>[?thumb=1]`: a message's (decrypted) media.
async fn serve_media(
    app: &tauri::AppHandle,
    request: &http::Request<Vec<u8>>,
) -> http::Response<Vec<u8>> {
    let uri = request.uri();
    let core = app.state::<AppCore>();
    let query = uri.query().unwrap_or("");
    let param = |name: &str| {
        query
            .split('&')
            .find_map(|kv| kv.strip_prefix(name)?.strip_prefix('='))
    };

    if let Some(name) = uri.path().strip_prefix("/_picked/") {
        return serve_picked(app, request, name);
    }
    let result = match uri.path().strip_prefix("/_event/") {
        Some(key) => {
            let key = percent_encoding::percent_decode_str(key).decode_utf8_lossy();
            let thumb = param("thumb") == Some("1");
            let media = core.event_media(&key, thumb).await;
            // HEIC photos (many phones' camera format) can't be shown by the web view.
            #[cfg(target_os = "android")]
            let media = match media {
                Ok((bytes, _)) if alwy_core::media::is_heic(&bytes) => {
                    heic_as_jpeg(app, &key, thumb, bytes).await
                }
                other => other,
            };
            media
        }
        None if param("file") == Some("1") => {
            let mxc = format!("mxc:/{}", uri.path());
            core.plain_file(&mxc).await.map(|bytes| {
                let mime = alwy_core::media::sniff(&bytes).to_owned();
                (bytes, mime)
            })
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

/// HEIC photos already converted for display (by media key + preview or not).
#[cfg(target_os = "android")]
static HEIC_JPEGS: std::sync::LazyLock<std::sync::Mutex<Vec<(String, Vec<u8>)>>> =
    std::sync::LazyLock::new(Default::default);

/// A received HEIC photo as JPEG, converted by Android (previews made smaller). Kept for a few
/// photos, so scrolling back doesn't convert them again.
#[cfg(target_os = "android")]
async fn heic_as_jpeg(
    app: &tauri::AppHandle,
    key: &str,
    thumb: bool,
    bytes: Vec<u8>,
) -> alwy_core::Result<(Vec<u8>, String)> {
    let id = format!("{key}#{thumb}");
    if let Some((_, jpeg)) = HEIC_JPEGS.lock().unwrap().iter().find(|(k, _)| *k == id) {
        return Ok((jpeg.clone(), "image/jpeg".into()));
    }
    let dir = app
        .path()
        .app_cache_dir()
        .map_err(|e| alwy_core::Error::InvalidMedia(e.to_string()))?
        .join("heic");
    std::fs::create_dir_all(&dir)?;
    let stem: String = id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    let (src, dst) = (
        dir.join(format!("{stem}.heic")),
        dir.join(format!("{stem}.jpg")),
    );
    std::fs::write(&src, &bytes)?;
    let converted = app
        .state::<android::Gallery>()
        .0
        .run_mobile_plugin_async::<serde_json::Value>(
            "heicToJpeg",
            serde_json::json!({ "path": src, "to": dst, "maxSide": if thumb { 1280 } else { 0 } }),
        )
        .await;
    let _ = std::fs::remove_file(&src);
    converted.map_err(|e| alwy_core::Error::InvalidMedia(e.to_string()))?;
    let jpeg = std::fs::read(&dst)?;
    let _ = std::fs::remove_file(&dst);
    let mut cache = HEIC_JPEGS.lock().unwrap();
    cache.push((id, jpeg.clone()));
    if cache.len() > 12 {
        cache.remove(0);
    }
    Ok((jpeg, "image/jpeg".into()))
}

/// `media://localhost/_picked/<file name>`: preview of a file waiting to be sent.
/// WebKitGTK refuses microphone / camera access unless the app answers its permission
/// requests (voice messages failed with NotAllowedError). Grant them to the app's own pages
/// (the CSP keeps anything else out); other requests keep the default answer.
///
/// Calls can't run in this webview on Arch/CachyOS: its WebKitGTK is built without WebRTC
/// (no `RTCPeerConnection`, whatever the settings), so LiveKit refuses to start.
#[cfg(target_os = "linux")]
fn enable_media_capture(app: &tauri::App) -> tauri::Result<()> {
    let Some(window) = app.get_webview_window("main") else {
        return Ok(());
    };
    window.with_webview(|webview| {
        use webkit2gtk::glib::prelude::Cast;
        use webkit2gtk::{
            PermissionRequestExt, SettingsExt, UserMediaPermissionRequest, WebViewExt,
        };
        let view = webview.inner();
        if let Some(settings) = WebViewExt::settings(&view) {
            settings.set_enable_media_stream(true);
        }
        view.connect_permission_request(|_, request| {
            if request
                .downcast_ref::<UserMediaPermissionRequest>()
                .is_some()
            {
                request.allow();
                return true;
            }
            false
        });
    })
}

/// Media bytes come from other people, with the type they declared: never let the webview
/// guess another type, and never let a response act as a page (no scripts, no forms).
fn harden(mut response: http::Response<Vec<u8>>) -> http::Response<Vec<u8>> {
    let headers = response.headers_mut();
    headers.insert(
        http::header::X_CONTENT_TYPE_OPTIONS,
        http::HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        http::header::CONTENT_SECURITY_POLICY,
        http::HeaderValue::from_static("default-src 'none'; sandbox"),
    );
    // Only this app's webview can reach media://; let its code read the bytes (Linux plays
    // audio/video from a blob: WebKitGTK's player can't stream a custom scheme).
    headers.insert(
        http::header::ACCESS_CONTROL_ALLOW_ORIGIN,
        http::HeaderValue::from_static("*"),
    );
    // Scripting content types are served as plain bytes.
    if let Some(kind) = headers
        .get(http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
    {
        let kind = kind.to_ascii_lowercase();
        if kind.contains("html") || kind.contains("xml") || kind.contains("javascript") {
            headers.insert(
                http::header::CONTENT_TYPE,
                http::HeaderValue::from_static("application/octet-stream"),
            );
        }
    }
    response
}

fn serve_picked(
    app: &tauri::AppHandle,
    request: &http::Request<Vec<u8>>,
    name: &str,
) -> http::Response<Vec<u8>> {
    let name = percent_encoding::percent_decode_str(name).decode_utf8_lossy();
    // File name only: no way out of the picked folder.
    let bytes = std::path::Path::new(name.as_ref())
        .file_name()
        .and_then(|n| std::fs::read(picked_dir(app).ok()?.join(n)).ok());
    match bytes {
        Some(bytes) => {
            let mime = alwy_core::media::sniff(&bytes).to_owned();
            respond_with_range(request, bytes, &mime)
        }
        None => http::Response::builder()
            .status(http::StatusCode::NOT_FOUND)
            .body(Vec::new())
            .expect("valid response"),
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

/// Android-only native bits (Kotlin side in gen/android/app/src/main/java/com/alwy/chat).
#[cfg(target_os = "android")]
mod android {
    use tauri::{Manager, Wry, plugin::PluginHandle};

    pub struct Gallery(pub PluginHandle<Wry>);
    pub struct System(pub PluginHandle<Wry>);

    /// Logcat output (tag `AlwyChat`). Called by the UI start and by a push, whichever is first.
    pub fn init_logging() {
        use tracing_subscriber::prelude::*;
        let filter = tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "info,matrix_sdk=warn".into());
        let _ = tracing_subscriber::registry()
            .with(filter)
            .with(paranoid_android::layer("AlwyChat"))
            .try_init();
    }

    /// The app, for events from Android code (the call notification's buttons).
    pub static APP: std::sync::OnceLock<tauri::AppHandle> = std::sync::OnceLock::new();

    /// Called by `CallService.kt` when a button of the call notification is tapped
    /// ("hang_up", "toggle_mic"): passed on to the UI.
    #[allow(unsafe_code)] // JNI entry points can't be mangled.
    #[unsafe(no_mangle)]
    pub extern "system" fn Java_com_alwy_chat_NativeCall_action<'c>(
        mut env: jni::EnvUnowned<'c>,
        _class: jni::objects::JClass<'c>,
        name: jni::objects::JString<'c>,
    ) {
        let name = env
            .with_env(|_| -> Result<_, jni::errors::Error> { Ok(name.to_string()) })
            .resolve::<jni::errors::ThrowRuntimeExAndDefault>();
        if let Some(app) = APP.get() {
            use tauri::Emitter;
            let _ = app.emit("call-action", name);
        }
    }

    /// Called by `PushService.kt` for each push (on a worker thread, app maybe not running):
    /// returns the notification as JSON, or "" when there's nothing to show.
    #[allow(unsafe_code)] // JNI entry points can't be mangled.
    #[unsafe(no_mangle)]
    pub extern "system" fn Java_com_alwy_chat_NativePush_notification<'c>(
        mut env: jni::EnvUnowned<'c>,
        _class: jni::objects::JClass<'c>,
        context: jni::objects::JObject<'c>,
        data_dir: jni::objects::JString<'c>,
        room_id: jni::objects::JString<'c>,
        event_id: jni::objects::JString<'c>,
    ) -> jni::objects::JString<'c> {
        env.with_env(|env| -> Result<_, jni::errors::Error> {
            init_logging();
            // HTTPS needs this before any request; the UI may not have started yet.
            rustls_platform_verifier::android::init_with_env(env, context)?;
            let (dir, room, event) = (
                data_dir.to_string(),
                room_id.to_string(),
                event_id.to_string(),
            );
            let json = tauri::async_runtime::block_on(notification(dir.into(), room, event));
            jni::objects::JString::from_str(env, json)
        })
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
    }

    /// Called by `UnifiedPushService.kt` with a new endpoint (empty: none any more) and the
    /// previous one: the homeserver's pusher follows (app maybe not running).
    #[allow(unsafe_code)] // JNI entry points can't be mangled.
    #[unsafe(no_mangle)]
    pub extern "system" fn Java_com_alwy_chat_NativePush_setEndpoint<'c>(
        mut env: jni::EnvUnowned<'c>,
        _class: jni::objects::JClass<'c>,
        context: jni::objects::JObject<'c>,
        data_dir: jni::objects::JString<'c>,
        endpoint: jni::objects::JString<'c>,
        old: jni::objects::JString<'c>,
    ) {
        env.with_env(|env| -> Result<_, jni::errors::Error> {
            init_logging();
            rustls_platform_verifier::android::init_with_env(env, context)?;
            let (dir, endpoint, old) =
                (data_dir.to_string(), endpoint.to_string(), old.to_string());
            tauri::async_runtime::block_on(async move {
                let core = super::shared_core(dir.into());
                if !matches!(core.restore().await, Ok(Some(_))) {
                    return;
                }
                if !old.is_empty()
                    && let Err(e) = core.remove_pusher(&old).await
                {
                    tracing::warn!("old UnifiedPush pusher: {e}");
                }
                if endpoint.is_empty() {
                    super::PUSH_ACTIVE.store(false, std::sync::atomic::Ordering::Relaxed);
                    return;
                }
                match core
                    .register_unified_pusher(&endpoint, "AlwyChat (Android)")
                    .await
                {
                    Ok(()) => super::PUSH_ACTIVE.store(true, std::sync::atomic::Ordering::Relaxed),
                    Err(e) => tracing::warn!("UnifiedPush pusher: {e}"),
                }
            });
            Ok(())
        })
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
    }

    /// Called by the incoming-call notification's "Decline" (app maybe not running).
    #[allow(unsafe_code)] // JNI entry points can't be mangled.
    #[unsafe(no_mangle)]
    pub extern "system" fn Java_com_alwy_chat_NativePush_decline<'c>(
        mut env: jni::EnvUnowned<'c>,
        _class: jni::objects::JClass<'c>,
        context: jni::objects::JObject<'c>,
        data_dir: jni::objects::JString<'c>,
        room_id: jni::objects::JString<'c>,
        event_id: jni::objects::JString<'c>,
    ) {
        env.with_env(|env| -> Result<_, jni::errors::Error> {
            init_logging();
            rustls_platform_verifier::android::init_with_env(env, context)?;
            let (dir, room, event) = (
                data_dir.to_string(),
                room_id.to_string(),
                event_id.to_string(),
            );
            tauri::async_runtime::block_on(async move {
                let core = super::shared_core(dir.into());
                if !matches!(core.restore().await, Ok(Some(_))) {
                    return;
                }
                if let Err(e) = core.decline_call(&room, &event).await {
                    tracing::warn!("decline from the notification: {e}");
                }
            });
            Ok(())
        })
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
    }

    async fn notification(
        data_dir: std::path::PathBuf,
        room_id: String,
        event_id: String,
    ) -> String {
        let core = super::shared_core(data_dir);
        match core.restore().await {
            Ok(Some(_)) => {}
            Ok(None) => return String::new(),
            Err(e) => {
                tracing::warn!("push: couldn't start the session: {e}");
                return String::new();
            }
        }
        // A push is also the moment to end mutes whose time is up (the app may be closed).
        tauri::async_runtime::spawn(async move {
            if let Err(e) = core.apply_mutes().await {
                tracing::warn!("push: mutes: {e}");
            }
        });
        match core.push_notification(&room_id, &event_id).await {
            Ok(Some(n)) => serde_json::to_string(&n).unwrap_or_default(),
            Ok(None) => String::new(),
            Err(e) => {
                tracing::warn!("push: {e}");
                String::new()
            }
        }
    }

    #[derive(serde::Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct SaveArgs {
        pub path: String,
        pub to_gallery: bool,
    }

    #[derive(serde::Deserialize)]
    pub struct Saved {
        pub path: String,
    }

    pub fn gallery_plugin() -> tauri::plugin::TauriPlugin<Wry> {
        tauri::plugin::Builder::new("alwy-gallery")
            .setup(|app, api| {
                let handle = api.register_android_plugin("com.alwy.chat", "GalleryPlugin")?;
                app.manage(Gallery(handle));
                Ok(())
            })
            .build()
    }

    /// Separate Tauri plugin: one Tauri plugin name maps to one Kotlin class (sharing the
    /// name sent gallery calls to SystemPlugin).
    pub fn system_plugin() -> tauri::plugin::TauriPlugin<Wry> {
        tauri::plugin::Builder::new("alwy-system")
            .setup(|app, api| {
                let handle = api.register_android_plugin("com.alwy.chat", "SystemPlugin")?;
                app.manage(System(handle));
                Ok(())
            })
            .build()
    }
}
