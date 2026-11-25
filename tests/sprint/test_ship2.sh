#!/usr/bin/env bash
# SHIP.2 gate: package -> checksum verifies -> install -> tools listed + one Chrome task on the INSTALLED binary.
# Goes red on: a tampered tarball (installer must refuse), a missing/garbage SBOM, a Chrome that is not there.
# usage: test_ship2.sh [path/to/manuk-mcp]   (default target/release/manuk-mcp)
R=$(cd "$(dirname "$0")/../.." && pwd); BIN=${1:-$R/target/release/manuk-mcp}
fail=0; ok(){ echo "ok   $*"; }; bad(){ echo "FAIL $*"; fail=1; }
W=$(mktemp -d); trap 'rm -rf "$W"' EXIT
case "$(uname -s)-$(uname -m)" in Linux-x86_64) N=manuk-mcp-linux-x86_64;; Darwin-arm64) N=manuk-mcp-macos-arm64;; *) echo skip; exit 0;; esac
"$R/scripts/mcp-package.sh" "$BIN" "$N" "$W/rel" >/dev/null && ok "package built (tar.gz, sha256, SBOM)" || bad "package"
(cd "$W/rel" && sha256sum -c "$N.tar.gz.sha256" >/dev/null) && ok "sha256sum -c verifies" || bad "checksum"
MANUK_BASE_URL="$W/rel" MANUK_PREFIX="$W/p" sh "$R/scripts/install-mcp.sh" >/dev/null && ok "install-mcp.sh installs" || bad "install"
# mutation 1: flip a byte in the tarball -> installer must refuse and install nothing
cp -r "$W/rel" "$W/bad"; printf 'X' | dd of="$W/bad/$N.tar.gz" bs=1 seek=100 conv=notrunc 2>/dev/null
if MANUK_BASE_URL="$W/bad" MANUK_PREFIX="$W/p2" sh "$R/scripts/install-mcp.sh" >/dev/null 2>&1 || [ -e "$W/p2/bin/manuk-mcp" ]; then bad "tampered tarball was installed"; else ok "tampered tarball refused"; fi
# mutation 2: garbage SBOM source -> packager must refuse
echo '{}' > "$W/junk.json"
if MANUK_SBOM_SRC="$W/junk.json" "$R/scripts/mcp-package.sh" "$BIN" "$N" "$W/rel2" >/dev/null 2>&1; then bad "junk SBOM accepted"; else ok "junk SBOM refused"; fi
python3 -I -c "import json,sys;d=json.load(open(sys.argv[1]));print('sbom components',len(d['components']))" "$W/rel/$N.cdx.json"
# regression (SHIP.2.a): CI packages into dist/ INSIDE the repo; the SBOM must survive the cleanup
IP="$R/ship2-probe-$$"; "$R/scripts/mcp-package.sh" "$BIN" "$N" "$IP" >/dev/null 2>&1 && [ -s "$IP/$N.cdx.json" ] && ok "in-repo output keeps SBOM" || bad "in-repo output lost SBOM"
rm -rf "$IP"
# the smoke on the INSTALLED binary
python3 -I "$R/scripts/mcp-smoke.py" "$W/p/bin/manuk-mcp" > "$W/smoke.out" 2>&1 && ok "installed binary: tools listed, Chrome task passes" || { bad "smoke"; tail -5 "$W/smoke.out"; }
# mutation 3: no Chrome -> smoke must fail
if MANUK_CHROME=/nonexistent/chrome python3 -I "$R/scripts/mcp-smoke.py" "$W/p/bin/manuk-mcp" >/dev/null 2>&1; then bad "smoke passed with no Chrome"; else ok "smoke red without Chrome"; fi
exit $fail
