#!/usr/bin/env bash
# ── Fetch the pinned SpiderMonkey prebuilt and FAIL unless it is the archive we pinned. ──
#
# Why this exists instead of MOZJS_ATTESTATION=strict alone: mozjs_sys's build.rs runs `gh attestation
# verify` through Command::output(), which is Ok on a non-zero exit. A tampered archive therefore links
# anyway; the only thing that ever stopped it was a missing `gh`. CONSTITUTION I2 forbids patching the
# sanctioned dep, so the check lives here: the archive is verified BEFORE cargo sees it, and
# .cargo/config.toml points MOZJS_ARCHIVE at the verified copy (mozjs_sys then never takes its own
# download/attestation path).
#
# Both checks must pass when gh is available:
#   1. sha256 equals the pin below. Offline, always. Bumping mozjs_sys means bumping this pin.
#   2. `gh attestation verify` when gh is authenticated (CI, or a dev who ran `gh auth login`).
#      Without auth we print a loud warning and rely on (1) alone.
#
#   usage: scripts/mozjs-fetch.sh     (prints the verified archive path on stdout)
#   env:   MOZJS_FETCH_URL=<url>      test hook: download from here instead of the release (mutation tests)
set -euo pipefail
cd "$(dirname "$0")/.."

# The pin. Only the linux x86_64 prebuilt is pinned; other hosts must set MOZJS_ARCHIVE themselves
# (cargo's [env] never overrides a variable already in the environment).
MOZJS_SYS_VER="140.17.0-0-lts"
PIN_x86_64_unknown_linux_gnu="69b2920f8787d202f3254bc6546b623c2f90e1224dab8a4f794b748f02dfa3a1"

die() { printf '✗ mozjs-fetch: %s\n' "$1" >&2; exit 1; }

TRIPLE=$(rustc -vV | sed -n 's/^host: //p')
PIN_VAR="PIN_${TRIPLE//-/_}"
PIN="${!PIN_VAR:-}"
[ -n "$PIN" ] || die "no pinned archive for host $TRIPLE; set MOZJS_ARCHIVE to a verified archive yourself"

# The version cargo actually resolved must be the one the pin was taken from.
LOCK_VER=$(awk '$0=="name = \"mozjs_sys\""{getline; sub(/^version = "/,""); sub(/"$/,""); print; exit}' Cargo.lock)
[ "$LOCK_VER" = "$MOZJS_SYS_VER" ] || die "Cargo.lock has mozjs_sys $LOCK_VER, pin is for $MOZJS_SYS_VER: update scripts/mozjs-fetch.sh"

ARCH="libmozjs-$TRIPLE.tar.gz"
URL="${MOZJS_FETCH_URL:-https://github.com/servo/mozjs/releases/download/mozjs-sys-v$MOZJS_SYS_VER/$ARCH}"
DEST_DIR="target/mozjs-verified"
DEST="$DEST_DIR/$ARCH"
mkdir -p "$DEST_DIR"

sha_of() { sha256sum "$1" | cut -d' ' -f1; }

if [ -f "$DEST" ] && [ "$(sha_of "$DEST")" = "$PIN" ]; then
  echo "cached, sha256 pin ok: $DEST" >&2
else
  TMP=$(mktemp "$DEST_DIR/fetch.XXXXXX")
  trap 'rm -f "$TMP"' EXIT
  curl -fsSL -o "$TMP" "$URL" || die "download failed: $URL"

  GOT=$(sha_of "$TMP")
  [ "$GOT" = "$PIN" ] || die "sha256 mismatch for $ARCH: got $GOT, pinned $PIN (refusing to link it)"

  if gh auth status >/dev/null 2>&1; then
    gh attestation verify "$TMP" -R servo/mozjs >/dev/null 2>&1 \
      || die "GitHub artifact attestation FAILED for $ARCH (refusing to link it)"
    echo "attestation ok: $ARCH" >&2
  else
    echo "warning: gh is not authenticated; attestation NOT checked for $ARCH, sha256 pin only" >&2
  fi

  mv -f "$TMP" "$DEST"
  trap - EXIT
fi

# mozjs_sys reuses an archive it already holds in OUT_DIR and extracts its libs there without re-checking
# them, so a build dir from an earlier, unverified download would keep linking. Delete those build dirs;
# cargo re-runs the build script and extracts from the verified archive.
for stale in target/*/build/mozjs_sys-*/out/"$ARCH"; do
  [ -e "$stale" ] || continue
  if [ "$(sha_of "$stale")" != "$PIN" ]; then
    rm -rf "$(dirname "$(dirname "$stale")")"
    echo "removed build dir with unverified archive: $(dirname "$(dirname "$stale")")" >&2
  fi
done

echo "$DEST"
