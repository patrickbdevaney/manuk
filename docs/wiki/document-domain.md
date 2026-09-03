# `document.domain` — a missing property that pages read as a string is a THROW, not a gap

> Landed t1406. Gate: `document_domain_is_a_string_that_a_page_can_call_replace_on`
> (`engine/page/tests/g_document_domain.rs`), red under 3 mutations. Oracle = CDP `Runtime.evaluate`
> on a live https origin.

Found by the corpus sweep, not by a spec list. `neutypechic.com`'s bundle does
`document.domain.replace(...)` and the engine answered:

```text
  TypeError: can't access property "replace", document.domain is undefined
```

⭐⭐ **`undefined.replace` ends the script**, and everything that script was going to render never
happens. That is why the board ranks throw-killers first for scorability: a site that does not boot
scores zero out of zero, and a zero is the M1 *ceiling* rather than a point on it.

```text
  chrome, https://danluu.com/, CDP Runtime.evaluate
  {"type":"string","value":"danluu.com","inDoc":true,"replace":"danluu.com"}
```

The value is the document's origin **host** — exactly what `location.hostname` already knows.

## The setter is a deliberate no-op, and that is said out loud

The legacy `document.domain = 'example.com'` widening is a **same-origin-policy relaxation** this
engine does not implement. Pretending to honour it would be worse than ignoring it, so the value is
remembered — which is what the compatibility idiom reads back — and no security consequence follows.
A half-built security feature that *looks* honoured is the failure mode; a named non-claim is not.

## Priced before building

`document.domain` appears on **2 of 52** freshly-fetched CrUX corpus pages (3.8%). Small, and the cost
is nine lines of prelude. Recorded because t1405 took the price *after* building and said so; this one
took it first.

## The histogram it came out of, and why the rest of that histogram is not a worklist

```text
  19  TypeError: can't access property "call", d is undefined     webpack: a module never loaded
  15  ReferenceError: $ is not defined                            jQuery never ran
   9  ReferenceError: jQuery is not defined
   5  SyntaxError: expected expression, got '<'                   ⭐ WE RAN AN HTML DOCUMENT AS JS
```

⚠ **A signature histogram names the tests, not the mechanism.** Twenty-four of those hits are ONE
upstream failure — *a script that should have loaded did not* — wearing that many page-internal symbol
names, and `expected expression, got '<'` says an HTML error page reached the JS parser. Both need a
PROBE, not a fix derived from the histogram.
