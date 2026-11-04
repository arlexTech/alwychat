#!/usr/bin/env bash
# Publish "updates itself" builds to a folder you serve yourself (a synced, publicly shared
# cloud folder, a web server…), outside GitHub. The apps built here check <url>/latest-<platform>.json
# and install the newer build from the same place.
#
#   scripts/release-local.sh [--android] [--linux] [--windows] [--notes "text"]   (default: --android)
#   scripts/release-local.sh --test     signed Android build at the published version, saved as
#                                       ~/alwychat-test.apk, NOT published (to try on a phone first)
#
# Settings in .release.env at the repo root (gitignored, never commit it):
#   RELEASE_DIR=~/path/to/the/synced/folder     where the files are copied
#   RELEASE_URL=https://host/path/to/folder     the same folder as the apps download it
#   ANDROID_KEYSTORE=~/release.jks              (if gen/android/keystore.properties is missing)
#   ANDROID_KEY_ALIAS=alias
#   UPDATER_KEY=~/.tauri/app.key                (Linux, Windows: the updater signing key)
#   ANDROID_ID=org.example.app                  (optional: another Android id; push registers
#                                                under it too, so the gateway must know it)
#   PUSH_GATEWAY=https://host/_matrix/push/v1/notify  (optional: used when the homeserver
#                                                doesn't advertise a gateway)
# Passwords are asked for (or taken from ANDROID_KEYSTORE_PASSWORD / TAURI_SIGNING_PRIVATE_KEY_PASSWORD).
#
# Windows is built here too: cargo-xwin (accepts Microsoft's SDK license) + makensis (official
# NSIS with a matching Linux makensis, see scripts/nsis/makensis);
# the result is the AlwyChat setup .exe, which installs updates over itself.
#
# Files: <name>-<version>-<commit>.apk / .AppImage / .exe at the top, the previous ones moved to
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
if [ -n "${ANDROID_ID:-}" ]; then export ALWY_ANDROID_ID="$ANDROID_ID" ALWY_PUSH_APP_ID="$ANDROID_ID"; fi
if [ -n "${PUSH_GATEWAY:-}" ]; then export ALWY_PUSH_GATEWAY="$PUSH_GATEWAY"; fi
[ -d "$RELEASE_DIR" ] || { echo "RELEASE_DIR $RELEASE_DIR doesn't exist"; exit 1; }

android=false linux=false windows=false notes="" test=false
while [ $# -gt 0 ]; do
  case "$1" in
    --android) android=true ;;
    --linux) linux=true ;;
    --windows) windows=true ;;
    --notes) notes="$2"; shift ;;
    --test) test=true android=true ;;
    *) echo "unknown option $1"; exit 1 ;;
  esac
  shift
done
$android || $linux || $windows || android=true

# Version 0.1.<n>: the commit count, and always above the last published one.
commit=$(git rev-parse --short HEAD)
count=$(git rev-list --count HEAD)
last=0
for m in "$RELEASE_DIR"/latest-*.json; do
  n=$(python3 -c "import json,sys; print(json.load(open(sys.argv[1]))['version'].split('.')[-1])" "$m" 2>/dev/null || echo 0)
  [ "$n" -gt "$last" ] && last=$n
done
patch=$(( count > last ? count : last + 1 ))
# A test build keeps the published version: it installs over it, and the next real release
# still counts as an update.
if $test; then patch=$last; linux=false windows=false; fi
# Test builds skip the slow whole-program optimisation (link-time optimisation on one core, the
# last ~5 minutes of a build): a bit bigger and slower app, fine for trying on a phone.
if $test; then
  export CARGO_PROFILE_RELEASE_LTO=false CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16
fi
version="0.1.$patch"
[ -n "$notes" ] || notes="AlwyChat $version ($commit)"
config="{\"version\":\"$version\"}"
echo "Building $version ($commit)"
[ -z "$(git status --porcelain)" ] || echo "note: uncommitted changes are included"

out=$(mktemp -d)
trap 'rm -rf "$out"' EXIT
cd app

# Everything that asks or can fail fast comes first (passwords, tools), so the builds can then
# run unattended, side by side.
made_props=false
props=src-tauri/gen/android/keystore.properties
if $android; then
  if [ ! -f "$props" ]; then
    [ -n "${ANDROID_KEYSTORE:-}" ] || { echo "no keystore.properties and no ANDROID_KEYSTORE"; exit 1; }
    pass=${ANDROID_KEYSTORE_PASSWORD:-}
    store=$(expand "$ANDROID_KEYSTORE")
    # Checked right away (keytool), so a typo doesn't cost a whole build.
    ok() { KS_PASS="$pass" keytool -list -keystore "$store" -alias "$ANDROID_KEY_ALIAS" -storepass:env KS_PASS >/dev/null 2>&1; }
    if [ -n "$pass" ] && ! ok; then echo "ANDROID_KEYSTORE_PASSWORD is wrong"; exit 1; fi
    tries=0
    while [ -z "$pass" ] || ! ok; do
      [ -t 0 ] || { echo "the keystore password is needed: run this in a terminal (or create $props)"; exit 1; }
      [ $tries -gt 0 ] && echo "Wrong password (or key alias $ANDROID_KEY_ALIAS), try again."
      [ $tries -ge 3 ] && exit 1
      read -rsp "Android keystore password: " pass; echo
      tries=$((tries + 1))
    done
    printf 'storeFile=%s\npassword=%s\nkeyAlias=%s\n' "$store" "$pass" "$ANDROID_KEY_ALIAS" > "$props"
    chmod 600 "$props"
    made_props=true
    trap 'rm -rf "$out"; rm -f "$root/app/$props"' EXIT
  fi
  [ -f src-tauri/gen/android/app/google-services.json ] ||
    echo "warning: no google-services.json, this build gets notifications only while it runs"
fi

if $linux || $windows; then
  key=$(expand "${UPDATER_KEY:-}")
  [ -f "$key" ] || { echo "UPDATER_KEY missing"; exit 1; }
  export TAURI_SIGNING_PRIVATE_KEY
  TAURI_SIGNING_PRIVATE_KEY=$(cat "$key")
  if [ -z "${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}" ]; then
    read -rsp "Updater key password (empty if none): " TAURI_SIGNING_PRIVATE_KEY_PASSWORD; echo
    export TAURI_SIGNING_PRIVATE_KEY_PASSWORD
  fi
fi

if $linux; then
  pkg-config --exists ayatana-appindicator3-0.1 || pkg-config --exists appindicator3-0.1 ||
    { echo "Linux builds need the tray library: sudo pacman -S libayatana-appindicator"; exit 1; }
fi

if $windows; then
  command -v cargo-xwin >/dev/null || { echo "cargo-xwin missing: cargo install --locked cargo-xwin"; exit 1; }
  nsis=${ALWY_NSIS:-$HOME/.local/opt/nsis-3.13}
  "$nsis/Bin/makensis" -HDRINFO 2>/dev/null | grep -q 'NSIS_MAX_STRLEN=1024' ||
    { echo "No matching NSIS in $nsis: see scripts/nsis/makensis"; exit 1; }
  # The Tauri CLI asks pkg-config for Linux's tray library even for Windows: a stand-in.
  pc=$(mktemp -d)
  printf 'Name: ayatana-appindicator3-0.1\nDescription: stand-in\nVersion: 0.5.0\nLibs: -L/usr/lib\n' > "$pc/ayatana-appindicator3-0.1.pc"
fi

# The interface once for all platforms (the same files); the builds then skip it.
pnpm build
skip_ui='"build":{"beforeBuildCommand":""}'

# Each platform compiles in its own folder: Cargo locks a build folder, so sharing one would
# make the builds wait for each other. Linux keeps the usual target/ (shared with dev builds).
split="$root/target/split"

build_android() {
  rm -rf src-tauri/gen/android/app/build/outputs/apk
  CARGO_TARGET_DIR="$split/android" ALWY_UPDATE_FEED="$RELEASE_URL/latest-android.json" \
    pnpm tauri android build --apk --target aarch64 --config "{\"version\":\"$version\",$skip_ui}"
  cp "$(find src-tauri/gen/android/app/build/outputs/apk/universal -name '*.apk' | head -1)" "$out/alwychat-$version-$commit.apk"
}

build_linux() {
  ALWY_UPDATE_FEED="$RELEASE_URL/latest-linux.json" pnpm tauri build --bundles appimage \
    --config "{\"version\":\"$version\",\"bundle\":{\"createUpdaterArtifacts\":true},$skip_ui}"
  local b=../target/release/bundle/appimage
  cp "$(ls $b/*.AppImage | head -1)" "$out/alwychat-$version-$commit.AppImage"
  cp "$(ls $b/*.AppImage.sig | head -1)" "$out/alwychat-$version-$commit.AppImage.sig"
}

build_windows() {
  rm -rf "$split/windows/x86_64-pc-windows-msvc/release/bundle/nsis"
  CARGO_TARGET_DIR="$split/windows" PATH="$root/scripts/nsis:$PATH" PKG_CONFIG_PATH="$pc" XWIN_ACCEPT_LICENSE=1 \
    ALWY_UPDATE_FEED="$RELEASE_URL/latest-windows.json" \
    pnpm tauri build --runner cargo-xwin --target x86_64-pc-windows-msvc --bundles nsis \
    --config "{\"version\":\"$version\",\"bundle\":{\"createUpdaterArtifacts\":true},$skip_ui}"
  local b=$split/windows/x86_64-pc-windows-msvc/release/bundle/nsis
  cp "$(ls $b/*-setup.exe | head -1)" "$out/AlwyChat-$version-$commit-setup.exe"
  cp "$(ls $b/*-setup.exe.sig | head -1)" "$out/AlwyChat-$version-$commit-setup.exe.sig"
}

platforms=()
$android && platforms+=(android)
$linux && platforms+=(linux)
$windows && platforms+=(windows)
if [ ${#platforms[@]} -eq 1 ]; then
  "build_${platforms[0]}"
else
  # Side by side: each build's last step (link-time optimisation) uses a single core.
  logs=$(mktemp -d)
  export CARGO_TERM_PROGRESS_WHEN=always CARGO_TERM_PROGRESS_WIDTH=100
  declare -A pids
  for p in "${platforms[@]}"; do
    ( set -e; t=$SECONDS; "build_$p"; echo $((SECONDS - t)) > "$logs/$p.time" ) > "$logs/$p.log" 2>&1 &
    pids[$p]=$!
  done
  echo "Building ${platforms[*]} at the same time. Follow one with: tail -f $logs/<platform>.log"
  # A countdown per platform, redrawn in place (one short line each, cut to the terminal's
  # width so nothing wraps). Crates 1–1055 take a minute; nearly all the time is the last one
  # (link-time optimisation on one core), so the estimate is that platform's previous build
  # time (saved in $times; ~10 min until there is one). Cargo's crate counter (written to the
  # log thanks to CARGO_TERM_PROGRESS_WHEN) says which step it's in.
  times="$root/target/split/build-times"
  mkdir -p "$(dirname "$times")"; touch "$times"
  declare -A started
  for p in "${platforms[@]}"; do started[$p]=$SECONDS; done
  clock() { printf '%d:%02d' $(($1 / 60)) $(($1 % 60)); }
  stage() {
    local log=$logs/$1.log elapsed=$((SECONDS - started[$1])) expected left step last n m
    if ! kill -0 "${pids[$1]}" 2>/dev/null; then echo "done in $(clock $elapsed)"; return; fi
    expected=$(sed -n "s/^$1 //p" "$times" | tail -n 1)
    [ -n "$expected" ] && [ "$expected" -gt 0 ] || expected=600
    if grep -q 'Finished `release`' "$log"; then step="packaging"
    else
      last=$(tail -c 20000 "$log" | tr '\r' '\n' | grep -oE '[0-9]+/[0-9]+: [A-Za-z0-9_-]+' | tail -n 1)
      if [ -z "$last" ]; then step="starting"
      else
        n=${last%%/*}; m=${last#*/}; m=${m%%:*}
        # The app itself counts twice (library + program): those are the slow last step.
        if [ "$n" -ge $((m - 2)) ]; then step="optimising"; else step="crates $n/$m"; fi
      fi
    fi
    left=$((expected - elapsed))
    if [ $left -gt 0 ]; then echo "~$(clock $left) left  $step"
    else echo "almost done (+$(clock $((-left))))  $step"; fi
  }
  if [ -t 1 ]; then
    first=true
    while :; do
      running=false
      for p in "${platforms[@]}"; do kill -0 "${pids[$p]}" 2>/dev/null && running=true; done
      cols=$(tput cols 2>/dev/null || echo 80)
      $first || printf '\033[%dA' "${#platforms[@]}"
      first=false
      for p in "${platforms[@]}"; do
        line=$(printf '%-8s %s' "$p" "$(stage "$p")")
        printf '\r\033[K%s\n' "${line:0:$((cols - 1))}"
      done
      $running || break
      sleep 2
    done
  fi
  failed=()
  for p in "${platforms[@]}"; do
    if wait "${pids[$p]}"; then
      echo "$p: built"
      # Remembered for the next run's percentage (only finished builds count).
      [ -n "${times:-}" ] && [ -s "$logs/$p.time" ] && echo "$p $(cat "$logs/$p.time")" >> "$times"
    else
      echo "$p: FAILED. End of its log ($logs/$p.log):"
      tail -n 25 "$logs/$p.log"
      failed+=("$p")
    fi
  done
  [ ${#failed[@]} -eq 0 ] || { echo "Nothing published (failed: ${failed[*]})."; exit 1; }
fi
if $made_props; then rm -f "$props"; fi
if $test; then
  cp "$out/alwychat-$version-$commit.apk" "$HOME/alwychat-test.apk"
  echo "Test build: ~/alwychat-test.apk (not published)"
  exit 0
fi

# Previous builds of the platforms just built go to archive/.
mkdir -p "$RELEASE_DIR/archive"
for ext in apk AppImage AppImage.sig exe; do
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
for exe in out.glob("*.exe"):
    entry = {"signature": (out / f"{exe.name}.sig").read_text().strip(), "url": f"{url}/{exe.name}"}
    write("windows", {"windows-x86_64-nsis": entry, "windows-x86_64": entry})
PY
ls -la "$RELEASE_DIR"
