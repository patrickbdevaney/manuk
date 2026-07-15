feat(loop tick 110): DOMException legacy codes + Event phase constants (+7)

Following tick 109's Node-constants win, swept the same class: DOMException legacy code constants
(NOT_FOUND_ERR=8, INDEX_SIZE_ERR=1, ... -- the setup defined codes by name but not the legacy *_ERR
numerics that `e.code === DOMException.NOT_FOUND_ERR` checks) and Event phase constants (AT_TARGET=2,
CAPTURING_PHASE, BUBBLING_PHASE). Added 25 DOMException codes + 4 Event constants (ctor + prototype).
Prelude-ordering gotcha: Event is defined via defEvent in the dom_bindings prelude, so the constants
attach there. Probe 6/6; gate G_INTERFACE_CONSTANTS, proven falsifiable.

MEASURED (full sweep): dom 2757->2764 (+7), TOTAL 389630->389637, crashes=0, no regression.
GATES 41->42. The interface-constants vein (109-110) is now largely mined.

WIKI: docs/wiki/dom-semantics.md -- "DOMException legacy codes + Event phase constants"

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
