#!/bin/bash
# Build signing for all your projects, each with its own keys. As root from your own account:
#
#   cargo build --release --manifest-path scripts/signing/tauri-update-sign/Cargo.toml
#   sudo scripts/signing/setup.sh              install / update the signer only
#   sudo scripts/signing/setup.sh PROJECT      the same, then add or replace PROJECT's keys
#
# Read it first: it runs as root. It
#  - creates the system account "buildsign" (no login, home /var/lib/build-sign);
#  - installs root-owned copies of build-sign, apksigner.jar (from your Android SDK) and
#    tauri-update-sign, so nothing your account can change ever runs as buildsign;
#  - for PROJECT: copies its Android keystore and/or Tauri updater key into
#    /etc/build-sign/PROJECT (only buildsign can read them) and asks their passwords, kept
#    there too; Enter on a question keeps what that project already has;
#  - lets YOUR account (only) run /usr/local/bin/build-sign as buildsign without a password
#    (/etc/sudoers.d/build-sign, checked with visudo).
# A build then signs with: sudo -n -u buildsign /usr/local/bin/build-sign PROJECT apk|tauri-update …
# Remove one project: sudo rm -r /etc/build-sign/PROJECT
# Remove everything: sudo rm -r /etc/sudoers.d/build-sign /etc/build-sign /usr/local/bin/build-sign \
#                      /usr/local/lib/build-sign /var/lib/build-sign && sudo userdel buildsign
set -euo pipefail
umask 077
[ "$(id -u)" -eq 0 ] || { echo "run it with sudo"; exit 1; }
user=${SUDO_USER:-}
[ -n "$user" ] && [ "$user" != root ] || { echo "run it with sudo from your own account"; exit 1; }
project=${1:-}
[ -z "$project" ] || [[ "$project" =~ ^[a-z0-9][a-z0-9_-]{0,63}$ ]] ||
  { echo "project names: lowercase letters, digits, - and _"; exit 1; }
here=$(cd "$(dirname "$0")" && pwd)
userhome=$(getent passwd "$user" | cut -d: -f6)

ask() { # variable prompt default
  local v
  read -rp "$2${3:+ [$3]}: " v
  printf -v "$1" '%s' "${v:-$3}"
}

# --- The account and the programs it runs (all root-owned) ---
signer=$here/tauri-update-sign/target/release/tauri-update-sign
[ -x "$signer" ] || { echo "build tauri-update-sign first (see the top of $0)"; exit 1; }
command -v java >/dev/null || { echo "java is missing"; exit 1; }
jar=$(ls "$userhome"/Android/Sdk/build-tools/*/lib/apksigner.jar 2>/dev/null | sort -V | tail -n 1)
ask jar "apksigner.jar (Android SDK)" "$jar"
[ -f "$jar" ] || { echo "no apksigner.jar there"; exit 1; }

id buildsign >/dev/null 2>&1 ||
  useradd --system --user-group --home-dir /var/lib/build-sign --no-create-home \
    --shell /usr/bin/nologin --comment "Build signing" buildsign
install -d -o root -g root -m 755 /usr/local/lib/build-sign
install -o root -g root -m 644 "$jar" /usr/local/lib/build-sign/apksigner.jar
install -o root -g root -m 755 "$signer" /usr/local/lib/build-sign/tauri-update-sign
install -o root -g root -m 755 "$here/build-sign" /usr/local/bin/build-sign
# Its home: work files (private) and the log (readable by everyone).
install -d -o buildsign -g buildsign -m 711 /var/lib/build-sign
[ -f /var/lib/build-sign/sign.log ] || install -o buildsign -g buildsign -m 644 /dev/null /var/lib/build-sign/sign.log
install -d -o buildsign -g buildsign -m 700 /etc/build-sign

rule=$(mktemp)
cat > "$rule" <<EOF
# Build signing (scripts/signing/setup.sh): $user may run the signer as buildsign, nothing else.
Defaults!/usr/local/bin/build-sign env_reset
$user ALL=(buildsign) NOPASSWD: /usr/local/bin/build-sign
EOF
visudo -cf "$rule" >/dev/null
install -o root -g root -m 440 "$rule" /etc/sudoers.d/build-sign
rm -f "$rule"
echo "Signer installed."
[ -n "$project" ] || exit 0

# --- One project's keys ---
keys=/etc/build-sign/$project
install -d -o buildsign -g buildsign -m 700 "$keys"
put() { install -o buildsign -g buildsign -m 600 "$1" "$keys/$2"; }
text() { printf '%s' "$2" > "$keys/$1"; chown buildsign:buildsign "$keys/$1"; chmod 600 "$keys/$1"; }
secret() { local p; read -rsp "$2: " p; echo; text "$1" "$p"; }
as_signer() { sudo -u buildsign -- "$@"; }
kept=()

echo "Keys for \"$project\" (Enter = keep / none)."
ask keystore "Android keystore file" ""
if [ -n "$keystore" ]; then
  keystore=${keystore/#\~/$userhome}
  [ -f "$keystore" ] || { echo "no keystore there"; exit 1; }
  ask alias "Its key alias" ""
  [[ "$alias" =~ ^[A-Za-z0-9._-]+$ ]] || { echo "unexpected alias"; exit 1; }
  put "$keystore" android.jks
  text android.alias "$alias"
  for try in 1 2 3; do
    secret android.pass "Keystore password"
    if as_signer keytool -list -keystore "$keys/android.jks" -alias "$alias" -storepass:file "$keys/android.pass" >/dev/null 2>&1; then
      echo "Android key: OK"; break
    fi
    echo "Wrong password (or alias)."
    [ $try -lt 3 ] || { rm -f "$keys"/android.*; exit 1; }
  done
  kept+=("$keystore")
fi

ask updater "Tauri updater key file (TAURI_SIGNING_PRIVATE_KEY's file)" ""
if [ -n "$updater" ]; then
  updater=${updater/#\~/$userhome}
  [ -f "$updater" ] || { echo "no key there"; exit 1; }
  put "$updater" tauri-update.key
  for try in 1 2 3; do
    secret tauri-update.pass "Its password (empty if none)"
    if echo test | as_signer /usr/local/lib/build-sign/tauri-update-sign sign "$keys/tauri-update.key" "$keys/tauri-update.pass" test >/dev/null; then
      echo "Tauri updater key: OK"; break
    fi
    [ $try -lt 3 ] || { rm -f "$keys"/tauri-update.*; exit 1; }
  done
  kept+=("$updater")
fi

echo
echo "\"$project\" has: $(cd "$keys" && ls | sed -n 's/^android.jks$/Android key/p; s/^tauri-update.key$/Tauri updater key/p' | paste -sd, -)"
if [ ${#kept[@]} -gt 0 ]; then
  echo "The key files are still in your account too (${kept[*]}): once you have a backup"
  echo "somewhere safe (offline), delete them there, or anything running as you can still copy"
  echo "them. Their passwords are no longer anywhere you can read."
fi
