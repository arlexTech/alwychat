<img src="app/src-tauri/icons/logo.svg" width="96" alt="AlwyChat logo" align="right" />

# AlwyChat

A chat app for [Matrix](https://matrix.org). Messages are end-to-end encrypted, and you can talk with people on Element, Element X, FluffyChat or any other Matrix app.

> AlwyChat is in beta. The first stable release isn't out yet, so take a **Beta** file for now.

## Which file do I download?

Find your device in the table and click the file next to it.

| Your device | Stable | Stable, updates itself | Beta | Beta, updates itself |
|---|---|---|---|---|
| 📱 **Android** phone or tablet (Android 7 or newer) | [.apk](https://github.com/arlexTech/alwychat/releases/latest/download/AlwyChat-android.apk) | [.apk](https://github.com/arlexTech/alwychat/releases/latest/download/AlwyChat-android-autoupdate.apk) | [.apk](https://github.com/arlexTech/alwychat/releases/download/beta/AlwyChat-android.apk) | [.apk](https://github.com/arlexTech/alwychat/releases/download/beta/AlwyChat-android-autoupdate.apk) |
| 🐧 **Linux**, any distribution | [.AppImage](https://github.com/arlexTech/alwychat/releases/latest/download/AlwyChat-linux-x86_64.AppImage) | [.AppImage](https://github.com/arlexTech/alwychat/releases/latest/download/AlwyChat-linux-x86_64-autoupdate.AppImage) | [.AppImage](https://github.com/arlexTech/alwychat/releases/download/beta/AlwyChat-linux-x86_64.AppImage) | [.AppImage](https://github.com/arlexTech/alwychat/releases/download/beta/AlwyChat-linux-x86_64-autoupdate.AppImage) |
| 🐧 **Linux**, Debian / Ubuntu / Mint | [.deb](https://github.com/arlexTech/alwychat/releases/latest/download/AlwyChat-linux-amd64.deb) | [.deb](https://github.com/arlexTech/alwychat/releases/latest/download/AlwyChat-linux-amd64-autoupdate.deb) | [.deb](https://github.com/arlexTech/alwychat/releases/download/beta/AlwyChat-linux-amd64.deb) | [.deb](https://github.com/arlexTech/alwychat/releases/download/beta/AlwyChat-linux-amd64-autoupdate.deb) |
| 🐧 **Linux**, Fedora / openSUSE | [.rpm](https://github.com/arlexTech/alwychat/releases/latest/download/AlwyChat-linux-x86_64.rpm) | [.rpm](https://github.com/arlexTech/alwychat/releases/latest/download/AlwyChat-linux-x86_64-autoupdate.rpm) | [.rpm](https://github.com/arlexTech/alwychat/releases/download/beta/AlwyChat-linux-x86_64.rpm) | [.rpm](https://github.com/arlexTech/alwychat/releases/download/beta/AlwyChat-linux-x86_64-autoupdate.rpm) |
| 🪟 **Windows** 10 / 11 | Not yet | Not yet | [.exe](https://github.com/arlexTech/alwychat/releases/download/beta/AlwyChat-windows-x64-setup.exe) | [.exe](https://github.com/arlexTech/alwychat/releases/download/beta/AlwyChat-windows-x64-autoupdate-setup.exe) |
| 🍏 **macOS** | Not yet | Not yet | [.dmg](https://github.com/arlexTech/alwychat/releases/download/beta/AlwyChat-macos.dmg) | [.dmg](https://github.com/arlexTech/alwychat/releases/download/beta/AlwyChat-macos-autoupdate.dmg) |
| 🍎 **iPhone / iPad** | Not yet | Not yet | Not yet | Not yet |

🌐 **Web browser**: AlwyChat also runs in a browser, hosted on your own server. See [how to host it](#web-version).

**Which column?**

- **Stable:** tested releases. **Beta:** every change as soon as it's made, so it may have bugs.
- **Updates itself:** AlwyChat checks a few times a day and offers each new version of its column (a banner in the chat list, one tap). Without it, you download new versions yourself, like from an app store.

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

## Privacy and security

What is protected, and what isn't (yet):

- **Messages** in encrypted chats (all private chats by default) are end-to-end encrypted, with the same encryption as Element. Your server can't read them.
- **Calls and voice channels** go through your server's LiveKit and are encrypted on the way, but **not end-to-end yet**: whoever runs that LiveKit server could listen. Voice channels are unencrypted on purpose, so people on Element can join them.
- **Stored on your server, readable by its admins:** your drafts, chat folders and app settings (synced between your devices), and who is in which chat, as with every Matrix app.
- **Android notifications**, where your server offers them, go through Google's push service and a push gateway, which only see a chat ID and a message ID. Your phone fetches and decrypts the message itself.
- **On your device:** your sign-in is kept in the system keychain (computer) or the app's private storage (Android). The search index and message cache aren't encrypted on disk; the app lock (PIN, fingerprint) hides the app, it doesn't encrypt it. In the web version, the sign-in is kept in the browser's storage.
- **Updates** that install themselves are signed: the app refuses a file that wasn't built and signed for it.
- **Files from others** can't be opened from AlwyChat if they could run a program (`.exe`, `.sh`, `.desktop`…). You can still save them.

Found a security problem? Please report it privately through [GitHub's security advisories](https://github.com/arlexTech/alwychat/security/advisories/new) rather than in a public issue.

## Which server can I use?

Your server needs the newer sign-in system, where you log in through your browser (Matrix Authentication Service). matrix.org has it. Servers that only accept a password typed into the app aren't supported yet.

### Running your own server

| Feature | What your server needs |
|---|---|
| Sign-in | [Synapse](https://github.com/element-hq/synapse) with [Matrix Authentication Service](https://github.com/element-hq/matrix-authentication-service) |
| Sync | A recent Synapse (simplified sliding sync is on by default) |
| Calls and voice channels | [LiveKit](https://livekit.io) and [lk-jwt-service](https://github.com/element-hq/lk-jwt-service), listed in `/.well-known/matrix/client` under `org.matrix.msc4143.rtc_foci` (the same setup Element Call uses) |
| Link previews | `url_preview_enabled: true` in Synapse |
| Android notifications while AlwyChat is closed | Not available for other servers yet ([UnifiedPush](https://unifiedpush.org) support is planned). Until then, AlwyChat notifies you while it's open or in the background. |

<a id="web-version"></a>
### Web version (host it yourself)

The web version is the same app running in the browser. It's a folder of static files: no database, no server program. Messages are encrypted and decrypted in the browser, like in the apps.

1. Download [AlwyChat-web.zip](https://github.com/arlexTech/alwychat/releases/latest/download/AlwyChat-web.zip) and unzip it into a folder on your server, for example `/srv/alwychat`.
2. Serve that folder over **https** on its own address, for example `chat.example.org`. Sign-in only works over https. With [Caddy](https://caddyserver.com):

   ```
   chat.example.org {
       root * /srv/alwychat
       file_server
       try_files {path} /index.html
       header /_app/immutable/* Cache-Control "public, max-age=31536000, immutable"
   }
   ```

   Any web server works (nginx, Apache…), as long as it sends `.wasm` files as `application/wasm`.
3. Open the address and sign in. The sign-in page opens in a popup, so allow popups for that site if your browser asks.

To serve it under a sub-path (like `example.org/chat/`), build it yourself with `ALWY_WEB_BASE=/chat pnpm build:web`.

What's different from the apps: there are no notifications while the tab is closed, and photos are downloaded instead of saved to a gallery.
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
pnpm build:web                                     # web version, in app/build-web/
```

The web build also needs the `wasm32-unknown-unknown` Rust target (`rustup target add wasm32-unknown-unknown`) and `wasm-bindgen-cli` at the version in `Cargo.lock`.

For Android push notifications, put your Firebase project's `google-services.json` in `app/src-tauri/gen/android/app/`. Without it, the app only notifies you while it's open.

Calls on Linux need WebKitGTK built with WebRTC. Some distributions (Arch, CachyOS) ship it without, and calls won't work there. Everything else does.

The code: [architecture notes](docs/architecture.md), the Rust core in [`crates/alwy-core`](crates/alwy-core), the app in [`app/`](app) (Tauri 2 and Svelte 5).

---

## License

[GNU AGPL v3 or later](LICENSE).
