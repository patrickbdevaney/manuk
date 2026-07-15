#!/usr/bin/env bash
# Fetch the upstream Web Platform Tests corpus for `manuk-wpt wpt`.
#
# Sparse + blob-filtered: the full WPT tree is enormous and we need a fraction of it. This lands ~92MB
# instead of several GB.
set -euo pipefail
DIR="${WPT_DIR:-$HOME/wpt}"
SUBSETS="resources common dom html/dom css/selectors css/css-flexbox css/css-grid cssom domparsing url encoding css/css-values css/css-position css/css-display css/css-color html/semantics"

if [ ! -d "$DIR/.git" ]; then
  echo "cloning WPT → $DIR"
  git clone --filter=blob:none --depth 1 --sparse https://github.com/web-platform-tests/wpt.git "$DIR"
fi
git -C "$DIR" sparse-checkout set $SUBSETS
echo
echo "WPT at $DIR  ($(du -sh "$DIR" | cut -f1))"
echo "  export WPT_DIR=$DIR"
echo "  cargo run --release -p manuk-wpt -- wpt dom --show-failures"
echo
echo "NOTE: the runner serves this tree over HTTP and OVERRIDES /resources/testharnessreport.js"
echo "      in the SERVER — the checkout is never modified, so 'git -C \$WPT_DIR status' stays clean."
