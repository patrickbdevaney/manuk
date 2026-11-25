#!/bin/bash
# CPU.3: build manuk-mcp at several opt-levels into a throwaway target dir; record wall build time + size. Never touches target/release.
set -u
cd "$(dirname "$0")/../../.."
OUT=${OUT:-/home/patrickd/cpu3bin}; mkdir -p $OUT
HOT=(manuk-css manuk-dom manuk-html manuk-layout manuk-a11y manuk-page manuk-text)
for v in "$@"; do
  T=/home/patrickd/cpu3-target-$v; rm -rf $T
  extra=(); lvl=$v
  if [ "$v" = hot3 ]; then lvl=s; for c in "${HOT[@]}"; do extra+=(--config "profile.release.package.$c.opt-level=3"); done; fi
  s=$(date +%s)
  systemd-run --user --scope -p MemoryMax=8G -p MemorySwapMax=0 env CARGO_TARGET_DIR=$T CARGO_PROFILE_RELEASE_OPT_LEVEL=$lvl \
    cargo build --release -p manuk-mcp "${extra[@]}" > $OUT/build-$v.log 2>&1 || { echo "$v FAILED"; tail -5 $OUT/build-$v.log; }
  e=$(date +%s)
  cp $T/release/manuk-mcp $OUT/manuk-mcp-$v && echo -e "$v\t$((e-s))\t$(stat -c %s $OUT/manuk-mcp-$v)\tjobs=${CARGO_BUILD_JOBS:-}" >> $OUT/build.tsv
  rm -rf $T
done
