#!/bin/bash
# W4.P0.a.b: per-task first-click classification. usage: ROLE=button|link|clickable click-classify.sh <task> <url> "<name>"  (needs target/release/click-probe)
# usage: run.sh <task> <url> <button-name>   (role=button, one click, 3 runs)
task=$1; url=$2; name=$3
PRE='(function(){var t=document.body.innerText;window.__b={href:location.href,len:t.length,h:(function(s){var x=0;for(var i=0;i<s.length;i++)x=(x*31+s.charCodeAt(i))|0;return x})(t)};})()'
POST='var t=document.body.innerText;var h=(function(s){var x=0;for(var i=0;i<s.length;i++)x=(x*31+s.charCodeAt(i))|0;return x})(t);return JSON.stringify({href_before:window.__b.href,href_after:location.href,len_before:window.__b.len,len_after:t.length,text_changed:h!==window.__b.h,dialogs:document.querySelectorAll("[role=dialog],dialog").length})'
for i in 1 2 3; do
  out=$(PROBE_PRE_JS="$PRE" PROBE_JS="$POST" PROBE_ROLE=${ROLE:-button} timeout 240 target/release/click-probe "$url" x seq "$ROLE:$name" 2>${TMPDIR:-/tmp}/click-classify-err.txt)
  echo "$task run$i | $(echo "$out" | grep -v '^boot-error' | tr '\n' ' ' | cut -c1-700) | panics=$(grep -c panicked ${TMPDIR:-/tmp}/click-classify-err.txt)"
done
