#!/bin/bash
# R0.4.a.c.a.b.a.a.a.a.a.a.a.a.a gate: the debug-only "document.documentElement.style is null" is the script
# deadline terminating g.__mkStyle (lazy_view turns the failed call into null); release never reaches it.
# usage: sf-style-null-gate.sh DEBUG_ERR RELEASE_ERR
set -u
d=${1:?debug err}; r=${2:?release err}; rc=0
fail(){ echo "RED: $*"; rc=1; }
# 1. debug: the termination frame is g.__mkStyle and the error follows it directly
l=$(grep -n 'Script terminated by timeout' "$d" | head -1 | cut -d: -f1)
[ -n "$l" ] || fail "debug log has no 'Script terminated by timeout'"
[ -n "$l" ] && sed -n "$((l+1))p" "$d" | grep -q '^g\.__mkStyle@' || fail "first termination frame is not g.__mkStyle"
[ -n "$l" ] && sed -n "$((l+1)),$((l+25))p" "$d" | grep -q 'document.documentElement.style is null' || fail "style-null error does not follow the termination"
# 2. release: neither the termination nor the error
grep -q 'Script terminated by timeout' "$r" && fail "release log has a termination"
grep -q 'style is null' "$r" && fail "release log has the style-null error"
# 3. code path: lazy_view maps a failed JS_CallFunctionName to null
grep -q 'let ok = mozjs::rust::wrappers2::JS_CallFunctionName' engine/js/src/dom_bindings.rs &&
grep -q '\*vp = if ok { rval.get() } else { NullValue() };' engine/js/src/dom_bindings.rs || fail "lazy_view null-on-failure path not found"
[ $rc = 0 ] && echo GREEN
exit $rc
