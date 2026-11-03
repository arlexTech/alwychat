#!/usr/bin/env bash
# Publish "updates itself" builds to a folder you serve yourself (a synced, publicly shared
# cloud folder, a web server…), outside GitHub. The apps built here check <url>/latest-<platform>.json
# and install the newer build from the same place.
#
#   scripts/release-local.sh [--android] [--linux] [--notes "text"]   (default: --android)
#
# Settings in .release.env at the repo root (gitignored, never commit it):
#   RELEASE_DIR=~/path/to/the/synced/folder     where the files are copied
#   RELEASE_URL=https://host/path/to/folder     the same folder as the apps download it
#   ANDROID_KEYSTORE=~/release.jks              (if gen/android/keystore.properties is missing)
#   ANDROID_KEY_ALIAS=alias
#   UPDATER_KEY=~/.tauri/app.key                (Linux: the updater signing key)
# Passwords are asked for (or taken from ANDROID_KEYSTORE_PASSWORD / TAURI_SIGNING_PRIVATE_KEY_PASSWORD).
#
# Files: <name>-<version>-<commit>.apk / .AppImage at the top, the previous ones moved to
# archive/, latest-android.json / latest-linux.json written last. Android only accepts an update signed with the same key.
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
[ -f .release.env ] || { echo ".release.env missing (see the top of $0)"; exit 1; }
# shellcheck disable=SC1091
source .release.env
expand() { eval echo "$1"; }
RELEASE_DIR=$(expand "$RELEASE_DIR")
RELEASE_URL=${RELEASE_URL%/}
[ -d "$RELEASE_DIR" ] || { echo "RELEASE_DIR $RELEASE_DIR doesn't exist"; exit 1; }

android=false linux=false notes=""
while [ $# -gt 0 ]; do
  case "$1" in
    --android) android=true ;;
    --linux) linux=true ;;
    --notes) notes="$2"; shift ;;
    *) echo "unknown option $1"; exit 1 ;;
  esac
  shift
done
$android || $linux || android=true

# Version 0.1.<n>: the commit count, and always above the last published one.
commit=$(git rev-parse --short HEAD)
count=$(git rev-list --count HEAD)
last=0
for m in "$RELEASE_DIR"/latest-*.json; do
  n=$(python3 -c "import json,sys; print(json.load(open(sys.argv[1]))['version'].split('.')[-1])" "$m" 2>/dev/null || echo 0)
  [ "$n" -gt "$last" ] && last=$n
done
patch=$(( count > last ? count : last + 1 ))
version="0.1.$patch"
[ -n "$notes" ] || notes="AlwyChat $version ($commit)"
config="{\"version\":\"$version\"}"
echo "Building $version ($commit)"
[ -z "$(git status --porcelain)" ] || echo "note: uncommitted changes are included"

out=$(mktemp -d)
trap 'rm -rf "$out"' EXIT
cd app

if $android; then
  props=src-tauri/gen/android/keystore.properties
  made_props=false
  if [ ! -f "$props" ]; then
    [ -n "${ANDROID_KEYSTORE:-}" ] || { echo "no keystore.properties and no ANDROID_KEYSTORE"; exit 1; }
    pass=${ANDROID_KEYSTORE_PASSWORD:-}
    [ -n "$pass" ] || { read -rsp "Android keystore password: " pass; echo; }
    printf 'storeFile=%s\npassword=%s\nkeyAlias=%s\n' "$(expand "$ANDROID_KEYSTORE")" "$pass" "$ANDROID_KEY_ALIAS" > "$props"
    chmod 600 "$props"
    made_props=true
    trap 'rm -rf "$out"; rm -f "$root/app/$props"' EXIT
  fi
  [ -f src-tauri/gen/android/app/google-services.json ] ||
    echo "warning: no google-services.json, this build gets notifications only while it runs"
  rm -rf src-tauri/gen/android/app/build/outputs/apk
  ALWY_UPDATE_FEED="$RELEASE_URL/latest-android.json" pnpm tauri android build --apk --target aarch64 --config "$config"
  cp "$(find src-tauri/gen/android/app/build/outputs/apk/universal -name '*.apk' | head -1)" "$out/alwychat-$version-$commit.apk"
  if $made_props; then rm -f "$props"; fi
fi

if $linux; then
  key=$(expand "${UPDATER_KEY:-}")
  [ -f "$key" ] || { echo "UPDATER_KEY missing"; exit 1; }
  export TAURI_SIGNING_PRIVATE_KEY
  TAURI_SIGNING_PRIVATE_KEY=$(cat "$key")
  if [ -z "${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}" ]; then
    read -rsp "Updater key password (empty if none): " TAURI_SIGNING_PRIVATE_KEY_PASSWORD; echo
    export TAURI_SIGNING_PRIVATE_KEY_PASSWORD
  fi
  ALWY_UPDATE_FEED="$RELEASE_URL/latest-linux.json" pnpm tauri build --bundles appimage --config "{\"version\":\"$version\",\"bundle\":{\"createUpdaterArtifacts\":true}}"
  b=../target/release/bundle/appimage
  cp "$(ls $b/*.AppImage | head -1)" "$out/alwychat-$version-$commit.AppImage"
  cp "$(ls $b/*.AppImage.sig | head -1)" "$out/alwychat-$version-$commit.AppImage.sig"
fi

# Previous builds of the platforms just built go to archive/.
mkdir -p "$RELEASE_DIR/archive"
for ext in apk AppImage AppImage.sig; do
  ls "$out"/*."$ext" >/dev/null 2>&1 || continue
  for old in "$RELEASE_DIR"/*."$ext"; do
    [ -e "$old" ] && mv "$old" "$RELEASE_DIR/archive/"
  done
done
for f in "$out"/*; do
  case "$f" in *.sig) ;; *) cp "$f" "$RELEASE_DIR/" ;; esac
done

python3 - "$RELEASE_URL" "$version" "$notes" "$out" "$RELEASE_DIR" <<'PY'
import datetime, json, pathlib, sys
url, version, notes, out, folder = sys.argv[1], sys.argv[2], sys.argv[3], pathlib.Path(sys.argv[4]), pathlib.Path(sys.argv[5])
def write(platform, platforms):
    manifest = {
        "version": version,
        "notes": notes,
        "pub_date": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "platforms": platforms,
    }
    (folder / f"latest-{platform}.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print(f"latest-{platform}.json: {version}")
for apk in out.glob("*.apk"):
    write("android", {"android": {"signature": "", "url": f"{url}/{apk.name}"}})
for image in out.glob("*.AppImage"):
    entry = {"signature": (out / f"{image.name}.sig").read_text().strip(), "url": f"{url}/{image.name}"}
    write("linux", {"linux-x86_64-appimage": entry, "linux-x86_64": entry})
PY
ls -la "$RELEASE_DIR"
