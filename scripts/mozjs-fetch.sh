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
#   2b. Without gh auth, `sigstore verify github --repository servo/mozjs` on the bundle from the public
#      attestation API (SIGSTORE=<cli>, `sigstore` on PATH, or python3 -m sigstore).
#      With neither we print a loud warning and rely on (1) alone.
#
#   usage: scripts/mozjs-fetch.sh     (prints the verified archive path on stdout)
#   env:   MOZJS_FETCH_URL=<url>      test hook: download from here instead of the release (mutation tests)
#          MOZJS_TRIPLE=<triple>      fetch/verify the archive for this triple instead of the rustc host
#                                     (cross-OS lanes; the pins below cover linux, mac arm64 and windows msvc)
#          MOZJS_ATTEST_URL=<url>    test hook: fetch the attestation list from here (mutation tests)
#          GITHUB_ENV                 when set (GitHub Actions), MOZJS_ARCHIVE=<abs path> is appended to it, so
#                                     later steps of mac/windows lanes (where [env] does not apply) link the
#                                     verified copy
set -euo pipefail
cd "$(dirname "$0")/.."

# The pins: linux x86_64, macOS arm64 and Windows msvc prebuilts. Other hosts must set MOZJS_ARCHIVE
# themselves (cargo's [env] never overrides a variable already in the environment). The mac/windows shas
# come from unauthenticated downloads (see docs/sprint/R1.5.a.a-attestation.txt for the linux provenance).
MOZJS_SYS_VER="140.17.0-0-lts"
PIN_x86_64_unknown_linux_gnu="69b2920f8787d202f3254bc6546b623c2f90e1224dab8a4f794b748f02dfa3a1"
PIN_aarch64_apple_darwin="cc6c9d79a70f95ff6eb8450d19f2ee4e3221a68820bf4301bee84078a9aecbaf"
PIN_x86_64_pc_windows_msvc="009c377adb193b1b7ec6e1b3bdd07d9d5d961030d545e0329b86f1223f0f7669"

die() { printf '✗ mozjs-fetch: %s\n' "$1" >&2; exit 1; }

TRIPLE="${MOZJS_TRIPLE:-$(rustc -vV | sed -n 's/^host: //p')}"
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

# sha256sum is absent on stock macOS; shasum ships there.
sha_of() { if command -v sha256sum >/dev/null; then sha256sum "$1"; else shasum -a 256 "$1"; fi | cut -d' ' -f1; }

# The sigstore CLI: $SIGSTORE, `sigstore` on PATH, or python3 -m sigstore (pip install sigstore).
find_sigstore() {
  if [ -n "${SIGSTORE:-}" ]; then echo "$SIGSTORE"
  elif command -v sigstore >/dev/null; then echo sigstore
  elif python3 -c 'import sigstore' >/dev/null 2>&1; then echo "python3 -m sigstore"
  else return 1; fi
}

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
  elif sigstore_cmd=$(find_sigstore); then
    # No gh: verify the SLSA bundle ourselves. The public attestation API serves it without auth.
    ATT_URL="${MOZJS_ATTEST_URL:-https://api.github.com/repos/servo/mozjs/attestations/sha256:$PIN}"
    BDIR=$(mktemp -d "$DEST_DIR/bundles.XXXXXX")
    curl -fsSL -H 'Accept: application/vnd.github+json' -o "$BDIR/att.json" "$ATT_URL" \
      || { rm -rf "$BDIR"; die "attestation download failed: $ATT_URL"; }
    python3 -I - "$BDIR" <<'PY' || { rm -rf "$BDIR"; die "attestation response has no bundles"; }
import json, sys
d = sys.argv[1]
for i, a in enumerate(json.load(open(d + "/att.json")).get("attestations", [])):
    json.dump(a["bundle"], open("%s/bundle%d.json" % (d, i), "w"))
PY
    # The archive must match the subject of at least one bundle signed by servo/mozjs's workflow. (Not every
    # record parses in every sigstore release, e.g. the GitHub release/v0.2 one; one verified bundle is enough.)
    VERIFIED=0
    for b in "$BDIR"/bundle*.json; do
      [ -e "$b" ] || continue
      if $sigstore_cmd verify github --repository servo/mozjs --bundle "$b" "$TMP" >/dev/null 2>&1; then
        VERIFIED=1; break
      fi
    done
    rm -rf "$BDIR"
    [ "$VERIFIED" = 1 ] || die "sigstore attestation FAILED for $ARCH (refusing to link it)"
    echo "attestation ok: $ARCH (sigstore, servo/mozjs)" >&2
  else
    echo "warning: neither authenticated gh nor sigstore available; attestation NOT checked for $ARCH, sha256 pin only" >&2
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

if [ -n "${GITHUB_ENV:-}" ]; then
  echo "MOZJS_ARCHIVE=$PWD/$DEST" >> "$GITHUB_ENV"
  echo "exported MOZJS_ARCHIVE to GITHUB_ENV" >&2
fi
echo "fetch ok: $TRIPLE" >&2
# Under Actions, also emit it as a check-run annotation: job logs need auth, annotations are public.
if [ -n "${GITHUB_ACTIONS:-}" ]; then printf '::notice title=mozjs-fetch::fetch ok: %s\n' "$TRIPLE" >&2; fi
echo "$DEST"
