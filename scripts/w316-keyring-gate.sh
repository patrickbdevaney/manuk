#!/bin/sh
# W3.16.a keyring gate: runs the ignored keyring tests of mcp/tests/secrets.rs inside a PRIVATE D-Bus session
# with its own gnome-keyring (isolated XDG dirs), so the user's login keyring is never touched.
# Usage: scripts/w316-keyring-gate.sh   (from the repo root; needs secret-tool, gnome-keyring, dbus-run-session)
set -eu
SECRET='Tr0ub4dor3-zq91x'
ITEM='w3a16a-pw'
ROOT=$(mktemp -d)
export XDG_DATA_HOME="$ROOT/data" XDG_CONFIG_HOME="$ROOT/config" XDG_RUNTIME_DIR="$ROOT/run"
mkdir -p "$XDG_DATA_HOME" "$XDG_CONFIG_HOME" "$XDG_RUNTIME_DIR" && chmod 700 "$XDG_RUNTIME_DIR"
exec dbus-run-session -- sh -c '
  set -eu
  printf "isolated-login-pw" | gnome-keyring-daemon --login --components=secrets >/dev/null 2>&1
  eval "$(printf "" | gnome-keyring-daemon --start --components=secrets 2>/dev/null)"
  printf "%s" "$1" | secret-tool store --label=manuk-w3a16a service manuk account "$2"
  cargo test -p manuk-mcp --test secrets keyring -- --ignored --test-threads=1
' sh "$SECRET" "$ITEM"
