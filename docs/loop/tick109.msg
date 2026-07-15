feat(loop tick 109): Node interface constants + compareDocumentPosition (+146, biggest flip since tick 100)

Hunting high-usage missing surface, the Node interface CONSTANTS were a major hole:
Node.ELEMENT_NODE / TEXT_NODE / COMMENT_NODE / ... and DOCUMENT_POSITION_* were absent, so the
ubiquitous `n.nodeType === Node.ELEMENT_NODE` compared a number to undefined -> silently false, and
compareDocumentPosition threw. Added all 12 node-type + 6 position constants on Node and
Node.prototype, and implemented compareDocumentPosition in the prelude (ancestor-chain containment +
common-ancestor child order). Probe 8/8; gate G_COMPARE_POSITION, proven falsifiable.

MEASURED (full sweep): dom 2745->2757 (+12), html/dom 22562->22690 (+128), domparsing 182->188 (+6),
TOTAL 389484->389630 (+146), crashes=0, no regression. GATES 40->41.

Lesson: probe by what the FAILING tests reference most (a cross-cutting primitive hiding in plain
sight), not by area. The high-usage missing-surface vein is thinner but not dry.

WIKI: docs/wiki/dom-semantics.md -- "The Node interface CONSTANTS were absent"

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
