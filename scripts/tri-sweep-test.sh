#!/usr/bin/env bash
# Gate for R0.4: tri-sweep tags NAVFAIL / TIMEOUT / CRASH / OK distinctly and persists per-site rows + stderr.
# Uses fake instrument binaries so no Chrome/engine is needed. Exit 0 = all assertions hold.
set -uo pipefail
R=/home/patrickd/manuk; T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
cat > "$T/dp" <<'F'
#!/usr/bin/env bash
echo "stderr-marker-$1" >&2
case "$1" in
  *navfail*) echo "$1 FAILED: dns error"; exit 0;;
  *hang*)    sleep 30;;
  *crash*)   kill -SEGV $$;;
  *) echo "$1   10  7  2  1  70.0%  80.0%  85.0%  90.0%"; echo "TOTAL 10 7 2 1 70.0% 80.0% 85.0% 90.0%";;
esac
F
cat > "$T/ax" <<'F'
#!/usr/bin/env bash
echo "$1 5 6 80.0% 70.0% 74.7%"
F
cat > "$T/mw" <<'F'
#!/usr/bin/env bash
echo "stderr-m1-$*" >&2
case "$*" in
  *m1hang*) sleep 30;;
  *m1crash*) kill -SEGV $$;;
  *m1nav*) echo "fetch http://m1nav.test/ failed: dns error"; exit 0;;
  *m1bot*) echo "UNMEASURABLE [bot-wall-403]: the origin answered 403 and refused this client" >&2; exit 0;;
  *m1blank*) echo "UNMEASURABLE [empty-202]: the origin answered 202 with a ZERO-BYTE body" >&2; exit 0;;
  *m1empty*) echo "nothing useful"; exit 0;;
esac
echo "structural: 90.0% (oracle 50 paths) SHAPE: 80.0% within 4px MEAN VISUAL: 0.9"
F
chmod +x "$T"/dp "$T"/ax "$T"/mw
printf 'cat\thttp://ok.test/\ncat\thttp://navfail.test/\ncat\thttp://hang.test/\ncat\thttp://crash.test/\n' > "$T/corpus"
printf 'cat\thttp://m1hang.test/\ncat\thttp://m1crash.test/\ncat\thttp://m1nav.test/\ncat\thttp://m1empty.test/\ncat\thttp://m1bot.test/\ncat\thttp://m1blank.test/\n' >> "$T/corpus"
MW="$T/mw" DP="$T/dp" AX="$T/ax" TRI_NO_CHROME_CHECK=1 PERSIST="$T/persist" MANUK_CHROME_PIN=testpin \
  "$R/scripts/tri-sweep.sh" --corpus "$T/corpus" --out "$T/out" --m1-timeout 2 --m2-timeout 2 --a11y-timeout 2 --jobs 2 >"$T/log" 2>&1
F=$(ls "$T"/persist/tri-sweep-*.tsv 2>/dev/null | head -1); fail=0
chk() { if [ "$2" = "$3" ]; then echo "ok   $1"; else echo "FAIL $1: want '$3' got '$2'"; fail=1; fi; }
[ -n "$F" ] || { echo "FAIL no persisted file"; cat "$T/log"; exit 1; }
tag() { awk -F'\t' -v u="$1" '$2==u{print $8}' "$F"; }
chk NAVFAIL "$(tag http://navfail.test/)" NAVFAIL
chk TIMEOUT "$(tag http://hang.test/)" TIMEOUT
chk CRASH   "$(tag http://crash.test/)" CRASH
chk OK      "$(tag http://ok.test/)" OK
chk ungrounded "$(awk -F'\t' '$2=="http://ok.test/"{print $13}' "$F")" 0
chk chrome_ver "$(awk -F'\t' '$2=="http://ok.test/"{print $20}' "$F")" testpin
m1() { awk -F'\t' -v u="$1" '$2==u{print $3}' "$F"; }
chk m1_TIMEOUT "$(m1 http://m1hang.test/)" TIMEOUT
chk m1_CRASH   "$(m1 http://m1crash.test/)" CRASH
chk m1_NAVFAIL "$(m1 http://m1nav.test/)" NAVFAIL
chk m1_EMPTY   "$(m1 http://m1empty.test/)" EMPTY
chk m1_BOTWALL "$(m1 http://m1bot.test/)" BOTWALL
chk m1_BLANK   "$(m1 http://m1blank.test/)" BLANK
chk m1_none_unreachable "$(awk -F'\t' '$3=="UNREACHABLE"' "$F"|wc -l)" 0
chk m1_stderr_kept "$(cat "$T"/out/err/*.m1.log|grep -c stderr-m1)" 10
n=$(cat "$T"/out/err/*.m2.log 2>/dev/null | grep -c stderr-marker); chk stderr_captured "$n" 4
exit $fail
