# webrtc-sys 0.3.52, patched for AlwyChat

A copy of the `webrtc-sys` crate (LiveKit, Apache-2.0) with one addition, used through
`[patch.crates-io]` in the workspace `Cargo.toml`:

- `audio_track::ffi::set_audio_track_volume(id, volume)`: the playback volume of one remote audio
  track (libwebrtc's `AudioSourceInterface::SetVolume`), for per-person volume in native calls
  (`crates/alwy-call`). Changed files: `src/audio_track.cpp`, `src/audio_track.rs`,
  `include/livekit/audio_track.h`.

When updating LiveKit: copy the new `webrtc-sys` here and re-apply these three small changes (or
drop this folder if upstream exposes a per-track volume).
