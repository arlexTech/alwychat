#!/usr/bin/env bash
# The app bundle (.aab) to upload to Google Play (docs/play-store.md has the whole checklist).
#
#   scripts/play-build.sh [VERSION]      → ~/alwychat-play-<version>.aab
#
# What differs from the builds published elsewhere:
#  - it never updates itself (no update feed; Play does it) and doesn't ask for the permission
#    to install apps (ALWY_PLAY, see gen/android/app/build.gradle.kts);
#  - the sign-in screen suggests PLAY_DEFAULT_SERVER from .release.env (matrix.org when unset),
#    not the server local builds are prefilled with; PLAY_PUSH_GATEWAY (optional) is the push
#    gateway for servers that don't name their own;
#  - Play wants a bundle, not an APK, and a version above every one uploaded before.
# VERSION defaults to the one after GitHub's beta (0.2.<n+1>); Play's version code comes from it
# (major·1000000 + minor·1000 + patch).
#
# Signing: with `sudo scripts/signing/setup.sh` done (it installs the signer's "aab" mode), the
# buildsign account signs the bundle with the project's Android key, which Play then knows as
# your upload key. Without it the bundle is left unsigned, with the command to sign it.
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
# shellcheck disable=SC1091
[ -f .release.env ] && source .release.env

version=${1:-}
if [ -z "$version" ]; then
  beta=$(curl -fsSL --max-time 10 https://github.com/arlexTech/alwychat/releases/download/beta/latest-android.json |
    python3 -c "import json,sys; print(json.load(sys.stdin)['version'])") ||
    { echo "couldn't read GitHub's beta version: give one (scripts/play-build.sh 0.2.50)"; exit 1; }
  version=$(python3 -c "import sys; a=sys.argv[1].split('.'); a[-1]=str(int(a[-1])+1); print('.'.join(a))" "$beta")
fi
[[ "$version" =~ ^[0-9]+\.[0-9]{1,3}\.[0-9]{1,3}$ ]] || { echo "version like 0.2.50"; exit 1; }
commit=$(git rev-parse --short HEAD)
echo "Building AlwyChat $version ($commit) for Google Play"
[ -z "$(git status --porcelain)" ] || echo "note: uncommitted changes are included"

props=app/src-tauri/gen/android/keystore.properties
[ ! -f "$props" ] || { echo "$props exists: remove it (the bundle is signed after the build)"; exit 1; }
[ -f app/src-tauri/gen/android/app/google-services.json ] ||
  { echo "no google-services.json: this build would get notifications only while it runs"; exit 1; }

cd app
export ALWY_PLAY=1 VITE_DEFAULT_MATRIX_SERVER="${PLAY_DEFAULT_SERVER:-}" VITE_GOOGLE_EMOJI=0
unset ALWY_UPDATE_FEED ALWY_ANDROID_ID ALWY_PUSH_APP_ID ALWY_PUSH_GATEWAY
# A public push gateway for servers that don't name one (PLAY_PUSH_GATEWAY in .release.env).
if [ -n "${PLAY_PUSH_GATEWAY:-}" ]; then export ALWY_PUSH_GATEWAY="$PLAY_PUSH_GATEWAY"; fi
rm -rf src-tauri/gen/android/app/build/outputs/bundle
# Its own build folder: the update feed is compiled in, so the other builds' objects don't fit.
CARGO_TARGET_DIR="$root/target/split/play" \
  pnpm tauri android build --aab --target aarch64 --config "{\"version\":\"$version\"}"
aab=$(find src-tauri/gen/android/app/build/outputs/bundle -name '*.aab' | head -1)
[ -n "$aab" ] || { echo "no bundle was built"; exit 1; }

out="$HOME/alwychat-play-$version.aab"
signer=/usr/local/bin/build-sign
project=${SIGN_PROJECT:-alwychat}
if [ -x "$signer" ] && grep -q ' aab)' "$signer" && sudo -n -u buildsign -l "$signer" >/dev/null 2>&1; then
  sudo -n -u buildsign "$signer" "$project" aab < "$aab" > "$out.part"
  mv "$out.part" "$out"
  echo "Signed bundle for Google Play: $out"
else
  cp "$aab" "$out"
  echo "UNSIGNED bundle: $out"
  echo "Sign it before uploading: update the signer once (sudo scripts/signing/setup.sh), then"
  echo "  sudo -n -u buildsign $signer $project aab < $out > $out.signed && mv $out.signed $out"
fi
