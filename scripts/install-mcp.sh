#!/usr/bin/env sh
# SHIP.2: one-line install of manuk-mcp (Linux x86_64, macOS arm64).
#   MANUK_BASE_URL=<dir or URL holding the release files> sh install-mcp.sh
# Downloads manuk-mcp-<os>-<arch>.tar.gz + .sha256 from MANUK_BASE_URL, verifies the checksum BEFORE unpacking,
# installs to ${MANUK_PREFIX:-$HOME/.local}/bin, and (MANUK_INSTALL_CHROME=1) fetches the pinned Chrome for Testing.
set -eu
BASE=${MANUK_BASE_URL:?set MANUK_BASE_URL to the release download location (a URL, or a local directory)}
PREFIX=${MANUK_PREFIX:-$HOME/.local}
case "$(uname -s)-$(uname -m)" in
  Linux-x86_64) NAME=manuk-mcp-linux-x86_64; CFT=linux64 ;;
  Darwin-arm64) NAME=manuk-mcp-macos-arm64; CFT=mac-arm64 ;;
  *) echo "install-mcp: unsupported platform $(uname -s)-$(uname -m) (Linux x86_64, macOS arm64)" >&2; exit 1 ;;
esac
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
fetch() { case "$BASE" in http*) curl -fsSL "$BASE/$1" -o "$T/$1" ;; *) cp "$BASE/$1" "$T/$1" ;; esac; }
fetch "$NAME.tar.gz"; fetch "$NAME.tar.gz.sha256"
want=$(cut -d' ' -f1 "$T/$NAME.tar.gz.sha256")
if command -v sha256sum >/dev/null; then got=$(sha256sum "$T/$NAME.tar.gz" | cut -d' ' -f1); else got=$(shasum -a 256 "$T/$NAME.tar.gz" | cut -d' ' -f1); fi
[ "$want" = "$got" ] || { echo "install-mcp: CHECKSUM MISMATCH (want $want, got $got); nothing installed" >&2; exit 1; }
tar -C "$T" -xzf "$T/$NAME.tar.gz"
mkdir -p "$PREFIX/bin"; install -m 755 "$T/$NAME/manuk-mcp" "$PREFIX/bin/manuk-mcp"
PIN=$(tr -d '[:space:]' < "$T/$NAME/CHROME_PIN")
if [ "${MANUK_INSTALL_CHROME:-0}" = 1 ]; then
  D="$HOME/.local/chrome-cft/$PIN"; mkdir -p "$D"
  curl -fsSL "https://storage.googleapis.com/chrome-for-testing-public/$PIN/$CFT/chrome-$CFT.zip" -o "$T/cft.zip"
  unzip -q -o "$T/cft.zip" -d "$D"
fi
echo "installed $PREFIX/bin/manuk-mcp (sha256 verified)"
echo "Chrome for Testing $PIN is required at ~/.local/chrome-cft/$PIN/ (rerun with MANUK_INSTALL_CHROME=1) or set MANUK_CHROME"
echo "register:  claude mcp add manuk -- $PREFIX/bin/manuk-mcp"
