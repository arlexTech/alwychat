#!/usr/bin/env bash
# Web version in development: Vite's dev server (live reload of the UI) over https on
# https://chat.example.org:8443, with a self-signed certificate (the browser warns once).
# Sign-in needs https, hence the name: point it to this machine in /etc/hosts:
#   127.0.0.1 chat.example.org
# Rust changes: run scripts/build-wasm.sh again (Vite reloads the page).
set -euo pipefail
cd "$(dirname "$0")/.."
host=${ALWY_DEV_HOST:-chat.example.org}
cert=.dev-cert
if [ ! -f "$cert/$host.crt" ]; then
  mkdir -p "$cert"
  openssl req -x509 -newkey rsa:2048 -nodes -days 825 -subj "/CN=$host" \
    -addext "subjectAltName=DNS:$host" \
    -keyout "$cert/$host.key" -out "$cert/$host.crt" 2>/dev/null
fi
[ -f src/lib/backend/wasm/alwy_web.js ] || PROFILE=${PROFILE:-release} scripts/build-wasm.sh
ALWY_WEB=1 ALWY_DEV_HOST="$host" ALWY_DEV_CERT="$cert" pnpm vite dev
