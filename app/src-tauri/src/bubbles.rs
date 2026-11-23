//! Android: the app's windows. Each Android activity that shows AlwyChat gets its own webview
//! window, created when the activity tells us it exists (`NativeBubble.window`):
//! - `MainActivity` → the app (label `main`);
//! - `BubbleActivity` → one chat in a notification bubble (label `bubble-<activity id>`),
//!   which the UI shows alone (`window.__alwyBubble` = the chat's room ID).
//!
//! Android starts a bubble's activity itself, so the window is attached to that running activity
//! (`ClassName#<id>`, see vendor/tao/ALWYCHAT-PATCH.md) instead of tao starting a new one. The
//! main window isn't created from tauri.conf.json either: if the first activity of the process
//! is a bubble, it would have landed there.

use std::sync::Mutex;

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

/// Which activity asked for a window.
#[derive(Debug, Clone)]
struct Wanted {
    activity: String,
    id: i32,
    room: Option<String>,
}

/// Windows asked for before the app finished starting (the first activity's `onCreate` runs
/// before Tauri's setup).
static PENDING: Mutex<Vec<Wanted>> = Mutex::new(Vec::new());

/// Called from setup: the app is ready, create the windows asked for so far.
pub fn ready(app: &AppHandle) {
    let wanted: Vec<_> = PENDING.lock().unwrap().drain(..).collect();
    for w in wanted {
        create(app, w);
    }
}

fn label(w: &Wanted) -> String {
    match &w.room {
        None => "main".to_owned(),
        Some(_) => format!("bubble-{}", w.id),
    }
}

fn create(app: &AppHandle, w: Wanted) {
    let label = label(&w);
    // Same activity again (recreated for a configuration change): its window is still there.
    if app.get_webview_window(&label).is_some() {
        return;
    }
    let Some(config) = app.config().app.windows.first().cloned() else {
        return;
    };
    let activity = format!("{}#{}", w.activity, w.id);
    let built = match &w.room {
        None => WebviewWindowBuilder::from_config(app, &config).map(|b| b.activity_name(activity)),
        Some(room) => {
            let room = serde_json::to_string(room).unwrap_or_default();
            Ok(
                WebviewWindowBuilder::new(app, &label, WebviewUrl::App("index.html".into()))
                    .activity_name(activity)
                    .initialization_script(format!("window.__alwyBubble = {room};")),
            )
        }
    }
    .and_then(|b| b.build());
    if let Err(e) = built {
        tracing::warn!("window for {}: {e}", w.activity);
    }
}

fn wanted(w: Wanted) {
    // The app may still be starting: setup creates it then.
    let app = {
        let mut pending = PENDING.lock().unwrap();
        match crate::android::APP.get() {
            Some(app) => app.clone(),
            None => return pending.push(w),
        }
    };
    // Not on Android's UI thread: building a window waits for the event loop.
    std::thread::spawn(move || create(&app, w));
}

/// Called by `MainActivity` / `BubbleActivity` once created: `room` is the bubble's chat
/// (empty for the app itself).
#[allow(unsafe_code)] // JNI entry points can't be mangled.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_alwy_chat_NativeBubble_window<'c>(
    mut env: jni::EnvUnowned<'c>,
    _class: jni::objects::JClass<'c>,
    activity: jni::objects::JString<'c>,
    id: jni::sys::jint,
    room: jni::objects::JString<'c>,
) {
    let names = env
        .with_env(|_| -> Result<_, jni::errors::Error> {
            Ok((activity.to_string(), room.to_string()))
        })
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>();
    let (activity, room) = names;
    if activity.is_empty() {
        return;
    }
    wanted(Wanted {
        activity,
        id,
        room: (!room.is_empty()).then_some(room),
    });
}
