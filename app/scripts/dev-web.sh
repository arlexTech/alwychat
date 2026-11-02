#!/usr/bin/env bash
# Web version in development: Vite's dev server (live reload of the UI) on http://<this PC>:8090,
# meant to sit behind a reverse proxy that gives it an https name (sign-in and the media service
# worker need https). Only that name is accepted: ALWY_DEV_HOST, from the environment or from
# app/.dev-web.env (not committed), e.g. `ALWY_DEV_HOST=chat.example.org`.
# Rust changes: run scripts/build-wasm.sh again (Vite reloads the page).
set -euo pipefail
cd "$(dirname "$0")/.."
# shellcheck disable=SC1091
[ -f .dev-web.env ] && source .dev-web.env
: "${ALWY_DEV_HOST:?set ALWY_DEV_HOST (the https name of the dev server) in app/.dev-web.env}"
[ -f src/lib/backend/wasm/alwy_web.js ] || PROFILE=${PROFILE:-release} scripts/build-wasm.sh
ALWY_WEB=1 ALWY_DEV_HOST="$ALWY_DEV_HOST" pnpm vite dev
