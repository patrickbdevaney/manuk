#!/bin/bash
# GUARD.R0.a.a: fresh a11y capture per spread site, N runs each. Output: one line per run:
#   url run manuk_nodes chrome_nodes match prec recall f1 curl_sha256
# usage: guard-spread-capture.sh OUT.tsv N url...
R=$(cd "$(dirname "$0")/.." && pwd); AX=${AX:-$R/target/debug/a11y-score}
out=$1; n=$2; shift 2; : > "$out"
for url in "$@"; do
  for i in $(seq 1 "$n"); do
    touch "$R/.git/sprint-working"
    sha=$(curl -sL --max-time 30 "$url" | sha256sum | cut -c1-16)
    row=$(timeout 240 "$AX" "$url" 2>/dev/null | awk '/^https?:/{print $2"\t"$3"\t"$4"\t"$5"\t"$6"\t"$7; exit}')
    printf '%s\t%s\t%s\t%s\n' "$url" "$i" "${row:-NA}" "$sha" >> "$out"
  done
done
