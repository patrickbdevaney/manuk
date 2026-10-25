#!/usr/bin/env bash
# Failure classification shared by tri-sweep.sh and tri-sweep-test.sh.
#   classify_tag <rc> <stdout> <stderr>  → OK | NAVFAIL | TIMEOUT | CRASH | NOROW
# Order matters: a navigation error is a clean exit with a "FAILED" row (NAVFAIL), distinct from a hang
# (timeout rc 124 → TIMEOUT) and from a dead process (signal rc>=128 or panic text → CRASH).
classify_tag() {
  local rc="$1" out="$2" err="$3"
  if   [ "$rc" -eq 124 ]; then echo TIMEOUT
  elif [ "$rc" -ge 128 ] || printf '%s\n%s' "$out" "$err" | grep -qiE 'segmentation fault|SIGSEGV|panicked at'; then echo CRASH
  elif printf '%s\n' "$out" | grep -qE '^https?:[^ ]* +(FAILED|ORACLE FAILED|MANUK FAILED)'; then echo NAVFAIL
  elif printf '%s\n' "$out" | grep -qE '^https?:'; then echo OK
  else echo NOROW; fi
}
# drive-probe row: url targets drivable ambiguous mishit rate% +landmark% +heading% ceiling%
# → "rate ceiling targets drivable ungrounded ambiguous mishit" (ungrounded = remainder; probe omits it)
parse_m2() {
  printf '%s\n' "$1" | awk '/^https?:[^ ]* +[0-9]/{
    t=$2;d=$3;a=$4;m=$5;r=$6;c=$9;gsub(/%/,"",r);gsub(/%/,"",c);
    print r" "c" "t" "d" "t-d-a-m" "a" "m; exit}'
}
