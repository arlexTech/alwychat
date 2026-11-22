//! Screen sharing on Windows, done by the app instead of the webview's `getDisplayMedia`, so a
//! share can follow an app across its windows as Discord does: a game's launcher hides or
//! closes when the game starts, and comes back when it ends (League of Legends: the client
//! `LeagueClientUx.exe` and the game `Game\League of Legends.exe`). The browser picker ties a
//! share to one window and ends it when that window closes.
//!
//! - Pictures: Windows.Graphics.Capture (`windows-capture`), BGRA frames written into a
//!   WebView2 shared buffer (3 slots, no copy through IPC); a note per frame over a Channel says
//!   which slot. The page turns them into a `MediaStreamTrackGenerator` track that its LiveKit
//!   room publishes as the screen share (`stores/winShare.ts`).
//! - Sound: WASAPI process loopback: the shared app's process tree only, or everything except
//!   AlwyChat's own (the call's voices are never sent back). 48 kHz stereo f32, 10 ms pieces
//!   over a raw Channel → an audio track generator.
//! - Following: every 500 ms the shared window is checked; closed, hidden or minimised with
//!   another window of the same app visible (same program, or one in the folder above / below
//!   it), the share switches to that one, sound included.

use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use tauri::{
    AppHandle, Manager,
    ipc::{Channel, InvokeResponseBody},
};
use tracing::{info, warn};
use webview2_com::Microsoft::Web::WebView2::Win32::{
    COREWEBVIEW2_SHARED_BUFFER_ACCESS_READ_ONLY, ICoreWebView2_2, ICoreWebView2_17,
    ICoreWebView2Environment12, ICoreWebView2SharedBuffer,
};
use windows::{
    Win32::{
        Foundation::{CloseHandle, HWND, RECT},
        Graphics::Dwm::{DWMWA_CLOAKED, DwmGetWindowAttribute},
        System::Threading::{
            OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
            QueryFullProcessImageNameW,
        },
        UI::WindowsAndMessaging::{
            GetWindowRect, GetWindowThreadProcessId, IsIconic, IsWindow, IsWindowVisible,
        },
    },
    core::{Interface, PCWSTR, PWSTR},
};
use windows_capture::{
    capture::{CaptureControl, Context, GraphicsCaptureApiHandler},
    frame::Frame,
    graphics_capture_api::GraphicsCaptureApi,
    graphics_capture_api::InternalCaptureControl,
    monitor::Monitor,
    settings::{
        ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings,
        MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings,
    },
    window::Window,
};

/// Something that can be shared.
#[derive(Debug, Clone, Serialize)]
pub struct Source {
    /// "screen:<device name>" or "window:<HWND>".
    pub id: String,
    pub kind: &'static str,
    pub title: String,
    /// The program's name ("League of Legends"), for windows.
    pub app: String,
    pub width: u32,
    pub height: u32,
}

/// Which sound goes with the share.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Sound {
    /// Every app except AlwyChat.
    All,
    None,
    /// The shared window's app (and what it started).
    App,
}

/// What the page is told besides frames.
#[derive(Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Event {
    /// Now showing this (first, and after following the app to another window).
    Source {
        title: String,
        app: String,
    },
    /// The shared window went away and no other window of its app is there yet.
    Waiting,
    /// A new shared buffer (the page receives it through WebView2's `sharedbufferreceived`).
    Buffer {
        generation: u64,
        slot: u64,
    },
    Ended {
        reason: String,
    },
}

/// One frame is ready: in slot `slot` of buffer `generation`, `width`×`height` BGRA, rows
/// `stride` bytes apart.
#[derive(Clone, Serialize)]
pub struct FrameNote {
    generation: u64,
    slot: u32,
    width: u32,
    height: u32,
    stride: u32,
    seq: u64,
}

/// Settings changed while sharing.
struct Live {
    fps: AtomicU32,
    max_width: AtomicU32,
    max_height: AtomicU32,
    sound: Mutex<Sound>,
}

struct Session {
    stop: Arc<AtomicBool>,
    live: Arc<Live>,
    sound_changed: Arc<AtomicBool>,
}

static SESSION: Mutex<Option<Session>> = Mutex::new(None);

/// Monitors, then windows (the biggest first), never AlwyChat's own.
#[tauri::command]
pub fn win_screen_sources() -> Result<Vec<Source>, String> {
    let mut out = Vec::new();
    for m in Monitor::enumerate().map_err(|e| e.to_string())? {
        let (Ok(device), Ok(width), Ok(height)) = (m.device_name(), m.width(), m.height()) else {
            continue;
        };
        let title = m
            .name()
            .ok()
            .filter(|n| !n.trim().is_empty())
            .unwrap_or_else(|| device.trim_start_matches("\\\\.\\").to_owned());
        out.push(Source {
            id: format!("screen:{device}"),
            kind: "screen",
            title,
            app: String::new(),
            width,
            height,
        });
    }
    let me = std::process::id();
    let mut windows: Vec<Source> = Window::enumerate()
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter_map(|w| {
            let hwnd = HWND(w.as_raw_hwnd());
            if !shareable(hwnd) || w.process_id().ok()? == me {
                return None;
            }
            let title = w.title().ok()?.trim().to_owned();
            if title.is_empty() || title == "Program Manager" {
                return None;
            }
            let (width, height) = size_of(hwnd)?;
            let path = exe_path(w.process_id().ok()?).unwrap_or_default();
            Some(Source {
                id: format!("window:{}", hwnd.0 as isize),
                kind: "window",
                title,
                app: app_name(&path),
                width,
                height,
            })
        })
        .collect();
    windows.sort_by_key(|s| std::cmp::Reverse(u64::from(s.width) * u64::from(s.height)));
    out.extend(windows);
    Ok(out)
}

/// Start sharing `source` (from `win_screen_sources`). Frames come through `on_frame` (notes,
/// the pixels are in the shared buffer), sound through `on_audio` (f32 stereo 48 kHz), the rest
/// through `on_event`.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn win_screen_start(
    app: AppHandle,
    source: String,
    fps: u32,
    max_width: u32,
    max_height: u32,
    sound: Sound,
    on_frame: Channel<FrameNote>,
    on_audio: Channel<InvokeResponseBody>,
    on_event: Channel<serde_json::Value>,
) -> Result<(), String> {
    win_screen_stop();
    let target = Target::parse(&source).ok_or("unknown screen or window")?;
    let stop = Arc::new(AtomicBool::new(false));
    let live = Arc::new(Live {
        fps: AtomicU32::new(fps.clamp(1, 60)),
        max_width: AtomicU32::new(max_width.max(320)),
        max_height: AtomicU32::new(max_height.max(180)),
        sound: Mutex::new(sound),
    });
    let sound_changed = Arc::new(AtomicBool::new(false));
    *SESSION.lock().unwrap() = Some(Session {
        stop: stop.clone(),
        live: live.clone(),
        sound_changed: sound_changed.clone(),
    });
    let send_event = move |e: Event| {
        let _ = on_event.send(serde_json::to_value(e).unwrap_or_default());
    };
    thread::Builder::new()
        .name("win-share".into())
        .spawn(move || {
            supervise(
                app,
                target,
                stop,
                live,
                sound_changed,
                on_frame,
                on_audio,
                send_event,
            )
        })
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn win_screen_stop() {
    if let Some(s) = SESSION.lock().unwrap().take() {
        s.stop.store(true, Ordering::Relaxed);
    }
}

/// Quality while sharing (frame rate and largest size).
#[tauri::command]
pub fn win_screen_quality(fps: u32, max_width: u32, max_height: u32) {
    if let Some(s) = SESSION.lock().unwrap().as_ref() {
        s.live.fps.store(fps.clamp(1, 60), Ordering::Relaxed);
        s.live
            .max_width
            .store(max_width.max(320), Ordering::Relaxed);
        s.live
            .max_height
            .store(max_height.max(180), Ordering::Relaxed);
    }
}

#[tauri::command]
pub fn win_screen_sound(sound: Sound) {
    if let Some(s) = SESSION.lock().unwrap().as_ref() {
        *s.live.sound.lock().unwrap() = sound;
        s.sound_changed.store(true, Ordering::Relaxed);
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Target {
    Screen(usize),
    Window(isize),
}

impl Target {
    fn parse(id: &str) -> Option<Self> {
        if let Some(device) = id.strip_prefix("screen:") {
            let monitors = Monitor::enumerate().ok()?;
            let index = monitors
                .iter()
                .position(|m| m.device_name().is_ok_and(|d| d == device))?;
            return Some(Self::Screen(index));
        }
        id.strip_prefix("window:")?.parse().ok().map(Self::Window)
    }
}

/// Runs the share: captures the target, follows its app to another window, restarts the
/// sound when it must, until stopped.
#[allow(clippy::too_many_arguments)]
fn supervise(
    app: AppHandle,
    mut target: Target,
    stop: Arc<AtomicBool>,
    live: Arc<Live>,
    sound_changed: Arc<AtomicBool>,
    on_frame: Channel<FrameNote>,
    on_audio: Channel<InvokeResponseBody>,
    send_event: impl Fn(Event),
) {
    let frames = Arc::new(Frames {
        app: app.clone(),
        buffer: Mutex::new(None),
        generation: AtomicU64::new(0),
        seq: AtomicU64::new(0),
        on_frame,
        events: Mutex::new(Box::new(|_| {})),
    });
    let (buffer_tx, buffer_rx) = mpsc::channel::<Event>();
    *frames.events.lock().unwrap() = Box::new(move |e| {
        let _ = buffer_tx.send(e);
    });
    let mut capture: Option<CaptureControl<Handler, String>> = None;
    let mut audio: Option<Audio> = None;
    // The program of the shared window: what the share follows.
    let mut program: Option<String> = None;
    let mut waiting = false;
    let mut last_check = Instant::now() - Duration::from_secs(1);
    loop {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        while let Ok(e) = buffer_rx.try_recv() {
            send_event(e);
        }
        if capture.is_none() && !waiting {
            match start_capture(target, frames.clone(), live.clone()) {
                Ok(c) => {
                    let (title, app_name, pid) = describe(target);
                    if let Target::Window(_) = target {
                        program = pid.and_then(exe_path);
                    }
                    info!("screen share: {title} ({app_name})");
                    send_event(Event::Source {
                        title,
                        app: app_name,
                    });
                    capture = Some(c);
                    audio = start_audio(*live.sound.lock().unwrap(), pid, &on_audio);
                }
                Err(e) => {
                    warn!("screen share: {e}");
                    send_event(Event::Ended { reason: e });
                    break;
                }
            }
        }
        if sound_changed.swap(false, Ordering::Relaxed) {
            // The old recording ends before the new one starts.
            drop(audio.take());
            let pid = describe(target).2;
            audio = start_audio(*live.sound.lock().unwrap(), pid, &on_audio);
        }
        if last_check.elapsed() >= Duration::from_millis(500) {
            last_check = Instant::now();
            if let Target::Window(hwnd) = target {
                let hwnd = HWND(hwnd as *mut _);
                let gone = !unsafe { IsWindow(Some(hwnd)) }.as_bool();
                let away = gone || !shareable(hwnd) || unsafe { IsIconic(hwnd) }.as_bool();
                let finished = capture.as_ref().is_some_and(|c| c.is_finished());
                if away || finished {
                    match program.as_deref().and_then(|p| other_window(p, hwnd)) {
                        Some(next) => {
                            info!("screen share: following the app to another window");
                            if let Some(c) = capture.take() {
                                let _ = c.stop();
                            }
                            audio = None;
                            target = Target::Window(next.0 as isize);
                            waiting = false;
                            continue;
                        }
                        None if (gone || finished) && !waiting => {
                            send_event(Event::Waiting);
                            waiting = true;
                            if let Some(c) = capture.take() {
                                let _ = c.stop();
                            }
                        }
                        // Minimised with nowhere to go: stays (shows its last picture).
                        None => {}
                    }
                }
            } else if capture.as_ref().is_some_and(|c| c.is_finished()) {
                send_event(Event::Ended {
                    reason: "the screen stopped".into(),
                });
                break;
            }
        }
        thread::sleep(Duration::from_millis(50));
    }
    if let Some(c) = capture.take() {
        let _ = c.stop();
    }
    drop(audio);
    frames.close();
    info!("screen share stopped");
}

/// Title, program name and process of a target.
fn describe(target: Target) -> (String, String, Option<u32>) {
    match target {
        Target::Screen(i) => {
            let name = Monitor::enumerate()
                .ok()
                .and_then(|m| m.get(i).and_then(|m| m.name().ok()))
                .unwrap_or_default();
            (name, String::new(), None)
        }
        Target::Window(hwnd) => {
            let w = Window::from_raw_hwnd(hwnd as *mut _);
            let pid = w.process_id().ok();
            let app = pid
                .and_then(exe_path)
                .map(|p| app_name(&p))
                .unwrap_or_default();
            (w.title().unwrap_or_default(), app, pid)
        }
    }
}

fn start_capture(
    target: Target,
    frames: Arc<Frames>,
    live: Arc<Live>,
) -> Result<CaptureControl<Handler, String>, String> {
    // Windows' own pacing at 60 fps; the handler drops what the chosen rate doesn't need.
    // Older Windows (10, 11 before 24H2) can't set the interval or the cursor: ask for them only
    // where they exist (otherwise the capture refuses to start).
    let interval = if GraphicsCaptureApi::is_minimum_update_interval_supported().unwrap_or(false) {
        MinimumUpdateIntervalSettings::Custom(Duration::from_micros(16_000))
    } else {
        MinimumUpdateIntervalSettings::Default
    };
    let cursor = if GraphicsCaptureApi::is_cursor_settings_supported().unwrap_or(false) {
        CursorCaptureSettings::WithCursor
    } else {
        CursorCaptureSettings::Default
    };
    let flags = (frames, live);
    let result = match target {
        Target::Screen(i) => {
            let monitor = Monitor::enumerate()
                .map_err(|e| e.to_string())?
                .into_iter()
                .nth(i)
                .ok_or("that screen is gone")?;
            Handler::start_free_threaded(Settings::new(
                monitor,
                cursor,
                DrawBorderSettings::Default,
                SecondaryWindowSettings::Default,
                interval,
                DirtyRegionSettings::Default,
                ColorFormat::Bgra8,
                flags,
            ))
        }
        Target::Window(hwnd) => Handler::start_free_threaded(Settings::new(
            Window::from_raw_hwnd(hwnd as *mut _),
            cursor,
            DrawBorderSettings::Default,
            SecondaryWindowSettings::Default,
            interval,
            DirtyRegionSettings::Default,
            ColorFormat::Bgra8,
            flags,
        )),
    };
    result.map_err(|e| format!("couldn't capture it: {e}"))
}

struct Handler {
    frames: Arc<Frames>,
    live: Arc<Live>,
    last: Option<Instant>,
    scratch: Vec<u8>,
}

impl GraphicsCaptureApiHandler for Handler {
    type Flags = (Arc<Frames>, Arc<Live>);
    type Error = String;

    fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
        let (frames, live) = ctx.flags;
        Ok(Self {
            frames,
            live,
            last: None,
            scratch: Vec::new(),
        })
    }

    fn on_frame_arrived(
        &mut self,
        frame: &mut Frame,
        _control: InternalCaptureControl,
    ) -> Result<(), Self::Error> {
        // The chosen frame rate (a little early is fine: Windows' own timing jitters).
        let fps = self.live.fps.load(Ordering::Relaxed).max(1);
        let every = Duration::from_micros(1_000_000 / u64::from(fps));
        if self
            .last
            .is_some_and(|t| t.elapsed() + Duration::from_millis(3) < every)
        {
            return Ok(());
        }
        self.last = Some(Instant::now());
        let mut buffer = frame.buffer().map_err(|e| e.to_string())?;
        let (w, h, pitch) = (buffer.width(), buffer.height(), buffer.row_pitch());
        if w == 0 || h == 0 {
            return Ok(());
        }
        let raw = buffer.as_raw_buffer();
        // Twice the largest size or more (a 4K screen): halved here, so the page and the
        // encoder get a quarter of the pixels.
        let max_w = self.live.max_width.load(Ordering::Relaxed);
        let max_h = self.live.max_height.load(Ordering::Relaxed);
        if w >= max_w * 2 && h >= max_h * 2 {
            let (hw, hh) = (w / 2, h / 2);
            self.scratch.resize((hw * hh * 4) as usize, 0);
            halve(
                raw,
                pitch as usize,
                hw as usize,
                hh as usize,
                &mut self.scratch,
            );
            let scratch = std::mem::take(&mut self.scratch);
            self.frames.put(&scratch, hw, hh, hw * 4);
            self.scratch = scratch;
        } else {
            self.frames.put(raw, w, h, pitch);
        }
        Ok(())
    }
}

/// 2×2 box filter of BGRA rows (`pitch` bytes apart) into `out` (`w`×`h`, packed).
fn halve(src: &[u8], pitch: usize, w: usize, h: usize, out: &mut [u8]) {
    for y in 0..h {
        let r0 = &src[(2 * y) * pitch..];
        let r1 = &src[(2 * y + 1) * pitch..];
        let o = &mut out[y * w * 4..(y + 1) * w * 4];
        for x in 0..w {
            for c in 0..4 {
                let i = 8 * x + c;
                let sum = u16::from(r0[i])
                    + u16::from(r0[i + 4])
                    + u16::from(r1[i])
                    + u16::from(r1[i + 4]);
                o[4 * x + c] = ((sum + 2) / 4) as u8;
            }
        }
    }
}

/// The shared buffer the page reads frames from: 3 slots, so the one being written is never the
/// one the page reads (it copies a frame as soon as it's told).
struct Frames {
    app: AppHandle,
    buffer: Mutex<Option<SharedBuffer>>,
    generation: AtomicU64,
    seq: AtomicU64,
    on_frame: Channel<FrameNote>,
    events: Mutex<Box<dyn Fn(Event) + Send>>,
}

struct SharedBuffer {
    com: ICoreWebView2SharedBuffer,
    ptr: *mut u8,
    slot: usize,
    generation: u64,
}

// The memory is plain shared memory; the COM object is only closed on the main thread.
unsafe impl Send for SharedBuffer {}

const SLOTS: u64 = 3;

impl Frames {
    fn put(&self, data: &[u8], w: u32, h: u32, stride: u32) {
        let need = stride as usize * h as usize;
        let mut buffer = self.buffer.lock().unwrap();
        if buffer.as_ref().is_none_or(|b| b.slot < need) {
            // Room for a bit more, so a window being resized doesn't make a buffer per frame.
            let slot = (need + need / 4).max(1920 * 1080 * 4);
            match self.new_buffer(slot) {
                Ok(b) => {
                    if let Some(old) = buffer.replace(b) {
                        self.close_buffer(old);
                    }
                }
                Err(e) => {
                    warn!("screen share: no shared buffer: {e}");
                    return;
                }
            }
        }
        let Some(b) = buffer.as_ref() else { return };
        let seq = self.seq.fetch_add(1, Ordering::Relaxed);
        let slot = (seq % SLOTS) as u32;
        let len = need.min(data.len());
        unsafe {
            std::ptr::copy_nonoverlapping(data.as_ptr(), b.ptr.add(slot as usize * b.slot), len);
        }
        let _ = self.on_frame.send(FrameNote {
            generation: b.generation,
            slot,
            width: w,
            height: h,
            stride,
            seq,
        });
    }

    /// Makes a buffer of `SLOTS` slots and gives it to the page (on the main thread, as
    /// WebView2 wants; this capture thread waits for it).
    fn new_buffer(&self, slot: usize) -> Result<SharedBuffer, String> {
        let generation = self.generation.fetch_add(1, Ordering::Relaxed) + 1;
        let window = self
            .app
            .get_webview_window("main")
            .ok_or("no main window")?;
        let (tx, rx) = mpsc::channel();
        window
            .with_webview(move |webview| {
                let result = (|| -> windows::core::Result<SharedBuffer> {
                    let core = unsafe { webview.controller().CoreWebView2()? };
                    let env: ICoreWebView2Environment12 =
                        unsafe { core.cast::<ICoreWebView2_2>()?.Environment()? }.cast()?;
                    let com = unsafe { env.CreateSharedBuffer(slot as u64 * SLOTS)? };
                    let mut ptr = std::ptr::null_mut();
                    unsafe { com.Buffer(&mut ptr)? };
                    let json: Vec<u16> = format!("{{\"alwyScreen\":{generation},\"slot\":{slot}}}")
                        .encode_utf16()
                        .chain([0])
                        .collect();
                    unsafe {
                        core.cast::<ICoreWebView2_17>()?.PostSharedBufferToScript(
                            &com,
                            COREWEBVIEW2_SHARED_BUFFER_ACCESS_READ_ONLY,
                            PCWSTR(json.as_ptr()),
                        )?;
                    }
                    Ok(SharedBuffer {
                        com,
                        ptr,
                        slot,
                        generation,
                    })
                })();
                let _ = tx.send(result.map_err(|e| e.to_string()));
            })
            .map_err(|e| e.to_string())?;
        let buffer = rx
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| "the window didn't answer".to_owned())??;
        (self.events.lock().unwrap())(Event::Buffer {
            generation,
            slot: slot as u64,
        });
        Ok(buffer)
    }

    fn close_buffer(&self, old: SharedBuffer) {
        let _ = self.app.run_on_main_thread(move || {
            // The whole buffer moves here (only it is Send, not its COM field alone).
            let old = old;
            let _ = unsafe { old.com.Close() };
        });
    }

    fn close(&self) {
        if let Some(b) = self.buffer.lock().unwrap().take() {
            self.close_buffer(b);
        }
    }
}

/// The share's sound, captured until dropped.
struct Audio {
    stop: Arc<AtomicBool>,
}

impl Drop for Audio {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

fn start_audio(sound: Sound, pid: Option<u32>, out: &Channel<InvokeResponseBody>) -> Option<Audio> {
    // App sound of a whole screen: no app to pick, so everything but us.
    let (pid, include) = match (sound, pid) {
        (Sound::None, _) => return None,
        (Sound::App, Some(pid)) => (pid, true),
        _ => (std::process::id(), false),
    };
    let stop = Arc::new(AtomicBool::new(false));
    let out = out.clone();
    let flag = stop.clone();
    let _ = thread::Builder::new()
        .name("win-share-sound".into())
        .spawn(move || {
            if let Err(e) = record(pid, include, &flag, &out) {
                warn!("screen share sound: {e}");
            }
        });
    Some(Audio { stop })
}

/// Records one process tree's sound (or everything but it) into 10 ms pieces for the page.
fn record(
    pid: u32,
    include: bool,
    stop: &AtomicBool,
    out: &Channel<InvokeResponseBody>,
) -> Result<(), String> {
    use wasapi::{AudioClient, Direction, SampleType, StreamMode, WaveFormat};
    let _ = wasapi::initialize_mta();
    let format = WaveFormat::new(32, 32, SampleType::Float, 48_000, 2, None);
    let mut client =
        AudioClient::new_application_loopback_client(pid, include).map_err(|e| e.to_string())?;
    client
        .initialize_client(
            &format,
            Direction::Capture,
            StreamMode::EventsShared {
                autoconvert: true,
                buffer_duration_hns: 0,
            },
            None,
        )
        .map_err(|e| e.to_string())?;
    let event = client.set_get_eventhandle().map_err(|e| e.to_string())?;
    let capture = client.get_audiocaptureclient().map_err(|e| e.to_string())?;
    client.start_stream().map_err(|e| e.to_string())?;
    info!(pid, include, "screen share sound started");
    // 10 ms of 48 kHz stereo f32.
    const PIECE: usize = 480 * 2 * 4;
    let mut queue: VecDeque<u8> = VecDeque::new();
    while !stop.load(Ordering::Relaxed) {
        if capture
            .get_next_packet_size()
            .map_err(|e| e.to_string())?
            .unwrap_or(0)
            > 0
        {
            capture
                .read_from_device_to_deque(&mut queue)
                .map_err(|e| e.to_string())?;
        }
        while queue.len() >= PIECE {
            let piece: Vec<u8> = queue.drain(..PIECE).collect();
            let _ = out.send(InvokeResponseBody::Raw(piece));
        }
        // Silence gives no events: wake up anyway to notice `stop`.
        let _ = event.wait_for_event(200);
    }
    let _ = client.stop_stream();
    Ok(())
}

/// Visible, not cloaked (a hidden Store app or a window on another virtual desktop).
fn shareable(hwnd: HWND) -> bool {
    if !unsafe { IsWindowVisible(hwnd) }.as_bool() {
        return false;
    }
    let mut cloaked = 0u32;
    let ok = unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            (&mut cloaked as *mut u32).cast(),
            std::mem::size_of::<u32>() as u32,
        )
    };
    ok.is_err() || cloaked == 0
}

fn size_of(hwnd: HWND) -> Option<(u32, u32)> {
    let mut r = RECT::default();
    unsafe { GetWindowRect(hwnd, &mut r) }.ok()?;
    let (w, h) = (
        (r.right - r.left).max(0) as u32,
        (r.bottom - r.top).max(0) as u32,
    );
    (w >= 80 && h >= 60).then_some((w, h))
}

fn exe_path(pid: u32) -> Option<String> {
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut name = vec![0u16; 1024];
        let mut len = name.len() as u32;
        let ok = QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(name.as_mut_ptr()),
            &mut len,
        );
        let _ = CloseHandle(process);
        ok.ok()?;
        Some(String::from_utf16_lossy(&name[..len as usize]))
    }
}

/// "C:\…\League of Legends.exe" → "League of Legends".
fn app_name(path: &str) -> String {
    let file = path.rsplit(['\\', '/']).next().unwrap_or(path);
    file.strip_suffix(".exe")
        .or_else(|| file.strip_suffix(".EXE"))
        .unwrap_or(file)
        .to_owned()
}

/// Lower-case folder of a program's path.
fn folder(path: &str) -> Option<String> {
    let lower = path.to_lowercase().replace('/', "\\");
    lower.rsplit_once('\\').map(|(dir, _)| dir.to_owned())
}

/// Same app: the same program, or programs in the same folder, or one folder right inside the
/// other's (a launcher and its game: `…\League of Legends\LeagueClientUx.exe` and
/// `…\League of Legends\Game\League of Legends.exe`).
fn same_app(a: &str, b: &str) -> bool {
    if a.eq_ignore_ascii_case(b) {
        return true;
    }
    let (Some(da), Some(db)) = (folder(a), folder(b)) else {
        return false;
    };
    let parent = |d: &str| d.rsplit_once('\\').map(|(p, _)| p.to_owned());
    // Never "same folder" for shared places full of unrelated programs.
    let generic = |d: &str| {
        d.len() <= 3
            || d.ends_with("\\windows\\system32")
            || d.ends_with("\\windows")
            || d.ends_with("\\program files")
            || d.ends_with("\\program files (x86)")
    };
    if generic(&da) || generic(&db) {
        return false;
    }
    da == db || parent(&da).as_deref() == Some(&db) || parent(&db).as_deref() == Some(&da)
}

/// Another visible, not minimised window of the same app (the biggest).
fn other_window(program: &str, not: HWND) -> Option<HWND> {
    Window::enumerate()
        .ok()?
        .into_iter()
        .filter_map(|w| {
            let hwnd = HWND(w.as_raw_hwnd());
            if hwnd == not || !shareable(hwnd) || unsafe { IsIconic(hwnd) }.as_bool() {
                return None;
            }
            let mut pid = 0u32;
            unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
            let path = exe_path(pid)?;
            if !same_app(program, &path) || w.title().ok()?.trim().is_empty() {
                return None;
            }
            let (width, height) = size_of(hwnd)?;
            Some((hwnd, u64::from(width) * u64::from(height)))
        })
        .max_by_key(|(_, area)| *area)
        .map(|(hwnd, _)| hwnd)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launcher_and_game() {
        let client = r"C:\Riot Games\League of Legends\LeagueClientUx.exe";
        let game = r"C:\Riot Games\League of Legends\Game\League of Legends.exe";
        let riot = r"C:\Riot Games\Riot Client\RiotClientServices.exe";
        assert!(same_app(client, game));
        assert!(same_app(game, client));
        assert!(!same_app(client, riot));
        assert!(!same_app(
            r"C:\Windows\notepad.exe",
            r"C:\Windows\explorer.exe"
        ));
        assert!(!same_app(
            r"C:\Program Files\A\a.exe",
            r"C:\Program Files\B\b.exe"
        ));
        assert_eq!(app_name(game), "League of Legends");
    }

    #[test]
    fn halves() {
        // 4×2 → 2×1, one channel varying.
        let mut src = vec![0u8; 4 * 2 * 4];
        for (i, px) in src.chunks_mut(4).enumerate() {
            px[0] = (i * 10) as u8;
        }
        let mut out = vec![0u8; 2 * 4];
        halve(&src, 16, 2, 1, &mut out);
        assert_eq!(out[0], ((10 + 40 + 50 + 2) / 4) as u8);
        assert_eq!(out[4], ((20 + 30 + 60 + 70 + 2) / 4) as u8);
    }
}
