# AlwyChat

A chat app for [Matrix](https://matrix.org). Messages are end-to-end encrypted, and you can talk with people on Element, Element X, FluffyChat or any other Matrix app.

> AlwyChat is in beta. The first release isn't out yet, so the download links below won't work until it is. Until then, you can [build it yourself](#build-it-yourself).

## Which file do I download?

Find your device in the table and click the file next to it.

| Your device | Download |
|---|---|
| 📱 **Android** phone or tablet (Android 7 or newer) | [AlwyChat-android.apk](https://github.com/arlexTech/alwychat/releases/latest/download/AlwyChat-android.apk) |
| 🐧 **Linux**, any distribution | [AlwyChat-linux-x86_64.AppImage](https://github.com/arlexTech/alwychat/releases/latest/download/AlwyChat-linux-x86_64.AppImage) |
| 🐧 **Linux**, Debian / Ubuntu / Mint | [AlwyChat-linux-amd64.deb](https://github.com/arlexTech/alwychat/releases/latest/download/AlwyChat-linux-amd64.deb) |
| 🐧 **Linux**, Fedora / openSUSE | [AlwyChat-linux-x86_64.rpm](https://github.com/arlexTech/alwychat/releases/latest/download/AlwyChat-linux-x86_64.rpm) |
| 🍎 **iPhone / iPad** | Not available yet |
| 🪟 **Windows** · 🍏 **macOS** | Not available yet |
| 🌐 **Web browser** | Not available yet ([why](#web-version)) |

Not sure which Linux file to pick? Take the **AppImage**, it works on every distribution.

Older versions are on the [releases page](https://github.com/arlexTech/alwychat/releases).

### Installing

**📱 Android**
1. Download the `.apk` file on your phone.
2. Open it. If Android asks, allow your browser or file manager to install apps.
3. Tap **Install**, then open AlwyChat and sign in.

**🐧 Linux, AppImage**
1. Download the `.AppImage` file.
2. Right-click it, open **Properties**, and tick **Allow executing file as program**.
3. Double-click it to start AlwyChat.

Or in a terminal:
```sh
chmod +x AlwyChat-linux-x86_64.AppImage
./AlwyChat-linux-x86_64.AppImage
```

**🐧 Linux, .deb or .rpm**
1. Download the file for your distribution.
2. Double-click it: your software center opens. Click **Install**.

Or in a terminal:
```sh
sudo apt install ./AlwyChat-linux-amd64.deb    # Debian, Ubuntu, Mint
sudo dnf install ./AlwyChat-linux-x86_64.rpm   # Fedora
```

---

## Features

- Replies, reactions, edits, threads, polls, mentions and voice messages
- Photo albums, camera, and a per-chat choice between HD and compressed photos
- Chat themes with illustrated backgrounds or your own photo, nicknames, a custom quick reaction
- Search through all your chats, encrypted ones included, and a timeline to jump to any day
- Folders, spaces, and voice channels you can drop in and out of
- Voice and video calls
- Send your location once, or share it live until you stop
- Drafts that follow you between devices
- Share photos and links from other apps (Android)
- Optional app lock with a PIN or fingerprint
- Light, dark and black themes

See [docs/features.md](docs/features.md) for the full list, including what's planned.

---

## Which server can I use?

Your server needs the newer sign-in system, where you log in through your browser (Matrix Authentication Service). matrix.org has it. Servers that only accept a password typed into the app aren't supported yet.

### Running your own server

| Feature | What your server needs |
|---|---|
| Sign-in | [Synapse](https://github.com/element-hq/synapse) with [Matrix Authentication Service](https://github.com/element-hq/matrix-authentication-service) |
| Sync | A recent Synapse (simplified sliding sync is on by default) |
| Calls and voice channels | [LiveKit](https://livekit.io) and [lk-jwt-service](https://github.com/element-hq/lk-jwt-service), listed in `/.well-known/matrix/client` under `org.matrix.msc4143.rtc_foci` (the same setup Element Call uses) |
| Link previews | `url_preview_enabled: true` in Synapse |
| Android notifications | Nothing. They go through AlwyChat's push gateway, which only sees a chat ID and a message ID. The phone downloads and decrypts the message itself. |

<a id="web-version"></a>
### Web version

There's no web version to host yet. The core of AlwyChat is written in Rust and has to be compiled to WebAssembly before it can run in a browser. That's planned. In the meantime, [Element Web](https://github.com/element-hq/element-web) or [Cinny](https://github.com/cinnyapp/cinny) can run on your server next to AlwyChat, with the same accounts and chats.

---

## Build it yourself

You need [Rust](https://rustup.rs), [Node.js](https://nodejs.org) 22 or newer, [pnpm](https://pnpm.io), and the [Tauri prerequisites](https://tauri.app/start/prerequisites/) for your system.

```sh
git clone https://github.com/arlexTech/alwychat.git
cd alwychat/app
pnpm install

pnpm tauri dev                                     # run it
pnpm tauri build                                   # Linux packages, in target/release/bundle/
pnpm tauri android build --apk --target aarch64    # Android APK (needs the Android SDK and NDK)
```

For Android push notifications, put your Firebase project's `google-services.json` in `app/src-tauri/gen/android/app/`. Without it, the app only notifies you while it's open.

Calls on Linux need WebKitGTK built with WebRTC. Some distributions (Arch, CachyOS) ship it without, and calls won't work there. Everything else does.

The code: [architecture notes](docs/architecture.md), the Rust core in [`crates/alwy-core`](crates/alwy-core), the app in [`app/`](app) (Tauri 2 and Svelte 5).

---

## License

[GNU AGPL v3 or later](LICENSE).
