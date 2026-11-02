//! Native calls for desktops whose webview has no WebRTC (WebKitGTK on Linux): the LiveKit
//! room runs here, with the microphone and speakers through WebRTC's own audio device module
//! (PulseAudio/PipeWire, with echo cancellation), and the UI gets snapshots of who is in the
//! call. The Matrix side (membership, LiveKit token) stays in alwy-core, as for browser calls.
//!
//! Video (cameras, screens) goes to the UI as JPEG frames (`video`). No per-person volume yet:
//! the platform audio module mixes remote tracks itself; "mute for me" disables their tracks.

mod video;

pub use video::{FrameSink, LOCAL_CAMERA, LOCAL_SCREEN};

use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
};

use livekit::{PlatformAudio, options::TrackPublishOptions, prelude::*};
use serde::Serialize;
use tokio::{sync::mpsc::UnboundedReceiver, task::JoinHandle};
use tracing::warn;

/// Someone in the call, as the UI shows them.
#[derive(Debug, Clone, Serialize)]
pub struct CallMember {
    /// LiveKit identity: `@user:server:DEVICE`.
    pub identity: String,
    pub is_local: bool,
    pub speaking: bool,
    pub mic_on: bool,
    /// Key of their camera's frames (see `FrameSink`), if it's on.
    pub camera: Option<String>,
    /// Key of their shared screen's frames, if any.
    pub screen: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CallEvent {
    /// Who is in the call now (us first).
    Members {
        members: Vec<CallMember>,
    },
    Reconnecting,
    Connected,
    /// Our screen share stopped by itself (portal refused, window closed).
    ScreenEnded,
    /// The call ended without us leaving (kicked, server gone…).
    Ended {
        reason: String,
    },
}

type Sink = Arc<dyn Fn(CallEvent) + Send + Sync>;

/// A microphone, speaker or camera the user can pick.
#[derive(Debug, Clone, Serialize)]
pub struct Device {
    /// Stable ID to save (audio: the platform's GUID; cameras: the V4L2 index).
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Devices {
    pub inputs: Vec<Device>,
    pub outputs: Vec<Device>,
    pub cameras: Vec<Device>,
}

/// The microphones, speakers and cameras of this machine.
pub fn devices() -> Result<Devices, String> {
    let audio = PlatformAudio::new().map_err(|e| format!("audio devices: {e}"))?;
    let inputs = audio
        .recording_devices()
        .map(|d| Device {
            id: d.id.as_str().to_owned(),
            name: d.name,
        })
        .collect();
    let outputs = audio
        .playout_devices()
        .map(|d| Device {
            id: d.id.as_str().to_owned(),
            name: d.name,
        })
        .collect();
    let mut cameras: Vec<Device> = v4l::context::enum_devices()
        .into_iter()
        // Each camera shows up twice (video + metadata node): keep the capture one.
        .filter(|node| {
            v4l::Device::new(node.index())
                .and_then(|d| d.query_caps())
                .is_ok_and(|caps| {
                    caps.capabilities
                        .contains(v4l::capability::Flags::VIDEO_CAPTURE)
                })
        })
        .map(|node| Device {
            id: node.index().to_string(),
            name: node
                .name()
                .unwrap_or_else(|| format!("Camera {}", node.index())),
        })
        .collect();
    cameras.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(Devices {
        inputs,
        outputs,
        cameras,
    })
}

/// Which devices and audio processing to use (empty IDs: the system default).
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct AudioSetup {
    pub input: Option<String>,
    pub output: Option<String>,
    pub camera: Option<String>,
    pub noise_suppression: bool,
    pub echo_cancellation: bool,
    pub auto_gain: bool,
}

fn apply_setup(audio: &PlatformAudio, setup: &AudioSetup) {
    if let Some(id) = setup.input.as_deref().filter(|id| !id.is_empty()) {
        let found = audio.recording_devices().find(|d| d.id.as_str() == id);
        if let Some(device) = found
            && let Err(e) = audio.switch_recording_device(&device.id)
        {
            warn!("microphone: {e}");
        }
    }
    if let Some(id) = setup.output.as_deref().filter(|id| !id.is_empty()) {
        let found = audio.playout_devices().find(|d| d.id.as_str() == id);
        if let Some(device) = found
            && let Err(e) = audio.switch_playout_device(&device.id)
        {
            warn!("speaker: {e}");
        }
    }
    if let Err(e) = audio.configure_audio_processing(livekit::AudioProcessingOptions {
        echo_cancellation: setup.echo_cancellation,
        noise_suppression: setup.noise_suppression,
        auto_gain_control: setup.auto_gain,
        prefer_hardware_processing: false,
    }) {
        warn!("audio processing: {e}");
    }
}

struct Shared {
    deafened: bool,
    /// User IDs muted for us only.
    muted_for_me: HashSet<String>,
    /// Remote videos sent to the UI, by track SID.
    watching: HashMap<String, tokio::task::JoinHandle<()>>,
    camera: Option<video::Capture>,
    screen: Option<video::Capture>,
    setup: AudioSetup,
}

/// A call in progress. Dropping it stops following events; `leave` disconnects cleanly.
pub struct NativeCall {
    room: Arc<Room>,
    audio: PlatformAudio,
    mic: LocalAudioTrack,
    shared: Arc<Mutex<Shared>>,
    events: JoinHandle<()>,
    frames: FrameSink,
    sink: Sink,
}

impl NativeCall {
    /// Join the LiveKit room at `url` with `token` (from the homeserver's focus, as the browser
    /// does) and publish the microphone.
    pub async fn join(
        url: &str,
        token: &str,
        mic_on: bool,
        setup: AudioSetup,
        on_event: impl Fn(CallEvent) + Send + Sync + 'static,
        on_frame: impl Fn(&str, Vec<u8>) + Send + Sync + 'static,
    ) -> Result<Self, String> {
        // LiveKit's signalling uses rustls, which can't pick a crypto provider on its own
        // when the build has two (it panicked inside the connection task).
        let _ = rustls::crypto::ring::default_provider().install_default();
        let audio = PlatformAudio::new().map_err(|e| format!("audio devices: {e}"))?;
        apply_setup(&audio, &setup);
        let (room, rx) = Room::connect(url, token, RoomOptions::default())
            .await
            .map_err(|e| format!("couldn't join the call: {e}"))?;
        let room = Arc::new(room);

        let mic = LocalAudioTrack::create_audio_track("microphone", audio.rtc_source());
        if !mic_on {
            mic.mute();
        }
        room.local_participant()
            .publish_track(
                LocalTrack::Audio(mic.clone()),
                TrackPublishOptions {
                    source: TrackSource::Microphone,
                    ..Default::default()
                },
            )
            .await
            .map_err(|e| format!("couldn't publish the microphone: {e}"))?;

        let shared = Arc::new(Mutex::new(Shared {
            deafened: false,
            muted_for_me: HashSet::new(),
            watching: HashMap::new(),
            camera: None,
            screen: None,
            setup,
        }));
        let sink: Sink = Arc::new(on_event);
        let frames: FrameSink = Arc::new(on_frame);
        // Videos already there when we arrive.
        for participant in room.remote_participants().values() {
            for publication in participant.track_publications().values() {
                if let Some(RemoteTrack::Video(track)) = publication.track() {
                    watch(&shared, track, publication, &frames);
                }
            }
        }
        sink(CallEvent::Connected);
        sink(CallEvent::Members {
            members: members(&room, &shared),
        });
        let events = tokio::spawn(follow(
            room.clone(),
            rx,
            shared.clone(),
            sink.clone(),
            frames.clone(),
        ));
        Ok(Self {
            room,
            audio,
            mic,
            shared,
            events,
            frames,
            sink,
        })
    }

    pub fn set_mic(&self, on: bool) {
        if on {
            self.mic.unmute();
        } else {
            self.mic.mute();
        }
    }

    /// Hear nobody (the mic is the UI's business, as in the browser).
    pub fn set_deafened(&self, deafened: bool) {
        self.shared.lock().unwrap().deafened = deafened;
        apply_audio(&self.room, &self.shared);
    }

    /// Silence one person (all their devices) for us only.
    pub fn set_muted_for_me(&self, user_id: &str, muted: bool) {
        {
            let mut shared = self.shared.lock().unwrap();
            if muted {
                shared.muted_for_me.insert(user_id.to_owned());
            } else {
                shared.muted_for_me.remove(user_id);
            }
        }
        apply_audio(&self.room, &self.shared);
    }

    /// New devices or audio processing, applied live.
    pub fn set_setup(&self, setup: AudioSetup) {
        apply_setup(&self.audio, &setup);
        self.shared.lock().unwrap().setup = setup;
    }

    /// Camera on or off (the first V4L2 camera).
    pub async fn set_camera(&self, on: bool) -> Result<(), String> {
        let old = self.shared.lock().unwrap().camera.take();
        if let Some(old) = old {
            old.stop();
            let _ = self
                .room
                .local_participant()
                .unpublish_track(&old.track.sid())
                .await;
        }
        if on {
            let index = self
                .shared
                .lock()
                .unwrap()
                .setup
                .camera
                .as_deref()
                .and_then(|id| id.parse().ok())
                .unwrap_or(0);
            let capture = video::start_camera(&self.room, index, self.frames.clone()).await?;
            self.shared.lock().unwrap().camera = Some(capture);
        }
        self.refresh();
        Ok(())
    }

    /// Share the screen (the desktop asks which one) or stop sharing.
    pub async fn set_screen(&self, on: bool) -> Result<(), String> {
        let old = self.shared.lock().unwrap().screen.take();
        if let Some(old) = old {
            old.stop();
            let _ = self
                .room
                .local_participant()
                .unpublish_track(&old.track.sid())
                .await;
        }
        if on {
            let sink = self.sink.clone();
            let capture = video::share_screen(&self.room, self.frames.clone(), move || {
                sink(CallEvent::ScreenEnded);
            })
            .await?;
            self.shared.lock().unwrap().screen = Some(capture);
        }
        self.refresh();
        Ok(())
    }

    fn refresh(&self) {
        (self.sink)(CallEvent::Members {
            members: members(&self.room, &self.shared),
        });
    }

    pub async fn leave(&self) {
        self.events.abort();
        {
            let mut shared = self.shared.lock().unwrap();
            shared.camera = None;
            shared.screen = None;
            for (_, task) in shared.watching.drain() {
                task.abort();
            }
        }
        if let Err(e) = self.room.close().await {
            warn!("leaving the call: {e}");
        }
    }
}

impl Drop for NativeCall {
    fn drop(&mut self) {
        self.events.abort();
        if let Ok(mut shared) = self.shared.lock() {
            shared.camera = None;
            shared.screen = None;
            for (_, task) in shared.watching.drain() {
                task.abort();
            }
        }
    }
}

/// Send a remote camera or screen to the UI, in the best quality the sender offers (the
/// server otherwise picks a low simulcast layer for a subscriber that doesn't say).
fn watch(
    shared: &Mutex<Shared>,
    track: RemoteVideoTrack,
    publication: &RemoteTrackPublication,
    frames: &FrameSink,
) {
    if publication.simulcasted() {
        publication.set_video_quality(livekit::track::VideoQuality::High);
    }
    let source = publication.source();
    let sid = track.sid().to_string();
    let mut shared = shared.lock().unwrap();
    if shared.watching.contains_key(&sid) {
        return;
    }
    let task = video::watch_remote(track, source == TrackSource::Screenshare, frames.clone());
    shared.watching.insert(sid, task);
}

/// "@alex:example.org:DEVICE" → "@alex:example.org".
fn user_of(identity: &str) -> &str {
    match (identity.find(':'), identity.rfind(':')) {
        (Some(first), Some(last)) if last > first => &identity[..last],
        _ => identity,
    }
}

fn has_live_mic(mut publications: impl Iterator<Item = (TrackSource, bool)>) -> bool {
    publications.any(|(source, muted)| source == TrackSource::Microphone && !muted)
}

/// Key of a participant's live video from `source` that we're showing.
fn video_key(
    publications: &HashMap<TrackSid, RemoteTrackPublication>,
    source: TrackSource,
    watching: &HashMap<String, tokio::task::JoinHandle<()>>,
) -> Option<String> {
    publications
        .values()
        .filter(|p| p.source() == source && !p.is_muted())
        .map(|p| p.sid().to_string())
        .find(|sid| watching.contains_key(sid))
}

fn members(room: &Room, shared: &Mutex<Shared>) -> Vec<CallMember> {
    let shared = shared.lock().unwrap();
    let me = room.local_participant();
    let mut out = vec![CallMember {
        identity: me.identity().to_string(),
        is_local: true,
        speaking: me.is_speaking(),
        mic_on: has_live_mic(
            me.track_publications()
                .values()
                .map(|p| (p.source(), p.is_muted())),
        ),
        camera: shared.camera.as_ref().map(|_| LOCAL_CAMERA.to_owned()),
        screen: shared.screen.as_ref().map(|_| LOCAL_SCREEN.to_owned()),
    }];
    let mut others: Vec<_> = room
        .remote_participants()
        .into_values()
        .map(|p| {
            let publications = p.track_publications();
            CallMember {
                identity: p.identity().to_string(),
                is_local: false,
                speaking: p.is_speaking(),
                mic_on: has_live_mic(publications.values().map(|p| (p.source(), p.is_muted()))),
                camera: video_key(&publications, TrackSource::Camera, &shared.watching),
                screen: video_key(&publications, TrackSource::Screenshare, &shared.watching),
            }
        })
        .collect();
    others.sort_by(|a, b| a.identity.cmp(&b.identity));
    out.extend(others);
    out
}

/// Enable or disable every remote audio track for deafen / "mute for me".
fn apply_audio(room: &Room, shared: &Mutex<Shared>) {
    let shared = shared.lock().unwrap();
    for participant in room.remote_participants().values() {
        let silent = shared.deafened
            || shared
                .muted_for_me
                .contains(user_of(participant.identity().as_str()));
        for publication in participant.track_publications().values() {
            if let Some(RemoteTrack::Audio(track)) = publication.track() {
                if silent {
                    track.disable();
                } else {
                    track.enable();
                }
            }
        }
    }
}

async fn follow(
    room: Arc<Room>,
    mut rx: UnboundedReceiver<RoomEvent>,
    shared: Arc<Mutex<Shared>>,
    sink: Sink,
    frames: FrameSink,
) {
    let snapshot = |room: &Room| CallEvent::Members {
        members: members(room, &shared),
    };
    while let Some(event) = rx.recv().await {
        match event {
            RoomEvent::Disconnected { reason } => {
                sink(CallEvent::Ended {
                    reason: format!("{reason:?}"),
                });
                return;
            }
            RoomEvent::Reconnecting => sink(CallEvent::Reconnecting),
            RoomEvent::Reconnected => sink(CallEvent::Connected),
            // A new audio track follows deafen / "mute for me" right away.
            RoomEvent::TrackSubscribed {
                track, publication, ..
            } => {
                if let RemoteTrack::Video(video) = track {
                    watch(&shared, video, &publication, &frames);
                }
                apply_audio(&room, &shared);
                sink(snapshot(&room));
            }
            RoomEvent::TrackUnsubscribed { publication, .. } => {
                if let Some(task) = shared
                    .lock()
                    .unwrap()
                    .watching
                    .remove(&publication.sid().to_string())
                {
                    task.abort();
                }
                sink(snapshot(&room));
            }
            RoomEvent::ParticipantConnected(_)
            | RoomEvent::ParticipantDisconnected(_)
            | RoomEvent::TrackPublished { .. }
            | RoomEvent::TrackUnpublished { .. }
            | RoomEvent::TrackMuted { .. }
            | RoomEvent::TrackUnmuted { .. }
            | RoomEvent::LocalTrackPublished { .. }
            | RoomEvent::ActiveSpeakersChanged { .. } => sink(snapshot(&room)),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::user_of;

    #[test]
    fn identity_to_user() {
        assert_eq!(user_of("@alex:example.org:ABCDEF"), "@alex:example.org");
        assert_eq!(user_of("@alex:example.org"), "@alex:example.org");
        assert_eq!(user_of("nobody"), "nobody");
    }
}
