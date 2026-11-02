#!/usr/bin/env python3
"""Write an update manifest the apps read (Tauri's updater format, plus Android).

    update-manifest.py <files dir> <download base URL> <version> <notes> [platform]

With a platform (android, linux, windows, macos), only its entries go in latest-<platform>.json,
which each platform's build publishes as soon as it's ready (one platform never waits for
another, and never points to another's older files). Without one: latest.json with every
platform (read by apps from before the per-platform files).

Files are the release files under their stable names, with the `.sig` signatures Tauri's
bundler made (only when the build had the update-signing key).
"""
import datetime
import json
import pathlib
import sys

files, base, version, notes = pathlib.Path(sys.argv[1]), sys.argv[2].rstrip("/"), sys.argv[3], sys.argv[4]
only = sys.argv[5] if len(sys.argv) > 5 else None
PREFIX = {"android": "android", "linux": "linux-", "windows": "windows-", "macos": "darwin-"}


def entry(name):
    sig = files / f"{name}.sig"
    if not (files / name).exists() or not sig.exists():
        return None
    return {"signature": sig.read_text().strip(), "url": f"{base}/{name}"}


# Updater targets: {os}-{arch}-{bundle type}, and {os}-{arch} as the fallback. Always the
# "-autoupdate" files: an app that updated itself must go on doing so.
targets = {
    "linux-x86_64-appimage": "AlwyChat-linux-x86_64-autoupdate.AppImage",
    "linux-x86_64-deb": "AlwyChat-linux-amd64-autoupdate.deb",
    "linux-x86_64-rpm": "AlwyChat-linux-x86_64-autoupdate.rpm",
    "linux-x86_64": "AlwyChat-linux-x86_64-autoupdate.AppImage",
    "windows-x86_64-nsis": "AlwyChat-windows-x64-autoupdate-setup.exe",
    "windows-x86_64-msi": "AlwyChat-windows-x64-autoupdate.msi",
    "windows-x86_64": "AlwyChat-windows-x64-autoupdate-setup.exe",
    "darwin-aarch64-app": "AlwyChat-macos-autoupdate.app.tar.gz",
    "darwin-x86_64-app": "AlwyChat-macos-autoupdate.app.tar.gz",
    "darwin-aarch64": "AlwyChat-macos-autoupdate.app.tar.gz",
    "darwin-x86_64": "AlwyChat-macos-autoupdate.app.tar.gz",
}
platforms = {t: e for t, name in targets.items() if (e := entry(name))}
# Android: no Tauri updater there; the app downloads the APK and Android checks that it's
# signed with the same key as the installed app.
if (files / "AlwyChat-android-autoupdate.apk").exists():
    platforms["android"] = {"signature": "", "url": f"{base}/AlwyChat-android-autoupdate.apk"}

if only:
    platforms = {t: e for t, e in platforms.items() if t.startswith(PREFIX[only])}

manifest = {
    "version": version,
    "notes": notes,
    "pub_date": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
    "platforms": platforms,
}
name = f"latest-{only}.json" if only else "latest.json"
(files / name).write_text(json.dumps(manifest, indent=2) + "\n")
print(f"{name}: {version}, {len(platforms)} entries")
