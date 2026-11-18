#!/bin/bash
# W4.E3.b.b gate: manuk-mcp navigate exits rc 0, no mutex-destroy line, 3 runs x {no cache, --http-cache-dir}.
BIN=${BIN:-target/release/manuk-mcp}; URL=${URL:-http://books.toscrape.com/}; fail=0
for mode in plain cache; do for i in 1 2 3; do
  args=(); [ $mode = cache ] && args=(--http-cache-dir "$(mktemp -d)")
  err=$(mktemp)
  ( printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"g","version":"1"}}}' \
      '{"jsonrpc":"2.0","method":"notifications/initialized"}' \
      "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/call\",\"params\":{\"name\":\"navigate\",\"arguments\":{\"url\":\"$URL\"}}}"; sleep 8 ) \
    | $BIN "${args[@]}" >/dev/null 2>"$err"
  rc=${PIPESTATUS[1]}
  bad=$(grep -c pthread_mutex_destroy "$err")
  echo "$mode run$i rc=$rc mutexlines=$bad"
  if [ "$rc" != 0 ] || [ "$bad" != 0 ]; then fail=1; fi
done; done
exit $fail
