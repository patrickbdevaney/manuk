#!/usr/bin/env bash
# TAIL.1.a gate: docs/sprint/TAIL.md has 40 site rows, none UNEXAMINED/blank class, every live row cites a number.
f=${1:-docs/sprint/TAIL.md}
rows=$(grep -c '^| https://' "$f")
bad=$(grep '^| https://' "$f" | awk -F'|' '{c=$7; e=$8; n=$9; gsub(/ /,"",c); gsub(/ /,"",e);
  if (c==""||c=="UNEXAMINED"||e=="none"||n !~ /[0-9]/) print $2}')
[ "$rows" = 40 ] && [ -z "$bad" ] && { echo "tail-check OK rows=$rows"; exit 0; }
echo "tail-check FAIL rows=$rows bad:"; echo "$bad"; exit 1
