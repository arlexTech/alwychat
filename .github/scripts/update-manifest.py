#!/usr/bin/env python3
"""Write latest.json, the update manifest the apps read (Tauri's updater format, plus Android).

    update-manifest.py <files dir> <download base URL> <version> <notes>

Files are the release files under their stable names, with the `.sig` signatures Tauri's
bundler made (only when the build had the update-signing key).
"""
import datetime
import json
import pathlib
import sys

files, base, version, notes = pathlib.Path(sys.argv[1]), sys.argv[2].rstrip("/"), sys.argv[3], sys.argv[4]


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

manifest = {
    "version": version,
    "notes": notes,
    "pub_date": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
    "platforms": platforms,
}
(files / "latest.json").write_text(json.dumps(manifest, indent=2) + "\n")
print(f"latest.json: {version}, {len(platforms)} platforms")
