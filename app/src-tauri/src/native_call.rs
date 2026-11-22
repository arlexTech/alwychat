//! Calls through the Rust LiveKit SDK (`alwy-call`) where the webview has no WebRTC: WebKitGTK
//! on Linux. The UI still does the Matrix side (`join_call` gives the LiveKit URL and token),
//! then hands the media part to these commands instead of livekit-client. Elsewhere they say
//! no and the browser engine is used.

use tauri::ipc::{Channel, InvokeResponseBody};

#[cfg(target_os = "linux")]
static CALL: std::sync::Mutex<Option<std::sync::Arc<alwy_call::NativeCall>>> =
    std::sync::Mutex::new(None);

/// Whether calls run natively on this build (the UI picks its call engine from this).
#[tauri::command]
pub fn native_calls() -> bool {
    cfg!(target_os = "linux")
}

#[cfg(target_os = "linux")]
fn with_call(f: impl FnOnce(&alwy_call::NativeCall)) -> Result<(), String> {
    match CALL.lock().unwrap().as_ref() {
        Some(call) => {
            f(call);
            Ok(())
        }
        None => Err("not in a call".into()),
    }
}

#[cfg(not(target_os = "linux"))]
fn unsupported<T>() -> Result<T, String> {
    Err("native calls aren't used on this system".into())
}

/// (identity, index, raw key) for every identity of each key.
#[cfg(target_os = "linux")]
fn key_triples(keys: &[alwy_core::call_keys::CallKey]) -> Vec<(String, u8, Vec<u8>)> {
    use base64::Engine;
    keys.iter()
        .filter_map(|k| {
            let raw = base64::engine::general_purpose::STANDARD
                .decode(&k.key)
                .ok()?;
            Some(
                k.identities
                    .iter()
                    .map(move |id| (id.clone(), k.index, raw.clone())),
            )
        })
        .flatten()
        .collect()
}

/// A new call media key from the core: straight to the native call, if one runs.
pub fn on_key(key: &alwy_core::call_keys::CallKey) {
    #[cfg(target_os = "linux")]
    if let Some(call) = CALL.lock().unwrap().as_ref() {
        for (identity, index, raw) in key_triples(std::slice::from_ref(key)) {
            call.set_key(&identity, index, raw);
        }
    }
    #[cfg(not(target_os = "linux"))]
    let _ = key;
}

/// Join with the token from `join_call`; `on_event` gets members, connection changes and the end.
#[tauri::command]
#[allow(clippy::too_many_arguments)] // the command's parameters, from the UI
pub async fn native_call_join(
    core: tauri::State<'_, crate::AppCore>,
    room_id: String,
    url: String,
    token: String,
    encrypted: bool,
    mic_on: bool,
    setup: serde_json::Value,
    on_event: Channel<serde_json::Value>,
    on_frame: Channel<InvokeResponseBody>,
) -> Result<(), String> {
    // libwebrtc writes its statistics as JSON with printf, which follows the system's number
    // format: "7,24e+18" on a French system. The Rust side can't parse that and aborts the
    // app (voice_meter reads them 10 times a second). GTK sets the locale when it starts,
    // so this comes after it; the interface runs in the webview and isn't affected.
    // SAFETY: nothing here keeps a pointer to the locale; the string is a valid C string.
    #[cfg(target_os = "linux")]
    #[allow(unsafe_code)]
    let _ = unsafe { libc::setlocale(libc::LC_NUMERIC, c"C".as_ptr()) };
    #[cfg(target_os = "linux")]
    {
        native_call_leave().await?;
        let setup = serde_json::from_value(setup).map_err(|e| e.to_string())?;
        let call = alwy_call::NativeCall::join(
            &url,
            &token,
            alwy_call::Encryption {
                encrypt: encrypted,
                keys: key_triples(&core.call_keys(&room_id)),
            },
            mic_on,
            setup,
            move |e| {
                if let Ok(value) = serde_json::to_value(e) {
                    let _ = on_event.send(value);
                }
            },
            // Raw bytes (no JSON): [key length][key][JPEG].
            move |key, jpeg| {
                let mut message = Vec::with_capacity(1 + key.len() + jpeg.len());
                message.push(key.len().min(255) as u8);
                message.extend_from_slice(&key.as_bytes()[..key.len().min(255)]);
                message.extend_from_slice(&jpeg);
                let _ = on_frame.send(InvokeResponseBody::Raw(message));
            },
        )
        .await?;
        // Keys that came in while connecting.
        for (identity, index, key) in key_triples(&core.call_keys(&room_id)) {
            call.set_key(&identity, index, key);
        }
        *CALL.lock().unwrap() = Some(std::sync::Arc::new(call));
        Ok(())
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (
            core, room_id, url, token, encrypted, mic_on, setup, on_event, on_frame,
        );
        unsupported()
    }
}

#[tauri::command]
pub async fn native_call_leave() -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        let call = CALL.lock().unwrap().take();
        if let Some(call) = call {
            call.leave().await;
        }
        Ok(())
    }
    #[cfg(not(target_os = "linux"))]
    unsupported()
}

#[tauri::command]
pub fn native_call_set_mic(on: bool) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    return with_call(|c| c.set_mic(on));
    #[cfg(not(target_os = "linux"))]
    {
        let _ = on;
        unsupported()
    }
}

#[tauri::command]
pub fn native_call_set_deafened(deafened: bool) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    return with_call(|c| c.set_deafened(deafened));
    #[cfg(not(target_os = "linux"))]
    {
        let _ = deafened;
        unsupported()
    }
}

#[tauri::command]
pub fn native_call_set_muted_for_me(user_id: String, muted: bool) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    return with_call(|c| c.set_muted_for_me(&user_id, muted));
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (user_id, muted);
        unsupported()
    }
}

/// How loud one person is for us (gain).
#[tauri::command]
pub fn native_call_set_volume(user_id: String, volume: f32) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    return with_call(|c| c.set_volume(&user_id, volume));
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (user_id, volume);
        unsupported()
    }
}

/// How loud others' soundboard sounds are for us (gain).
#[tauri::command]
pub fn native_call_set_soundboard_volume(volume: f32) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    return with_call(|c| c.set_soundboard_volume(volume));
    #[cfg(not(target_os = "linux"))]
    {
        let _ = volume;
        unsupported()
    }
}

/// Hear others' soundboard sounds or not (the volume slider at 0).
#[tauri::command]
pub fn native_call_set_soundboard_off(off: bool) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    return with_call(|c| c.set_soundboard_off(off));
    #[cfg(not(target_os = "linux"))]
    {
        let _ = off;
        unsupported()
    }
}

/// Devices and audio processing changed in the settings: applied to the call live.
#[tauri::command]
pub fn native_call_set_setup(setup: serde_json::Value) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        let setup = serde_json::from_value(setup).map_err(|e| e.to_string())?;
        with_call(|c| c.set_setup(setup))
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = setup;
        unsupported()
    }
}

/// Microphones, speakers and cameras the native engine can use.
#[tauri::command]
pub async fn native_devices() -> Result<serde_json::Value, String> {
    #[cfg(target_os = "linux")]
    {
        let devices = tauri::async_runtime::spawn_blocking(alwy_call::devices)
            .await
            .map_err(|e| e.to_string())??;
        serde_json::to_value(devices).map_err(|e| e.to_string())
    }
    #[cfg(not(target_os = "linux"))]
    unsupported()
}

/// Camera / screen share start asynchronously: the call is shared, not held under the lock.
#[cfg(target_os = "linux")]
async fn with_call_async<F, Fut>(f: F) -> Result<(), String>
where
    F: FnOnce(std::sync::Arc<alwy_call::NativeCall>) -> Fut,
    Fut: std::future::Future<Output = Result<(), String>>,
{
    let call = CALL.lock().unwrap().clone();
    match call {
        Some(call) => f(call).await,
        None => Err("not in a call".into()),
    }
}

#[tauri::command]
pub async fn native_call_set_camera(on: bool) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    return with_call_async(|c| async move { c.set_camera(on).await }).await;
    #[cfg(not(target_os = "linux"))]
    {
        let _ = on;
        unsupported()
    }
}

#[tauri::command]
pub async fn native_call_set_screen(
    app: tauri::AppHandle,
    on: bool,
    fps: Option<u32>,
    max_bitrate: Option<u64>,
    max_width: Option<u32>,
    max_height: Option<u32>,
    codec: Option<String>,
) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        let mut quality = screen_quality(fps, max_bitrate, max_width, max_height);
        quality.encoder = match codec.as_deref() {
            Some("av1") => alwy_call::ScreenEncoder::Av1,
            Some("h264") => alwy_call::ScreenEncoder::H264,
            Some("vp8") => alwy_call::ScreenEncoder::Vp8,
            _ => alwy_call::ScreenEncoder::Auto,
        };
        // Monitor sizes (device pixels, and the scaled size for fractional scaling) tell a
        // whole screen from a window.
        let monitors: Vec<(u32, u32)> = app
            .available_monitors()
            .unwrap_or_default()
            .iter()
            .flat_map(|m| {
                let size = m.size();
                let scale = m.scale_factor().max(0.1);
                [
                    (size.width, size.height),
                    (
                        (f64::from(size.width) / scale).round() as u32,
                        (f64::from(size.height) / scale).round() as u32,
                    ),
                ]
            })
            .collect();
        return with_call_async(|c| async move { c.set_screen(on, quality, &monitors).await })
            .await;
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (app, on, fps, max_bitrate, max_width, max_height, codec);
        unsupported()
    }
}

/// What our screen share really sends (encoder, frame rate, size, what limits it), or null.
#[tauri::command]
pub fn native_call_screen_stats() -> Option<serde_json::Value> {
    #[cfg(target_os = "linux")]
    return alwy_call::screen_send_stats().and_then(|s| serde_json::to_value(s).ok());
    #[cfg(not(target_os = "linux"))]
    None
}

/// The shared screen's sound: `{choice: {kind: all|none|app, app?}, apps: [{id, name}]}`, or
/// null when it has none.
#[tauri::command]
pub fn native_call_screen_sound() -> Result<serde_json::Value, String> {
    #[cfg(target_os = "linux")]
    {
        let call = CALL.lock().unwrap().clone().ok_or("not in a call")?;
        Ok(match call.screen_sound() {
            Some((choice, apps)) => serde_json::json!({ "choice": choice, "apps": apps }),
            None => serde_json::Value::Null,
        })
    }
    #[cfg(not(target_os = "linux"))]
    unsupported()
}

#[tauri::command]
pub fn native_call_set_screen_sound(choice: serde_json::Value) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        let choice: alwy_call::SoundChoice =
            serde_json::from_value(choice).map_err(|e| e.to_string())?;
        with_call(|c| c.set_screen_sound(choice))
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = choice;
        unsupported()
    }
}

/// Watch someone's shared screen or not; hear its sound or not, and how loud (gain).
#[tauri::command]
pub fn native_call_watch_screen(
    identity: String,
    watch: bool,
    sound: bool,
    volume: f32,
) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    return with_call(|c| {
        c.set_watching_screen(&identity, watch);
        c.set_screen_sound_off(&identity, !sound, volume);
    });
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (identity, watch, sound, volume);
        unsupported()
    }
}

#[cfg(target_os = "linux")]
fn screen_quality(
    fps: Option<u32>,
    max_bitrate: Option<u64>,
    max_width: Option<u32>,
    max_height: Option<u32>,
) -> alwy_call::ScreenQuality {
    let mut quality = alwy_call::ScreenQuality::default();
    if let Some(fps) = fps {
        quality.fps = fps.clamp(1, 60);
    }
    if let Some(b) = max_bitrate {
        quality.max_bitrate = b;
    }
    if let (Some(w), Some(h)) = (max_width, max_height) {
        quality.max_width = w.max(320);
        quality.max_height = h.max(180);
    }
    quality
}

/// Change a screen share's quality while sharing.
#[tauri::command]
pub fn native_call_screen_quality(
    fps: Option<u32>,
    max_bitrate: Option<u64>,
    max_width: Option<u32>,
    max_height: Option<u32>,
) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        let quality = screen_quality(fps, max_bitrate, max_width, max_height);
        with_call(|c| c.set_screen_quality(quality))
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (fps, max_bitrate, max_width, max_height);
        unsupported()
    }
}

/// Draw our own screen share's preview (hidden by default).
#[tauri::command]
pub fn native_call_screen_preview(on: bool) {
    #[cfg(target_os = "linux")]
    alwy_call::set_screen_preview(on);
    #[cfg(not(target_os = "linux"))]
    let _ = on;
}

#[cfg(target_os = "linux")]
static PREVIEW: std::sync::Mutex<Option<alwy_call::CameraPreview>> = std::sync::Mutex::new(None);

/// Settings' camera preview through the native capture (the one calls use): frames under
/// `alwy_call::CAMERA_PREVIEW` on `on_frame`, until called again with `on: false`.
#[tauri::command]
pub async fn native_camera_preview(
    on: bool,
    camera: Option<String>,
    on_frame: Channel<InvokeResponseBody>,
) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        // Stop the previous one first: a camera opens once at a time.
        drop(PREVIEW.lock().unwrap().take());
        if on {
            let index = camera.and_then(|c| c.parse().ok()).unwrap_or(0);
            let preview = alwy_call::CameraPreview::start(
                index,
                std::sync::Arc::new(move |key: &str, jpeg: Vec<u8>| {
                    let mut message = Vec::with_capacity(1 + key.len() + jpeg.len());
                    message.push(key.len().min(255) as u8);
                    message.extend_from_slice(&key.as_bytes()[..key.len().min(255)]);
                    message.extend_from_slice(&jpeg);
                    let _ = on_frame.send(InvokeResponseBody::Raw(message));
                }),
            )
            .await?;
            *PREVIEW.lock().unwrap() = Some(preview);
        }
        Ok(())
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (on, camera, on_frame);
        unsupported()
    }
}

/// Play a soundboard sound in the native call: 48 kHz mono 16-bit samples, base64.
#[tauri::command]
pub async fn native_call_play_sound(pcm: String) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        use base64::Engine;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(pcm)
            .map_err(|e| e.to_string())?;
        let samples = bytes
            .chunks_exact(2)
            .map(|b| i16::from_le_bytes([b[0], b[1]]))
            .collect();
        let call = CALL.lock().unwrap().clone().ok_or("not in a call")?;
        call.play_sound(samples).await
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = pcm;
        unsupported()
    }
}
