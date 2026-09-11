# The proxy is refused because it is also a shell

*Tick 1501. The acceptance test is doing its job; the app does not boot under the proxy either.*

## Where METHOD stands

The compass, measured twice, ranks the refusals `ORIGIN 58/60 · METHOD 23/17 · ENGINE 6/5`. This arc
took `METHOD` from 23 to 17 — `probe-blocked` 8 → 4 (t1486/t1487, the meta-refresh work) and two
`tree-divergence` sites converted (t1498). What is left is `shell-only` (4) and `oracle-module-shell`
(3), and **those already reach the proxy's floor**: they are getting the one-origin treatment and it is
being refused.

## The refusals, read

```text
  site                 proxied render   what only LIVE has
  comix.to                   4 tags     div×1258 span×371 a×223 img×167 svg×167 button×161
  allticketscol.com        168 tags     span×298 div×245 img×89 button×66 app-evento-card×39
  vk.com                   147 tags     path×14 p×4 svg×3 button×2 …
  esaj.tjsp.jus.br          37 tags     path×54 div×39 svg×31 a×27 li×26 h3×13 button×10
  house.udn.com              6 tags     a×155 div×153 option×91 script×80 source×80 img×55
  awlyaa.education.dz       10 tags     ACCEPTED (10 vs 9 — a genuinely small page)
  www.amazon.com.mx            —        `empty-` : the origin returned nothing to this client
```

**Five of seven are refused, and in every one the proxied render is itself a shell.** `comix.to` boots
to **four tags** against a live page carrying 1,258 divs. `house.udn.com` reaches **six**.

`proxy::renders_agree` is doing exactly what its doc says it is for: *"a proxy that cannot be shown to
agree with the live page is not a reference"*, and *"a HALF-BUILT reference is worse than an honest
shell"*. **The refusals are correct.** What they say is that one origin is necessary and not
sufficient — the apps do not boot under it.

## One named sub-mechanism, and only one

`house.udn.com`'s document is 195 bytes whose whole body is `window.location.href="/house/index"`. The
proxy **does** serve arbitrary same-origin paths — `format!("{upstream}{path}")` fetches them from the
live host — but it injects the probe **only at `doc_path`**. So the navigation is served, and lands on
a page the probe never reached.

That is the same family as t1486's `<meta http-equiv="refresh">` finding, one mechanism over: *the
document redirects, and the instrument measures the document it left.* The meta-refresh case was fixed
by following the redirect **before** injecting; the JS-navigation case cannot be, because the redirect
is only known by running the page.

⚠ **The other four are not explained**, and this page does not pretend they are. `comix.to` at four
tags and `allticketscol.com` at 168 are apps that fail to boot for some reason the tag histogram does
not name.

## What this decides

**Before any more proxy work, establish why the app does not boot under one origin.** The proxy was
built on the premise that the origin wall is what stops these pages (a module bundle is CORS-fetched, a
snapshot serves it cross-origin), and for the two sites t1498 converted that premise held. For these
five it does not, and widening the trigger further would only produce more correct refusals.

`Page::boot_errors` (t1480) is the instrument for that question and it already exists: run the proxied
document through our own engine and read what threw.
