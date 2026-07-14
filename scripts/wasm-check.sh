#!/usr/bin/env bash
# Prove the render pipeline (minus JS) still compiles to wasm32-unknown-unknown — the in-browser demo's
# target. Run on the EPOCH cadence; a break here means a dependency or a `usize` assumption regressed the
# 32-bit portability the demo depends on.
#
# NOT in the verify wall by default: the wasm target build is heavy and the demo is a separate lane
# (per the demo directive — its own CI, never gating a tick). This is the manual/CI checkpoint.
set -euo pipefail
cd "$(dirname "$0")/.."
rustup target add wasm32-unknown-unknown >/dev/null 2>&1 || true
echo "── wasm32 render-pipeline build (dom · css+stylo · layout · paint · html · text) ──"
for c in manuk-dom manuk-html manuk-text manuk-layout manuk-paint; do
  printf '  %-14s ' "$c"
  cargo build -q -p "$c" --target wasm32-unknown-unknown 2>/dev/null && echo "✓" || { echo "✗ FAILS ON WASM"; exit 1; }
done
printf '  %-14s ' "manuk-css+stylo"
cargo build -q -p manuk-css --features stylo --target wasm32-unknown-unknown 2>/dev/null && echo "✓" || { echo "✗ STYLO FAILS ON WASM"; exit 1; }
echo "the whole render pipeline compiles to wasm — the in-browser demo is feasible."
