# AlwyChat

A chat app for [Matrix](https://matrix.org). Messages are end-to-end encrypted, and you can talk with people on Element, Element X, FluffyChat or any other Matrix app.

> AlwyChat is in beta. The first release isn't out yet, so the **Download** links below won't work until it is. Until then, take the **Test version**.

## Which file do I download?

Find your device in the table and click the file next to it.

| Your device | Download | Test version |
|---|---|---|
| 📱 **Android** phone or tablet (Android 7 or newer) | [AlwyChat-android.apk](https://github.com/arlexTech/alwychat/releases/latest/download/AlwyChat-android.apk) | [AlwyChat-android.apk](https://github.com/arlexTech/alwychat/releases/download/beta/AlwyChat-android.apk) |
| 🐧 **Linux**, any distribution | [AlwyChat-linux-x86_64.AppImage](https://github.com/arlexTech/alwychat/releases/latest/download/AlwyChat-linux-x86_64.AppImage) | [AlwyChat-linux-x86_64.AppImage](https://github.com/arlexTech/alwychat/releases/download/beta/AlwyChat-linux-x86_64.AppImage) |
| 🐧 **Linux**, Debian / Ubuntu / Mint | [AlwyChat-linux-amd64.deb](https://github.com/arlexTech/alwychat/releases/latest/download/AlwyChat-linux-amd64.deb) | [AlwyChat-linux-amd64.deb](https://github.com/arlexTech/alwychat/releases/download/beta/AlwyChat-linux-amd64.deb) |
| 🐧 **Linux**, Fedora / openSUSE | [AlwyChat-linux-x86_64.rpm](https://github.com/arlexTech/alwychat/releases/latest/download/AlwyChat-linux-x86_64.rpm) | [AlwyChat-linux-x86_64.rpm](https://github.com/arlexTech/alwychat/releases/download/beta/AlwyChat-linux-x86_64.rpm) |
| 🪟 **Windows** 10 / 11 | Not available yet | [AlwyChat-windows-x64-setup.exe](https://github.com/arlexTech/alwychat/releases/download/beta/AlwyChat-windows-x64-setup.exe) |
| 🍏 **macOS** | Not available yet | [AlwyChat-macos.dmg](https://github.com/arlexTech/alwychat/releases/download/beta/AlwyChat-macos.dmg) |
| 🍎 **iPhone / iPad** | Not available yet | Not available yet |

🌐 **Web browser**: AlwyChat also runs in a browser, hosted on your own server. See [how to host it](#web-version).

The **Test version** has the newest changes as soon as they're made, so it may have bugs.

**Updates:** the apps don't update themselves yet. To get a newer version, download the file again and install it over the old one.

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
