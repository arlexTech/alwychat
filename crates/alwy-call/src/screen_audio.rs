//! The sound of a screen share (Linux): what other apps play, never our own call audio.
//!
//! Through PulseAudio's API (pipewire-pulse on current desktops), each app's playback stream
//! ("sink input") gets its own recording stream ("monitor stream"), so the call's playback —
//! the others' voices, which they'd hear back — is left out, as Discord does. New apps are picked
//! up as they start playing. The streams are mixed every 10 ms into a 48 kHz stereo track
//! published as the screen share's audio.

use std::{
    cell::RefCell,
    collections::{HashMap, VecDeque},
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

use libpulse_binding as pulse;
use livekit::{
    options::{AudioEncoding, TrackPublishOptions},
    prelude::*,
    webrtc::{
        audio_frame::AudioFrame,
        audio_source::{AudioSourceOptions, RtcAudioSource, native::NativeAudioSource},
    },
};
use pulse::{
    callbacks::ListResult,
    context::{
        Context, FlagSet as ContextFlags, State as ContextState,
        subscribe::{Facility, InterestMaskSet, Operation},
    },
    mainloop::standard::{IterateResult, Mainloop},
    sample::{Format, Spec},
    stream::{FlagSet as StreamFlags, PeekResult, Stream},
};
use tracing::{info, warn};

const RATE: u32 = 48_000;
const CHANNELS: u32 = 2;
/// Samples (both channels) in 10 ms.
const FRAME: usize = (RATE / 100 * CHANNELS) as usize;
/// At most this much waits per app (0.3 s): beyond, the oldest is dropped (clock drift).
const MAX_BUFFERED: usize = FRAME * 30;

/// Each app's samples waiting to be mixed, by sink input index.
type Buffers = Arc<Mutex<HashMap<u32, VecDeque<f32>>>>;
/// The apps being recorded, by sink input index.
type Apps = Arc<Mutex<HashMap<u32, ScreenApp>>>;

/// An app playing sound that the share can carry.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ScreenApp {
    /// Stable key: the program's file name (else its name), the same for all its streams.
    pub id: String,
    pub name: String,
}

/// Which sound goes with the shared screen.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", content = "app", rename_all = "snake_case")]
pub enum SoundChoice {
    /// Every app but AlwyChat (the call's voices are never sent back).
    All,
    None,
    /// Only this app (`ScreenApp::id`).
    App(String),
}

pub(crate) struct ScreenAudio {
    pub track: LocalAudioTrack,
    stop: Arc<AtomicBool>,
    mixer: tokio::task::JoinHandle<()>,
    apps: Apps,
    choice: Arc<Mutex<SoundChoice>>,
}

impl ScreenAudio {
    /// The apps playing now (one entry per app), by name.
    pub fn apps(&self) -> Vec<ScreenApp> {
        let mut apps: Vec<ScreenApp> = self.apps.lock().unwrap().values().cloned().collect();
        apps.sort_by_cached_key(|a| (a.name.to_lowercase(), a.id.clone()));
        apps.dedup_by(|a, b| a.id == b.id);
        apps
    }

    pub fn choice(&self) -> SoundChoice {
        self.choice.lock().unwrap().clone()
    }

    pub fn set_choice(&self, choice: SoundChoice) {
        info!("screen sound: {choice:?}");
        *self.choice.lock().unwrap() = choice;
    }

    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
        self.mixer.abort();
    }
}

impl Drop for ScreenAudio {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Starts capturing and publishes the track. Fails when there's no PulseAudio-compatible server.
pub(crate) async fn start(room: &Room, choice: SoundChoice) -> Result<ScreenAudio, String> {
    let buffers: Buffers = Arc::default();
    let apps: Apps = Arc::default();
    let choice = Arc::new(Mutex::new(choice));
    let stop = Arc::new(AtomicBool::new(false));
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel::<Result<(), String>>();
    {
        let buffers = buffers.clone();
        let apps = apps.clone();
        let stop = stop.clone();
        thread::Builder::new()
            .name("screen-audio".into())
            .spawn(move || capture(buffers, apps, stop, ready_tx))
            .map_err(|e| format!("screen sound: {e}"))?;
    }
    ready_rx
        .await
        .map_err(|_| "screen sound stopped".to_owned())??;

    let source = NativeAudioSource::new(
        // Game and video sound: none of the voice processing (it would eat music and effects).
        AudioSourceOptions {
            echo_cancellation: false,
            noise_suppression: false,
            auto_gain_control: false,
        },
        RATE,
        CHANNELS,
        100,
    );
    let track =
        LocalAudioTrack::create_audio_track("screen-audio", RtcAudioSource::Native(source.clone()));
    if let Err(e) = room
        .local_participant()
        .publish_track(
            LocalTrack::Audio(track.clone()),
            TrackPublishOptions {
                source: TrackSource::ScreenshareAudio,
                dtx: false,
                audio_encoding: Some(AudioEncoding {
                    max_bitrate: 128_000,
                }),
                ..Default::default()
            },
        )
        .await
    {
        stop.store(true, Ordering::Relaxed);
        return Err(format!("couldn't publish the screen sound: {e}"));
    }

    // 10 ms frames; the source's queue paces them in real time.
    let mixer = {
        let apps = apps.clone();
        let choice = choice.clone();
        tokio::spawn(async move {
            let mut mixed = vec![0f32; FRAME];
            loop {
                mixed.iter_mut().for_each(|s| *s = 0.0);
                {
                    let choice = choice.lock().unwrap().clone();
                    let apps = apps.lock().unwrap();
                    let mut buffers = buffers.lock().unwrap();
                    for (index, buffer) in buffers.iter_mut() {
                        let n = buffer.len().min(FRAME);
                        // Apps left out are still read (and dropped), so switching to one later
                        // doesn't play its last 0.3 s late.
                        let wanted = match &choice {
                            SoundChoice::All => true,
                            SoundChoice::None => false,
                            SoundChoice::App(id) => apps.get(index).is_some_and(|a| &a.id == id),
                        };
                        let samples = buffer.drain(..n);
                        if wanted {
                            for (out, s) in mixed.iter_mut().zip(samples) {
                                *out += s;
                            }
                        }
                    }
                }
                let data: Vec<i16> = mixed
                    .iter()
                    .map(|s| (s.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16)
                    .collect();
                let frame = AudioFrame {
                    data: data.into(),
                    sample_rate: RATE,
                    num_channels: CHANNELS,
                    samples_per_channel: FRAME as u32 / CHANNELS,
                };
                if let Err(e) = source.capture_frame(&frame).await {
                    warn!("screen sound: {e}");
                    return;
                }
            }
        })
    };
    Ok(ScreenAudio {
        track,
        stop,
        mixer,
        apps,
        choice,
    })
}

/// Is this process us, or started by us (the web view's processes play sounds too)?
fn is_ours(pid: u32) -> bool {
    let me = std::process::id();
    let mut pid = pid;
    for _ in 0..6 {
        if pid == me {
            return true;
        }
        let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) else {
            return false;
        };
        // "pid (name) state ppid …": the name may hold spaces, so after the last ')'.
        let Some(parent) = stat
            .rsplit_once(')')
            .and_then(|(_, rest)| rest.split_whitespace().nth(1))
            .and_then(|p| p.parse::<u32>().ok())
        else {
            return false;
        };
        if parent <= 1 {
            return false;
        }
        pid = parent;
    }
    false
}

/// The PulseAudio side, on its own thread (its objects aren't `Send`).
fn capture(
    buffers: Buffers,
    apps: Apps,
    stop: Arc<AtomicBool>,
    ready: tokio::sync::oneshot::Sender<Result<(), String>>,
) {
    let Some(mainloop) = Mainloop::new() else {
        let _ = ready.send(Err("no sound server".into()));
        return;
    };
    let mainloop = Rc::new(RefCell::new(mainloop));
    let Some(context) = Context::new(&*mainloop.borrow(), "AlwyChat screen sound") else {
        let _ = ready.send(Err("no sound server".into()));
        return;
    };
    let context = Rc::new(RefCell::new(context));
    if let Err(e) = context
        .borrow_mut()
        .connect(None, ContextFlags::NOFLAGS, None)
    {
        let _ = ready.send(Err(format!("sound server: {e}")));
        return;
    }
    // Wait for the connection.
    loop {
        if let IterateResult::Quit(_) | IterateResult::Err(_) = mainloop.borrow_mut().iterate(true)
        {
            let _ = ready.send(Err("sound server closed".into()));
            return;
        }
        match context.borrow().get_state() {
            ContextState::Ready => break,
            ContextState::Failed | ContextState::Terminated => {
                let _ = ready.send(Err("couldn't reach the sound server".into()));
                return;
            }
            _ => {}
        }
    }
    let _ = ready.send(Ok(()));

    // Recording streams by sink input index (each read callback holds its stream too: the
    // callback is cleared when the stream goes, which frees it).
    let streams: Streams = Rc::default();
    // Sink inputs already being followed (or looked up): the list of apps playing at the start
    // and the "new app" event can name the same one at once, and following it twice recorded it
    // twice into one buffer (the first recording then ran on, unclosed).
    let known: Rc<RefCell<std::collections::HashSet<u32>>> = Rc::default();

    // Follows one app's playback: records just that stream from its sink's monitor.
    let follow: Rc<dyn Fn(u32)> = {
        let context = context.clone();
        let streams = streams.clone();
        let buffers = buffers.clone();
        let apps = apps.clone();
        let known = known.clone();
        Rc::new(move |index: u32| {
            if !known.borrow_mut().insert(index) {
                return;
            }
            let ctx = context.clone();
            let streams = streams.clone();
            let buffers = buffers.clone();
            let apps = apps.clone();
            context
                .borrow()
                .introspect()
                .get_sink_input_info(index, move |result| {
                    let ListResult::Item(input) = result else {
                        return;
                    };
                    if streams.borrow().contains_key(&input.index) {
                        return;
                    }
                    let pid = input
                        .proplist
                        .get_str("application.process.id")
                        .and_then(|p| p.parse::<u32>().ok());
                    if pid.is_some_and(is_ours) {
                        return;
                    }
                    let name = input
                        .proplist
                        .get_str("application.name")
                        .unwrap_or_else(|| "?".into());
                    let id = input
                        .proplist
                        .get_str("application.process.binary")
                        .unwrap_or_else(|| name.clone())
                        .to_lowercase();
                    apps.lock().unwrap().insert(
                        input.index,
                        ScreenApp {
                            id,
                            name: name.clone(),
                        },
                    );
                    let (input_index, sink) = (input.index, input.sink);
                    let ctx2 = ctx.clone();
                    let streams = streams.clone();
                    let buffers = buffers.clone();
                    ctx.borrow()
                        .introspect()
                        .get_sink_info_by_index(sink, move |result| {
                            let ListResult::Item(sink) = result else {
                                return;
                            };
                            let Some(monitor) = sink.monitor_source_name.as_deref() else {
                                return;
                            };
                            let spec = Spec {
                                format: Format::F32le,
                                channels: CHANNELS as u8,
                                rate: RATE,
                            };
                            let Some(mut stream) = Stream::new(
                                &mut ctx2.borrow_mut(),
                                "AlwyChat screen share",
                                &spec,
                                None,
                            ) else {
                                return;
                            };
                            if stream.set_monitor_stream(input_index).is_err() {
                                return;
                            }
                            // Small fragments: 20 ms, so the sound stays close to the picture.
                            let attr = pulse::def::BufferAttr {
                                maxlength: u32::MAX,
                                tlength: u32::MAX,
                                prebuf: u32::MAX,
                                minreq: u32::MAX,
                                fragsize: (RATE / 50 * CHANNELS * 4),
                            };
                            if let Err(e) = stream.connect_record(
                                Some(monitor),
                                Some(&attr),
                                StreamFlags::ADJUST_LATENCY | StreamFlags::DONT_MOVE,
                            ) {
                                warn!("screen sound: couldn't follow {name}: {e}");
                                return;
                            }
                            buffers.lock().unwrap().insert(input_index, VecDeque::new());
                            let stream = Rc::new(RefCell::new(stream));
                            let reader = stream.clone();
                            let buffers = buffers.clone();
                            stream
                                .borrow_mut()
                                .set_read_callback(Some(Box::new(move |_| {
                                    let Ok(mut s) = reader.try_borrow_mut() else {
                                        return;
                                    };
                                    while let Ok(peeked) = s.peek() {
                                        match peeked {
                                            PeekResult::Empty => break,
                                            PeekResult::Hole(_) => {}
                                            PeekResult::Data(bytes) => {
                                                let mut all = buffers.lock().unwrap();
                                                let Some(buffer) = all.get_mut(&input_index) else {
                                                    break;
                                                };
                                                buffer.extend(bytes.chunks_exact(4).map(|b| {
                                                    f32::from_le_bytes([b[0], b[1], b[2], b[3]])
                                                }));
                                                let extra =
                                                    buffer.len().saturating_sub(MAX_BUFFERED);
                                                buffer.drain(..extra);
                                            }
                                        }
                                        let _ = s.discard();
                                    }
                                })));
                            info!("screen sound: following {name}");
                            streams.borrow_mut().insert(input_index, stream);
                        });
                });
        })
    };

    // Apps already playing, then whatever starts or stops.
    {
        let follow = follow.clone();
        context
            .borrow()
            .introspect()
            .get_sink_input_info_list(move |result| {
                if let ListResult::Item(input) = result {
                    follow(input.index);
                }
            });
    }
    {
        let follow = follow.clone();
        let streams = streams.clone();
        let buffers = buffers.clone();
        let apps = apps.clone();
        let known = known.clone();
        context.borrow_mut().set_subscribe_callback(Some(Box::new(
            move |facility, operation, index| {
                if facility != Some(Facility::SinkInput) {
                    return;
                }
                match operation {
                    Some(Operation::New) => follow(index),
                    Some(Operation::Removed) => {
                        known.borrow_mut().remove(&index);
                        if let Some(s) = streams.borrow_mut().remove(&index) {
                            close(&s);
                        }
                        buffers.lock().unwrap().remove(&index);
                        apps.lock().unwrap().remove(&index);
                    }
                    _ => {}
                }
            },
        )));
        context
            .borrow_mut()
            .subscribe(InterestMaskSet::SINK_INPUT, |_| {});
    }

    while !stop.load(Ordering::Relaxed) {
        match mainloop.borrow_mut().iterate(false) {
            IterateResult::Quit(_) | IterateResult::Err(_) => break,
            IterateResult::Success(0) => thread::sleep(Duration::from_millis(5)),
            IterateResult::Success(_) => {}
        }
    }
    for (_, s) in streams.borrow_mut().drain() {
        close(&s);
    }
    context.borrow_mut().disconnect();
    info!("screen sound stopped");
}

type Streams = Rc<RefCell<HashMap<u32, Rc<RefCell<Stream>>>>>;

fn close(stream: &Rc<RefCell<Stream>>) {
    let mut s = stream.borrow_mut();
    s.set_read_callback(None);
    let _ = s.disconnect();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Plays a quiet sound with `paplay` and checks it's captured (needs a sound server; run by
    /// hand: `cargo test -p alwy-call screen_audio -- --ignored`).
    #[test]
    #[ignore]
    fn captures_other_apps() {
        let buffers: Buffers = Arc::default();
        let stop = Arc::new(AtomicBool::new(false));
        let (tx, mut rx) = tokio::sync::oneshot::channel();
        let apps: Apps = Arc::default();
        let t = {
            let (b, a, s) = (buffers.clone(), apps.clone(), stop.clone());
            thread::spawn(move || capture(b, a, s, tx))
        };
        thread::sleep(Duration::from_millis(300));
        assert!(
            matches!(rx.try_recv(), Ok(Ok(()))),
            "sound server not reached"
        );
        let file = std::env::var("SOUND").expect("SOUND=<wav file>");
        // Started through a shell that exits at once: a child of ours would count as our own
        // sound (`is_ours`) and be left out.
        let out = std::process::Command::new("sh")
            .args([
                "-c",
                "paplay --volume=12000 \"$0\" >/dev/null 2>&1 & echo $!",
                &file,
            ])
            .output()
            .unwrap();
        let player = String::from_utf8_lossy(&out.stdout).trim().to_owned();
        let mut peak = 0f32;
        let mut seen = 0usize;
        for _ in 0..20 {
            thread::sleep(Duration::from_millis(100));
            let mut all = buffers.lock().unwrap();
            for b in all.values_mut() {
                seen += b.len();
                peak = b.drain(..).fold(peak, |p, s| p.max(s.abs()));
            }
        }
        let _ = std::process::Command::new("kill").arg(&player).status();
        stop.store(true, Ordering::Relaxed);
        t.join().unwrap();
        println!(
            "captured {seen} samples, peak {peak}, apps {:?}",
            apps.lock().unwrap()
        );
        assert!(seen > 48_000 && peak > 0.0001);
    }
}
