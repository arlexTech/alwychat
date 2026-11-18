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
| 🍏 **macOS** | Not yet | Not yet | Not yet | Not yet |
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

**🪟 Windows**
1. Download the `.exe` file and open it.
2. Windows may say it protected your PC (the installer isn't signed by a paid certificate yet): click **More info**, then **Run anyway**.
3. AlwyChat installs for your user only (no administrator needed) and opens.

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
- **Calls** in encrypted chats are end-to-end encrypted, the same way as Element Call: your server's LiveKit passes them on without being able to listen. **Voice channels** are only encrypted on the way (not end to end), on purpose, so people on Element can join them.
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
| Android notifications while AlwyChat is closed | Nothing: install a [UnifiedPush](https://unifiedpush.org) app such as [ntfy](https://ntfy.sh) on the phone and choose it in AlwyChat (Settings → Notifications → Push service). Without one, AlwyChat notifies you while it's open or in the background. |

<a id="web-version"></a>
### Web version (host it yourself)

The web version is the same app in a browser tab. There's no database and nothing to maintain: messages are encrypted and decrypted in the browser, like in the apps, and the server only hands out the app. It's published as a Docker image, `ghcr.io/arlextech/alwychat-web`, for regular PCs and ARM boards (Raspberry Pi…). The image is tiny: about 10 MB, around 12 MB of memory, nothing inside but the app and its own small web server.

**If you already have a reverse proxy** (Caddy, nginx, Traefik…), save this as `compose.yml` on your server:

```yaml
services:
  alwychat:
    image: ghcr.io/arlextech/alwychat-web:beta
    restart: unless-stopped
    environment:
      DEFAULT_MATRIX_SERVER: example.org   # your Matrix server, suggested on the sign-in screen
    ports:
      - "127.0.0.1:8080:80"
```

Then start it with `docker compose up -d`, and point your reverse proxy's https address (for example `chat.example.org`) to `http://127.0.0.1:8080`. With Caddy that's one block:

```
chat.example.org {
    reverse_proxy 127.0.0.1:8080
}
```

**If nothing else runs on that server**, use [`docker/web/compose.https.yml`](docker/web/compose.https.yml) instead: it adds [Caddy](https://caddyserver.com) in front, which gets the https certificate by itself. Point `chat.example.org` to the server in your DNS, and change the two lines marked ✏️ in the file.

Open the address and sign in. It must be **https** (browsers only allow sign-in and encryption there), and allow popups for the site if your browser asks: the sign-in page opens in one.

| Setting | What it does |
|---|---|
| `DEFAULT_MATRIX_SERVER` | The Matrix server the sign-in screen suggests (people can still type another). Default: matrix.org |
| `PORT` | The port inside the container. Default: 80 |

**Updating:** `docker compose pull && docker compose up -d`. Open tabs switch to the new version the next time they're opened.

**Which tag:** `beta` gets every change as soon as it's made, like the Beta apps; `latest` will be the stable release once the first one is out. Each version also has its own tag (`0.2.40`…) if you want to stay on one.

What's different from the apps: no notifications while the tab is closed, and photos are downloaded instead of saved to a gallery.

To serve it under a sub-path (like `example.org/chat/`) or change anything else, build it yourself: `ALWY_WEB_BASE=/chat pnpm build:web`, or the whole image from source with [`docker/web/Dockerfile`](docker/web/Dockerfile).

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
