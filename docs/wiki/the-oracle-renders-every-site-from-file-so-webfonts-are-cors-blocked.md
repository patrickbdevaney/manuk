# The oracle renders every site from `file://`, so a self-hosted webfont is CORS-blocked

**t1513.** t1510 proved the reference was using a fallback face and could not say why — *"deferring
the probe to `document.fonts.ready` changed nothing, so it is not a swap-timing race, and the cause
is still unestablished."* The constitution check (#146) made finding it a ⭐⭐⭐ steer, because I5
names the differential oracle *the discovery engine*. This is the cause.

## The oracle's own recipe

`capture_seen_all_paths` fetches the document over HTTP, splices in a `<base href>` and the probe,
writes it to a temp file, and loads it as:

```rust
cmd.arg(format!("file://{}", tmp.display()));
```

**From a `file://` origin every webfont is cross-origin**, and a webfont fetch is CORS-gated. A
self-hosted face — served same-origin by the site, needing no `Access-Control-Allow-Origin` in real
life — is therefore blocked, and Chrome lays the page out in its fallback.

## One flag, three runs, on the oracle's exact recipe

```text
  as the oracle runs it              fira_sansbook|error    check false   adv 129 == no-such-family 129
  + --disable-web-security           fira_sansbook|loaded   check TRUE    adv 140
  + --allow-file-access-from-files   fira_sansbook|error                  (not sufficient)
```

⭐⭐⭐ **140 is the font file's value** (`firasans-book-webfont.ttf`, upem 2048, 20509 units → 140.20px
at 14) **and it is ours.** One flag flips the reference from its fallback onto the real face and it
agrees with us exactly.

And `adv_fira14 == adv_missing14 == 129` in the blocked runs is the same fact from the other side:
the oracle's number is, precisely, what it measures for a family it does not have.

## ⭐⭐ Why `fonts.ready` could never have worked

```text
  document.fonts.status  "loaded"        <-- font loading is DONE
  the face's own status  "error"
```

**A failed font is a finished font.** `document.fonts.ready` resolves, so waiting for it changes
nothing — which is exactly what t1510 measured and could not explain.

## ⚠⚠⚠ The loop had already banked this trap — about its own fixtures

From t1367-1374, in this project's own memory:

> *"a `file://` fixture does NOT fetch a cross-origin webfont, so Chrome silently renders the
> FALLBACK; arbitrate font metrics against the FONT FILE."*

**The lesson was learned about hand-written fixtures and never applied to the instrument that does
the same thing on every site in the corpus.** A rule that names one caller does not generalise
itself — t1403's shape, one level up.

It also caught this tick mid-diagnosis: the first probe re-hosted the document on a local HTTP
origin with a `<base>`, reproduced the symptom, and **the reproduction was a confound** — until the
same flag experiment showed the confound *is* the mechanism.

## What it costs, priced before proposing a fix

`WEBFONTS: N of M @font-face families delivered a usable face` is printed by our side on every run.
Across 60 site runs in this session's logs, **45 report at least one `@font-face` family — 75%.**

⚠ **Not all 75% are affected, and the bound must be stated honestly.** A font served with
`Access-Control-Allow-Origin: *` — Google Fonts and most CDNs — loads fine from `file://`. The
affected set is **self-hosted faces**, which is what `jatekshop.eu` and `a11yproject.com` both are.
The exact count needs the oracle to report its own fallback advance, which it does not yet.

## ⚠ The fix is NOT in this tick, and that is deliberate

`--disable-web-security` is the only flag that works, and it is broad: it disables the same-origin
policy for XHR, iframes and canvas tainting too, and Chrome requires a `--user-data-dir` with it. It
would move banked numbers on three quarters of the corpus. **That deserves its own tick with a
before/after sweep** (t1491's rule: price the mechanism on the corpus before building it).

What landed instead is the *label*: the `font-resolution` cluster has carried an `UNATTRIBUTED`
warning since t1510, and it now names the proven cause instead of a suspicion.

## Where it is

`tests/wpt/src/oracle.rs` — `FONT_RESOLUTION_UNATTRIBUTED`, and the doc on it.
Gate: `a_divergence_between_two_faces_is_not_a_geometry_cause`, red under three mutations — drop the
clause, drop the proven cause from it, lose the `font-resolution:` prefix other consumers match on.

⚠ **No engine behaviour changed and no site moved.** The probe is byte-identical, so every banked row
stays comparable.
