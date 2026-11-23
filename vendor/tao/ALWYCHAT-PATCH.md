# tao 0.37.2, patched for AlwyChat

A copy of the `tao` crate (Tauri's windowing library, Apache-2.0/MIT) used through
`[patch.crates-io]` in the workspace `Cargo.toml`, for Android chat bubbles. Changed file:
`src/platform_impl/android/mod.rs` (`Window::new`, marked "AlwyChat patch"):

- An activity name of the form `ClassName#<activity id>` attaches the new window to that
  already-running activity (Android starts a bubble's activity itself) instead of starting a new
  activity. Used by `app/src-tauri/src/bubbles.rs`.
- The activity a window lands in is marked as having one (`window_created`, which upstream never
  sets), so a later window without an activity name never shares it.

When updating Tauri: copy the new `tao` here and re-apply both changes (or drop this folder if
upstream can attach a window to a given activity).
- `src/lib.rs`: `#![allow(warnings)]` (a path dependency's warnings aren't silenced by Cargo).
