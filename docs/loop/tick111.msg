feat(loop tick 111): global HTMLElement attribute reflection (+18,245 -- the session's largest flip)

html/dom's IDL-get-undefined mass was the GLOBAL HTMLElement attributes (dir, hidden, tabIndex,
accessKey, autocapitalize, autofocus, nonce, draggable, spellcheck, translate) -- reflected by
every element but absent from the per-tag reflection table, so div.dir / span.hidden returned
undefined. Fix: a "*" global row in reflect_table + descFor falling back to it (byTag[tag] ||
byTag['*']), reusing the generic reflection mechanism unchanged. Probe 8/8; gate G_GLOBAL_REFLECT
(incl. setter round-trip + tag-specific inertness preserved), proven falsifiable.

MEASURED (full sweep): html/dom 22690 -> 40935 (+18245), TOTAL 389637 -> 407882, crashes=0, every
other area held exactly. The tick-95 mass-reflector Bar-0 did NOT trip at 10 global accessors; the
remaining mass (ARIA + whole-tree access) stays crash-gated on the stack-quota fix.

Cadences due at 111, both performed: self-audit (clean); constitution-check #4 (H0, gate not
scoreboard, +18k on the reflection surface, no invariant bent; next 119).

Lesson: find the SHARED cause behind the biggest failing cluster -- one "*" row beat 400
per-attribute edits.

WIKI: docs/wiki/dom-semantics.md -- "The reflection GAP was the GLOBAL attributes"

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
