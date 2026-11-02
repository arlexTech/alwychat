#!/usr/bin/env bash
# Web version in development: Vite's dev server (live reload of the UI) on http://<this PC>:8090,
# meant to sit behind a reverse proxy that serves it as https://chat.example.org (sign-in and the
# media service worker need https). Only the proxy's name is accepted (ALWY_DEV_HOST).
# Rust changes: run scripts/build-wasm.sh again (Vite reloads the page).
set -euo pipefail
cd "$(dirname "$0")/.."
[ -f src/lib/backend/wasm/alwy_web.js ] || PROFILE=${PROFILE:-release} scripts/build-wasm.sh
ALWY_WEB=1 ALWY_DEV_HOST="${ALWY_DEV_HOST:-chat.example.org}" pnpm vite dev
