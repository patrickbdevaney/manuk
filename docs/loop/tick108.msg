feat(loop tick 108): high-usage DOM ergonomics -- isConnected, toggleAttribute, webkitMatchesSelector (+6 dom)

After tick 107 confirmed neutral niche APIs don't flip, targeted high-usage missing methods that
the failing tests actually call:
- node.isConnected -- is the node in the document tree (walk to root == document root). Every
  framework's detach guard. Was absent.
- element.toggleAttribute(name[, force]) -- add-or-remove, force pins direction, returns presence.
- element.webkitMatchesSelector -- legacy alias for matches.

Probe 8/8. Gate G_NODE_ERGONOMICS, proven falsifiable. MEASURED (full sweep): dom 2739 -> 2745
(+6), TOTAL 389478 -> 389484, crashes=0, no regression. A real flip (vs tick 107's neutral) --
target what the failing tests call, not what is easy to add. GATES 39 -> 40.

WIKI: docs/wiki/dom-semantics.md -- "isConnected / toggleAttribute / webkitMatchesSelector"

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
