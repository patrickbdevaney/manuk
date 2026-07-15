feat(loop tick 103): document.elementFromPoint hit-testing (+29) + surface-audit & constitution-check

document.elementFromPoint was entirely missing (css-transforms: 84 "is not a function"). It is a
high-usage DOM API (drag-and-drop, tooltips, custom controls, hit-test suites). Implemented over
the layout-rect snapshot: among laid-out element boxes containing the client point, return the
deepest (smallest area, later document order on a tie); miss / non-finite coord -> null. Gate
G_ELEMENT_FROM_POINT, proven falsifiable.

Honest bounds (in code + docs): rects are pre-transform, scroll assumed zero -- transform-aware
hit-testing is the follow-on. Still flipped the untransformed-box cases.

MEASURED (full sweep): TOTAL 389069 -> 389098 (+29); css-transforms 20->45 (+25), flexbox +3,
overflow +1; crashes=0 (isolation-retry holding). GATES 37->38.

Cadences due at 103, both performed: constitution-check #3 (H0, ticks 96-103 judged gate-not-
scoreboard, +420 real subtests, no invariant bent, flexbox UAF a tracked open Bar-0; next 111);
surface-audit #3 (finding: WPT checkout aperture is narrow -- css-values/position/display/color +
html/semantics not checked out; next map-expansion is a wpt-setup.sh checkout expansion; next 113).

WIKI: docs/wiki/interaction-surface.md -- "document.elementFromPoint bridges the layout-rect snapshot"

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
