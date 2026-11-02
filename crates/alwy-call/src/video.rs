//! Video for native calls: remote cameras and screens are decoded by WebRTC and handed to the
//! UI as small JPEG frames (the webview draws them on a canvas); our screen (through the
//! desktop portal on Wayland) and camera (V4L2) are captured here and published.

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use futures_util::StreamExt;
use livekit::{
    options::TrackPublishOptions,
    prelude::*,
    webrtc::{
        desktop_capturer::{DesktopCaptureSourceType, DesktopCapturer, DesktopCapturerOptions},
        native::yuv_helper,
        prelude::{
            I420Buffer, RtcVideoSource, VideoBuffer, VideoFrame, VideoResolution, VideoRotation,
        },
        video_source::native::NativeVideoSource,
        video_stream::native::NativeVideoStream,
    },
};
use tracing::warn;

/// Frames for the UI: `(key, JPEG)`. The key names the tile's picture (a track SID, or
/// `local:camera` / `local:screen` for our own previews).
pub type FrameSink = Arc<dyn Fn(&str, Vec<u8>) + Send + Sync>;

pub const LOCAL_CAMERA: &str = "local:camera";
pub const LOCAL_SCREEN: &str = "local:screen";

/// Turns frames into JPEGs for the UI, at most `fps` a second and no larger than `max`.
struct Preview {
    key: String,
    sink: FrameSink,
    every: Duration,
    last: Option<Instant>,
    max: (u32, u32),
}

impl Preview {
    fn new(key: &str, sink: FrameSink, fps: u32, max: (u32, u32)) -> Self {
        Self {
            key: key.to_owned(),
            sink,
            every: Duration::from_millis(1000 / u64::from(fps.max(1))),
            last: None,
            max,
        }
    }

    fn offer(&mut self, buffer: &dyn VideoBuffer) {
        let now = Instant::now();
        if self.last.is_some_and(|last| now - last < self.every) {
            return;
        }
        self.last = Some(now);
        let (w, h) = (buffer.width(), buffer.height());
        if w == 0 || h == 0 {
            return;
        }
        let scale = f64::min(
            1.0,
            f64::min(self.max.0 as f64 / w as f64, self.max.1 as f64 / h as f64),
        );
        // Even sizes: the converters work on 2×2 chroma blocks.
        let tw = ((w as f64 * scale) as u32 & !1).max(2);
        let th = ((h as f64 * scale) as u32 & !1).max(2);
        let mut i420 = buffer.to_i420();
        if (tw, th) != (w, h) {
            i420 = i420.scale(tw as i32, th as i32);
        }
        let mut rgba = vec![0u8; (tw * th * 4) as usize];
        let (sy, su, sv) = i420.strides();
        let (y, u, v) = i420.data();
        // libyuv's "ABGR" is R, G, B, A in memory.
        yuv_helper::i420_to_abgr(y, sy, u, su, v, sv, &mut rgba, tw * 4, tw as i32, th as i32);
        let mut jpeg = Vec::with_capacity(rgba.len() / 10);
        let encoder = jpeg_encoder::Encoder::new(&mut jpeg, 85);
        if let Err(e) = encoder.encode(&rgba, tw as u16, th as u16, jpeg_encoder::ColorType::Rgba) {
            warn!("video preview: {e}");
            return;
        }
        (self.sink)(&self.key, jpeg);
    }
}

/// Follow a remote camera or screen and send it to the UI until aborted.
pub(crate) fn watch_remote(
    track: RemoteVideoTrack,
    screen: bool,
    sink: FrameSink,
) -> tokio::task::JoinHandle<()> {
    let key = track.sid().to_string();
    tokio::spawn(async move {
        let mut stream = NativeVideoStream::new(track.rtc_track());
        let mut preview = if screen {
            Preview::new(&key, sink, 12, (1920, 1080))
        } else {
            Preview::new(&key, sink, 24, (1280, 720))
        };
        while let Some(frame) = stream.next().await {
            preview.offer(frame.buffer.as_ref());
        }
    })
}

fn now_us() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_micros() as i64)
        .unwrap_or_default()
}

fn send(source: &NativeVideoSource, buffer: I420Buffer) -> I420Buffer {
    let frame = VideoFrame {
        rotation: VideoRotation::VideoRotation0,
        timestamp_us: now_us(),
        frame_metadata: None,
        buffer,
    };
    source.capture_frame(&frame);
    frame.buffer
}

/// Something we capture and publish (screen or camera) until stopped.
pub(crate) struct Capture {
    pub track: LocalVideoTrack,
    stop: Arc<AtomicBool>,
}

impl Capture {
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        self.stop();
    }
}

async fn publish(
    room: &Room,
    name: &str,
    source: TrackSource,
    video: NativeVideoSource,
) -> Result<LocalVideoTrack, String> {
    // Enough for a sharp picture (LiveKit's defaults are cautious); screens favour detail.
    let encoding = if source == TrackSource::Screenshare {
        livekit::options::VideoEncoding {
            max_bitrate: 3_000_000,
            max_framerate: 15.0,
        }
    } else {
        livekit::options::VideoEncoding {
            max_bitrate: 1_700_000,
            max_framerate: 30.0,
        }
    };
    let track = LocalVideoTrack::create_video_track(name, RtcVideoSource::Native(video));
    room.local_participant()
        .publish_track(
            LocalTrack::Video(track.clone()),
            TrackPublishOptions {
                source,
                video_encoding: Some(encoding),
                ..Default::default()
            },
        )
        .await
        .map_err(|e| format!("couldn't publish the {name}: {e}"))?;
    Ok(track)
}

/// Share the screen: the desktop portal asks which screen or window (Wayland), then frames
/// are captured ~15 times a second. `on_end` runs if the capture stops by itself (refused,
/// window closed).
pub(crate) async fn share_screen(
    room: &Room,
    sink: FrameSink,
    on_end: impl Fn() + Send + 'static,
) -> Result<Capture, String> {
    let video = NativeVideoSource::new(
        VideoResolution {
            width: 1920,
            height: 1080,
        },
        true,
    );
    let stop = Arc::new(AtomicBool::new(false));
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel::<Result<(), String>>();
    {
        let video = video.clone();
        let stop = stop.clone();
        thread::Builder::new()
            .name("screen-capture".into())
            .spawn(move || {
                let mut options = DesktopCapturerOptions::new(DesktopCaptureSourceType::Generic);
                options.set_include_cursor(true);
                let Some(mut capturer) = DesktopCapturer::new(options) else {
                    let _ = ready_tx.send(Err("screen capture isn't available here".into()));
                    return;
                };
                let _ = ready_tx.send(Ok(()));
                let mut preview = Preview::new(LOCAL_SCREEN, sink, 5, (960, 540));
                let failures = Arc::new(std::sync::atomic::AtomicU32::new(0));
                let seen_frame = Arc::new(AtomicBool::new(false));
                {
                    let failures = failures.clone();
                    let seen_frame = seen_frame.clone();
                    capturer.start_capture(None, move |result| match result {
                        Ok(frame) => {
                            failures.store(0, Ordering::Relaxed);
                            seen_frame.store(true, Ordering::Relaxed);
                            let (w, h) = (frame.width(), frame.height());
                            if w <= 0 || h <= 0 {
                                return;
                            }
                            let mut buffer = I420Buffer::new(w as u32, h as u32);
                            let (sy, su, sv) = buffer.strides();
                            let (y, u, v) = buffer.data_mut();
                            // Desktop frames are BGRA in memory: libyuv's "ARGB".
                            yuv_helper::argb_to_i420(
                                frame.data(),
                                frame.stride(),
                                y,
                                sy,
                                u,
                                su,
                                v,
                                sv,
                                w,
                                h,
                            );
                            let buffer = send(&video, buffer);
                            preview.offer(&buffer);
                        }
                        Err(_) => {
                            failures.fetch_add(1, Ordering::Relaxed);
                        }
                    });
                }
                // The portal dialog may stay open a while: only give up on errors after a
                // frame came, or after a minute without any.
                let started = Instant::now();
                while !stop.load(Ordering::Relaxed) {
                    capturer.capture_frame();
                    let failing = failures.load(Ordering::Relaxed);
                    let seen = seen_frame.load(Ordering::Relaxed);
                    if (seen && failing > 45)
                        || (!seen && started.elapsed() > Duration::from_secs(60))
                    {
                        warn!("screen capture stopped (failures: {failing}, frames: {seen})");
                        on_end();
                        break;
                    }
                    thread::sleep(Duration::from_millis(66));
                }
            })
            .map_err(|e| format!("screen capture: {e}"))?;
    }
    ready_rx
        .await
        .map_err(|_| "screen capture stopped".to_owned())??;
    let track = match publish(room, "screen", TrackSource::Screenshare, video).await {
        Ok(track) => track,
        Err(e) => {
            stop.store(true, Ordering::Relaxed);
            return Err(e);
        }
    };
    Ok(Capture { track, stop })
}

/// Turn the camera on (V4L2 device `index`): 640×480 YUYV (every webcam offers it, at full
/// frame rate over USB 2).
pub(crate) async fn start_camera(
    room: &Room,
    index: usize,
    sink: FrameSink,
) -> Result<Capture, String> {
    const W: u32 = 640;
    const H: u32 = 480;
    let video = NativeVideoSource::new(
        VideoResolution {
            width: W,
            height: H,
        },
        false,
    );
    let stop = Arc::new(AtomicBool::new(false));
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel::<Result<(u32, u32), String>>();
    {
        let video = video.clone();
        let stop = stop.clone();
        thread::Builder::new()
            .name("camera".into())
            .spawn(move || {
                camera_loop(
                    index,
                    (W, H),
                    Some(&video),
                    &stop,
                    sink,
                    LOCAL_CAMERA,
                    ready_tx,
                )
            })
            .map_err(|e| format!("camera: {e}"))?;
    }
    ready_rx
        .await
        .map_err(|_| "the camera stopped".to_owned())??;
    match publish(room, "camera", TrackSource::Camera, video).await {
        Ok(track) => Ok(Capture { track, stop }),
        Err(e) => {
            stop.store(true, Ordering::Relaxed);
            Err(e)
        }
    }
}

fn camera_loop(
    index: usize,
    (width, height): (u32, u32),
    video: Option<&NativeVideoSource>,
    stop: &AtomicBool,
    sink: FrameSink,
    preview_key: &str,
    ready: tokio::sync::oneshot::Sender<Result<(u32, u32), String>>,
) {
    use v4l::{
        FourCC, buffer::Type, io::mmap::Stream, io::traits::CaptureStream, video::Capture as _,
    };
    let opened = (|| -> Result<(v4l::Device, u32, u32), String> {
        let device = v4l::Device::new(index).map_err(|_| "no camera found".to_owned())?;
        let mut format = device.format().map_err(|e| format!("camera: {e}"))?;
        format.width = width;
        format.height = height;
        format.fourcc = FourCC::new(b"YUYV");
        let format = device
            .set_format(&format)
            .map_err(|e| format!("camera: {e}"))?;
        if format.fourcc != FourCC::new(b"YUYV") {
            return Err(format!("unsupported camera format {}", format.fourcc));
        }
        Ok((device, format.width, format.height))
    })();
    let (device, w, h) = match opened {
        Ok(opened) => opened,
        Err(e) => {
            let _ = ready.send(Err(e));
            return;
        }
    };
    let mut stream = match Stream::with_buffers(&device, Type::VideoCapture, 4) {
        Ok(stream) => stream,
        Err(e) => {
            let _ = ready.send(Err(format!("camera: {e}")));
            return;
        }
    };
    let _ = ready.send(Ok((w, h)));
    let mut preview = Preview::new(preview_key, sink, 24, (640, 480));
    while !stop.load(Ordering::Relaxed) {
        let Ok((data, _meta)) = stream.next() else {
            warn!("camera frame lost");
            thread::sleep(Duration::from_millis(30));
            continue;
        };
        let buffer = yuyv_to_i420(data, w, h);
        let buffer = match video {
            Some(video) => send(video, buffer),
            None => buffer,
        };
        preview.offer(&buffer);
    }
}

/// YUYV (4:2:2, Y0 U Y1 V) → I420: chroma from even rows.
fn yuyv_to_i420(data: &[u8], width: u32, height: u32) -> I420Buffer {
    let mut buffer = I420Buffer::new(width, height);
    let (sy, su, sv) = buffer.strides();
    let (y, u, v) = buffer.data_mut();
    let row = (width * 2) as usize;
    for r in 0..height as usize {
        let start = (r * row).min(data.len());
        let line = &data[start..(start + row).min(data.len())];
        for c in 0..width as usize {
            y[r * sy as usize + c] = line.get(c * 2).copied().unwrap_or(16);
        }
        if r % 2 == 0 {
            for c in 0..(width as usize / 2) {
                u[(r / 2) * su as usize + c] = line.get(c * 4 + 1).copied().unwrap_or(128);
                v[(r / 2) * sv as usize + c] = line.get(c * 4 + 3).copied().unwrap_or(128);
            }
        }
    }
    buffer
}

/// Frames key of the settings' camera preview.
pub const CAMERA_PREVIEW: &str = "local:preview";

/// The camera shown in Settings without publishing it anywhere (same capture as calls).
pub struct CameraPreview {
    stop: Arc<AtomicBool>,
}

impl Drop for CameraPreview {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

impl CameraPreview {
    pub async fn start(index: usize, sink: FrameSink) -> Result<Self, String> {
        let stop = Arc::new(AtomicBool::new(false));
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
        {
            let stop = stop.clone();
            thread::Builder::new()
                .name("camera-preview".into())
                .spawn(move || {
                    camera_loop(
                        index,
                        (640, 480),
                        None,
                        &stop,
                        sink,
                        CAMERA_PREVIEW,
                        ready_tx,
                    )
                })
                .map_err(|e| format!("camera: {e}"))?;
        }
        ready_rx
            .await
            .map_err(|_| "the camera stopped".to_owned())??;
        Ok(Self { stop })
    }
}
