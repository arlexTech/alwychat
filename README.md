# AlwyChat

A light, fast Matrix client that takes the best of Messenger, WhatsApp, Telegram and Discord:
photo albums, shared conversation themes, Discord-style voice channels and more, all end-to-end encrypted and compatible with Element and FluffyChat.

Status: early development.

- Features: [docs/features.md](docs/features.md)
- Architecture: [docs/architecture.md](docs/architecture.md)

## Stack

Rust core on [matrix-rust-sdk](https://github.com/matrix-org/matrix-rust-sdk) · [Tauri 2](https://tauri.app) · Svelte 5 · LiveKit (MatrixRTC) for calls.
Targets: Linux, Android, iOS (then Windows, macOS, web).

## Development

Requirements: Rust (stable), pnpm, Node ≥ 22, and the [Tauri prerequisites](https://tauri.app/start/prerequisites/) for your platform.

```sh
cd app
pnpm install
pnpm tauri dev            # desktop
pnpm tauri android dev    # Android (needs Android SDK/NDK)
pnpm tauri ios dev        # iOS (macOS + Xcode only)
```

Core only:

```sh
cargo run -p alwy-core --example discover -- example.org
```

## License

[AGPL-3.0-or-later](LICENSE).
