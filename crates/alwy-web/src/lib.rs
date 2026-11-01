#![recursion_limit = "512"]
//! AlwyChat in a browser: the same commands as the Tauri app (`app/src-tauri/src/lib.rs`),
//! called by the web UI through `invoke(cmd, args)`. Commands that only pass their arguments
//! to the core are generated from the Tauri shell by `build.rs`; the ones that need the app
//! shell (sign-in, files, Android, desktop) get browser versions here.

use std::{fmt::Display, marker::PhantomData, sync::OnceLock};

use alwy_core::{Core, LoginRedirect};
use send_wrapper::SendWrapper;
use serde::{Serialize, de::DeserializeOwned};
use wasm_bindgen::prelude::*;

mod generated {
    #![allow(unused_imports, unused_braces, clippy::all, clippy::pedantic)]
    use super::{Args, Channel, Reply, fallible, infallible};
    include!(concat!(env!("OUT_DIR"), "/generated.rs"));
}

/// Handled below rather than generated (read by `build.rs`).
const HANDWRITTEN: &[&str] = &[
    "login",
    "report_activity",
    "app_lock_state",
    "unlock_biometric",
    "save_media",
    "file_action",
    "send_file",
    "take_share",
    "capture",
    "pick_files",
    "send_picked",
    "discard_picked",
    "system_status",
    "set_system_bars",
    "set_secure_screen",
    "setup_push",
    "test_push",
    "launch_room",
    "clear_notifications",
    "log_ui_error",
    // Web only:
    "media_filename",
];

type Reply = Result<JsValue, JsValue>;

/// One core per page. Its data lives in IndexedDB and localStorage under this name.
fn core() -> &'static Core {
    static CORE: OnceLock<SendWrapper<&'static Core>> = OnceLock::new();
    CORE.get_or_init(|| SendWrapper::new(Box::leak(Box::new(Core::new("alwychat")))))
}

/// The UI's event handler: `(name, payload)`, like Tauri's `listen`.
fn emit(name: &str, payload: impl Serialize) {
    EVENTS.with(|events| {
        if let Some(handler) = events.borrow().as_ref()
            && let Ok(payload) = to_js(&payload)
        {
            let _ = handler.call2(&JsValue::NULL, &JsValue::from_str(name), &payload);
        }
    });
}

thread_local! {
    static EVENTS: std::cell::RefCell<Option<js_sys::Function>> = const { std::cell::RefCell::new(None) };
}

fn to_js(value: &impl Serialize) -> Result<JsValue, JsValue> {
    value
        .serialize(&serde_wasm_bindgen::Serializer::json_compatible())
        .map_err(|e| JsValue::from_str(&e.to_string()))
}

fn error(e: impl Display) -> JsValue {
    JsValue::from_str(&e.to_string())
}

fn fallible<T: Serialize, E: Display>(result: Result<T, E>) -> Reply {
    to_js(&result.map_err(error)?)
}

fn infallible<T: Serialize>(value: T) -> Reply {
    to_js(&value)
}

/// A command's arguments: a JS object with camelCase keys (as Tauri's `invoke` takes them).
pub(crate) struct Args(JsValue);

impl Args {
    fn raw(&self, key: &str) -> JsValue {
        js_sys::Reflect::get(&self.0, &JsValue::from_str(key)).unwrap_or(JsValue::UNDEFINED)
    }

    pub(crate) fn get<T: DeserializeOwned>(&self, key: &str) -> Result<T, JsValue> {
        serde_wasm_bindgen::from_value(self.raw(key)).map_err(|e| error(format!("{key}: {e}")))
    }

    pub(crate) fn channel<T>(&self, key: &str) -> Result<Channel<T>, JsValue> {
        let target = self.raw(key);
        if target.is_undefined() {
            return Err(error(format!("{key}: missing channel")));
        }
        Ok(Channel {
            target: SendWrapper::new(target),
            kind: PhantomData,
        })
    }

    fn bytes(&self, key: &str) -> Vec<u8> {
        js_sys::Uint8Array::new(&self.raw(key)).to_vec()
    }
}

/// Stand-in for Tauri's `ipc::Channel`: calls the UI channel object's `onmessage`.
pub(crate) struct Channel<T> {
    target: SendWrapper<JsValue>,
    kind: PhantomData<fn(T)>,
}

impl<T> Clone for Channel<T> {
    fn clone(&self) -> Self {
        Self {
            target: SendWrapper::new((*self.target).clone()),
            kind: PhantomData,
        }
    }
}

impl<T: Serialize> Channel<T> {
    pub(crate) fn send(&self, message: T) -> Result<(), JsValue> {
        let handler = js_sys::Reflect::get(&self.target, &JsValue::from_str("onmessage"))?;
        if let Some(handler) = handler.dyn_ref::<js_sys::Function>() {
            handler.call1(&self.target, &to_js(&message)?)?;
        }
        Ok(())
    }
}

/// Logs to the browser console, and a readable message for panics.
#[wasm_bindgen(start)]
pub fn start() {
    std::panic::set_hook(Box::new(|info| {
        web_sys::console::error_1(&JsValue::from_str(&format!("AlwyChat crashed: {info}")));
    }));
}

/// Register the UI's event handler and connect the core's events to it.
#[wasm_bindgen(js_name = setEventHandler)]
pub fn set_event_handler(handler: js_sys::Function) {
    EVENTS.with(|events| *events.borrow_mut() = Some(handler));
    let core = core();
    core.on_session_ended(|| emit("session-ended", ()));
    core.on_call_signal(|signal| emit("call-signal", signal));
    core.on_draft(|change| emit("draft", change));
    core.on_notification(|n| emit("notification", n));
}

/// The sign-in popup came back to our page with this URL.
#[wasm_bindgen(js_name = finishLogin)]
pub fn finish_login(url: String) -> bool {
    url::Url::parse(&url).is_ok_and(|url| core().handle_login_redirect(url))
}

/// Media for the service worker (the browser's `media://`): `path` is what follows `/_media/`,
/// `<server>/<id>` (an avatar or picture, `size=` px) or `_event/<key>` (a message's media,
/// decrypted; `thumb=1` for a preview). Resolves to `{ bytes, mime }`.
#[wasm_bindgen]
pub async fn media(path: String, query: String) -> Reply {
    let param = |name: &str| {
        query
            .trim_start_matches('?')
            .split('&')
            .find_map(|kv| kv.strip_prefix(name)?.strip_prefix('='))
            .map(str::to_owned)
    };
    let core = core();
    let (bytes, mime) = match path.strip_prefix("_event/") {
        Some(key) => {
            let key = js_sys::decode_uri_component(key)
                .map(String::from)
                .unwrap_or_else(|_| key.to_owned());
            core.event_media(&key, param("thumb").as_deref() == Some("1"))
                .await
                .map_err(error)?
        }
        None => {
            let size = param("size").and_then(|v| v.parse().ok()).unwrap_or(96);
            let bytes = core
                .thumbnail(&format!("mxc://{path}"), size)
                .await
                .map_err(error)?;
            let mime = alwy_core::media::sniff(&bytes).to_owned();
            (bytes, mime)
        }
    };
    let out = js_sys::Object::new();
    js_sys::Reflect::set(
        &out,
        &"bytes".into(),
        &js_sys::Uint8Array::from(bytes.as_slice()),
    )?;
    js_sys::Reflect::set(&out, &"mime".into(), &JsValue::from_str(&mime))?;
    Ok(out.into())
}

/// Run a command, as Tauri's `invoke` does.
#[wasm_bindgen]
pub async fn invoke(cmd: String, args: JsValue) -> Reply {
    let args = Args(if args.is_undefined() || args.is_null() {
        js_sys::Object::new().into()
    } else {
        args
    });
    if let Some(reply) = handwritten(&cmd, &args).await {
        return reply;
    }
    if let Some(reply) = generated::generated(core(), &cmd, &args).await {
        return reply;
    }
    Err(error(format!("unknown command {cmd}")))
}

/// The commands the browser can run (to check the UI against).
#[wasm_bindgen]
pub fn commands() -> Vec<String> {
    HANDWRITTEN
        .iter()
        .chain(generated::GENERATED)
        .map(|s| (*s).to_owned())
        .collect()
}

#[derive(Serialize)]
struct AppLockState {
    enabled: bool,
    biometry: Option<&'static str>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SystemStatus {
    notifications: bool,
    battery_unrestricted: bool,
    background_data: bool,
}

async fn handwritten(cmd: &str, args: &Args) -> Option<Reply> {
    let core = core();
    Some(match cmd {
        "login" => {
            let server: String = match args.get("server") {
                Ok(s) => s,
                Err(e) => return Some(Err(e)),
            };
            let redirect: String = match args.get("redirect") {
                Ok(s) => s,
                Err(e) => return Some(Err(e)),
            };
            let Ok(redirect) = url::Url::parse(&redirect) else {
                return Some(Err(error("invalid redirect page")));
            };
            let result = core
                .login(&server, LoginRedirect::Web(redirect), |url| {
                    // The page opens it (a popup must come from the click): hand it over.
                    emit("login-url", url.as_str());
                    Ok(())
                })
                .await;
            fallible(result)
        }
        "report_activity" => match args.get::<bool>("active") {
            Ok(active) => fallible(core.report_activity(active).await),
            Err(e) => Err(e),
        },
        "app_lock_state" => infallible(AppLockState {
            enabled: core.app_lock_enabled(),
            biometry: None,
        }),
        "unlock_biometric" => infallible(false),
        "media_filename" => match args.get::<String>("media") {
            Ok(key) => fallible(core.media_filename(&key)),
            Err(e) => Err(e),
        },
        "send_file" => {
            let room_id: String = match args.get("roomId") {
                Ok(s) => s,
                Err(e) => return Some(Err(e)),
            };
            let file: alwy_core::upload::OutgoingFile = match args.get("file") {
                Ok(f) => f,
                Err(e) => return Some(Err(e)),
            };
            fallible(core.send_file(&room_id, file, args.bytes("bytes")).await)
        }
        "system_status" => infallible(SystemStatus {
            notifications: true,
            battery_unrestricted: true,
            background_data: true,
        }),
        "set_system_bars" | "set_secure_screen" | "clear_notifications" | "discard_picked" => {
            infallible(())
        }
        "setup_push" => infallible(false),
        "take_share" | "launch_room" => infallible(None::<()>),
        "log_ui_error" => {
            if let Ok(message) = args.get::<String>("message") {
                web_sys::console::error_1(&JsValue::from_str(&format!("ui: {message}")));
            }
            infallible(())
        }
        // Saving goes through a browser download, picking through a file input (in the UI).
        "save_media" | "file_action" | "capture" | "pick_files" | "send_picked" | "test_push" => {
            Err(error(format!("{cmd} isn't available in a browser")))
        }
        _ => return None,
    })
}
