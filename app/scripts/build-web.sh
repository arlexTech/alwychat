#!/usr/bin/env bash
# Builds the web version into app/build-web/: the core compiled to WebAssembly
# (crates/alwy-web) next to the same UI as the apps. Static files: serve them from any web server.
# Needs the wasm32-unknown-unknown Rust target and wasm-bindgen-cli at the crate's version.
set -euo pipefail
cd "$(dirname "$0")/.."
root=..

profile=${PROFILE:-web}
flag=$([ "$profile" = dev ] && echo "" || echo "--profile $profile")
RUSTFLAGS='--cfg getrandom_backend="wasm_js"' \
  cargo build -p alwy-web --target wasm32-unknown-unknown $flag --manifest-path "$root/Cargo.toml"

rm -rf src/lib/backend/wasm
wasm-bindgen --target web --out-dir src/lib/backend/wasm \
  "$root/target/wasm32-unknown-unknown/$([ "$profile" = dev ] && echo debug || echo "$profile")/alwy_web.wasm"
if command -v wasm-opt >/dev/null && [ "$profile" != dev ]; then
  wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int \
    -o src/lib/backend/wasm/alwy_web_bg.wasm src/lib/backend/wasm/alwy_web_bg.wasm
fi

ALWY_WEB=1 pnpm vite build
echo "web version in app/build-web/"
