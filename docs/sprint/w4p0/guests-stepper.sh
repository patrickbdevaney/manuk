#!/bin/bash
# W4.P0.a.b.a: staynb stay/54 guests stepper. usage: [FOCUS=1] guests-stepper.sh   (needs target/release/click-probe)
# FOCUS=1 runs input.focus() on the GUESTS field before its click (what a real pointer press does); unset = engine click only.
POST='var u=location.href;var m=u.match(/guestsCount=([^&]*)/);return "guestsCount="+(m?decodeURIComponent(m[1]):"none")+" | url="+u.slice(0,40)+" | input="+Array.from(document.querySelectorAll("input")).map(function(i){return i.value}).slice(-1)[0]'
PRE=':'
[ -n "$FOCUS" ] && PRE='if(!document.querySelector("[aria-label*=guest]")){var x=document.querySelectorAll("input")[6];if(x)x.focus()}'
for i in 1 2 3; do
  out=$(PROBE_PRE_JS="$PRE" PROBE_JS="$POST" timeout 280 target/release/click-probe https://evals-staynb.vercel.app/stay/54 x seq "textbox:GUESTS" "button:Add Adults guest" "button:Add Children guest" "button:Add Children guest" "button:Reserve" 2>/dev/null)
  echo "run$i FOCUS=${FOCUS:-0} | $(echo "$out" | grep -E 'Add |guestsCount' | sed -E 's/scroll_y.*//' | tr '\n' ' ')"
done
