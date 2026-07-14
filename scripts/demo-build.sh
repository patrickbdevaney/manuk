#!/usr/bin/env bash
# Build the in-browser demo: engine → wasm → demo/www (a static, self-contained site).
set -euo pipefail
cd "$(dirname "$0")/.."

# Snapshot the tree before doing anything. PROCESS #37: `git checkout` is a delete, and I have
# run it on uncommitted work twice. This makes that recoverable. It is free when nothing has changed.
./scripts/snap.sh || true
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


# ── G_DEMO_LIVE. A build that cannot prove the engine PAINTED is not a build of a demo; it is a build of
# a web page. See scripts/demo-verify.py for why this is a CDP probe and not a screenshot.
echo "── G_DEMO_LIVE: driving the demo in a real browser"
python3 scripts/demo-verify.py
