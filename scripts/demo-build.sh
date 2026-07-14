#!/usr/bin/env bash
# Build the in-browser demo: engine → wasm → demo/www (a static, self-contained site).
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"
rustup target add wasm32-unknown-unknown >/dev/null 2>&1 || true
command -v wasm-bindgen >/dev/null || cargo install wasm-bindgen-cli --locked

echo "── building the engine for wasm32 (release)"
cargo build -p manuk-demo --release --target wasm32-unknown-unknown

echo "── generating JS bindings"
wasm-bindgen target/wasm32-unknown-unknown/release/manuk_demo.wasm \
  --out-dir demo/www --target web --no-typescript

ls -lh demo/www/manuk_demo_bg.wasm | awk '{print "   wasm: "$5}'
echo "── demo/www is a complete static site (index.html + wasm + snapshots)"
