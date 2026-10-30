# AlwyChat — architecture

Status: draft v1 (2026-10-30). Decisions here are defaults; change them via a short ADR in `docs/adr/`.

## 1. Principles

1. **Rust owns all state and logic.** The UI renders and sends intents, nothing more.
2. **Stand on matrix-rust-sdk.** Never reimplement sync, crypto, timeline or room list (it's what Element X runs on).
3. **Spec first, `com.alwy.chat.*` second.** Use a stable Matrix feature or a widely deployed MSC when one exists. Custom events must degrade gracefully in Element/FluffyChat.
4. **Native only where the webview can't:** push, notification decryption, CallKit, photo library, camera, global shortcuts.
5. **Light by default:** lazy-load screens, virtualised lists, no heavy UI framework, no Electron.

## 2. Repository layout

```
alwychat/
├─ crates/
│  ├─ alwy-core/        # Matrix client logic: session, sync, rooms, timeline, media, RTC signalling
│  │  └─ src/events/    # com.alwy.chat.* event types (ruma macros) — single source of truth
│  └─ alwy-ffi/         # UniFFI bindings of a *small* core subset for iOS/Android notification extensions
├─ app/                 # Svelte 5 + TypeScript + Vite UI
│  ├─ src/lib/backend/  # typed wrappers over Tauri commands/channels (only place that calls invoke())
│  ├─ src/lib/stores/   # Svelte stores fed by core diffs
│  ├─ src/lib/features/ # one folder per feature (chat, albums, theme, voice, …)
│  └─ src-tauri/        # Tauri shell: command handlers (thin), plugin registration, platform config
├─ plugins/
│  └─ tauri-plugin-alwy-native/   # Swift (iOS) + Kotlin (Android) + Rust (desktop) native bits
├─ ios-extensions/
│  └─ NotificationService/        # iOS NSE: decrypts pushes via alwy-ffi
├─ dev/
│  └─ docker-compose.yml          # local Synapse + LiveKit for tests (never test against prod)
└─ docs/ (features.md, architecture.md, events.md, adr/)
```

## 3. Runtime data flow

```
Svelte component ──invoke("send_gallery", …)──▶ src-tauri command ──▶ alwy-core
       ▲                                                                │
       └── Svelte store ◀── tauri::ipc::Channel<Diff> ◀── stream of VectorDiff
```

- **Commands** are async request/response: send, edit, set theme, join call…
- **Subscriptions** (room list, timeline, typing, call members) are streams of *diffs*, not full snapshots. matrix-sdk-ui already emits `VectorDiff`s, so we forward them as they are. This keeps big rooms cheap.
- All DTOs crossing the bridge are generated to TypeScript (`specta` + `tauri-specta`), so the UI never hand-writes types.

## 4. alwy-core modules

| Module | Built on | Notes |
|---|---|---|
| `session` | `matrix_sdk::Client`, OAuth 2.0 (MAS) | Login via auth.example.org with system browser + deep link. Tokens in OS keychain. |
| `store` | `matrix-sdk-sqlite` | One encrypted SQLite store per account; passphrase in keychain/Keystore. |
| `sync` | `matrix_sdk_ui::SyncService` | Simplified sliding sync (enabled on example.org). |
| `rooms` | `RoomListService` | Filters, folders, pins, unread, spaces tree. |
| `timeline` | `matrix_sdk_ui::Timeline` | Plus our grouping layer: albums, burst grouping, effects. |
| `media` | `image`, `fast_image_resize`, `blurhash` | Parallel encrypted uploads, thumbnails, HD/compressed policy, save-to-gallery handoff. |
| `encryption` | `Encryption`, `Recovery` | Verification (emoji/QR), recovery key, key backup. |
| `custom` | `events/` | Theme, quick reaction, nicknames, countdowns, disappearing timers… |
| `rtc` | MatrixRTC events + OpenID | Call membership (+ delayed leave event), JWT from lk-jwt-service, call encryption keys via to-device. |
| `search` | `tantivy` (local) | Encrypted message search index. P3. |

## 5. Calls and voice channels

- **Signalling (Rust):** MatrixRTC membership state events (`org.matrix.msc3401.call.member`), a delayed "leave" event refreshed by heartbeat (MSC4140), and a LiveKit token from `https://rtc.example.org/sfu/get`. Compatible with Element Call / Element X.
- **Media (UI):** `livekit-client` (JS) in the webview: voice, video, screen share, speaking detection, per-user volume, E2EE via insertable streams.
- **Sidebar avatars:** derived from membership state, so they're visible **without joining**.
- **Voice channel room type:** `m.room.create` `type: "org.matrix.msc3417.call"` (same as Element "video rooms").
- **Platform specifics:**
  - iOS: CallKit + `AVAudioSession` background audio (native plugin).
  - Android: foreground service + full-screen intent for ringing; screen share via `MediaProjection` (native).
  - Linux: **risk.** WebKitGTK WebRTC/screen-share support is weak. Spike first; fallback = LiveKit Rust SDK inside alwy-core on desktop.

## 6. Notifications

| Platform | Transport | Content |
|---|---|---|
| iOS | APNs ← Sygnal (to deploy on example.org) | Notification Service Extension calls `alwy-ffi` to decrypt (24 MB memory limit → minimal store access). **Needs paid Apple account.** |
| Android | UnifiedPush (ntfy, self-hostable) or FCM | Decrypt in-process via the core. |
| Desktop | No push; app keeps syncing (tray) | libnotify / OS notifications. |

## 7. Native plugin surface (`tauri-plugin-alwy-native`)

| Capability | iOS (Swift) | Android (Kotlin) | Desktop (Rust) |
|---|---|---|---|
| Save to gallery | PhotoKit (add-only permission) | MediaStore | file dialog / ~/Pictures |
| Pick multiple media | PHPicker | Photo Picker | file dialog |
| Camera | AVFoundation | CameraX | — |
| Video compression | AVAssetExportSession | Media3 Transformer | ffmpeg (optional) |
| Push registration | APNs | UnifiedPush/FCM | — |
| Calls UI | CallKit | ConnectionService / notif | — |
| App lock | LocalAuthentication | BiometricPrompt | — |
| Global PTT shortcut | — | — | `global-hotkey` / XDG portal |

## 8. Custom events (summary — full schemas go in `docs/events.md`)

| Feature | Mechanism | Type |
|---|---|---|
| Albums | field on each `m.image`/`m.video` | `com.alwy.chat.gallery: {id, index, count}` |
| Conversation theme | room state | `com.alwy.chat.room.theme` |
| Quick reaction | room state | `com.alwy.chat.room.quick_reaction` |
| Nicknames for others | room state (map user → name) | `com.alwy.chat.room.nicknames` |
| Countdown / anniversary | room state | `com.alwy.chat.room.countdowns` |
| Disappearing messages | room state + sender schedules delayed redactions | `com.alwy.chat.room.disappearing` (≤24h on example.org) |
| View once | content flag; recipient redacts after view | `com.alwy.chat.view_once` |
| Per-chat media prefs (HD, auto-save) | room account data (private) | `com.alwy.chat.room.media_prefs` |
| Chat folders | account data | `com.alwy.chat.folders` |
| Drafts | room account data | `com.alwy.chat.draft` |
| Saved messages | account data → room id | `com.alwy.chat.saved_messages` |
| Status + emoji, timezone | extended profile fields | `com.alwy.chat.status`, `m.tz` |
| Chat name / avatar | spec | `m.room.name`, `m.room.avatar` |
| Voice messages | spec-ish (Element) | `org.matrix.msc3245.voice` + waveform |
| Emoji/sticker packs | MSC2545 (Cinny/FluffyChat) | `im.ponies.*` |
| Message effects | Element effect msgtypes | confetti, snow, fireworks, hearts… (verify list) |
| Soundboard | room state (sound list) + LiveKit data message to play | `com.alwy.chat.soundboard` |

Note: room **state is not end-to-end encrypted** today, so theme, nicknames and countdowns are visible to the homeserver (yours). Wallpaper images are uploaded unencrypted for the same reason.

## 9. Performance budget (to be enforced in CI later)

- Cold start to cached chat list: **< 1 s** on desktop, **< 1.5 s** on phones
- Idle RAM (desktop): **< 150 MB**
- Timeline scroll: 60 fps with 10 000 loaded events (virtualised list)
- Installer size: desktop < 20 MB, mobile < 40 MB

## 10. Tooling and conventions

- Rust stable, `cargo fmt`, `clippy -D warnings`. TypeScript strict, Biome for lint and format.
- Tests: core integration tests run against `dev/docker-compose.yml` (local Synapse + LiveKit). **Never against example.org.**
- Conventional commits; one ADR per significant decision.
- CI: Linux/Android builds on Forgejo Actions. iOS builds on a macOS machine (local or CI runner).

## 11. Milestones

1. **Spikes (risk first):** OAuth login with MAS in Tauri · WebKitGTK call + screen share on Linux · Tauri iOS build installed on the iPhone.
2. **MVP (P1):** login, E2EE + verification, chat list, timeline, replies/edits/reactions, albums, save to gallery, HD/compressed policy, Linux + Android + iOS.
3. **Personalisation and messaging (P2):** theme, quick reaction, nicknames, voice messages, polls, scheduled messages, folders…
4. **Servers and calls (P2):** spaces UI, voice channels with avatars, DM calls, screen share, PTT.
5. **P3 backlog.**
