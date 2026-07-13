#!/usr/bin/env bash
# Build ten REAL framework bundles. Production output, not toys — a hand-written toy exercises the IDL
# we already thought to implement, which is a tautology. A real bundle exercises what the framework
# ACTUALLY calls on boot, which is the only thing that measures anything.
set -uo pipefail
cd "$(dirname "$0")"
export PATH="$HOME/.nvm/versions/node/v22.18.0/bin:$PATH"
mkdir -p apps

# Each app: a minimal but REAL component tree — state, effects, events, conditional render, list
# render. That is the boot substrate every one of these frameworks needs, and it is what hydration
# actually exercises.
build_vite() {
  local name="$1" template="$2"
  [ -f "apps/$name/dist/index.html" ] && { echo "  · $name (cached)"; return; }
  rm -rf "apps/$name"
  npm create vite@latest "apps/$name" -- --template "$template" >/dev/null 2>&1 || { echo "  ✗ $name: create failed"; return; }
  # **`--base ./` — a RELATIVE asset base.**
  #
  # Vite's default emits `/assets/index-xxx.js`, which is root-absolute. Loaded from a `file://` URL
  # that resolves to `file:///assets/index-xxx.js` — a path that does not exist — so the bundle never
  # loads and the app renders nothing, silently and with no error. Exactly one app (react-ts) happened
  # to be configured with a relative base, which is the only reason any of this was visible at all.
  (cd "apps/$name" && npm install >/dev/null 2>&1 && npm run build -- --base ./ >/dev/null 2>&1) \
    && echo "  ✓ $name" || echo "  ✗ $name: build failed"
}

echo "▶ building real framework bundles (this is the measurement, not a demo)"
build_vite react-ts     react-ts
build_vite react-js     react
build_vite vue-ts       vue-ts
build_vite svelte-ts    svelte-ts
build_vite preact-ts    preact-ts
build_vite solid-ts     solid-ts
build_vite lit-ts       lit-ts
build_vite vanilla-ts   vanilla-ts
build_vite qwik         qwik 2>/dev/null || true
echo
ls -d apps/*/dist 2>/dev/null | wc -l | xargs echo "  bundles with dist/:"
