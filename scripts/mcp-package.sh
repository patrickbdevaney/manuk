#!/usr/bin/env bash
# SHIP.2: package a built manuk-mcp for release: <name>.tar.gz + <name>.tar.gz.sha256 + <name>.cdx.json (SBOM).
# usage: mcp-package.sh <binary> <name e.g. manuk-mcp-linux-x86_64> <outdir>
# The SBOM is the CycloneDX 1.5 dependency graph of the manuk-mcp crate (cargo-cyclonedx, run from the repo).
set -euo pipefail
BIN=$1; NAME=$2; OUT=$3
R=$(cd "$(dirname "$0")/.." && pwd)
mkdir -p "$OUT"; OUT=$(cd "$OUT" && pwd)
S=$(mktemp -d); trap 'rm -rf "$S"' EXIT
mkdir "$S/$NAME"; cp "$BIN" "$S/$NAME/manuk-mcp"; chmod 755 "$S/$NAME/manuk-mcp"
cp "$R/SECURITY.md" "$S/$NAME/"; cp "$R/docs/sprint/CHROME_PIN" "$S/$NAME/CHROME_PIN"
tar -C "$S" -czf "$OUT/$NAME.tar.gz" "$NAME"
if [ -n "${MANUK_SBOM_SRC:-}" ]; then cp "$MANUK_SBOM_SRC" "$OUT/$NAME.cdx.json"   # test hook
else
  (cd "$R" && cargo cyclonedx --manifest-path mcp/Cargo.toml --format json --spec-version 1.5 --override-filename "$NAME.cdx" >/dev/null)
  # cargo-cyclonedx writes one file per workspace member; keep manuk-mcp's, drop the rest
  mv "$R/mcp/$NAME.cdx.json" "$OUT/$NAME.cdx.json"
  # never delete the copy we just moved: $OUT may itself live inside the repo (CI uses dist/)
  find "$R" -name "$NAME.cdx.json" -not -path "$R/target/*" -not -path "$OUT/*" -delete
fi
python3 -I -c "import json,sys; d=json.load(open(sys.argv[1])); assert d['bomFormat']=='CycloneDX' and d['specVersion']=='1.5' and len(d['components'])>10, 'bad SBOM'" "$OUT/$NAME.cdx.json"
(cd "$OUT" && sha256sum "$NAME.tar.gz" > "$NAME.tar.gz.sha256")
echo "packaged $OUT/$NAME.tar.gz ($(wc -c < "$OUT/$NAME.tar.gz") B) sha256=$(cut -d' ' -f1 "$OUT/$NAME.tar.gz.sha256")"
