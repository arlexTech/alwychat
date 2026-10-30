# AlwyChat — feature catalog

Legend: `[x]` confirmed · `[ ]` proposed (tick or delete) · priority **P1** MVP / **P2** next / **P3** later
Compat: 🟢 Matrix spec (works with Element/FluffyChat) · 🟡 custom `com.alwy.chat.*` (degrades gracefully) · 🔵 client-only

## 1. Media
- [x] P1 🔵 Save photo/video to gallery (long-press → Save, "Save all" on albums)
- [x] P1 🟡 Multi-photo albums (grid bubble, swipe viewer; also auto-groups bursts sent by other apps)
- [x] P1 🔵 Choose HD original vs compressed when sending (not asking everytime, chose it in conversation settings with a general default setting in the app set to compressed)
- [x] P2 🔵 Photo editor before sending (crop, rotate, draw, text, blur faces) (propose it but don't make every photo sent go with this process)
- [x] P2 🔵 In-app camera (photo + video, quick capture)
- [x] P2 🔵 Per-chat media/files/links browser ("Shared media")
- [x] P2 🔵 Auto-save received media to gallery (per chat toggle)
- [x] P2 🟢 Voice messages with waveform + playback speed 1×/1.5×/2× (Element-compatible)
- [ ] P3 🟡 Round video notes (Telegram style) (telle me more about that)
- [x] P3 🟢 View-once photos (MSC-less: client hides + redacts after view; not secure against other clients)

## 2. Messaging
- [x] P1 🟢 Replies, edits (+ edit history), delete, reactions, mentions, markdown, spoilers
- [x] P1 🔵 Swipe to reply · double-tap to send the chat's quick reaction
- [x] P1 🟢 Read receipts, typing indicators (with privacy toggles to hide yours)
- [x] P2 🟢 Threads
- [x] P2 🟢 Pinned messages
- [x] P2 🟢 Polls
- [x] P2 🟢 Scheduled messages (delayed events — enabled on example.org, max 24h)
- [x] P2 🟢 Custom emoji & sticker packs (MSC2545 — FluffyChat/Cinny compatible)
- [ ] P2 🔵 GIF search (needs a provider key; privacy trade-off)
- [x] P2 🔵 Forward / multi-select / copy
- [x] P2 🔵 "Saved messages" (personal notes room, Telegram style)
- [x] P2 🟢 Link previews (needs `url_preview_enabled` on Synapse — currently off)
- [x] P2 🔵 Drafts synced between devices (account data)
- [x] P3 🟢 Location + live location sharing
- [x] P3 🟡 Disappearing messages (per-chat timer; uses delayed redactions — to validate)
- [ ] P3 🔵 Message translation
- [x] P3 🔵 Encrypted message search (local index)

## 3. Conversation personalisation (Messenger-style)
- [x] P2 🟡 Shared conversation theme (colors, bubbles, wallpaper) — synced via room state
- [x] P2 🟡 Custom quick-reaction emoji per chat (shared)
- [x] P2 🟢 Per-room nicknames (native per-room display names; setting someone *else's* nickname = 🟡 shared map)
- [x] P2 🟢 Message effects: confetti, hearts, fireworks, snow (Element-compatible event types)
- [ ] P3 🟡 Word effects (e.g. typing "je t'aime" rains hearts — configurable per chat)
- [x] P3 🟡 Chat emoji/avatar for 1:1 chat, shared custom chat name
- [x] P3 🟡 Shared countdown / anniversary / "days together" widget

## 4. Chat list & organisation
- [x] P1 🔵 Pin, mute, archive, mark unread
- [x] P2 🔵 Chat folders/filters (Telegram style, synced via account data)
- [x] P2 🟢 Custom status + emoji (extended profiles — supported on example.org)
- [x] P3 🟢 Pronouns / timezone / "local time" shown on profile (not pronouns)

## 5. Servers (Spaces) — Discord-style
- [x] P2 🟢 Voice channels with live avatars in the sidebar (MatrixRTC, Element Call compatible)
- [x] P2 🟢 Voice, video, screen sharing (with audio on desktop)
- [x] P2 🔵 Speaking rings, mute/deafen icons, per-user volume
- [x] P2 🔵 Push-to-talk (global shortcut on desktop) + noise suppression
- [x] P2 🔵 Text chat inside voice channels
- [x] P2 🔵 Channel categories, unread badges per server, server icons/banners
- [x] P3 🟢 Roles & permissions UI (power levels)
- [x] P3 🔵 Picture-in-picture / pop-out call window
- [x] P3 🟡 Soundboard (what do you mean)
- [ ] P3 🟡 Stage channels (speakers/listeners)  (what do you mean)
- [ ] P3 🟡 Forum channels (thread-first)  (what do you mean)
- [ ] P3 🟡 Server events with RSVP  (what do you mean)

## 6. Calls in DMs
- [x] P2 🟢 1:1 and group voice/video calls (MatrixRTC — same stack as voice channels)
- [x] P2 🔵 Ringing UI + ringtone (Android full-screen intent)
- [x] P3 🔵 Call history

## 7. Security & privacy
- [x] P1 🟢 E2EE, device verification (emoji/QR), cross-signing, key backup + recovery key
- [x] P1 🟢 OIDC login via MAS (auth.example.org)
- [x] P2 🔵 App lock (PIN / biometrics), hide content in app switcher (but not enable by default, fully optionnal and not even proposed, just in the settings)
- [ ] P3 🔵 Multi-account

## 8. Platform
- [x] P1 🔵 Linux desktop (tray, notifications, Wayland)
- [x] P1 🔵 iOS native app (Tauri 2 iOS + Swift plugins: APNs push, notification decryption, CallKit, PhotoKit)
- [X] P3 🔵 Web version (would need the Rust core compiled to WebAssembly)
- [x] P2 🔵 Android (UnifiedPush or FCM notifications)
- [x] P2 🔵 App-wide light / dark / AMOLED themes
- [x] P3 🔵 Windows / macOS builds
