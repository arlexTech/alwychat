#!/usr/bin/env bash
# Builds the web version into app/build-web/: the core as WebAssembly next to the same UI as
# the apps. Static files: serve them from any web server (or the Docker image, docker/web/).
set -euo pipefail
cd "$(dirname "$0")/.."
scripts/build-wasm.sh
ALWY_WEB=1 pnpm vite build
echo "web version in app/build-web/"
