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

/// Join with the token from `join_call`; `on_event` gets members, connection changes and the end.
#[tauri::command]
pub async fn native_call_join(
    url: String,
    token: String,
    mic_on: bool,
    setup: serde_json::Value,
    on_event: Channel<serde_json::Value>,
    on_frame: Channel<InvokeResponseBody>,
) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        native_call_leave().await?;
        let setup = serde_json::from_value(setup).map_err(|e| e.to_string())?;
        let call = alwy_call::NativeCall::join(
            &url,
            &token,
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
        *CALL.lock().unwrap() = Some(std::sync::Arc::new(call));
        Ok(())
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (url, token, mic_on, setup, on_event, on_frame);
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
pub async fn native_call_set_screen(on: bool) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    return with_call_async(|c| async move { c.set_screen(on).await }).await;
    #[cfg(not(target_os = "linux"))]
    {
        let _ = on;
        unsupported()
    }
}
