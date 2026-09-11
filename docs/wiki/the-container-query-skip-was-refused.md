# The container-query skip was refused

*Tick 1500. A single-run pair said −26%. Three runs a side said nothing.*

## The change

`container_query_recascade` returned `true` **unconditionally** whenever any sheet's source contained
the substring `@container`, and the caller reads `true` as *"re-lay out the whole document."* A page
that merely mentions `@container` therefore paid a second full layout whether or not one container
query's match had changed.

That is a **sufficient condition used as a necessary one** — the third instance of that exact shape
this window, after the proxy's shell floor (t1498) and the `bold: bool` weight key (t1494). It looked
like the same bug a third time.

The change: apply natural sizes to the freshly cascaded map, compare it with the pass-1 map, and skip
the second layout when they are equal.

⚠ The comparison **must** come after `apply_natural_sizes`. The caller applies natural sizes to the
pass-1 map, and a freshly cascaded map has none — comparing the raw maps reports *"changed"* for every
page carrying one decoded image, which is nearly all of them, and the saving would be exactly zero
while looking like it worked.

## It does not fire on the cohort that motivated it

`trivago.*` is four of the five `css-starved` rows across two sweeps, and its styles **genuinely
change**:

```text
  www.trivago.fr   container_query_ms=3996  cq_relaid=true
  www.trivago.be   container_query_ms=3749  cq_relaid=true
```

So trivago's container-query relayout is **necessary work**, not spurious. That is itself worth
knowing: t1499's next step should be *making that relayout cheaper* (an incremental-layout problem),
not skipping it.

## Where it does fire, it buys nothing measurable

Priced over a 40-site slice: **38 passes on 4 sites** take the new early return —
`m.youm7.com`, `agoda.com`, `ebay.com`, `otomoto.pl`. All four are real, major sites, so the
population is not a corner.

```text
  m.youm7.com, same binary otherwise, one flag apart

  first pair, ONE RUN EACH     BEFORE 41174 ms    AFTER 30465 ms     -26%  (!)

  three runs a side
    AFTER  (fix in)   31425  33567  30331     mean 31774   range 30331-33567
    BEFORE (fix out)  33075  30535  27461     mean 30357   range 27461-33075
```

**The bands overlap completely**, and the mean is marginally *worse* with the fix — also inside the
band. The skipped layouts on this site are the cheap kind (`container_query_ms` 254–313 ms a pass, not
trivago's 3,900), and the `StyleMap` comparison the skip requires is itself O(nodes) on every
`@container` page.

## Refused

Correct in principle, worth nothing in practice on this corpus, and it adds a per-page cost to buy it.
**Reverted whole.**

⚠ *One run refuses nothing* (t1410) — and here one run very nearly **accepted** something. The first
pair was a 10.7-second difference on a real site, which is exactly the size of result that gets
written up. The repeat is the only thing that stopped it.

## What the refusal establishes

* **trivago's container-query relayout is necessary**, so t1499's ranked next step is narrower than it
  was: make the second layout cheaper, not conditional.
* **The `@container` trigger is still a sufficient-condition-as-necessary**, and it is still not worth
  fixing — which is a different conclusion from "it is fine", and only a price could tell them apart.
