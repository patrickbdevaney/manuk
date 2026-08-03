
## Building the SUCCESSOR is not building the feature — a deprecated API is still a shipped API (tick 866)

`dashboard.twitch.tv` rendered **2 box-bearing elements**. The whole page died on one line at the
bundle's top level:

```text
  TypeError: can't access property "navigationStart", performance.timing is undefined
```

`performance.timing` has been deprecated for a decade. It is also in **every shipping browser**, and
the code written against it did not disappear when the replacement landed. After adding it, the site
builds **59** elements.

The instructive part is that this engine had not overlooked navigation timing — it had built the
**modern** interface (`performance.getEntriesByType('navigation')`) deliberately, and the comment
above that code says so:

> That is the modern, non-deprecated replacement for `performance.timing`, and it is what **every**
> RUM/analytics library reads — web-vitals, Google Analytics, Sentry, Datadog.

Every word true, and none of it a reason the predecessor can be missing. The page's feature-detect
finds `performance`, succeeds, and the next property read throws. That is the [[js-engine]]
half-installed-API law — *absence routes a caller to its fallback; HALF-presence routes it into a
wall* — with the two halves being two **generations** of one API instead of two methods of one
object. Same rung as `performance.clearMarks` (t777), one version boundary out.

**The general form: shipping the successor is not shipping the feature.** When an API has a
deprecated predecessor that browsers still expose, the predecessor is part of the surface.

### One source, two views

The legacy fields are **accessors** over the same `__navTiming` instants the navigation entry
reports, converted from relative doubles to absolute epoch milliseconds — never a second copy. The
gate asserts the two views agree (`timing.loadEventEnd == timeOrigin + navEntry.loadEventEnd`),
because two copies of one dataset is exactly how an instrument starts answering one question two
ways.

### What is 0, what is ABSENT, and why they are different answers

Transcribed from Chrome rather than chosen:

| field | value | why |
|---|---|---|
| `redirect*`, `unload*`, `secureConnectionStart` | **0** | the spec's "this phase did not occur", and what Chrome itself reports for a same-origin navigation with no redirect. True, not a stand-in |
| an event that has not fired yet | **0** | also per spec; becomes real when `__fireLoad`/`__fireDOMContentLoaded` records it |
| `fetchStart`, `domainLookup*`, `connect*`, `request*`, `response*` | **ABSENT** | this layer does not observe them. A `0` is indistinguishable from a real 0ms and makes every RUM library report a confident, wrong TTFB; `undefined` propagates to `NaN`, which is loud |

The absences are copied from `__navTiming`'s existing choice rather than re-decided, so the two views
cannot disagree about what is **unknown** — which would be a subtler lie than either value alone.

[[js-engine]] [[frameworks]] [[performance]]
