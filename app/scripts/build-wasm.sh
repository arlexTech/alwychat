#!/usr/bin/env bash
# Compiles the core for the browser (crates/alwy-web) into src/lib/backend/wasm/, where the UI
# picks it up. PROFILE: web (default, small), release, or dev (fast to build, slow to run).
# Needs the wasm32-unknown-unknown Rust target and wasm-bindgen-cli at the crate's version.
set -euo pipefail
cd "$(dirname "$0")/.."
root=..

profile=${PROFILE:-web}
flag=$([ "$profile" = dev ] && echo "" || echo "--profile $profile")
dir=$([ "$profile" = dev ] && echo debug || echo "$profile")
RUSTFLAGS='--cfg getrandom_backend="wasm_js"' \
  cargo build -p alwy-web --target wasm32-unknown-unknown $flag --manifest-path "$root/Cargo.toml"

rm -rf src/lib/backend/wasm
wasm-bindgen --target web --out-dir src/lib/backend/wasm \
  "$root/target/wasm32-unknown-unknown/$dir/alwy_web.wasm"
if command -v wasm-opt >/dev/null && [ "$profile" != dev ]; then
  wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int \
    -o src/lib/backend/wasm/alwy_web_bg.wasm src/lib/backend/wasm/alwy_web_bg.wasm
fi
