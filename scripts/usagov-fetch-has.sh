#!/usr/bin/env bash
# R0.4.a.b.a.a.a.a.a: for each of the 11 usa.gov oracle path keys (the paths-diff table), say whether the element is in the
# DOM that Chrome builds from the stored engine-fetch HTML with JavaScript disabled (RAW), or only appears once Chrome runs
# the page's JS (JS_ONLY). Prints "class<TAB>key" per key.
# usage: usagov-fetch-has.sh [FETCH_HTML] [KEYS_FILE]
#   FETCH_HTML defaults to docs/bench/engine-fetch/www.usa.gov.engine-fetch.html
#   KEYS_FILE defaults to the keys printed by usagov-paths-diff.sh (the 11 keys in USAGOV-ATTRIB.md)
set -uo pipefail
R=/home/patrickd/manuk
touch "$R/.git/sprint-working" 2>/dev/null || true
HTML=${1:-$R/docs/bench/engine-fetch/www.usa.gov.engine-fetch.html}
W=$(mktemp -d /tmp/usagov-fetch-has.XXXXXX); trap 'rm -rf "$W"' EXIT
K=${2:-}
if [ -z "$K" ]; then
  K="$W/keys.txt"; "$R/scripts/usagov-paths-diff.sh" 2>/dev/null | awk -F'`' '/^\| `/{print $2}' > "$K"
fi
# JavaScript off: --disable-javascript is ignored by this Chrome (a script still writes its node), so a CSP meta that
# blocks every script is inserted straight after the doctype (before it, the doctype is dropped and the page goes quirks).
# The gate checks this with a fixture whose script adds an element.
META='<meta http-equiv="Content-Security-Policy" content="script-src '"'"'none'"'"'">'
if grep -qi '<!doctype' "$HTML"; then sed -E "0,/<![Dd][Oo][Cc][Tt][Yy][Pp][Ee][^>]*>/s||&$META|" "$HTML" > "$W/page.html"
else { echo "$META"; cat "$HTML"; } > "$W/page.html"; fi
timeout 90 google-chrome --headless --no-sandbox --disable-gpu --dump-dom "file://$W/page.html" > "$W/dom.html" 2>/dev/null \
  || { echo "chrome --dump-dom failed" >&2; exit 1; }
node "$R/scripts/usagov-fetch-has.js" "$W/dom.html" "$K"
