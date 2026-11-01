# AlwyChat

A light, fast and private chat app for [Matrix](https://matrix.org), with the best ideas of Messenger, WhatsApp, Telegram and Discord.
Your messages are end-to-end encrypted, and you can talk with people who use Element, Element X or FluffyChat.

> **Status: beta.** AlwyChat is used every day by its author, but it's young: expect rough edges.
> There is no published release yet. Until the first one, you can [build it yourself](#build-it-yourself).

---

## Which file do I download?

Go to the **[Releases page](https://github.com/arlexTech/alwychat/releases)**, open the newest release, and pick the file whose name **ends with**:

| Your device | File ending in | Status |
|---|---|---|
| 📱 **Android** phone or tablet (Android 7 or newer) | **`.apk`** | First release coming soon |
| 🐧 **Linux**, any distribution | **`.AppImage`** | First release coming soon |
| 🐧 **Linux**, Debian / Ubuntu / Mint | **`.deb`** | First release coming soon |
| 🐧 **Linux**, Fedora / openSUSE | **`.rpm`** | First release coming soon |
| 🍎 **iPhone / iPad** | | Planned (App Store) |
| 🪟 **Windows** · 🍏 **macOS** | | Planned |
| 🌐 **Web browser** | | Planned ([see below](#web-version)) |

Not sure which Linux file? Take the **AppImage**: it works everywhere.

### Installing

**Android**
1. Download the `.apk` file on your phone.
2. Open it. If Android asks, allow your browser or file manager to "install unknown apps".
3. Open AlwyChat and sign in.

**Linux, AppImage**
```sh
chmod +x AlwyChat_*.AppImage
./AlwyChat_*.AppImage
```

**Linux, .deb / .rpm**: double-click the file, or:
```sh
sudo apt install ./AlwyChat_*.deb     # Debian, Ubuntu, Mint
sudo dnf install ./AlwyChat-*.rpm     # Fedora
```

---

## What you get

- **Messaging**: replies, reactions, edits, threads, polls, pinned messages, mentions, voice messages, drafts synced between your devices, sharing from other apps (Android).
- **Photos and files**: albums, HD or compressed per chat, camera, everything saved in one tap, a media/files/links gallery per chat.
- **Make it yours**: Messenger-style chat themes with illustrated backgrounds (or your own photo), quick reaction, nicknames, message effects, light / dark / black themes.
- **Find anything**: fast search in all your chats, encrypted ones included, and a timeline to jump to any day of a conversation.
- **Organise**: pin, mute, archive, folders (Telegram style), spaces with Discord-style voice channels.
- **Calls**: voice and video calls, and voice channels (MatrixRTC, the same technology as Element Call).
- **Location**: send your position, or share it live until you stop.
- **Privacy**: end-to-end encryption, device verification, optional app lock (PIN / fingerprint), status and local time only if you want to share them.

The full list, with what's done and what's planned: [docs/features.md](docs/features.md).

---

## Which server can I use?

AlwyChat works with Matrix servers that have the modern sign-in system:

- ✅ **[matrix.org](https://matrix.org)** and other servers using the **Matrix Authentication Service** (sign-in through your browser).
- ❌ Servers that only offer the old password sign-in aren't supported yet.

### Running your own server

For every feature to work, your homeserver needs:

| Feature | What your server needs |
|---|---|
| Sign-in | [Synapse](https://github.com/element-hq/synapse) with [Matrix Authentication Service](https://github.com/element-hq/matrix-authentication-service) |
| Fast sync | A recent Synapse, with simplified sliding sync (on by default in current versions) |
| Calls and voice channels | [LiveKit](https://livekit.io) + [lk-jwt-service](https://github.com/element-hq/lk-jwt-service), announced in `/.well-known/matrix/client` (`org.matrix.msc4143.rtc_foci`), the same as for Element Call |
| Link previews | `url_preview_enabled: true` in Synapse |
| Notifications on Android | Nothing to do: AlwyChat uses its own push gateway, run by its author. It only receives which chat and which message (two IDs), never the content or the sender: the phone fetches and decrypts the message itself. |

<a id="web-version"></a>
### Web version (self-hosted)

Not available yet. AlwyChat's core is written in Rust and runs inside the app; a browser version needs that core compiled to WebAssembly, which is planned.
Until then, if you want a web client on your own server, [Element Web](https://github.com/element-hq/element-web) and [Cinny](https://github.com/cinnyapp/cinny) work well alongside AlwyChat: they share the same accounts, chats and encryption.

---

## Build it yourself

You need [Rust](https://rustup.rs), [Node.js](https://nodejs.org) 22+, [pnpm](https://pnpm.io), and the [Tauri prerequisites](https://tauri.app/start/prerequisites/) for your system.

```sh
git clone https://github.com/arlexTech/alwychat.git
cd alwychat/app
pnpm install

pnpm tauri dev                                     # run it on your computer
pnpm tauri build                                   # Linux packages, in target/release/bundle/
pnpm tauri android build --apk --target aarch64    # Android APK (needs the Android SDK and NDK)
```

Android push notifications need a Firebase project: put its `google-services.json` in `app/src-tauri/gen/android/app/`. Without it, the app still works and notifies you while it's open.

**Known limit on Linux:** calls need WebRTC in WebKitGTK, and some distributions (Arch, CachyOS…) build it without. Messages, voice messages and everything else work.

For developers: [architecture](docs/architecture.md) · Rust core in [`crates/alwy-core`](crates/alwy-core) · app in [`app/`](app) (Tauri 2 + Svelte 5).

---

## License

AlwyChat is free software under the [GNU AGPL v3 or later](LICENSE): you can use, study, share and change it.
